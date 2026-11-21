//! Formats de broderie supplémentaires : JEF et SEW (Janome), EXP (Melco), DST (Tajima),
//! HUS (Husqvarna) et VIP (Pfaff). Portages de pyembroidery / libembroidery.
//!
//! Dans ces fichiers, l'axe vertical monte ; on l'inverse pour dessiner à l'écran,
//! comme pour les PES et VP3. Unités : 0,1 mm.

use crate::compress::expand;
use crate::design::{rgb_hex, Block, ParseError, RawDesign, Stitch, Thread};
use crate::palettes;

pub const HUS_MAGIC: u32 = 0x00C8_AF5B;
pub const VIP_MAGIC: u32 = 0x0190_FC5D;

/// Couleurs utilisées quand le fichier n'indique pas ses fils (DST, EXP, VIP).
const DEFAULT_COLORS: [(u32, &str); 10] = [
    (0x2f5f8a, "Fil 1"), (0xc8566e, "Fil 2"), (0x4e7f63, "Fil 3"), (0xe3b04b, "Fil 4"), (0x6a4c93, "Fil 5"),
    (0xd9733b, "Fil 6"), (0x3b3b3b, "Fil 7"), (0x58a4b0, "Fil 8"), (0x9c6644, "Fil 9"), (0xb5838d, "Fil 10"),
];

fn thread_of(rgb: u32, name: &str) -> Thread {
    Thread { color: rgb_hex((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8), name: name.trim().to_string() }
}

fn default_thread(i: usize) -> Thread {
    let (c, n) = DEFAULT_COLORS[i % DEFAULT_COLORS.len()];
    thread_of(c, n)
}

/// Construit la liste des blocs de couleur à partir de déplacements relatifs.
struct Builder<'a> {
    raw: &'a mut RawDesign,
    threads: Vec<Thread>,
    ci: usize,
    x: f32,
    y: f32,
    cur: Vec<Stitch>,
}

impl<'a> Builder<'a> {
    fn new(raw: &'a mut RawDesign, threads: Vec<Thread>) -> Self {
        Builder { raw, threads, ci: 0, x: 0.0, y: 0.0, cur: Vec::new() }
    }
    fn stitch(&mut self, dx: i32, dy: i32) {
        self.x += dx as f32;
        self.y += dy as f32;
        self.cur.push(Stitch { x: self.x, y: self.y, jump: false });
    }
    fn jump(&mut self, dx: i32, dy: i32) {
        self.x += dx as f32;
        self.y += dy as f32;
        self.cur.push(Stitch { x: self.x, y: self.y, jump: true });
    }
    fn thread(&self, i: usize) -> Thread {
        self.threads.get(i).cloned().unwrap_or_else(|| default_thread(i))
    }
    fn color_change(&mut self) {
        let t = self.thread(self.ci);
        let cur = std::mem::take(&mut self.cur);
        self.raw.blocks.push(Block { thread: t, stitches: cur });
        self.cur.push(Stitch { x: self.x, y: self.y, jump: true });
        self.ci += 1;
    }
    fn end(mut self) {
        let t = self.thread(self.ci);
        let cur = std::mem::take(&mut self.cur);
        self.raw.blocks.push(Block { thread: t, stitches: cur });
    }
}

fn s8(b: u8) -> i32 {
    b as i8 as i32
}
fn u16le(b: &[u8], p: usize) -> Result<u16, ParseError> {
    b.get(p..p + 2).map(|s| u16::from_le_bytes([s[0], s[1]])).ok_or(ParseError::Truncated)
}
fn u32le(b: &[u8], p: usize) -> Result<u32, ParseError> {
    b.get(p..p + 4).map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]])).ok_or(ParseError::Truncated)
}

// ---------------------------------------------------------------------------
// JEF (Janome)

