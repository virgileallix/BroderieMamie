//! Formats supplémentaires et repérage du même motif dans plusieurs formats.
//! Valeurs de référence : pyembroidery (fichiers générés par pyembroidery, sauf HUS et SEW faits à la main).
use broderie_core::{library::EntryUpdate, parse_named, shape_similarity, Format, GroupChoice, Library};
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
fn doublons_montres_puis_choisis() {
    let dir = tempfile::tempdir().unwrap();
    let mut lib = Library::open(dir.path()).unwrap();
    let r = lib.import(&[fx("Rose_ancienne.jef"), fx("Ourson_Leon.hus")], |_, _, _| {});
    assert_eq!(r.added.len(), 2);
    let jef = r.added[0].id.clone();
    lib.update(&jef, EntryUpdate { tags: Some(vec!["fleurs".into()]), favorite: Some(true), ..Default::default() }).unwrap();

    // DST et EXP ont exactement les mêmes points que le JEF : écartés d'office (rien n'est perdu,
    // les originaux restent où ils sont). Les PES sont ajoutés, mais signalés comme ressemblants.
    let r = lib.import(&[fx("Rose_ancienne.dst"), fx("Rose_ancienne.pes"), fx("Rose_ancienne.exp"), fx("Ourson_Leon.pes")], |_, _, _| {});
    assert_eq!(r.added.len(), 2, "{r:?}");
    assert_eq!(r.duplicates.len(), 2, "{r:?}");
    assert_eq!(r.similar.len(), 2, "{r:?}");
    assert_eq!(lib.entries().len(), 4);

    // Deux groupes ; le PES est proposé en premier (à garder). Rien n'a encore été supprimé.
    let groups = lib.find_duplicates();
    assert_eq!(groups.len(), 2, "{groups:?}");
    let fmt = |id: &str| lib.get(id).unwrap().format.clone().unwrap();
    for g in &groups {
        assert_eq!(g.ids.len(), 2);
        assert_eq!(fmt(&g.ids[0]), "PES");
    }
    let rose = groups.iter().find(|g| g.ids.contains(&jef)).unwrap().clone();
    let ourson = groups.iter().find(|g| !g.ids.contains(&jef)).unwrap().clone();

    // Mamie met le JEF de la rose à la corbeille, et garde les deux oursons.
    let n = lib
        .remove_duplicates(&[
            GroupChoice { keep: vec![rose.ids[0].clone()], remove: vec![jef.clone()] },
            GroupChoice { keep: ourson.ids.clone(), remove: vec![] },
        ])
        .unwrap();
    assert_eq!(n, 1);
    assert_eq!(lib.entries().len(), 3);
    let kept = lib.get(&rose.ids[0]).unwrap();
    assert_eq!(kept.tags, ["fleurs"]);
    assert!(kept.favorite);
    assert!(dir.path().join("Corbeille/Rose ancienne.jef").exists());

    // Les oursons gardés ensemble ne sont plus proposés, même après réouverture.
    let lib = Library::open(dir.path()).unwrap();
    assert!(lib.find_duplicates().is_empty());
    assert!(lib.entries().iter().all(|e| e.shape.is_some()));
}

#[test]
fn rien_ne_sort_du_dossier_de_la_bibliotheque() {
    // Les fichiers d'origine ne sont jamais modifiés ni supprimés.
    let src = tempfile::tempdir().unwrap();
    let dir = tempfile::tempdir().unwrap();
    for f in ["Rose_ancienne.jef", "Rose_ancienne.pes"] {
        fs::copy(fx(f), src.path().join(f)).unwrap();
    }
    let mut lib = Library::open(dir.path()).unwrap();
    lib.import(&[src.path().to_path_buf()], |_, _, _| {});
    let g = lib.find_duplicates();
    lib.remove_duplicates(&[GroupChoice { keep: vec![g[0].ids[0].clone()], remove: g[0].ids[1..].to_vec() }]).unwrap();
    let id = lib.entries()[0].id.clone();
    lib.delete(&id).unwrap();
    for f in ["Rose_ancienne.jef", "Rose_ancienne.pes"] {
        assert_eq!(fs::read(src.path().join(f)).unwrap(), fs::read(fx(f)).unwrap());
    }
}
