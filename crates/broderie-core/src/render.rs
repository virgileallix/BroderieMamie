//! Dessin des aperçus (PNG) avec un rendu « fil » : contour sombre, couleur, petit reflet.

use crate::design::Design;
use tiny_skia::{LineCap, LineJoin, Paint, PathBuilder, Pixmap, Stroke, Transform};

fn parse_hex(c: &str) -> (u8, u8, u8) {
    let v = u32::from_str_radix(c.trim_start_matches('#'), 16).unwrap_or(0x888888);
    ((v >> 16) as u8, (v >> 8) as u8, v as u8)
}

/// Dessine la broderie dans une image carrée de `size` pixels, fond transparent.
pub fn render_png(d: &Design, size: u32) -> Option<Vec<u8>> {
    render_png_on(d, size, None)
}

/// Comme `render_png`, sur un fond de couleur (pour l'analyse par l'IA, sur fond blanc).
pub fn render_png_on(d: &Design, size: u32, background: Option<(u8, u8, u8)>) -> Option<Vec<u8>> {
    let mut pm = Pixmap::new(size, size)?;
    if let Some((r, g, b)) = background {
        pm.fill(tiny_skia::Color::from_rgba8(r, g, b, 255));
    }
    draw(d, &mut pm, 0.06);
    pm.encode_png().ok()
}

/// Dessine les points de la broderie, centrés, avec une marge (en part de la taille de l'image).
fn draw(d: &Design, pm: &mut Pixmap, margin: f32) {
    let size = pm.width();
    let pad = size as f32 * margin;
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
}

/// Aperçu en JPEG (léger : avec des dizaines de milliers de motifs, la place compte), sur le fond
/// des cartes, avec le cercle en pointillés du « tambour à broder » dessiné dedans.
pub fn render_thumb_jpeg(d: &Design, size: u32) -> Option<Vec<u8>> {
    let mut pm = Pixmap::new(size, size)?;
    pm.fill(tiny_skia::Color::from_rgba8(0xff, 0xfd, 0xf9, 255)); // fond des cartes
    let c = size as f32 / 2.0;
    let circle = PathBuilder::from_circle(c, c, size as f32 * 0.46)?;
    let mut paint = Paint::default();
    paint.anti_alias = true;
    paint.set_color_rgba8(0xf6, 0xf0, 0xe6, 255); // lin
    pm.fill_path(&circle, &paint, tiny_skia::FillRule::Winding, Transform::identity(), None);
    paint.set_color_rgba8(0xe5, 0xd9, 0xc8, 255);
    let dash = tiny_skia::StrokeDash::new(vec![size as f32 * 0.025, size as f32 * 0.018], 0.0);
    let stroke = Stroke { width: (size as f32 / 128.0).max(1.0), dash, ..Default::default() };
    pm.stroke_path(&circle, &paint, &stroke, Transform::identity(), None);
    draw(d, &mut pm, 0.12);
    to_jpeg(&pm, 82)
}

/// Image JPEG sur fond uni (pour l'analyse par l'IA).
pub fn render_jpeg_on(d: &Design, size: u32, (r, g, b): (u8, u8, u8)) -> Option<Vec<u8>> {
    let mut pm = Pixmap::new(size, size)?;
    pm.fill(tiny_skia::Color::from_rgba8(r, g, b, 255));
    draw(d, &mut pm, 0.06);
    to_jpeg(&pm, 85)
}

fn to_jpeg(pm: &Pixmap, quality: u8) -> Option<Vec<u8>> {
    // Le fond est opaque : les couleurs ne sont pas prémultipliées en pratique.
    let rgb: Vec<u8> = pm.data().chunks_exact(4).flat_map(|p| [p[0], p[1], p[2]]).collect();
    let mut out = Vec::new();
    let enc = jpeg_encoder::Encoder::new(&mut out, quality);
    enc.encode(&rgb, pm.width() as u16, pm.height() as u16, jpeg_encoder::ColorType::Rgb).ok()?;
    Some(out)
}