pub(crate) fn parse_jef(b: &[u8], raw: &mut RawDesign) -> Result<(), ParseError> {
    let stitch_offset = u32le(b, 0)? as usize;
    let n_colors = (u32le(b, 24)? as usize).min(256);
    // Couleurs : un index dans la palette Janome ; 0 = « arrêt » sans changement de fil.
    let mut slots: Vec<Option<Thread>> = Vec::with_capacity(n_colors);
    for i in 0..n_colors {
        let idx = (u32le(b, 116 + 4 * i)? as i32).unsigned_abs() as usize;
        slots.push(if idx == 0 {
            None
        } else {
            let (c, n) = palettes::JEF[idx % palettes::JEF.len()];
            Some(thread_of(c, n))
        });
    }
    let threads: Vec<Thread> = slots.iter().flatten().cloned().collect();
    let mut bld = Builder::new(raw, threads);
    let mut slot = 0usize;
    let mut p = stitch_offset;
    while p + 1 < b.len() {
        let (a, c) = (b[p], b[p + 1]);
        p += 2;
        if a != 0x80 {
            bld.stitch(s8(a), -s8(c));
            continue;
        }
        if p + 1 >= b.len() {
            break;
        }
        let (x, y) = (s8(b[p]), -s8(b[p + 1]));
        p += 2;
        match c {
            0x02 => bld.jump(x, y),
            0x01 => {
                slot += 1;
                // Un emplacement vide (0) est un simple arrêt de la machine : même fil.
                if slots.get(slot).is_some_and(|s| s.is_none()) {
                    continue;
                }
                bld.color_change();
            }
            _ => break, // 0x10 = fin
        }
    }
    bld.end();
    Ok(())
}

// ---------------------------------------------------------------------------
// SEW (Janome, ancien format)

pub(crate) fn parse_sew(b: &[u8], raw: &mut RawDesign) -> Result<(), ParseError> {
    let n = u16le(b, 0)? as usize;
    let mut threads = Vec::with_capacity(n);
    for i in 0..n.min(256) {
        let idx = u16le(b, 2 + 2 * i)? as usize % palettes::SEW.len();
        let (c, name) = palettes::SEW[idx];
        threads.push(thread_of(c, name));
    }
    let mut bld = Builder::new(raw, threads);
    let mut p = 0x1D78;
    if p >= b.len() {
        return Err(ParseError::Truncated);
    }
    while p + 1 < b.len() {
        let (a, c) = (b[p], b[p + 1]);
        p += 2;
        if a != 0x80 {
            bld.stitch(s8(a), -s8(c));
            continue;
        }
        if p + 1 >= b.len() {
            break;
        }
        let (x, y) = (s8(b[p]), -s8(b[p + 1]));
        p += 2;
        if c & 1 != 0 {
            bld.color_change();
        } else if c == 0x04 || c == 0x02 {
            bld.jump(x, y);
        } else if c == 0x10 {
            bld.stitch(x, y);
        } else {
            break;
        }
    }
    bld.end();
    Ok(())
}

// ---------------------------------------------------------------------------
// EXP (Melco)

pub(crate) fn parse_exp(b: &[u8], raw: &mut RawDesign) -> Result<(), ParseError> {
    let mut bld = Builder::new(raw, Vec::new());
    let mut p = 0;
    while p + 1 < b.len() {
        let (a, c) = (b[p], b[p + 1]);
        p += 2;
        if a != 0x80 {
            bld.stitch(s8(a), -s8(c));
            continue;
        }
        if p + 1 >= b.len() {
            break;
        }
        let (x, y) = (s8(b[p]), -s8(b[p + 1]));
        p += 2;
        match c {
            0x80 => {} // coupe du fil
            0x02 => bld.stitch(x, y),
            0x04 => bld.jump(x, y),
            0x01 => {
                bld.color_change();
                if x != 0 || y != 0 {
                    bld.jump(x, y);
                }
            }
            _ => break,
        }
    }
    bld.end();
    Ok(())
}

// ---------------------------------------------------------------------------
// DST (Tajima)

fn bit(b: u8, pos: u8) -> i32 {
    ((b >> pos) & 1) as i32
}

