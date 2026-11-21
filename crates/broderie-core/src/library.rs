//! La bibliothèque : un dossier sur le disque (par défaut « Documents/Broderies de Mamie »)
//!
//! ```text
//! Broderies de Mamie/
//! ├── Motifs/              ← les fichiers .pes / .vp3, lisibles par n'importe quel logiciel
//! ├── Corbeille/           ← les broderies supprimées (rien n'est jamais vraiment perdu)
//! ├── Aperçus/             ← les petites images des motifs (refaites automatiquement)
//! └── bibliotheque.json    ← noms, mots-clés, favoris, empreintes
//! ```

use crate::design::{self, Design, Thread};
use crate::render;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const EXTENSIONS: [&str; 4] = ["pes", "vp3", "pv3", "pec"];
const INDEX: &str = "bibliotheque.json";
const MOTIFS: &str = "Motifs";
const TRASH: &str = "Corbeille";
const THUMBS: &str = "Aperçus";
const THUMB_SIZE: u32 = 360;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub id: String,
    pub name: String,
    /// Nom du fichier dans le dossier « Motifs ».
    pub file_name: String,
    pub original_name: String,
    pub ext: String,
    pub format: Option<String>,
    pub size: u64,
    pub file_hash: String,
    pub design_hash: Option<String>,
    #[serde(rename = "widthMM")]
    pub width_mm: f32,
    #[serde(rename = "heightMM")]
    pub height_mm: f32,
    pub stitches: usize,
    pub colors: Vec<Thread>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub favorite: bool,
    pub added_at: u64,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub read_error: Option<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct IndexFile {
    version: u32,
    entries: Vec<Entry>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Duplicate {
    pub file: String,
    pub same_as: String,
}

#[derive(Debug, Default, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportReport {
    pub added: Vec<Entry>,
    pub duplicates: Vec<Duplicate>,
    pub ignored: usize,
    pub problems: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntryUpdate {
    pub name: Option<String>,
    pub tags: Option<Vec<String>>,
    pub favorite: Option<bool>,
}

pub struct Library {
    root: PathBuf,
    entries: Vec<Entry>,
}

/// Un fichier à importer : soit un fichier du disque, soit un fichier sorti d'un .zip.
enum Source {
    Path(PathBuf),
    Bytes(String, Vec<u8>),
}

impl Source {
    fn name(&self) -> String {
        match self {
            Source::Path(p) => p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
            Source::Bytes(n, _) => n.clone(),
        }
    }
    fn bytes(self) -> io::Result<Vec<u8>> {
        match self {
            Source::Path(p) => fs::read(p),
            Source::Bytes(_, b) => Ok(b),
        }
    }
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

fn ext_of(name: &str) -> String {
    Path::new(name).extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default()
}

pub fn is_embroidery(name: &str) -> bool {
    EXTENSIONS.contains(&ext_of(name).as_str())
}

/// « rose_des_vents-02.pes » → « rose des vents-02 »
pub fn nice_name(file_name: &str) -> String {
    let stem = Path::new(file_name).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let n = stem.replace('_', " ").split_whitespace().collect::<Vec<_>>().join(" ");
    if n.is_empty() { file_name.to_string() } else { n }
}

fn safe_file_stem(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| if "\\/:*?\"<>|".contains(c) || c.is_control() { ' ' } else { c })
        .collect();
    let s = s.split_whitespace().collect::<Vec<_>>().join(" ");
    let s = s.trim_matches('.').trim();
    if s.is_empty() { "broderie".into() } else { s.chars().take(120).collect() }
}

impl Library {
    /// Ouvre (ou crée) la bibliothèque dans `root`.
    pub fn open(root: impl Into<PathBuf>) -> io::Result<Self> {
        let root = root.into();
        for d in [MOTIFS, TRASH, THUMBS] {
            fs::create_dir_all(root.join(d))?;
        }
        let entries = match fs::read(root.join(INDEX)) {
            Ok(b) => serde_json::from_slice::<IndexFile>(&b).map(|i| i.entries).unwrap_or_default(),
            Err(_) => Vec::new(),
        };
        let mut lib = Library { root, entries };
        // Les fichiers effacés à la main du dossier disparaissent aussi de la liste.
        let before = lib.entries.len();
        let motifs = lib.root.join(MOTIFS);
        lib.entries.retain(|e| motifs.join(&e.file_name).exists());
        if lib.entries.len() != before {
            lib.save()?;
        }
        Ok(lib)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
    pub fn motifs_dir(&self) -> PathBuf {
        self.root.join(MOTIFS)
    }
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }
    pub fn thumb_path(&self, id: &str) -> PathBuf {
        self.root.join(THUMBS).join(format!("{id}.png"))
    }
    pub fn file_path(&self, e: &Entry) -> PathBuf {
        self.root.join(MOTIFS).join(&e.file_name)
    }
    pub fn get(&self, id: &str) -> Option<&Entry> {
        self.entries.iter().find(|e| e.id == id)
    }

    fn save(&self) -> io::Result<()> {
        let idx = IndexFile { version: 1, entries: self.entries.clone() };
        let json = serde_json::to_vec_pretty(&idx).map_err(io::Error::other)?;
        let tmp = self.root.join(format!("{INDEX}.tmp"));
        fs::write(&tmp, json)?;
        fs::rename(tmp, self.root.join(INDEX))
    }

    pub fn design(&self, id: &str) -> Result<Design, String> {
        let e = self.get(id).ok_or("broderie introuvable")?;
        let bytes = fs::read(self.file_path(e)).map_err(|e| e.to_string())?;
        design::parse(&bytes).map_err(|e| e.to_string())
    }

    /// Ajoute des fichiers, des dossiers entiers ou des .zip.
    /// Les doublons (même fichier, ou même dessin enregistré autrement) sont ignorés.
    pub fn import(&mut self, paths: &[PathBuf], mut progress: impl FnMut(usize, usize, &str)) -> ImportReport {
        let mut report = ImportReport::default();
        let mut todo = Vec::new();
        for p in paths {
            collect(p, &mut todo, &mut report);
        }

        let mut by_file: HashMap<String, String> =
            self.entries.iter().map(|e| (e.file_hash.clone(), e.name.clone())).collect();
        let mut by_design: HashMap<String, String> = self
            .entries
            .iter()
            .filter_map(|e| e.design_hash.clone().map(|h| (h, e.name.clone())))
            .collect();
        let total = todo.len();
        for (k, src) in todo.into_iter().enumerate() {
            let original = src.name();
            progress(k, total, &original);
            let bytes = match src.bytes() {
                Ok(b) => b,
                Err(e) => {
                    report.problems.push(format!("{original} : impossible de lire le fichier ({e})"));
                    continue;
                }
            };
            let fh = design::file_hash(&bytes);
            if let Some(same) = by_file.get(&fh) {
                report.duplicates.push(Duplicate { file: original, same_as: same.clone() });
                continue;
            }
            let parsed = design::parse(&bytes);
            let dh = parsed.as_ref().ok().map(|d| d.design_hash());
            if let Some(same) = dh.as_ref().and_then(|h| by_design.get(h)) {
                report.duplicates.push(Duplicate { file: original, same_as: same.clone() });
                continue;
            }
            match self.add(&original, &bytes, fh.clone(), parsed.as_ref().ok(), dh.clone(), k) {
                Ok(entry) => {
                    if let Err(e) = &parsed {
                        report.problems.push(format!("{original} : ajoutée, mais l'aperçu n'a pas pu être fait ({e})"));
                    }
                    by_file.insert(fh, entry.name.clone());
                    if let Some(h) = dh {
                        by_design.insert(h, entry.name.clone());
                    }
                    report.added.push(entry);
                }
                Err(e) => report.problems.push(format!("{original} : {e}")),
            }
        }
        progress(total, total, "");
        if !report.added.is_empty() {
            if let Err(e) = self.save() {
                report.problems.push(format!("Impossible d'enregistrer la liste : {e}"));
            }
        }
        report
    }

    fn add(
        &mut self,
        original: &str,
        bytes: &[u8],
        fh: String,
        d: Option<&Design>,
        dh: Option<String>,
        order: usize,
    ) -> io::Result<Entry> {
        let ext = ext_of(original);
        let name = nice_name(original);
        let file_name = self.unique_file_name(&name, &ext, None);
        fs::write(self.motifs_dir().join(&file_name), bytes)?;
        let mut id = fh[..12].to_string();
        while self.get(&id).is_some() {
            id.push('x');
        }
        if let Some(png) = d.and_then(|d| render::render_png(d, THUMB_SIZE)) {
            let _ = fs::write(self.thumb_path(&id), png);
        }
        let entry = Entry {
            id,
            name,
            file_name,
            original_name: original.to_string(),
            ext: ext.to_uppercase(),
            format: d.map(|d| d.format.label().to_string()),
            size: bytes.len() as u64,
            file_hash: fh,
            design_hash: dh,
            width_mm: d.map(|d| d.width_mm()).unwrap_or(0.0),
            height_mm: d.map(|d| d.height_mm()).unwrap_or(0.0),
            stitches: d.map(|d| d.stitch_count).unwrap_or(0),
            colors: d.map(|d| d.threads()).unwrap_or_default(),
            tags: Vec::new(),
            favorite: false,
            added_at: now_ms() + order as u64,
            label: d.map(|d| d.label.clone()).unwrap_or_default(),
            read_error: None,
        };
        self.entries.push(entry.clone());
        Ok(entry)
    }

    fn unique_file_name(&self, name: &str, ext: &str, except: Option<&str>) -> String {
        let used: HashSet<String> = fs::read_dir(self.motifs_dir())
            .map(|rd| rd.filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().to_lowercase()).collect())
            .unwrap_or_default();
        let stem = safe_file_stem(name);
        let mut candidate = format!("{stem}.{ext}");
        let mut k = 2;
        while used.contains(&candidate.to_lowercase()) && Some(candidate.as_str()) != except {
            candidate = format!("{stem} ({k}).{ext}");
            k += 1;
        }
        candidate
    }

    /// Renommer, changer les mots-clés, mettre en favori.
    pub fn update(&mut self, id: &str, up: EntryUpdate) -> Result<Entry, String> {
        let pos = self.entries.iter().position(|e| e.id == id).ok_or("broderie introuvable")?;
        if let Some(name) = up.name.map(|n| n.trim().to_string()).filter(|n| !n.is_empty()) {
            if name != self.entries[pos].name {
                // On renomme aussi le fichier pour que le dossier reste bien rangé.
                let e = &self.entries[pos];
                let ext = ext_of(&e.file_name);
                let new_file = self.unique_file_name(&name, &ext, Some(&e.file_name));
                if new_file != e.file_name
                    && fs::rename(self.motifs_dir().join(&e.file_name), self.motifs_dir().join(&new_file)).is_ok()
                {
                    self.entries[pos].file_name = new_file;
                }
                self.entries[pos].name = name;
            }
        }
        if let Some(tags) = up.tags {
            let mut seen = HashSet::new();
            self.entries[pos].tags = tags
                .into_iter()
                .map(|t| t.trim().to_string())
                .filter(|t| !t.is_empty() && seen.insert(t.to_lowercase()))
                .collect();
        }
        if let Some(f) = up.favorite {
            self.entries[pos].favorite = f;
        }
        self.save().map_err(|e| e.to_string())?;
        Ok(self.entries[pos].clone())
    }

    /// Met la broderie à la corbeille (dossier « Corbeille » de la bibliothèque).
    pub fn delete(&mut self, id: &str) -> Result<(), String> {
        let pos = self.entries.iter().position(|e| e.id == id).ok_or("broderie introuvable")?;
        let e = self.entries.remove(pos);
        let src = self.motifs_dir().join(&e.file_name);
        let mut dst = self.root.join(TRASH).join(&e.file_name);
        let mut k = 2;
        while dst.exists() {
            dst = self.root.join(TRASH).join(format!("{} ({k}).{}", safe_file_stem(&e.name), ext_of(&e.file_name)));
            k += 1;
        }
        if fs::rename(&src, &dst).is_err() {
            let _ = fs::copy(&src, &dst).and_then(|_| fs::remove_file(&src));
        }
        let _ = fs::remove_file(self.thumb_path(&e.id));
        self.save().map_err(|e| e.to_string())
    }

    /// Copie une broderie dans un dossier (par exemple une clé USB). Renvoie le chemin créé.
    pub fn export(&self, id: &str, dest_dir: &Path) -> Result<PathBuf, String> {
        let e = self.get(id).ok_or("broderie introuvable")?;
        let ext = ext_of(&e.file_name);
        let stem = safe_file_stem(&e.name);
        let mut dst = dest_dir.join(format!("{stem}.{ext}"));
        let mut k = 2;
        while dst.exists() {
            dst = dest_dir.join(format!("{stem} ({k}).{ext}"));
            k += 1;
        }
        fs::copy(self.file_path(e), &dst).map_err(|e| e.to_string())?;
        Ok(dst)
    }

    /// Enregistre toutes les broderies dans un seul fichier .zip.
    pub fn backup(&self, dest: &Path) -> Result<usize, String> {
        let f = fs::File::create(dest).map_err(|e| e.to_string())?;
        let mut z = zip::ZipWriter::new(f);
        let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        let mut used = HashSet::new();
        for e in &self.entries {
            let ext = ext_of(&e.file_name);
            let stem = safe_file_stem(&e.name);
            let mut name = format!("{stem}.{ext}");
            let mut k = 2;
            while !used.insert(name.to_lowercase()) {
                name = format!("{stem} ({k}).{ext}");
                k += 1;
            }
            let bytes = fs::read(self.file_path(e)).map_err(|e| e.to_string())?;
            z.start_file(name, opts).map_err(|e| e.to_string())?;
            z.write_all(&bytes).map_err(|e| e.to_string())?;
        }
        z.finish().map_err(|e| e.to_string())?;
        Ok(self.entries.len())
    }
}

