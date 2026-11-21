//! La bibliothèque : un dossier sur le disque (par défaut « Documents/Broderies de Mamie »)
//!
//! ```text
//! Broderies de Mamie/
//! ├── Motifs/              ← les fichiers (.pes, .jef, .vp3…), rangés par dossier importé
//! │   ├── Noël/            ← un dossier par dossier ou .zip ajouté, à son nom
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
    /// Chemin du fichier dans le dossier « Motifs » (« Noël/Rose.pes », toujours avec « / »).
    pub file_name: String,
    /// Le dossier (ou le .zip) d'où vient la broderie ; vide si elle a été ajoutée seule.
    #[serde(default)]
    pub folder: String,
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
    /// Ce que l'IA voit sur le motif (« un ange aux ailes déployées »), pour la recherche.
    #[serde(default)]
    pub ai_description: String,
    #[serde(default)]
    pub ai_keywords: Vec<String>,
    /// L'IA a déjà regardé ce motif.
    #[serde(default)]
    pub ai_done: bool,
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
    /// Paires de fichiers (empreintes) que Mamie a décidé de garder toutes les deux :
    /// on ne les lui repropose plus comme doublons.
    #[serde(default)]
    distinct: Vec<(String, String)>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Duplicate {
    pub file: String,
    pub same_as: String,
    /// Le dossier où se trouve la broderie déjà rangée (vide si aucun).
    #[serde(default)]
    pub same_folder: String,
}

impl Duplicate {
    fn of(file: String, e: &Entry) -> Self {
        Duplicate { file, same_as: e.name.clone(), same_folder: e.folder.clone() }
    }
}

#[derive(Debug, Default, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportReport {
    pub added: Vec<Entry>,
    pub duplicates: Vec<Duplicate>,
    /// Ajoutées, mais qui ressemblent à une broderie déjà rangée (même motif, autre format…) :
    /// à vérifier avec « Chercher les doublons ».
    pub similar: Vec<Duplicate>,
    pub ignored: usize,
    pub problems: Vec<String>,
}

/// Un groupe de broderies qui semblent être le même motif. La première est celle qu'on
/// conseille de garder (le PES, sinon la plus ancienne).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateGroup {
    pub ids: Vec<String>,
}

/// Ce que Mamie a choisi pour un groupe : les broderies gardées et celles à mettre à la corbeille.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupChoice {
    pub keep: Vec<String>,
    pub remove: Vec<String>,
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
    distinct: HashSet<(String, String)>,
}

fn pair(a: &str, b: &str) -> (String, String) {
    if a <= b { (a.to_string(), b.to_string()) } else { (b.to_string(), a.to_string()) }
}

/// Un fichier à importer : soit un fichier du disque, soit un fichier sorti d'un .zip.
enum Source {
    Path(PathBuf, String),
    Bytes(String, Vec<u8>, String),
}

impl Source {
    fn name(&self) -> String {
        match self {
            Source::Path(p, _) => p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
            Source::Bytes(n, _, _) => n.clone(),
        }
    }
    /// Le dossier de la bibliothèque où ranger la broderie.
    fn folder(&self) -> String {
        match self {
            Source::Path(_, f) | Source::Bytes(_, _, f) => f.clone(),
        }
    }
    fn bytes(self) -> io::Result<Vec<u8>> {
        match self {
            Source::Path(p, _) => fs::read(p),
            Source::Bytes(_, b, _) => Ok(b),
        }
    }
}