pub(crate) fn parse_dst(b: &[u8], raw: &mut RawDesign) -> Result<(), ParseError> {
    if b.len() < 512 {
        return Err(ParseError::Truncated);
    }
    // En-tête texte : « LA:nom »
    if let Some(line) = b[..512].split(|&c| c == b'\r' || c == b'\n').next() {
        if line.starts_with(b"LA:") {
            raw.label = String::from_utf8_lossy(&line[3..]).trim().to_string();
        }
    }
    let mut bld = Builder::new(raw, Vec::new());
    for t in b[512..].chunks_exact(3) {
        let (b0, b1, b2) = (t[0], t[1], t[2]);
        let dx = bit(b2, 2) * 81 - bit(b2, 3) * 81 + bit(b1, 2) * 27 - bit(b1, 3) * 27 + bit(b0, 2) * 9
            - bit(b0, 3) * 9
            + bit(b1, 0) * 3
            - bit(b1, 1) * 3
            + bit(b0, 0)
            - bit(b0, 1);
        let dy = -(bit(b2, 5) * 81 - bit(b2, 4) * 81 + bit(b1, 5) * 27 - bit(b1, 4) * 27 + bit(b0, 5) * 9
            - bit(b0, 4) * 9
            + bit(b1, 7) * 3
            - bit(b1, 6) * 3
            + bit(b0, 7)
            - bit(b0, 6));
        if b2 & 0b1111_0011 == 0b1111_0011 {
            break; // fin
        } else if b2 & 0b1100_0011 == 0b1100_0011 {
            bld.color_change();
            bld.jump(dx, dy);
        } else if b2 & 0b0100_0011 == 0b0100_0011 {
            bld.jump(dx, dy); // paillettes : on ignore
        } else if b2 & 0b1000_0011 == 0b1000_0011 {
            bld.jump(dx, dy);
        } else {
            bld.stitch(dx, dy);
        }
    }
    bld.end();
    Ok(())
}

// ---------------------------------------------------------------------------
// HUS (Husqvarna) et VIP (Pfaff) : même structure, données compressées.

pub(crate) fn parse_hus(b: &[u8], raw: &mut RawDesign, vip: bool) -> Result<(), ParseError> {
    let n_stitches = (u32le(b, 4)? as usize).min(5_000_000);
    let n_colors = (u32le(b, 8)? as usize).min(256);
    let cmd_off = u32le(b, 20)? as usize;
    let x_off = u32le(b, 24)? as usize;
    let y_off = u32le(b, 28)? as usize;
    if !(cmd_off <= x_off && x_off <= y_off && y_off <= b.len()) {
        return Err(ParseError::Truncated);
    }
    let threads: Vec<Thread> = if vip {
        Vec::new() // les couleurs des VIP sont chiffrées : couleurs par défaut
    } else {
        (0..n_colors)
            .map(|i| {
                let idx = u16le(b, 42 + 2 * i).unwrap_or(0) as usize % palettes::HUS.len();
                let (c, n) = palettes::HUS[idx];
                thread_of(c, n)
            })
            .collect()
    };
    let cmds = expand(&b[cmd_off..x_off], n_stitches);
    let xs = expand(&b[x_off..y_off], n_stitches);
    let ys = expand(&b[y_off..], n_stitches);
    let n = cmds.len().min(xs.len()).min(ys.len()).min(n_stitches);
    let mut bld = Builder::new(raw, threads);
    for i in 0..n {
        let (x, y) = (s8(xs[i]), -s8(ys[i]));
        match cmds[i] {
            0x80 => bld.stitch(x, y),
            0x81 => bld.jump(x, y),
            0x84 => {
                bld.color_change();
                bld.jump(x, y);
            }
            0x88 => bld.jump(x, y), // coupe
            0x90 => break,          // fin
            _ if vip => bld.stitch(x, y),
            _ => break,
        }
    }
    bld.end();
    Ok(())
}
