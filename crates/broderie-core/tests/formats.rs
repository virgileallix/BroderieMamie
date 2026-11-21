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

#[test]
fn un_dossier_par_dossier_ou_zip_importe() {
    let root = tempfile::tempdir().unwrap();
    let noel = root.path().join("Noël de Mamie");
    fs::create_dir_all(noel.join("sous-dossier")).unwrap();
    fs::copy(fx("Rose_ancienne.pes"), noel.join("Rose_ancienne.pes")).unwrap();
    fs::copy(fx("Sapin_enneige.pes"), noel.join("sous-dossier/Sapin_enneige.pes")).unwrap();
    // un .zip dans le dossier : il garde son propre nom
    let zip_path = noel.join("animaux_2024.zip");
    {
        use std::io::Write;
        let mut z = zip::ZipWriter::new(fs::File::create(&zip_path).unwrap());
        z.start_file("ours/Ourson_Leon.pes", zip::write::SimpleFileOptions::default()).unwrap();
        z.write_all(&fs::read(fx("Ourson_Leon.pes")).unwrap()).unwrap();
        z.finish().unwrap();
    }
    let dir = tempfile::tempdir().unwrap();
    let mut lib = Library::open(dir.path()).unwrap();
    let r = lib.import(&[noel.clone(), fx("Rose_ancienne.jef"), fx("Sapin_enneige.sew")], |_, _, _| {});
    assert_eq!(r.added.len(), 4, "{r:?}");
    let by_name = |n: &str| lib.entries().iter().find(|e| e.name == n && e.ext == "PES").unwrap().clone();
    assert_eq!(by_name("Rose ancienne").folder, "Noël de Mamie");
    assert_eq!(by_name("Sapin enneige").folder, "Noël de Mamie");
    assert_eq!(by_name("Ourson Leon").folder, "animaux 2024");
    assert!(dir.path().join("Motifs/Noël de Mamie/Rose ancienne.pes").exists());
    assert!(dir.path().join("Motifs/animaux 2024/Ourson Leon.pes").exists());
    // Le JEF a exactement les mêmes points que la rose : pas ajouté, on dit dans quel dossier elle est.
    assert_eq!(r.duplicates[0].file, "Rose_ancienne.jef");
    assert_eq!(r.duplicates[0].same_folder, "Noël de Mamie");
    // Un fichier ajouté seul n'a pas de dossier ; le SEW ressemble au sapin du dossier « Noël de Mamie ».
    let sew = lib.entries().iter().find(|e| e.ext == "SEW").unwrap().clone();
    assert_eq!(sew.folder, "");
    assert!(dir.path().join("Motifs/Sapin enneige.sew").exists());
    assert_eq!(r.similar[0].same_folder, "Noël de Mamie");

    // Le même fichier sous un autre nom, dans un autre dossier : reconnu, pas ajouté.
    let autre = root.path().join("Autre");
    fs::create_dir_all(&autre).unwrap();
    fs::copy(fx("Rose_ancienne.pes"), autre.join("ma jolie fleur.pes")).unwrap();
    let r = lib.import(&[autre], |_, _, _| {});
    assert!(r.added.is_empty());
    assert_eq!(r.duplicates[0].same_as, "Rose ancienne");
    assert_eq!(r.duplicates[0].same_folder, "Noël de Mamie");

    // Renommer garde le dossier ; la sauvegarde garde les dossiers.
    let id = lib.entries().iter().find(|e| e.name == "Rose ancienne" && e.ext == "PES").unwrap().id.clone();
    let e = lib.update(&id, EntryUpdate { name: Some("Rose rouge".into()), ..Default::default() }).unwrap();
    assert_eq!(e.file_name, "Noël de Mamie/Rose rouge.pes");
    assert!(dir.path().join("Motifs/Noël de Mamie/Rose rouge.pes").exists());
    let out = tempfile::tempdir().unwrap();
    lib.backup(&out.path().join("s.zip")).unwrap();
    let z = zip::ZipArchive::new(fs::File::open(out.path().join("s.zip")).unwrap()).unwrap();
    let names: Vec<_> = z.file_names().map(str::to_string).collect();
    assert!(names.contains(&"Noël de Mamie/Rose rouge.pes".to_string()), "{names:?}");
    // Supprimer range le fichier dans la corbeille.
    let mut lib = lib;
    lib.delete(&id).unwrap();
    assert!(dir.path().join("Corbeille/Rose rouge.pes").exists());
}

