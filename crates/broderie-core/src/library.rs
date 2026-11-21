//! La bibliothèque : un dossier sur le disque (par défaut « Documents/Broderies de Mamie »)
//!
//! ```text
//! Broderies de Mamie/
//! ├── Motifs/              ← les fichiers (.pes, .jef, .vp3…), lisibles par n'importe quel logiciel
//! ├── Corbeille/           ← les broderies supprimées (rien n'est jamais vraiment perdu)
//! ├── Aperçus/             ← les petites images des motifs (refaites automatiquement)
//! └── bibliotheque.json    ← noms, mots-clés, favoris, empreintes
//! ```

use crate::design::{self, Design, Thread};
use crate::render;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const EXTENSIONS: [&str; 10] = ["pes", "pec", "vp3", "pv3", "jef", "sew", "exp", "dst", "hus", "vip"];
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
    /// Empreinte de la forme du motif (voir `Design::shape`), pour repérer le même motif
    /// enregistré dans un autre format.
    #[serde(default)]
    pub shape: Option<String>,
    #[serde(skip)]
    shape_bits: Option<Vec<u8>>,
}

impl Entry {
    fn set_shape(&mut self, d: &Design) {
        let bits = d.shape();
        self.shape = Some(design::to_hex(&bits));
        self.shape_bits = Some(bits);
    }

    /// Ordre de préférence du format (0 = PES, le format de la Brother de Mamie).
    fn preference(&self) -> u8 {
        match self.format.as_deref() {
            Some("PES") => 0,
            Some("PEC") => 1,
            _ => 2,
        }
    }
}

/// Le même motif ? Même taille (à 1 mm ou 2 % près), à peu près le même nombre de points
/// et presque la même forme.
fn same_motif(e: &Entry, w: f32, h: f32, stitches: usize, shape: &[u8]) -> bool {
    let close = |a: f32, b: f32| (a - b).abs() <= 1.0f32.max(a.max(b) * 0.02);
    if !(close(e.width_mm, w) && close(e.height_mm, h)) {
        return false;
    }
    let (a, b) = (e.stitches.max(1) as f32, stitches.max(1) as f32);
    if a.max(b) / a.min(b) > 1.15 {
        return false;
    }
    e.shape_bits.as_deref().is_some_and(|s| design::shape_similarity(s, shape) >= 0.85)
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
    /// Motifs déjà présents dans un autre format, remplacés par la version PES qu'on vient d'ajouter.
    pub replaced: Vec<Duplicate>,
    pub ignored: usize,
    pub problems: Vec<String>,
}

