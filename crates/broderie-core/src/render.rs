//! Dessin des aperçus (PNG) avec un rendu « fil » : contour sombre, couleur, petit reflet.

use crate::design::Design;
use tiny_skia::{LineCap, LineJoin, Paint, PathBuilder, Pixmap, Stroke, Transform};

fn parse_hex(c: &str) -> (u8, u8, u8) {
    let v = u32::from_str_radix(c.trim_start_matches('#'), 16).unwrap_or(0x888888);
    ((v >> 16) as u8, (v >> 8) as u8, v as u8)
}

/// Dessine la broderie dans une image carrée de `size` pixels, fond transparent.
pub fn render_png(d: &Design, size: u32) -> Option<Vec<u8>> {
    let mut pm = Pixmap::new(size, size)?;
    let pad = size as f32 * 0.06;
    let w = (d.max_x - d.min_x).max(1.0);
    let h = (d.max_y - d.min_y).max(1.0);
    let s = ((size as f32 - 2.0 * pad) / w).min((size as f32 - 2.0 * pad) / h);
    let ox = size as f32 / 2.0 - (d.min_x + d.max_x) / 2.0 * s;
    let oy = size as f32 / 2.0 - (d.min_y + d.max_y) / 2.0 * s;
    let lw = (3.5 * s).clamp(0.7, 6.0); // fil ≈ 0,35 mm

    for b in &d.blocks {
        let mut pb = PathBuilder::new();
        let mut started = false;
        for st in &b.stitches {
            let (x, y) = (st.x * s + ox, st.y * s + oy);
            if st.jump || !started {
                pb.move_to(x, y);
                started = true;
            } else {
                pb.line_to(x, y);
            }
        }
        let Some(path) = pb.finish() else { continue };
        let (r, g, bl) = parse_hex(&b.thread.color);
        let layers: [(f32, (u8, u8, u8, u8)); 3] = [
            (lw * 1.25, ((r as f32 * 0.72) as u8, (g as f32 * 0.72) as u8, (bl as f32 * 0.72) as u8, 255)),
            (lw, (r, g, bl, 255)),
            (lw * 0.35, (255, 255, 255, if lw > 1.6 { 56 } else { 0 })),
        ];
        for (width, (r, g, b, a)) in layers {
            if a == 0 {
                continue;
            }
            let mut paint = Paint::default();
            paint.set_color_rgba8(r, g, b, a);
            paint.anti_alias = true;
            let stroke = Stroke { width, line_cap: LineCap::Round, line_join: LineJoin::Round, ..Default::default() };
            pm.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
        }
    }
    pm.encode_png().ok()
}
