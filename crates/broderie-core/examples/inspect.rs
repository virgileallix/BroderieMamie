//! Affiche ce que le logiciel lit dans des fichiers de broderie, et leur ressemblance deux à deux.
//! cargo run -p broderie-core --example inspect -- fichier1 fichier2 …
use broderie_core::{parse_named, shape_similarity};

fn main() {
    let files: Vec<String> = std::env::args().skip(1).collect();
    let mut shapes = Vec::new();
    for f in &files {
        let name = std::path::Path::new(f).file_name().unwrap().to_string_lossy().to_string();
        match parse_named(&std::fs::read(f).unwrap(), &name) {
            Ok(d) => {
                println!("{name}: {} {} points, {} couleurs, x {}..{} y {}..{}", d.format.label(), d.stitch_count, d.blocks.len(), d.min_x, d.max_x, d.min_y, d.max_y);
                shapes.push((name, Some(d.shape())));
            }
            Err(e) => { println!("{name}: ERREUR {e}"); shapes.push((name, None)); }
        }
    }
    for i in 0..shapes.len() {
        for j in i + 1..shapes.len() {
            if let (Some(a), Some(b)) = (&shapes[i].1, &shapes[j].1) {
                println!("  {} ~ {} : {:.3}", shapes[i].0, shapes[j].0, shape_similarity(a, b));
            }
        }
    }
}