/// Nom de dossier (ou de .zip) utilisable comme dossier de la bibliothèque.
fn folder_name(p: &Path) -> String {
    let stem = p.file_stem().map(|s| s.to_string_lossy().replace('_', " ")).unwrap_or_default();
    let s = safe_file_stem(&stem);
    if s == "broderie" && stem.trim().is_empty() { String::new() } else { s }
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
        let index = fs::read(root.join(INDEX))
            .ok()
            .and_then(|b| serde_json::from_slice::<IndexFile>(&b).ok())
            .unwrap_or_default();
        let (entries, distinct) = (index.entries, index.distinct.into_iter().collect());
        let mut lib = Library { root, entries, distinct };
        // Les fichiers effacés à la main du dossier disparaissent aussi de la liste.
        let before = lib.entries.len();
        let motifs = lib.root.join(MOTIFS);
        lib.entries.retain(|e| { let mut p = motifs.clone(); p.extend(e.file_name.split('/')); p.exists() });
        let mut changed = lib.entries.len() != before;
        // Empreintes de forme (calculées une fois pour les anciennes broderies) et aperçus manquants.
        for i in 0..lib.entries.len() {
            let e = &lib.entries[i];
            let bits = e.shape.as_deref().and_then(design::from_hex);
            let thumb_missing = !lib.thumb_path(&e.id).exists();
            if e.format.is_some() && (bits.is_none() || thumb_missing) {
                if let Ok(d) = fs::read(lib.file_path(e))
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
        self.motif_path(&e.file_name)
    }
    /// Chemin sur le disque d'un fichier de « Motifs » (« Noël/Rose.pes » → …\Motifs\Noël\Rose.pes).
    fn motif_path(&self, rel: &str) -> PathBuf {
        let mut p = self.root.join(MOTIFS);
        p.extend(rel.split('/'));
        p
    }
    pub fn get(&self, id: &str) -> Option<&Entry> {
        self.entries.iter().find(|e| e.id == id)
    }

    fn save(&self) -> io::Result<()> {
        let mut distinct: Vec<_> = self.distinct.iter().cloned().collect();
        distinct.sort();
        let idx = IndexFile { version: 1, entries: self.entries.clone(), distinct };
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
            collect(p, None, &mut todo, &mut report);
        }

        let total = todo.len();
        for (k, src) in todo.into_iter().enumerate() {
            let original = src.name();
            let folder = src.folder();
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
                report.duplicates.push(Duplicate::of(original, same));
                continue;
            }
            let parsed = design::parse_named(&bytes, &original);
            let dh = parsed.as_ref().ok().map(|d| d.design_hash());
            let mut similar_to = None;
            if let Ok(d) = &parsed {
                // Exactement les mêmes points, dans un format qui n'est pas meilleur : rien à ajouter.
                if let Some(e) = self.entries.iter().find(|e| {
                    e.design_hash.as_deref() == dh.as_deref() && d.format.preference() >= e.preference()
                }) {
                    report.duplicates.push(Duplicate::of(original, e));
                    continue;
                }
                // Le même motif autrement (autre format, autre version) : on l'ajoute, mais on le signale.
                let shape = d.shape();
                similar_to = self
                    .entries
                    .iter()
                    .find(|e| e.design_hash.as_deref() == dh.as_deref() || same_motif(e, d.width_mm(), d.height_mm(), d.stitch_count, &shape))
                    .map(|e| (e.name.clone(), e.folder.clone()));
            }
            match self.add(&original, &folder, &bytes, fh, parsed.as_ref().ok(), dh, k) {
                Ok(entry) => {
                    if let Some((same_as, same_folder)) = similar_to {
                        report.similar.push(Duplicate { file: original.clone(), same_as, same_folder });
                    }
                    if let Err(e) = &parsed {
                        report.problems.push(format!("{original} : ajoutée, mais l'aperçu n'a pas pu être fait ({e})"));
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

    #[allow(clippy::too_many_arguments)]
    fn add(
        &mut self,
        original: &str,
        folder: &str,
        bytes: &[u8],
        fh: String,
        d: Option<&Design>,
        dh: Option<String>,
        order: usize,
    ) -> io::Result<Entry> {
        let ext = ext_of(original);
        let name = nice_name(original);
        fs::create_dir_all(self.motifs_dir().join(folder))?;
        let file_name = self.unique_file_name(folder, &name, &ext, None);
        fs::write(self.motif_path(&file_name), bytes)?;
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
            folder: folder.to_string(),
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
            ai_description: String::new(),
            ai_keywords: Vec::new(),
            ai_done: false,
        };
        let mut entry = entry;
        if let Some(d) = d {
            entry.set_shape(d);
        }
        self.entries.push(entry.clone());
        Ok(entry)
    }

    /// Range un fichier du dossier « Motifs » dans la « Corbeille ».
    fn move_to_trash(&self, file_name: &str, name: &str) {
        let src = self.motif_path(file_name);
        let base = file_name.rsplit('/').next().unwrap_or(file_name);
        let mut dst = self.root.join(TRASH).join(base);
        let mut k = 2;
        while dst.exists() {
            dst = self.root.join(TRASH).join(format!("{} ({k}).{}", safe_file_stem(name), ext_of(file_name)));
            k += 1;
        }
        if fs::rename(&src, &dst).is_err() {
            let _ = fs::copy(&src, &dst).and_then(|_| fs::remove_file(&src));
        }
    }

    /// Cherche les doublons dans la collection, sans rien supprimer : le même motif en plusieurs
    /// exemplaires ou plusieurs formats. Les paires déjà gardées par Mamie sont ignorées.
    pub fn find_duplicates(&self) -> Vec<DuplicateGroup> {
        let mut order: Vec<usize> = (0..self.entries.len()).collect();
        order.sort_by_key(|&i| (self.entries[i].preference(), self.entries[i].added_at));
        let mut groups: Vec<Vec<usize>> = Vec::new();
        for i in order {
            let e = &self.entries[i];
            let twin = groups.iter().position(|g| {
                let o = &self.entries[g[0]];
                !self.distinct.contains(&pair(&o.file_hash, &e.file_hash))
                    && (o.file_hash == e.file_hash
                        || (e.design_hash.is_some() && o.design_hash == e.design_hash)
                        || e.shape_bits.as_deref().is_some_and(|s| same_motif(o, e.width_mm, e.height_mm, e.stitches, s)))
            });
            match twin {
                Some(g) => groups[g].push(i),
                None => groups.push(vec![i]),
            }
        }
        groups
            .into_iter()
            .filter(|g| g.len() > 1)
            .map(|g| DuplicateGroup { ids: g.into_iter().map(|i| self.entries[i].id.clone()).collect() })
            .collect()
    }

    /// Applique les choix de Mamie : les broderies cochées vont à la corbeille (leurs mots-clés
    /// et leur étoile passent à celle qu'on garde), et les broderies gardées ensemble ne seront
    /// plus proposées comme doublons. Renvoie le nombre de broderies mises à la corbeille.
    pub fn remove_duplicates(&mut self, choices: &[GroupChoice]) -> Result<usize, String> {
        let mut removed = 0;
        for c in choices {
            let Some(target) = c.keep.first().and_then(|id| self.entries.iter().position(|e| &e.id == id)) else {
                continue; // on garde toujours au moins une broderie par groupe
            };
            for id in &c.remove {
                let Some(i) = self.entries.iter().position(|e| &e.id == id) else { continue };
                if i == target || c.keep.contains(id) {
                    continue;
                }
                let gone = self.entries[i].clone();
                let keep = &mut self.entries[target];
                for t in &gone.tags {
                    if !keep.tags.iter().any(|x| x.to_lowercase() == t.to_lowercase()) {
                        keep.tags.push(t.clone());
                    }
                }
                keep.favorite |= gone.favorite;
                self.move_to_trash(&gone.file_name, &gone.name);
                let _ = fs::remove_file(self.thumb_path(&gone.id));
                self.entries.retain(|e| e.id != gone.id);
                removed += 1;
            }
            let kept: Vec<String> = c
                .keep
                .iter()
                .filter_map(|id| self.get(id).map(|e| e.file_hash.clone()))
                .collect();
            for (k, a) in kept.iter().enumerate() {
                for b in &kept[k + 1..] {
                    self.distinct.insert(pair(a, b));
                }
            }
        }
        self.save().map_err(|e| e.to_string())?;
        Ok(removed)
    }

    /// Un nom de fichier libre dans le dossier `folder` de « Motifs » ; renvoie « dossier/nom.ext ».
    fn unique_file_name(&self, folder: &str, name: &str, ext: &str, except: Option<&str>) -> String {
        let used: HashSet<String> = fs::read_dir(self.motifs_dir().join(folder))
            .map(|rd| rd.filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().to_lowercase()).collect())
            .unwrap_or_default();
        let with_dir = |f: String| if folder.is_empty() { f } else { format!("{folder}/{f}") };
        let stem = safe_file_stem(name);
        let mut candidate = format!("{stem}.{ext}");
        let mut k = 2;
        while used.contains(&candidate.to_lowercase()) && Some(with_dir(candidate.clone()).as_str()) != except {
            candidate = format!("{stem} ({k}).{ext}");
            k += 1;
        }
        with_dir(candidate)
    }

    /// Les broderies que l'IA n'a pas encore regardées (au plus `limit`), et leur nombre total.
    pub fn ai_pending(&self, limit: usize) -> (Vec<String>, usize) {
        let pending: Vec<&Entry> = self.entries.iter().filter(|e| e.format.is_some() && !e.ai_done).collect();
        (pending.iter().take(limit).map(|e| e.id.clone()).collect(), pending.len())
    }

    /// L'image d'un motif à montrer à l'IA (sur fond blanc).
    pub fn ai_image(&self, id: &str) -> Result<Vec<u8>, String> {
        let d = self.design(id)?;
        render::render_png_on(&d, 384, Some((255, 255, 255))).ok_or_else(|| "image impossible".to_string())
    }

    /// Enregistre ce que l'IA a vu sur un motif.
    pub fn set_ai(&mut self, id: &str, description: &str, keywords: &[String]) -> Result<(), String> {
        let e = self.entries.iter_mut().find(|e| e.id == id).ok_or("broderie introuvable")?;
        e.ai_description = description.trim().to_string();
        let mut seen = HashSet::new();
        e.ai_keywords = keywords
            .iter()
            .map(|k| k.trim().to_lowercase())
            .filter(|k| !k.is_empty() && k.chars().count() <= 40 && seen.insert(k.clone()))
            .take(20)
            .collect();
        e.ai_done = true;
        Ok(())
    }

    /// Enregistre la liste sur le disque (après une série d'analyses par l'IA).
    pub fn flush(&self) -> Result<(), String> {
        self.save().map_err(|e| e.to_string())
    }

    /// Renommer, changer les mots-clés, mettre en favori.
    pub fn update(&mut self, id: &str, up: EntryUpdate) -> Result<Entry, String> {
        let pos = self.entries.iter().position(|e| e.id == id).ok_or("broderie introuvable")?;
        if let Some(name) = up.name.map(|n| n.trim().to_string()).filter(|n| !n.is_empty()) {
            if name != self.entries[pos].name {
                // On renomme aussi le fichier pour que le dossier reste bien rangé.
                let e = &self.entries[pos];
                let ext = ext_of(&e.file_name);
                let dir = e.file_name.rsplit_once('/').map(|(d, _)| d.to_string()).unwrap_or_default();
                let new_file = self.unique_file_name(&dir, &name, &ext, Some(&e.file_name));
                if new_file != e.file_name
                    && fs::rename(self.motif_path(&e.file_name), self.motif_path(&new_file)).is_ok()
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
            let dir = if e.folder.is_empty() { String::new() } else { format!("{}/", e.folder) };
            let mut name = format!("{dir}{stem}.{ext}");
            let mut k = 2;
            while !used.insert(name.to_lowercase()) {
                name = format!("{dir}{stem} ({k}).{ext}");
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

/// Rassemble les fichiers à importer. Un dossier ou un .zip ajouté donne son nom au dossier
/// de la bibliothèque où ses broderies sont rangées ; un fichier ajouté seul n'a pas de dossier.
fn collect(p: &Path, folder: Option<&str>, todo: &mut Vec<Source>, report: &mut ImportReport) {
    if p.is_dir() {
        let folder = folder.map(str::to_string).unwrap_or_else(|| folder_name(p));
        for e in walkdir::WalkDir::new(p).sort_by_file_name().into_iter().filter_map(|e| e.ok()) {
            if e.file_type().is_file() {
                collect(e.path(), Some(&folder), todo, report);
            }
        }
        return;
    }
    let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    if ext_of(&name) == "zip" {
        // Un .zip garde son propre nom, même s'il est dans un dossier ajouté.
        let zip_folder = folder_name(p);
        match read_zip(p) {
            Ok(files) => {
                for (n, b) in files {
                    if is_embroidery(&n) {
                        todo.push(Source::Bytes(n, b, zip_folder.clone()));
                    } else {
                        report.ignored += 1;
                    }
                }
            }
            Err(e) => report.problems.push(format!("{name} : {e}")),
        }
    } else if is_embroidery(&name) {
        todo.push(Source::Path(p.to_path_buf(), folder.unwrap_or_default().to_string()));
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