#[derive(Debug, Default, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DedupeReport {
    /// Les broderies retirées (rangées dans la Corbeille) et celle qui est gardée à leur place.
    pub removed: Vec<Duplicate>,
    pub kept: usize,
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
        let mut changed = lib.entries.len() != before;
        // Empreintes de forme (calculées une fois pour les anciennes broderies) et aperçus manquants.
        for i in 0..lib.entries.len() {
            let e = &lib.entries[i];
            let bits = e.shape.as_deref().and_then(design::from_hex);
            let thumb_missing = !lib.thumb_path(&e.id).exists();
            if e.format.is_some() && (bits.is_none() || thumb_missing) {
                if let Ok(d) = fs::read(motifs.join(&e.file_name))
                    .map_err(|_| ())
                    .and_then(|b| design::parse_named(&b, &e.file_name).map_err(|_| ()))
                {
                    if thumb_missing {
                        if let Some(png) = render::render_png(&d, THUMB_SIZE) {
                            let _ = fs::write(lib.thumb_path(&e.id), png);
                        }
                    }
                    if bits.is_none() {
                        lib.entries[i].set_shape(&d);
                        changed = true;
                    }
                    continue;
                }
            }
            lib.entries[i].shape_bits = bits;
        }
        if changed {
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
        design::parse_named(&bytes, &e.file_name).map_err(|e| e.to_string())
    }

    /// Ajoute des fichiers, des dossiers entiers ou des .zip.
    /// Les doublons (même fichier, ou même dessin enregistré autrement) sont ignorés.
    pub fn import(&mut self, paths: &[PathBuf], mut progress: impl FnMut(usize, usize, &str)) -> ImportReport {
        let mut report = ImportReport::default();
        let mut todo = Vec::new();
        for p in paths {
            collect(p, &mut todo, &mut report);
        }

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
            if let Some(same) = self.entries.iter().find(|e| e.file_hash == fh) {
                report.duplicates.push(Duplicate { file: original, same_as: same.name.clone() });
                continue;
            }
            let parsed = design::parse_named(&bytes, &original);
            let dh = parsed.as_ref().ok().map(|d| d.design_hash());
            if let Ok(d) = &parsed {
                // Le même motif est-il déjà là (même dessin, éventuellement dans un autre format) ?
                let shape = d.shape();
                let found = self.entries.iter().position(|e| {
                    e.design_hash.as_deref() == dh.as_deref()
                        || same_motif(e, d.width_mm(), d.height_mm(), d.stitch_count, &shape)
                });
                if let Some(i) = found {
                    let same_as = self.entries[i].name.clone();
                    if d.format.preference() < self.entries[i].preference() {
                        // On garde la version PES : elle remplace l'autre format.
                        match self.replace_file(i, &original, &bytes, fh, d, dh) {
                            Ok(()) => report.replaced.push(Duplicate { file: original, same_as }),
                            Err(e) => report.problems.push(format!("{original} : {e}")),
                        }
                    } else {
                        report.duplicates.push(Duplicate { file: original, same_as });
                    }
                    continue;
                }
            }
            match self.add(&original, &bytes, fh, parsed.as_ref().ok(), dh, k) {
                Ok(entry) => {
                    if let Err(e) = &parsed {
                        report.problems.push(format!("{original} : ajoutée, mais l'aperçu n'a pas pu être fait ({e})"));
                    }
                    report.added.push(entry);
                }
                Err(e) => report.problems.push(format!("{original} : {e}")),
            }
        }
        progress(total, total, "");
        if !report.added.is_empty() || !report.replaced.is_empty() {
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
            shape: None,
            shape_bits: None,
        };
        let mut entry = entry;
        if let Some(d) = d {
            entry.set_shape(d);
        }
        self.entries.push(entry.clone());
        Ok(entry)
    }

    /// Remplace le fichier d'une broderie par une autre version du même motif (le PES),
    /// en gardant son nom, ses mots-clés et son étoile. L'ancien fichier va à la corbeille.
    fn replace_file(
        &mut self,
        i: usize,
        original: &str,
        bytes: &[u8],
        fh: String,
        d: &Design,
        dh: Option<String>,
    ) -> io::Result<()> {
        let ext = ext_of(original);
        let old = self.entries[i].clone();
        self.move_to_trash(&old.file_name, &old.name);
        let file_name = self.unique_file_name(&old.name, &ext, None);
        fs::write(self.motifs_dir().join(&file_name), bytes)?;
        let _ = fs::remove_file(self.thumb_path(&old.id));
        let mut id = fh[..12].to_string();
        while self.get(&id).is_some() {
            id.push('x');
        }
        if let Some(png) = render::render_png(d, THUMB_SIZE) {
            let _ = fs::write(self.thumb_path(&id), png);
        }
        let e = &mut self.entries[i];
        e.id = id;
        e.file_name = file_name;
        e.original_name = original.to_string();
        e.ext = ext.to_uppercase();
        e.format = Some(d.format.label().to_string());
        e.size = bytes.len() as u64;
        e.file_hash = fh;
        e.design_hash = dh;
        e.width_mm = d.width_mm();
        e.height_mm = d.height_mm();
        e.stitches = d.stitch_count;
        e.colors = d.threads();
        e.label = d.label.clone();
        e.read_error = None;
        e.set_shape(d);
        Ok(())
    }

    /// Range un fichier du dossier « Motifs » dans la « Corbeille ».
    fn move_to_trash(&self, file_name: &str, name: &str) {
        let src = self.motifs_dir().join(file_name);
        let mut dst = self.root.join(TRASH).join(file_name);
        let mut k = 2;
        while dst.exists() {
            dst = self.root.join(TRASH).join(format!("{} ({k}).{}", safe_file_stem(name), ext_of(file_name)));
            k += 1;
        }
        if fs::rename(&src, &dst).is_err() {
            let _ = fs::copy(&src, &dst).and_then(|_| fs::remove_file(&src));
        }
    }

    /// Cherche les doublons déjà présents dans la collection : le même motif en plusieurs
    /// exemplaires ou plusieurs formats. On garde le PES (sinon le plus ancien), on lui
    /// ajoute les mots-clés et l'étoile des autres, et les autres vont à la corbeille.
    pub fn dedupe(&mut self) -> Result<DedupeReport, String> {
        let mut order: Vec<usize> = (0..self.entries.len()).collect();
        order.sort_by_key(|&i| (self.entries[i].preference(), self.entries[i].added_at));
        let mut kept: Vec<usize> = Vec::new();
        let mut removed: Vec<(usize, usize)> = Vec::new();
        for i in order {
            let e = &self.entries[i];
            let twin = kept.iter().copied().find(|&k| {
                let o = &self.entries[k];
                o.file_hash == e.file_hash
                    || (e.design_hash.is_some() && o.design_hash == e.design_hash)
                    || e.shape_bits.as_deref().is_some_and(|s| same_motif(o, e.width_mm, e.height_mm, e.stitches, s))
            });
            match twin {
                Some(k) => removed.push((i, k)),
                None => kept.push(i),
            }
        }
        let mut report = DedupeReport { kept: kept.len(), ..Default::default() };
        for &(i, k) in &removed {
            let (tags, fav) = (self.entries[i].tags.clone(), self.entries[i].favorite);
            let keep = &mut self.entries[k];
            for t in tags {
                if !keep.tags.iter().any(|x| x.to_lowercase() == t.to_lowercase()) {
                    keep.tags.push(t);
                }
            }
            keep.favorite |= fav;
            let gone = &self.entries[i];
            report.removed.push(Duplicate { file: gone.file_name.clone(), same_as: self.entries[k].name.clone() });
            self.move_to_trash(&gone.file_name, &gone.name);
            let _ = fs::remove_file(self.thumb_path(&gone.id));
        }
        if !removed.is_empty() {
            let gone: HashSet<usize> = removed.iter().map(|&(i, _)| i).collect();
            let mut i = 0;
            self.entries.retain(|_| {
                let keep = !gone.contains(&i);
                i += 1;
                keep
            });
            self.save().map_err(|e| e.to_string())?;
        }
        Ok(report)
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
        self.move_to_trash(&e.file_name, &e.name);
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
