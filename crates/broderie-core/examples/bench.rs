//! Mesure les temps sur une grosse collection : cargo run --release --example bench -- <source> <bibliothèque>
use broderie_core::Library;
use std::path::PathBuf;
use std::time::Instant;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let (src, root) = (PathBuf::from(&a[1]), PathBuf::from(&a[2]));
    let t = Instant::now();
    let mut lib = Library::open(&root).unwrap();
    let rep = lib.import(&[src.clone()], |_, _, _| {});
    println!("import : {} ajoutées, {} déjà là, {} ressemblantes en {:.1} s", rep.added.len(), rep.duplicates.len(), rep.similar.len(), t.elapsed().as_secs_f32());
    let t = Instant::now();
    let rep = lib.import(&[src], |_, _, _| {});
    println!("ré-import (tout en double) : {} déjà là en {:.1} s", rep.duplicates.len(), t.elapsed().as_secs_f32());
    let t = Instant::now();
    let lib = Library::open(&root).unwrap();
    println!("ouverture : {} broderies en {:.2} s", lib.entries().len(), t.elapsed().as_secs_f32());
    let t = Instant::now();
    let g = lib.find_duplicates();
    println!("doublons : {} groupes en {:.2} s", g.len(), t.elapsed().as_secs_f32());
    let size = std::fs::metadata(root.join("bibliotheque.json")).unwrap().len();
    println!("bibliotheque.json : {} Ko ({} octets par broderie)", size / 1024, size / lib.entries().len().max(1) as u64);
}
