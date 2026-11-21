//! Lecture des fichiers de broderie.
//!
//! Formats pris en charge :
//! - **PES** (Brother, toutes versions) : on lit le bloc PEC, et les vraies couleurs RGB
//!   des versions 5 et plus quand elles sont présentes ;
//! - **PEC** (Brother) ;
//! - **VP3** (Husqvarna Viking / Pfaff). Les fichiers `.pv3` sont lus de la même façon.
//!
//! Le format est reconnu d'après le contenu du fichier, pas l'extension.
//! Unités : 1 unité = 0,1 mm.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("format de fichier non reconnu")]
    UnknownFormat,
    #[error("fichier incomplet ou abîmé")]
    Truncated,
    #[error("aucun point de broderie trouvé")]
    Empty,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Format {
    #[serde(rename = "PES")]
    Pes,
    #[serde(rename = "PEC")]
    Pec,
    #[serde(rename = "VP3")]
    Vp3,
    #[serde(rename = "JEF")]
    Jef,
    #[serde(rename = "SEW")]
    Sew,
    #[serde(rename = "EXP")]
    Exp,
    #[serde(rename = "DST")]
    Dst,
    #[serde(rename = "HUS")]
    Hus,
    #[serde(rename = "VIP")]
    Vip,
}

impl Format {
    pub fn label(self) -> &'static str {
        match self {
            Format::Pes => "PES",
            Format::Pec => "PEC",
            Format::Vp3 => "VP3",
            Format::Jef => "JEF",
            Format::Sew => "SEW",
            Format::Exp => "EXP",
            Format::Dst => "DST",
            Format::Hus => "HUS",
            Format::Vip => "VIP",
        }
    }

    /// Ordre de préférence quand un même motif existe en plusieurs formats :
    /// la machine de Mamie est une Brother, elle lit les PES.
    pub fn preference(self) -> u8 {
        match self {
            Format::Pes => 0,
            Format::Pec => 1,
            _ => 2,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Stitch {
    pub x: f32,
    pub y: f32,
    /// `true` : déplacement sans fil (saut / coupe), on ne dessine pas de trait jusqu'à ce point.
    pub jump: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Thread {
    /// Couleur au format `#rrggbb`.
    pub color: String,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct Block {
    pub thread: Thread,
    pub stitches: Vec<Stitch>,
}

#[derive(Debug, Clone)]
pub struct Design {
    pub format: Format,
    pub label: String,
    pub blocks: Vec<Block>,
    pub min_x: f32,
    pub min_y: f32,
    pub max_x: f32,
    pub max_y: f32,
    pub stitch_count: usize,
}

impl Design {
    pub fn width_mm(&self) -> f32 {
        (self.max_x - self.min_x) / 10.0
    }
    pub fn height_mm(&self) -> f32 {
        (self.max_y - self.min_y) / 10.0
    }
    pub fn threads(&self) -> Vec<Thread> {
        self.blocks.iter().map(|b| b.thread.clone()).collect()
    }

    /// Empreinte du dessin lui-même : deux fichiers différents (autre nom interne,
    /// autre version de PES…) mais avec exactement les mêmes points ont la même empreinte.
    pub fn design_hash(&self) -> String {
        let mut h = Sha256::new();
        for b in &self.blocks {
            for s in b.stitches.iter().filter(|s| !s.jump) {
                h.update(((s.x - self.min_x).round() as i32).to_le_bytes());
                h.update(((s.y - self.min_y).round() as i32).to_le_bytes());
            }
            h.update([0xFFu8; 8]);
        }
        format!("d{}-{}", self.stitch_count, hex(&h.finalize()[..16]))
    }
}

/// Côté de la grille utilisée pour comparer la forme de deux motifs.
pub const SHAPE_SIZE: usize = 40;

impl Design {
    /// Empreinte de la forme : grille 40 × 40 (étirée sur le motif) des cases traversées par le fil.
    /// Deux fichiers du même motif dans des formats différents ont presque la même empreinte.
    pub fn shape(&self) -> Vec<u8> {
        let n = SHAPE_SIZE;
        let mut grid = vec![false; n * n];
        let w = (self.max_x - self.min_x).max(1.0);
        let h = (self.max_y - self.min_y).max(1.0);
        let (cw, ch) = (w / n as f32, h / n as f32);
        let mut mark = |x: f32, y: f32| {
            let gx = (((x - self.min_x) / w) * n as f32).floor().clamp(0.0, (n - 1) as f32) as usize;
            let gy = (((y - self.min_y) / h) * n as f32).floor().clamp(0.0, (n - 1) as f32) as usize;
            grid[gy * n + gx] = true;
        };
        for b in &self.blocks {
            let mut prev: Option<(f32, f32)> = None;
            for s in &b.stitches {
                if !s.jump {
                    if let Some((px, py)) = prev {
                        let steps = (((s.x - px).abs() / cw).max((s.y - py).abs() / ch) * 2.0).ceil() as usize;
                        for k in 1..steps.min(4 * n) {
                            let t = k as f32 / steps as f32;
                            mark(px + (s.x - px) * t, py + (s.y - py) * t);
                        }
                    }
                    mark(s.x, s.y);
                }
                prev = Some((s.x, s.y));
            }
        }
        let mut bits = vec![0u8; n * n / 8];
        for (i, &on) in grid.iter().enumerate() {
            if on {
                bits[i / 8] |= 1 << (i % 8);
            }
        }
        bits
    }
}

/// Ressemblance entre deux empreintes de forme : 1,0 = identiques, 0,0 = rien en commun.
pub fn shape_similarity(a: &[u8], b: &[u8]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let (mut inter, mut union) = (0u32, 0u32);
    for (x, y) in a.iter().zip(b) {
        inter += (x & y).count_ones();
        union += (x | y).count_ones();
    }
    if union == 0 { 0.0 } else { inter as f32 / union as f32 }
}

pub fn to_hex(b: &[u8]) -> String {
    hex(b)
}

pub fn from_hex(s: &str) -> Option<Vec<u8>> {
    if s.len() % 2 != 0 {
        return None;
    }
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(s.get(i..i + 2)?, 16).ok()).collect()
}

/// Empreinte exacte du contenu d'un fichier.
pub fn file_hash(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

pub(crate) fn rgb_hex(r: u8, g: u8, b: u8) -> String {
    format!("#{r:02x}{g:02x}{b:02x}")
}

/// Reconnaît le format d'après le contenu du fichier (formats qui ont une signature).
pub fn detect_format(bytes: &[u8]) -> Option<Format> {
    let magic = |m: u32| bytes.len() >= 4 && u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) == m;
    if bytes.starts_with(b"#PES") {
        Some(Format::Pes)
    } else if bytes.starts_with(b"#PEC") {
        Some(Format::Pec)
    } else if bytes.starts_with(b"%vsm%") {
        Some(Format::Vp3)
    } else if magic(crate::formats::HUS_MAGIC) {
        Some(Format::Hus)
    } else if magic(crate::formats::VIP_MAGIC) {
        Some(Format::Vip)
    } else if bytes.starts_with(b"LA:") && bytes.len() > 512 {
        Some(Format::Dst)
    } else {
        None
    }
}

/// Format d'après l'extension, pour les formats sans signature (JEF, SEW, EXP, DST).
pub fn format_from_name(name: &str) -> Option<Format> {
    let ext = std::path::Path::new(name).extension()?.to_string_lossy().to_lowercase();
    Some(match ext.as_str() {
        "pes" => Format::Pes,
        "pec" => Format::Pec,
        "vp3" | "pv3" => Format::Vp3,
        "jef" => Format::Jef,
        "sew" => Format::Sew,
        "exp" => Format::Exp,
        "dst" => Format::Dst,
        "hus" => Format::Hus,
        "vip" => Format::Vip,
        _ => return None,
    })
}

/// Lit un fichier de broderie en reconnaissant le format par son contenu uniquement.
pub fn parse(bytes: &[u8]) -> Result<Design, ParseError> {
    parse_as(bytes, detect_format(bytes).ok_or(ParseError::UnknownFormat)?)
}

/// Lit un fichier de broderie : par son contenu si possible, sinon d'après son nom.
pub fn parse_named(bytes: &[u8], file_name: &str) -> Result<Design, ParseError> {
    let format = detect_format(bytes).or_else(|| format_from_name(file_name)).ok_or(ParseError::UnknownFormat)?;
    parse_as(bytes, format)
}

fn parse_as(bytes: &[u8], format: Format) -> Result<Design, ParseError> {
    let mut raw = RawDesign::default();
    match format {
        Format::Pes => parse_pes(bytes, &mut raw)?,
        Format::Pec => {
            let mut r = Reader::new(bytes);
            r.skip(8)?;
            read_pec(&mut r, &mut raw, Vec::new())?;
        }
        Format::Vp3 => parse_vp3(bytes, &mut raw)?,
        Format::Jef => crate::formats::parse_jef(bytes, &mut raw)?,
        Format::Sew => crate::formats::parse_sew(bytes, &mut raw)?,
        Format::Exp => crate::formats::parse_exp(bytes, &mut raw)?,
        Format::Dst => crate::formats::parse_dst(bytes, &mut raw)?,
        Format::Hus => crate::formats::parse_hus(bytes, &mut raw, false)?,
        Format::Vip => crate::formats::parse_hus(bytes, &mut raw, true)?,
    }
    finish(format, raw)
}

#[derive(Default)]
pub(crate) struct RawDesign {
    pub(crate) label: String,
    pub(crate) blocks: Vec<Block>,
}

fn finish(format: Format, raw: RawDesign) -> Result<Design, ParseError> {
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    let mut n = 0;
    let blocks: Vec<Block> = raw.blocks.into_iter().filter(|b| b.stitches.len() > 1).collect();
    for s in blocks.iter().flat_map(|b| b.stitches.iter()).filter(|s| !s.jump) {
        n += 1;
        min_x = min_x.min(s.x);
        max_x = max_x.max(s.x);
        min_y = min_y.min(s.y);
        max_y = max_y.max(s.y);
    }
    if n == 0 {
        return Err(ParseError::Empty);
    }
    Ok(Design {
        format,
        label: raw.label,
        blocks,
        min_x,
        min_y,
        max_x,
        max_y,
        stitch_count: n,
    })
}

// ---------------------------------------------------------------------------
// Lecteur binaire

struct Reader<'a> {
    b: &'a [u8],
    p: usize,
}

impl<'a> Reader<'a> {
    fn new(b: &'a [u8]) -> Self {
        Reader { b, p: 0 }
    }
    fn left(&self) -> usize {
        self.b.len().saturating_sub(self.p)
    }
    fn take(&mut self, n: usize) -> Result<&'a [u8], ParseError> {
        if self.p + n > self.b.len() {
            return Err(ParseError::Truncated);
        }
        let s = &self.b[self.p..self.p + n];
        self.p += n;
        Ok(s)
    }
    fn skip(&mut self, n: usize) -> Result<(), ParseError> {
        self.take(n).map(|_| ())
    }
    fn u8(&mut self) -> Result<u8, ParseError> {
        Ok(self.take(1)?[0])
    }
    fn u16le(&mut self) -> Result<u16, ParseError> {
        let s = self.take(2)?;
        Ok(u16::from_le_bytes([s[0], s[1]]))
    }
    fn u16be(&mut self) -> Result<u16, ParseError> {
        let s = self.take(2)?;
        Ok(u16::from_be_bytes([s[0], s[1]]))
    }
    fn u24be(&mut self) -> Result<u32, ParseError> {
        let s = self.take(3)?;
        Ok(((s[0] as u32) << 16) | ((s[1] as u32) << 8) | s[2] as u32)
    }
    fn u32le(&mut self) -> Result<u32, ParseError> {
        let s = self.take(4)?;
        Ok(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
    }
    fn u32be(&mut self) -> Result<u32, ParseError> {
        let s = self.take(4)?;
        Ok(u32::from_be_bytes([s[0], s[1], s[2], s[3]]))
    }
    fn i32be(&mut self) -> Result<i32, ParseError> {
        Ok(self.u32be()? as i32)
    }
    fn string(&mut self, n: usize) -> Result<String, ParseError> {
        Ok(latin1(self.take(n)?))
    }
}

fn latin1(b: &[u8]) -> String {
    b.iter().map(|&c| c as char).collect::<String>().replace('\0', "").trim().to_string()
}

// ---------------------------------------------------------------------------
// PES / PEC

/// Palette standard des fils Brother (index PEC).
const PEC_THREADS: [(u8, u8, u8, &str); 65] = [
    (0, 0, 0, "Inconnu"),
    (14, 31, 124, "Bleu de Prusse"), (10, 85, 163, "Bleu"), (0, 135, 119, "Vert sarcelle"),
    (75, 107, 175, "Bleuet"), (237, 23, 31, "Rouge"), (209, 92, 0, "Brun rougeâtre"),
    (145, 54, 151, "Magenta"), (228, 154, 203, "Lilas clair"), (145, 95, 172, "Lilas"),
    (158, 214, 125, "Vert menthe"), (232, 169, 0, "Or foncé"), (254, 186, 53, "Orange"),
    (255, 255, 0, "Jaune"), (112, 188, 31, "Vert citron"), (186, 152, 0, "Laiton"),
    (168, 168, 168, "Argent"), (125, 111, 0, "Brun roux"), (255, 255, 179, "Crème"),
    (79, 85, 86, "Étain"), (0, 0, 0, "Noir"), (11, 61, 145, "Outremer"),
    (119, 1, 118, "Violet royal"), (41, 49, 51, "Gris foncé"), (42, 19, 1, "Brun foncé"),
    (246, 74, 138, "Rose vif"), (178, 118, 36, "Brun clair"), (252, 187, 197, "Rose saumon"),
    (254, 55, 15, "Vermillon"), (240, 240, 240, "Blanc"), (106, 28, 138, "Violet"),
    (168, 221, 196, "Vert d'eau"), (37, 132, 187, "Bleu ciel"), (254, 179, 67, "Citrouille"),
    (255, 243, 107, "Jaune crème"), (208, 166, 96, "Kaki"), (209, 84, 0, "Brun argile"),
    (102, 186, 73, "Vert feuille"), (19, 74, 70, "Bleu paon"), (135, 135, 135, "Gris"),
    (216, 204, 198, "Gris chaud"), (67, 86, 7, "Olive foncé"), (253, 217, 222, "Rose chair"),
    (249, 147, 188, "Rose"), (0, 56, 34, "Vert profond"), (178, 175, 212, "Lavande"),
    (104, 106, 176, "Glycine"), (239, 227, 185, "Beige"), (247, 56, 102, "Carmin"),
    (181, 75, 100, "Rouge ambré"), (19, 43, 26, "Vert olive"), (199, 1, 86, "Fuchsia foncé"),
    (254, 158, 50, "Mandarine"), (168, 222, 235, "Bleu clair"), (0, 103, 62, "Vert émeraude"),
    (78, 41, 144, "Pourpre"), (47, 126, 32, "Vert mousse"), (255, 204, 204, "Rose chair"),
    (255, 217, 17, "Or moisson"), (9, 91, 166, "Bleu électrique"), (240, 249, 112, "Jaune citron"),
    (227, 243, 91, "Vert frais"), (255, 153, 0, "Orange"), (255, 240, 141, "Jaune crème"),
    (255, 200, 200, "Appliqué"),
];

fn read_pes_string(r: &mut Reader) -> Result<String, ParseError> {
    let n = r.u8()? as usize;
    r.string(n)
}

fn parse_pes(bytes: &[u8], d: &mut RawDesign) -> Result<(), ParseError> {
    let mut r = Reader::new(bytes);
    let version = r.take(8)?.to_vec();
    let pec_pos = r.u32le()? as usize;
    // Les versions 5 et + contiennent la liste des fils avec leurs couleurs RGB.
    // Si quelque chose ne va pas, on se rabat sur la palette Brother.
    let chart = read_pes_header(&mut r, &version, d).unwrap_or_default();
    let mut r = Reader::new(bytes);
    r.p = pec_pos;
    read_pec(&mut r, d, chart)
}

fn read_pes_header(r: &mut Reader, version: &[u8], d: &mut RawDesign) -> Result<Vec<Thread>, ParseError> {
    // (saut avant l'image, saut après) — ou trois sauts pour les versions 9 et 10.
    let layout: &[usize] = match version {
        b"#PES0040" => &[],
        b"#PES0050" | b"#PES0055" | b"#PES0056" => &[24, 24],
        b"#PES0060" | b"#PES0070" => &[36, 24],
        b"#PES0080" => &[38, 26],
        b"#PES0090" => &[14, 30, 34],
        b"#PES0100" => &[14, 38, 34],
        _ => return Ok(Vec::new()),
    };
    r.skip(4)?;
    let name = read_pes_string(r)?;
    if !name.is_empty() {
        d.label = name;
    }
    for _ in 0..4 {
        read_pes_string(r)?; // catégorie, auteur, mots-clés, commentaires
    }
    if layout.is_empty() {
        return Ok(Vec::new());
    }
    if layout.len() == 3 {
        r.skip(layout[0])?;
        read_pes_string(r)?; // nom du cadre
        r.skip(layout[1])?;
        read_pes_string(r)?; // image
        r.skip(layout[2])?;
    } else {
        r.skip(layout[0])?;
        read_pes_string(r)?; // image
        r.skip(layout[1])?;
    }
    let (fills, motifs, feathers) = (r.u16le()?, r.u16le()?, r.u16le()?);
    if fills != 0 || motifs != 0 || feathers != 0 {
        return Ok(Vec::new());
    }
    let n = r.u16le()?.min(256);
    let mut chart = Vec::with_capacity(n as usize);
    for _ in 0..n {
        let catalog = read_pes_string(r)?;
        let rgb = r.u24be()?;
        r.skip(5)?;
        let desc = read_pes_string(r)?;
        let brand = read_pes_string(r)?;
        read_pes_string(r)?;
        let name = [brand, catalog, desc].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" ");
        chart.push(Thread {
            color: rgb_hex((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8),
            name,
        });
    }
    Ok(chart)
}

fn signed12(v: u16) -> f32 {
    let v = (v & 0xFFF) as i32;
    (if v > 0x7FF { v - 0x1000 } else { v }) as f32
}
fn signed7(v: u8) -> f32 {
    (if v > 63 { v as i32 - 128 } else { v as i32 }) as f32
}

fn read_pec(r: &mut Reader, d: &mut RawDesign, chart: Vec<Thread>) -> Result<(), ParseError> {
    let start = r.p;
    r.skip(3)?; // "LA:"
    let label = r.string(16)?;
    if d.label.is_empty() {
        d.label = label;
    }
    r.p = start + 48;
    let n_colors = r.u8()? as usize + 1;
    let idx = r.take(n_colors)?.to_vec();

    // Attribution des couleurs, comme le font les logiciels Brother.
    let threads: Vec<Thread> = if chart.len() >= n_colors {
        chart.into_iter().take(n_colors).collect()
    } else {
        let mut pool = chart.into_iter();
        let mut map: Vec<Option<Thread>> = vec![None; PEC_THREADS.len()];
        idx.iter()
            .map(|&b| {
                let ci = b as usize % PEC_THREADS.len();
                map[ci]
                    .get_or_insert_with(|| {
                        pool.next().unwrap_or_else(|| {
                            let (r, g, b, name) = PEC_THREADS[ci];
                            Thread {
                                color: rgb_hex(r, g, b),
                                name: if ci == 0 { name.into() } else { format!("{name} (n°{ci})") },
                            }
                        })
                    })
                    .clone()
            })
            .collect()
    };

    r.p = start + 512 + 2 + 3 + 11; // en-tête PEC puis en-tête du bloc des points
    let (mut x, mut y) = (0f32, 0f32);
    let mut ci = 0usize;
    let mut stitches = Vec::new();
    let thread_at = |i: usize| {
        threads.get(i.min(threads.len().saturating_sub(1))).cloned().unwrap_or(Thread {
            color: "#888888".into(),
            name: String::new(),
        })
    };
    while r.left() >= 2 {
        let v1 = r.u8()?;
        let mut v2 = r.u8()?;
        if v1 == 0xFF && v2 == 0x00 {
            break;
        }
        if v1 == 0xFE && v2 == 0xB0 {
            if r.left() > 0 {
                r.u8()?;
            }
            d.blocks.push(Block { thread: thread_at(ci), stitches: std::mem::take(&mut stitches) });
            stitches.push(Stitch { x, y, jump: true });
            ci += 1;
            continue;
        }
        let mut jump = false;
        let dx = if v1 & 0x80 != 0 {
            jump |= v1 & 0x30 != 0;
            let dx = signed12(((v1 as u16) << 8) | v2 as u16);
            if r.left() == 0 {
                break;
            }
            v2 = r.u8()?;
            dx
        } else {
            signed7(v1)
        };
        let dy = if v2 & 0x80 != 0 {
            jump |= v2 & 0x30 != 0;
            if r.left() == 0 {
                break;
            }
            let v3 = r.u8()?;
            signed12(((v2 as u16) << 8) | v3 as u16)
        } else {
            signed7(v2)
        };
        x += dx;
        y += dy;
        stitches.push(Stitch { x, y, jump });
    }
    d.blocks.push(Block { thread: thread_at(ci), stitches });
    Ok(())
}

// ---------------------------------------------------------------------------
// VP3

fn skip_vp3_string(r: &mut Reader) -> Result<(), ParseError> {
    let n = r.u16be()? as usize;
    r.skip(n)
}
fn read_vp3_string(r: &mut Reader) -> Result<String, ParseError> {
    let n = r.u16be()? as usize;
    Ok(String::from_utf8_lossy(r.take(n)?).trim().to_string())
}

fn parse_vp3(bytes: &[u8], d: &mut RawDesign) -> Result<(), ParseError> {
    let mut r = Reader::new(bytes);
    r.skip(6)?;
    skip_vp3_string(&mut r)?;
    r.skip(7)?;
    skip_vp3_string(&mut r)?;
    r.skip(32)?;
    let cx = r.i32be()? as f32 / 100.0;
    let cy = -(r.i32be()? as f32) / 100.0;
    r.skip(27)?;
    skip_vp3_string(&mut r)?;
    r.skip(24)?;
    skip_vp3_string(&mut r)?;
    let n_blocks = r.u16be()?;
    let (mut x, mut y) = (0f32, 0f32);
    for _ in 0..n_blocks {
        if r.left() < 10 {
            break;
        }
        r.skip(3)?;
        let len = r.u32be()? as usize;
        let end = (r.p + len).min(bytes.len());
        let sx = r.i32be()? as f32 / 100.0;
        let sy = -(r.i32be()? as f32) / 100.0;
        let (ax, ay) = (sx + cx, sy + cy);
        if ax != 0.0 && ay != 0.0 {
            x = ax;
            y = ay;
        }
        // Fil
        let n_col = r.u8()?;
        r.u8()?;
        let mut rgb = 0u32;
        for _ in 0..n_col {
            rgb = r.u24be()?;
            r.u8()?;
            r.u16be()?;
        }
        r.u8()?;
        r.u8()?;
        let catalog = read_vp3_string(&mut r)?;
        let desc = read_vp3_string(&mut r)?;
        let brand = read_vp3_string(&mut r)?;
        let color = rgb_hex((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8);
        let name = [brand, catalog, desc]
            .into_iter()
            .filter(|s| !s.is_empty() && !s.eq_ignore_ascii_case(&color))
            .collect::<Vec<_>>()
            .join(" ");
        r.skip(15 + 3)?;

        let mut stitches = vec![Stitch { x, y, jump: true }];
        let s8 = |v: u8| v as i8 as f32;
        let s16 = |a: u8, b: u8| i16::from_be_bytes([a, b]) as f32;
        let mut i = r.p;
        let mut trimmed = false;
        while i + 1 < end {
            let (a, c) = (bytes[i], bytes[i + 1]);
            i += 2;
            if a != 0x80 {
                x += s8(a);
                y += s8(c);
                stitches.push(Stitch { x, y, jump: trimmed });
                trimmed = false;
                continue;
            }
            match c {
                0x01 => {
                    if i + 4 > end {
                        break;
                    }
                    x += s16(bytes[i], bytes[i + 1]);
                    y += s16(bytes[i + 2], bytes[i + 3]);
                    i += 6; // 4 octets de déplacement + "80 02"
                    // Long déplacement (> 12,7 mm) : on ne dessine pas de fil.
                    stitches.push(Stitch { x, y, jump: true });
                    trimmed = false;
                }
                0x03 => trimmed = true, // coupe du fil
                _ => {}
            }
        }
        d.blocks.push(Block { thread: Thread { color, name }, stitches });
        r.p = end;
    }
    Ok(())
}
