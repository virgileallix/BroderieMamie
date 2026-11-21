//! Formats supplémentaires et repérage du même motif dans plusieurs formats.
//! Valeurs de référence : pyembroidery (fichiers générés par pyembroidery, sauf HUS et SEW faits à la main).
use broderie_core::{library::EntryUpdate, parse_named, shape_similarity, Format, Library};
use std::fs;
use std::path::PathBuf;

fn fx(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/formats").join(name)
}
fn read(name: &str) -> broderie_core::Design {
    parse_named(&fs::read(fx(name)).unwrap(), name).unwrap()
}

#[test]
fn lit_les_autres_formats() {
    for (f, fmt, n, b) in [
        ("Rose_ancienne.jef", Format::Jef, 3300, (-443.0, -327.0, 443.0, 445.0)),
        ("Rose_ancienne.dst", Format::Dst, 3300, (-443.0, -327.0, 443.0, 445.0)),
        ("Rose_ancienne.exp", Format::Exp, 3300, (-443.0, -327.0, 443.0, 445.0)),
        ("Ourson_Leon.hus", Format::Hus, 4448, (-350.0, -350.0, 350.0, 320.0)),
        ("Sapin_enneige.sew", Format::Sew, 3452, (-378.0, -668.0, 379.0, 420.0)),
    ] {
        let d = read(f);
        assert_eq!(d.format, fmt, "{f}");
        assert_eq!(d.stitch_count, n, "{f}");
        assert_eq!((d.min_x, d.min_y, d.max_x, d.max_y), b, "{f}");
    }
    assert_eq!(read("Rose_ancienne.jef").blocks.len(), 4);
    assert_eq!(read("Ourson_Leon.hus").blocks.len(), 4);
}

#[test]
fn meme_motif_dans_plusieurs_formats() {
    let pes = read("Rose_ancienne.pes").shape();
    for f in ["Rose_ancienne.jef", "Rose_ancienne.dst", "Rose_ancienne.exp"] {
        assert!(shape_similarity(&pes, &read(f).shape()) > 0.95, "{f}");
    }
    assert!(shape_similarity(&pes, &read("Ourson_Leon.pes").shape()) < 0.7);
    assert!(shape_similarity(&pes, &read("Sapin_enneige.pes").shape()) < 0.7);
}

#[test]
fn import_garde_le_pes() {
    let dir = tempfile::tempdir().unwrap();
    let mut lib = Library::open(dir.path()).unwrap();
    // D'abord la version Janome, avec un mot-clé et une étoile…
    let r = lib.import(&[fx("Rose_ancienne.jef"), fx("Ourson_Leon.hus")], |_, _, _| {});
    assert_eq!(r.added.len(), 2);
    let id = r.added[0].id.clone();
    lib.update(&id, EntryUpdate { name: Some("Rose de Mamie".into()), tags: Some(vec!["fleurs".into()]), favorite: Some(true) }).unwrap();
    // … puis les autres formats : le PES remplace le JEF, les autres sont des doublons.
    let r = lib.import(&[fx("Rose_ancienne.dst"), fx("Rose_ancienne.pes"), fx("Rose_ancienne.exp"), fx("Ourson_Leon.pes")], |_, _, _| {});
    assert!(r.added.is_empty(), "{r:?}");
    assert_eq!(r.replaced.len(), 2, "{r:?}");
    assert_eq!(r.duplicates.len(), 2, "{r:?}");
    assert_eq!(lib.entries().len(), 2);
    let rose = lib.entries().iter().find(|e| e.name == "Rose de Mamie").unwrap();
    assert_eq!(rose.format.as_deref(), Some("PES"));
    assert_eq!(rose.file_name, "Rose de Mamie.pes");
    assert_eq!(rose.tags, ["fleurs"]);
    assert!(rose.favorite);
    assert!(lib.thumb_path(&rose.id).exists());
    assert!(dir.path().join("Corbeille/Rose de Mamie.jef").exists());
    assert!(lib.entries().iter().all(|e| e.format.as_deref() == Some("PES")));
    // La bibliothèque se rouvre avec les empreintes.
    let lib = Library::open(dir.path()).unwrap();
    assert!(lib.entries().iter().all(|e| e.shape.is_some()));
}

#[test]
fn nettoyage_des_doublons_existants() {
    // Une ancienne collection pleine de doublons : on assemble à la main l'index de deux bibliothèques.
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    Library::open(a.path()).unwrap().import(&[fx("Rose_ancienne.jef"), fx("Sapin_enneige.sew")], |_, _, _| {});
    let mut lb = Library::open(b.path()).unwrap();
    lb.import(&[fx("Rose_ancienne.pes")], |_, _, _| {});
    let id = lb.entries()[0].id.clone();
    lb.update(&id, EntryUpdate { tags: Some(vec!["Noël".into()]), ..Default::default() }).unwrap();
    for e in lb.entries() {
        fs::copy(b.path().join("Motifs").join(&e.file_name), a.path().join("Motifs").join(&e.file_name)).unwrap();
    }
    let mut ja: serde_json::Value = serde_json::from_slice(&fs::read(a.path().join("bibliotheque.json")).unwrap()).unwrap();
    let jb: serde_json::Value = serde_json::from_slice(&fs::read(b.path().join("bibliotheque.json")).unwrap()).unwrap();
    for mut e in jb["entries"].as_array().unwrap().clone() {
        e["shape"] = serde_json::Value::Null; // ancienne version, sans empreinte
        ja["entries"].as_array_mut().unwrap().push(e);
    }
    fs::write(a.path().join("bibliotheque.json"), serde_json::to_vec(&ja).unwrap()).unwrap();

    let mut lib = Library::open(a.path()).unwrap();
    assert_eq!(lib.entries().len(), 3);
    let rep = lib.dedupe().unwrap();
    assert_eq!(rep.removed.len(), 1, "{rep:?}");
    assert_eq!(rep.kept, 2);
    let rose = lib.entries().iter().find(|e| e.name == "Rose ancienne").unwrap();
    assert_eq!(rose.format.as_deref(), Some("PES"));
    assert_eq!(rose.tags, ["Noël"]);
    assert!(a.path().join("Corbeille/Rose ancienne.jef").exists());
    assert!(lib.dedupe().unwrap().removed.is_empty());
}
