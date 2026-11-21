//! Crée une bibliothèque de démonstration et exporte ses données pour tester l'interface
//! dans un navigateur (voir `dev/`).
//!
//! cargo run -p broderie-core --example demo -- <dossier source> <dossier bibliothèque>

use broderie_core::Library;
use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (src, root) = (PathBuf::from(&args[1]), PathBuf::from(&args[2]));
    let mut lib = Library::open(&root).expect("bibliothèque");
    let report = lib.import(&[src], |_, _, _| {});
    eprintln!("{} ajoutées, {} doublons", report.added.len(), report.duplicates.len());
    let designs: serde_json::Map<String, serde_json::Value> = lib
        .entries()
        .iter()
        .filter_map(|e| {
            let d = lib.design(&e.id).ok()?;
            let blocks: Vec<_> = d
                .blocks
                .iter()
                .map(|b| {
                    let pts: Vec<f32> = b.stitches.iter().flat_map(|s| [s.x, s.y, if s.jump { 1.0 } else { 0.0 }]).collect();
                    serde_json::json!({ "color": b.thread.color, "name": b.thread.name, "pts": pts })
                })
                .collect();
            Some((e.id.clone(), serde_json::json!({
                "format": d.format.label(), "label": d.label, "blocks": blocks,
                "minX": d.min_x, "minY": d.min_y, "maxX": d.max_x, "maxY": d.max_y,
                "stitchCount": d.stitch_count, "widthMM": d.width_mm(), "heightMM": d.height_mm()
            })))
        })
        .collect();
    let out = serde_json::json!({
        "root": root, "thumbs": root.join("Aperçus"), "entries": lib.entries(),
        "report": report, "designs": designs
    });
    println!("{out}");
}