fn collect(p: &Path, todo: &mut Vec<Source>, report: &mut ImportReport) {
    if p.is_dir() {
        for e in walkdir::WalkDir::new(p).sort_by_file_name().into_iter().filter_map(|e| e.ok()) {
            if e.file_type().is_file() {
                collect(e.path(), todo, report);
            }
        }
        return;
    }
    let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    if ext_of(&name) == "zip" {
        match read_zip(p) {
            Ok(files) => {
                for (n, b) in files {
                    if is_embroidery(&n) {
                        todo.push(Source::Bytes(n, b));
                    } else {
                        report.ignored += 1;
                    }
                }
            }
            Err(e) => report.problems.push(format!("{name} : {e}")),
        }
    } else if is_embroidery(&name) {
        todo.push(Source::Path(p.to_path_buf()));
    } else {
        report.ignored += 1;
    }
}

fn read_zip(p: &Path) -> Result<Vec<(String, Vec<u8>)>, String> {
    let f = fs::File::open(p).map_err(|e| e.to_string())?;
    let mut z = zip::ZipArchive::new(f).map_err(|_| "ce fichier .zip est abîmé".to_string())?;
    let mut out = Vec::new();
    for i in 0..z.len() {
        let Ok(mut file) = z.by_index(i) else { continue };
        if file.is_dir() {
            continue;
        }
        let name = file.name().rsplit(['/', '\\']).next().unwrap_or_default().to_string();
        if !is_embroidery(&name) {
            out.push((name, Vec::new()));
            continue;
        }
        let mut b = Vec::with_capacity(file.size() as usize);
        if file.read_to_end(&mut b).is_ok() {
            out.push((name, b));
        }
    }
    Ok(out)
}
