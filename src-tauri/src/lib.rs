//! Application de bureau : fait le lien entre l'interface (dossier `ui/`) et `broderie-core`.

mod ai;

use broderie_core::library::{DuplicateGroup, Entry, EntryUpdate, GroupChoice, ImportReport, Library};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, State};

type Lib = Arc<Mutex<Library>>;

// ---------------------------------------------------------------------------
// Réglages (dans le dossier de configuration de l'application, pas dans la bibliothèque :
// la clé ne part donc pas dans les sauvegardes .zip).

#[derive(Default, Serialize, Deserialize)]
struct Settings {
    #[serde(default)]
    gemini_key: Option<String>,
    #[serde(default)]
    gemini_model: Option<String>,
}

fn settings_path(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_config_dir().ok().map(|d| d.join("reglages.json"))
}

fn load_settings(app: &AppHandle) -> Settings {
    settings_path(app)
        .and_then(|p| std::fs::read(p).ok())
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

fn save_settings(app: &AppHandle, s: &Settings) -> Result<(), String> {
    let p = settings_path(app).ok_or("dossier de réglages introuvable")?;
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
    }
    std::fs::write(p, serde_json::to_vec_pretty(s).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Analyse des motifs par l'IA, en arrière-plan.

#[derive(Default)]
struct AiState {
    running: AtomicBool,
    stop: AtomicBool,
    error: Mutex<Option<String>>,
}

fn ai_status_of(app: &AppHandle) -> ai::AiStatus {
    let lib = app.state::<Lib>();
    let st = app.state::<Arc<AiState>>();
    let l = lock(&lib);
    let total = l.entries().iter().filter(|e| e.format.is_some()).count();
    let (_, pending) = l.ai_pending(0);
    let error = st.error.lock().unwrap_or_else(|e| e.into_inner()).clone();
    ai::AiStatus {
        has_key: load_settings(app).gemini_key.is_some(),
        running: st.running.load(Ordering::SeqCst),
        analysed: total - pending,
        total,
        error,
    }
}

fn emit_ai(app: &AppHandle) {
    let _ = app.emit("ai-progress", ai_status_of(app));
}

/// Lance l'analyse des motifs pas encore regardés par l'IA (si une clé est enregistrée).
fn start_ai(app: &AppHandle) {
    let settings = load_settings(app);
    let Some(key) = settings.gemini_key else { return };
    let st = app.state::<Arc<AiState>>().inner().clone();
    if st.running.swap(true, Ordering::SeqCst) {
        return; // déjà en route
    }
    st.stop.store(false, Ordering::SeqCst);
    *st.error.lock().unwrap_or_else(|e| e.into_inner()) = None;
    let app = app.clone();
    std::thread::spawn(move || {
        let lib = app.state::<Lib>().inner().clone();
        let mut models: Vec<String> = settings.gemini_model.into_iter().collect();
        models.extend(ai::MODELS.iter().map(|m| m.to_string()));
        models.dedup();
        let (mut model_i, mut failures, mut wait) = (0usize, 0u32, 30u64);
        emit_ai(&app);
        loop {
            if st.stop.load(Ordering::SeqCst) {
                break;
            }
            let (ids, _) = lock(&lib).ai_pending(ai::BATCH);
            if ids.is_empty() {
                break;
            }
            let mut images = Vec::new();
            {
                let mut l = lock(&lib);
                for id in &ids {
                    match l.ai_image(id) {
                        Ok(png) => images.push((id.clone(), png)),
                        Err(_) => {
                            let _ = l.set_ai(id, "", &[]); // illisible : on ne réessaie pas
                        }
                    }
                }
            }
            if images.is_empty() {
                continue;
            }
            let Some(model) = models.get(model_i).cloned() else {
                *st.error.lock().unwrap() = Some("Aucun modèle Gemini disponible pour cette clé.".into());
                break;
            };
            match ai::describe(&key, &model, &images) {
                Ok(results) => {
                    failures = 0;
                    wait = 30;
                    let mut l = lock(&lib);
                    for r in &results {
                        let _ = l.set_ai(&r.id, &r.description, &r.keywords);
                    }
                    // Un motif oublié par l'IA est marqué comme vu, pour ne pas tourner en rond.
                    for (id, _) in &images {
                        if !results.iter().any(|r| &r.id == id) {
                            let _ = l.set_ai(id, "", &[]);
                        }
                    }
                    let _ = l.flush();
                    drop(l);
                    emit_ai(&app);
                    std::thread::sleep(Duration::from_secs(5)); // rester sous la limite gratuite
                }
                Err(ai::AiError::ModelNotFound) => model_i += 1,
                Err(ai::AiError::RateLimited) => {
                    // Quota gratuit atteint : on patiente de plus en plus longtemps.
                    for _ in 0..wait {
                        if st.stop.load(Ordering::SeqCst) {
                            break;
                        }
                        std::thread::sleep(Duration::from_secs(1));
                    }
                    wait = (wait * 2).min(900);
                }
                Err(ai::AiError::BadKey(msg)) => {
                    *st.error.lock().unwrap() = Some(format!("La clé Gemini a été refusée : {msg}"));
                    break;
                }
                Err(ai::AiError::Other(msg)) => {
                    failures += 1;
                    if failures >= 3 {
                        *st.error.lock().unwrap() = Some(msg);
                        break;
                    }
                    std::thread::sleep(Duration::from_secs(20));
                }
            }
        }
        st.running.store(false, Ordering::SeqCst);
        emit_ai(&app);
    });
}

#[tauri::command]
fn ai_status(app: AppHandle) -> ai::AiStatus {
    ai_status_of(&app)
}

/// Vérifie et enregistre la clé Gemini, puis lance l'analyse.
#[tauri::command]
async fn ai_set_key(app: AppHandle, key: String) -> Result<ai::AiStatus, String> {
    let key = key.trim().to_string();
    if key.len() < 20 {
        return Err("Cette clé semble incomplète. Copiez-la en entier depuis Google AI Studio.".into());
    }
    let k = key.clone();
    let model = tauri::async_runtime::spawn_blocking(move || ai::check_key(&k))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| match e {
            ai::AiError::BadKey(_) => "Google refuse cette clé. Vérifiez qu'elle est copiée en entier.".to_string(),
            ai::AiError::RateLimited => "Trop de demandes pour le moment : réessayez dans une minute.".to_string(),
            ai::AiError::ModelNotFound => "Aucun modèle Gemini disponible pour cette clé.".to_string(),
            ai::AiError::Other(m) => m,
        })?;
    save_settings(&app, &Settings { gemini_key: Some(key), gemini_model: Some(model) })?;
    start_ai(&app);
    Ok(ai_status_of(&app))
}

/// Oublie la clé et arrête l'analyse (ce qui a déjà été analysé est gardé).
#[tauri::command]
fn ai_remove_key(app: AppHandle) -> Result<ai::AiStatus, String> {
    app.state::<Arc<AiState>>().stop.store(true, Ordering::SeqCst);
    save_settings(&app, &Settings::default())?;
    Ok(ai_status_of(&app))
}

#[tauri::command]
fn ai_start(app: AppHandle) -> ai::AiStatus {
    start_ai(&app);
    ai_status_of(&app)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LibraryInfo {
    root: String,
    thumbs: String,
    entries: Vec<Entry>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BlockView {
    color: String,
    name: String,
    /// x, y, saut (0/1), x, y, saut…
    pts: Vec<f32>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DesignView {
    format: String,
    label: String,
    blocks: Vec<BlockView>,
    min_x: f32,
    min_y: f32,
    max_x: f32,
    max_y: f32,
    stitch_count: usize,
    #[serde(rename = "widthMM")]
    width_mm: f32,
    #[serde(rename = "heightMM")]
    height_mm: f32,
}

#[derive(Clone, Serialize)]
struct Progress {
    done: usize,
    total: usize,
    file: String,
}

fn lock(lib: &Lib) -> std::sync::MutexGuard<'_, Library> {
    lib.lock().unwrap_or_else(|e| e.into_inner())
}

#[tauri::command]
fn library_info(lib: State<Lib>) -> LibraryInfo {
    let l = lock(&lib);
    LibraryInfo {
        root: l.root().to_string_lossy().into_owned(),
        thumbs: l.thumb_path("").parent().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default(),
        entries: l.entries().to_vec(),
    }
}

#[tauri::command]
async fn import_paths(app: AppHandle, lib: State<'_, Lib>, paths: Vec<String>) -> Result<ImportReport, String> {
    let lib = lib.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let paths: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
        let mut last = std::time::Instant::now();
        let report = lock(&lib).import(&paths, |done, total, file| {
            if done == 0 || done == total || last.elapsed().as_millis() > 60 {
                last = std::time::Instant::now();
                let _ = app.emit("import-progress", Progress { done, total, file: file.to_string() });
            }
        });
        // Les nouvelles broderies sont ensuite regardées par l'IA (si elle est activée).
        start_ai(&app);
        report
    })
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
fn update_entry(lib: State<Lib>, id: String, update: EntryUpdate) -> Result<Entry, String> {
    lock(&lib).update(&id, update)
}

#[tauri::command]
fn delete_entry(lib: State<Lib>, id: String) -> Result<(), String> {
    lock(&lib).delete(&id)
}

#[tauri::command]
async fn get_design(lib: State<'_, Lib>, id: String) -> Result<DesignView, String> {
    let d = lock(&lib).design(&id)?;
    Ok(DesignView {
        format: d.format.label().into(),
        label: d.label.clone(),
        width_mm: d.width_mm(),
        height_mm: d.height_mm(),
        min_x: d.min_x,
        min_y: d.min_y,
        max_x: d.max_x,
        max_y: d.max_y,
        stitch_count: d.stitch_count,
        blocks: d
            .blocks
            .into_iter()
            .map(|b| BlockView {
                color: b.thread.color,
                name: b.thread.name,
                pts: b.stitches.iter().flat_map(|s| [s.x, s.y, if s.jump { 1.0 } else { 0.0 }]).collect(),
            })
            .collect(),
    })
}

/// Chemin complet du fichier d'une broderie (pour l'ouvrir dans l'Explorateur).
#[tauri::command]
fn entry_path(lib: State<Lib>, id: String) -> Result<String, String> {
    let l = lock(&lib);
    let e = l.get(&id).ok_or("broderie introuvable")?;
    Ok(l.file_path(e).to_string_lossy().into_owned())
}

#[tauri::command]
fn find_duplicates(lib: State<Lib>) -> Vec<DuplicateGroup> {
    lock(&lib).find_duplicates()
}

#[tauri::command]
fn remove_duplicates(lib: State<Lib>, choices: Vec<GroupChoice>) -> Result<usize, String> {
    lock(&lib).remove_duplicates(&choices)
}

#[tauri::command]
fn export_entry(lib: State<Lib>, id: String, dir: String) -> Result<String, String> {
    lock(&lib).export(&id, &PathBuf::from(dir)).map(|p| p.to_string_lossy().into_owned())
}

#[tauri::command]
async fn backup(lib: State<'_, Lib>, dest: String) -> Result<usize, String> {
    let lib = lib.inner().clone();
    tauri::async_runtime::spawn_blocking(move || lock(&lib).backup(&PathBuf::from(dest)))
        .await
        .map_err(|e| e.to_string())?
}

fn library_root(app: &AppHandle) -> PathBuf {
    let base = app
        .path()
        .document_dir()
        .ok()
        .or_else(dirs::document_dir)
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("Broderies de Mamie")
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            let root = library_root(app.handle());
            let lib = Library::open(&root)?;
            // Les aperçus sont affichés directement depuis le dossier de la bibliothèque.
            let _ = app.asset_protocol_scope().allow_directory(&root, true);
            app.manage::<Lib>(Arc::new(Mutex::new(lib)));
            app.manage(Arc::new(AiState::default()));
            start_ai(app.handle());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            library_info,
            import_paths,
            update_entry,
            delete_entry,
            get_design,
            export_entry,
            find_duplicates,
            entry_path,
            remove_duplicates,
            ai_status,
            ai_set_key,
            ai_remove_key,
            ai_start,
            backup
        ])
        .run(tauri::generate_context!())
        .expect("impossible de démarrer l'application");
}
