//! Application de bureau : fait le lien entre l'interface (dossier `ui/`) et `broderie-core`.

use broderie_core::library::{Entry, EntryUpdate, ImportReport, Library};
use serde::Serialize;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager, State};

type Lib = Arc<Mutex<Library>>;

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
        lock(&lib).import(&paths, |done, total, file| {
            if done == 0 || done == total || last.elapsed().as_millis() > 60 {
                last = std::time::Instant::now();
                let _ = app.emit("import-progress", Progress { done, total, file: file.to_string() });
            }
        })
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
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            library_info,
            import_paths,
            update_entry,
            delete_entry,
            get_design,
            export_entry,
            backup
        ])
        .run(tauri::generate_context!())
        .expect("impossible de démarrer l'application");
}