#[test]
fn analyse_par_ia_enregistree() {
    let dir = tempfile::tempdir().unwrap();
    let mut lib = Library::open(dir.path()).unwrap();
    lib.import(&[fx("Rose_ancienne.pes"), fx("Ourson_Leon.pes")], |_, _, _| {});
    let (ids, total) = lib.ai_pending(1);
    assert_eq!((ids.len(), total), (1, 2));
    let png = lib.ai_image(&ids[0]).unwrap();
    assert!(png.starts_with(&[0xFF, 0xD8]), "JPEG");
    lib.set_ai(&ids[0], " Une rose rouge ", &["Rose".into(), "fleur".into(), "rose".into(), "".into()]).unwrap();
    lib.flush().unwrap();
    let lib = Library::open(dir.path()).unwrap();
    let e = lib.get(&ids[0]).unwrap();
    assert_eq!(e.ai_description, "Une rose rouge");
    assert_eq!(e.ai_keywords, ["rose", "fleur"]);
    assert_eq!(lib.ai_pending(10).1, 1);
}

#[test]
fn sous_dossiers_en_mots_cles() {
    let root = tempfile::tempdir().unwrap();
    let base = root.path().join("Broderies de Josiane");
    fs::create_dir_all(base.join("Noël/Anges")).unwrap();
    fs::create_dir_all(base.join("PES/Fleurs_du_jardin")).unwrap();
    fs::copy(fx("Rose_ancienne.pes"), base.join("PES/Fleurs_du_jardin/rose.pes")).unwrap();
    fs::copy(fx("Sapin_enneige.pes"), base.join("Noël/Anges/sapin.pes")).unwrap();
    let zip_path = base.join("Animaux.zip");
    {
        use std::io::Write;
        let mut z = zip::ZipWriter::new(fs::File::create(&zip_path).unwrap());
        z.start_file("Bébé/Ours/ourson.pes", zip::write::SimpleFileOptions::default()).unwrap();
        z.write_all(&fs::read(fx("Ourson_Leon.pes")).unwrap()).unwrap();
        z.finish().unwrap();
    }
    let dir = tempfile::tempdir().unwrap();
    let mut lib = Library::open(dir.path()).unwrap();
    lib.import(&[base], |_, _, _| {});
    let tags = |n: &str| lib.entries().iter().find(|e| e.name == n).unwrap().tags.clone();
    assert_eq!(tags("sapin"), ["Noël", "Anges"]);
    assert_eq!(tags("rose"), ["Fleurs du jardin"], "« PES » n'est pas un mot-clé utile");
    assert_eq!(tags("ourson"), ["Bébé", "Ours"]);
    assert_eq!(lib.entries().iter().find(|e| e.name == "ourson").unwrap().folder, "Animaux");

    // Un mot-clé pour tout un dossier d'un coup.
    let ids: Vec<String> = lib.entries().iter().filter(|e| e.folder == "Broderies de Josiane").map(|e| e.id.clone()).collect();
    assert_eq!(ids.len(), 2);
    let mut lib = lib;
    assert_eq!(lib.add_tag_to(&ids, "Josiane").unwrap(), 2);
    assert_eq!(lib.add_tag_to(&ids, "josiane").unwrap(), 0, "déjà là");
    let lib = Library::open(dir.path()).unwrap();
    assert!(lib.entries().iter().filter(|e| ids.contains(&e.id)).all(|e| e.tags.contains(&"Josiane".to_string())));
}
