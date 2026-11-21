use broderie_core::{library::EntryUpdate, parse, Format, Library};
use std::fs;
use std::path::PathBuf;

fn fx(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)
}

// Valeurs de référence obtenues avec la bibliothèque Python « pyembroidery ».
#[test]
fn lit_les_pes() {
    for f in ["fleur.pes", "fleur_v6.pes"] {
        let d = parse(&fs::read(fx(f)).unwrap()).unwrap();
        assert_eq!(d.format, Format::Pes);
        assert_eq!(d.stitch_count, 850, "{f}");
        assert_eq!((d.min_x, d.min_y, d.max_x, d.max_y), (-230.0, -300.0, 340.0, 296.0), "{f}");
        assert_eq!(d.blocks.len(), 3);
    }
    let v6 = parse(&fs::read(fx("fleur_v6.pes")).unwrap()).unwrap();
    let colors: Vec<_> = v6.blocks.iter().map(|b| b.thread.color.as_str()).collect();
    assert_eq!(colors, ["#ff0000", "#00aa00", "#2244ff"]);
    let v1 = parse(&fs::read(fx("fleur.pes")).unwrap()).unwrap();
    assert_eq!(v1.blocks[0].thread.color, "#ed171f");
}

#[test]
fn lit_les_vp3() {
    // Les longs déplacements (> 12,7 mm) ne comptent pas comme des points cousus.
    let d = parse(&fs::read(fx("fleur.vp3")).unwrap()).unwrap();
    assert_eq!(d.format, Format::Vp3);
    assert_eq!(d.blocks.len(), 3);
    assert_eq!((d.min_x.round(), d.max_x.round(), d.min_y.round(), d.max_y.round()), (-229.0, 339.0, -296.0, 296.0));
    let r = parse(&fs::read(fx("rosace.vp3")).unwrap()).unwrap();
    assert_eq!(r.blocks.len(), 5);
    assert!(r.stitch_count > 7000);
}

#[test]
fn meme_dessin_meme_empreinte() {
    let a = parse(&fs::read(fx("fleur.pes")).unwrap()).unwrap();
    let b = parse(&fs::read(fx("fleur_v6.pes")).unwrap()).unwrap();
    assert_eq!(a.design_hash(), b.design_hash());
}

#[test]
fn refuse_les_autres_fichiers() {
    assert!(parse(b"bonjour").is_err());
    assert!(parse(b"#PES0001").is_err());
}

#[test]
fn bibliotheque_sans_doublons() {
    let dir = tempfile::tempdir().unwrap();
    let src = tempfile::tempdir().unwrap();
    // un dossier avec des copies renommées
    for (f, to) in [("fleur.pes", "fleur.pes"), ("fleur.pes", "copie de fleur.pes"), ("fleur_v6.pes", "fleur_v6.pes"),
                    ("fleur.vp3", "fleur.vp3"), ("fleur.vp3", "fleur.pv3"), ("rosace.pes", "rosace.pes")] {
        fs::copy(fx(f), src.path().join(to)).unwrap();
    }
    fs::write(src.path().join("notes.txt"), "x").unwrap();

    let mut lib = Library::open(dir.path()).unwrap();
    let rep = lib.import(&[src.path().to_path_buf(), fx("lot.zip")], |_, _, _| {});
    assert_eq!(rep.added.len(), 3, "{rep:?}"); // fleur.pes, fleur.vp3, rosace.pes
    assert_eq!(rep.duplicates.len(), 5, "{rep:?}");
    assert_eq!(rep.ignored, 2); // notes.txt + lisezmoi.txt du zip
    assert!(rep.problems.is_empty());
    for e in lib.entries() {
        assert!(lib.thumb_path(&e.id).exists());
        assert!(lib.file_path(e).exists());
    }

    // ré-import : tout est en double
    let rep2 = lib.import(&[fx("fleur.pes")], |_, _, _| {});
    assert!(rep2.added.is_empty() && rep2.duplicates.len() == 1);

    // renommer + mots-clés, puis rouvrir
    let id = lib.entries().iter().find(|e| e.ext == "PES" && e.name == "fleur").unwrap().id.clone();
    let e = lib.update(&id, EntryUpdate { name: Some("Fleur de Noël".into()), tags: Some(vec!["Noël".into(), "noël".into(), " fleurs ".into()]), favorite: Some(true) }).unwrap();
    assert_eq!(e.file_name, "Fleur de Noël.pes");
    assert_eq!(e.tags, ["Noël", "fleurs"]);
    let lib = Library::open(dir.path()).unwrap();
    assert_eq!(lib.get(&id).unwrap().name, "Fleur de Noël");
    assert!(lib.get(&id).unwrap().favorite);

    // export, sauvegarde, suppression vers la corbeille
    let out = tempfile::tempdir().unwrap();
    let p = lib.export(&id, out.path()).unwrap();
    assert_eq!(fs::read(p).unwrap(), fs::read(fx("fleur.pes")).unwrap());
    assert_eq!(lib.backup(&out.path().join("sauvegarde.zip")).unwrap(), 3);
    let mut lib = lib;
    lib.delete(&id).unwrap();
    assert_eq!(lib.entries().len(), 2);
    assert!(dir.path().join("Corbeille/Fleur de Noël.pes").exists());
}
