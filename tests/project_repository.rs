use atelier_bim::{
    dxf::{self, inventory},
    editor::Editor,
    model::Project,
    project_repository::{LengthUnit, Repository},
    storage,
};
use std::{fs, path::PathBuf};
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "oa-repository-{}",
            storage::new_document_identity()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn dxf(units: i32) -> Vec<u8> {
    format!("0\nSECTION\n2\nHEADER\n9\n$INSUNITS\n70\n{units}\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n0\nLINE\n5\nAA\n8\nWalls\n10\n0\n20\n0\n11\n1000\n21\n0\n0\nINSERT\n5\naa\n8\nFurniture\n2\nChair\n0\nENDSEC\n0\nEOF\n").into_bytes()
}
#[test]
fn inventory_covers_unsupported_entities_without_import_or_unit_approval() {
    let bytes = dxf(0);
    let result = inventory::inspect(&bytes).unwrap();
    assert_eq!(result.summary.records, 2);
    assert_eq!(result.summary.declared_units, vec![0]);
    assert_eq!(result.summary.by_type["INSERT"], 1);
    assert_eq!(result.summary.duplicate_handles, 1);
    assert_eq!(result.entities[1].block.as_deref(), Some("Chair"));
    assert!(result.entities[1].group_pair_index > result.entities[0].group_pair_index);
    assert!(dxf::parse(&bytes).is_err());
    let mut raw = dxf(6);
    let at = raw.windows(5).position(|s| s == b"Walls").unwrap();
    raw[at] = 0xff;
    let result = inventory::inspect(&raw).unwrap();
    assert_eq!(result.summary.non_utf8_values, 1);
    assert_eq!(result.entities[0].layer.as_deref(), Some("hex:ff616c6c73"));
    for bytes in [
        b"AutoCAD Binary DXF\r\n".as_slice(),
        b"0\nSECTION\n2\nENTITIES\n0\nEOF\n",
        b"0\nSECTION\n2\nENTITIES\n0\nENDSEC\n",
        b"0\nSECTION\n2\nENTITIES\n0\nENDSEC\n0\nEOF\n0\nLINE\n",
        b"BAD\nLINE\n",
        b"0\nSECTION\n2",
    ] {
        assert!(inventory::inspect(bytes).is_err());
    }
    let conflict = String::from_utf8(dxf(0)).unwrap().replace(
        "0\nENDSEC\n0\nSECTION",
        "9\n$INSUNITS\n70\n6\n0\nENDSEC\n0\nSECTION",
    );
    assert_eq!(
        inventory::inspect(conflict.as_bytes())
            .unwrap()
            .summary
            .declared_units,
        vec![0, 6]
    );
}
#[test]
fn portable_source_admission_and_editable_copy_preserve_original_and_model() {
    let fixture = Fixture::new();
    let root = fixture.0.join("project");
    let source = fixture.0.join("source.dxf");
    let raw = dxf(0);
    fs::write(&source, &raw).unwrap();
    let mut editor = Editor::new(Project::default());
    editor
        .place_column(atelier_bim::model::Point::new(3., 4.))
        .unwrap();
    let model = editor.project.clone();
    let context = (
        editor.revision,
        editor.undo.len(),
        editor.dirty(),
        editor.project_identity().to_owned(),
    );
    let mut repository = Repository::create_for_editor(&root, &editor).unwrap();
    let id = repository.add_source(&source).unwrap();
    let item = &repository.manifest.sources[0];
    assert_eq!(fs::read(root.join(&item.blob.path)).unwrap(), raw);
    assert_eq!(item.summary.as_ref().unwrap().records, 2);
    assert!(item.confirmed_unit.is_none());
    repository.add_source(&source).unwrap();
    assert_eq!(repository.manifest.revision, 1);
    assert_eq!(repository.manifest.sources.len(), 1);
    let renamed = fixture.0.join("renamed.dxf");
    fs::write(&renamed, &raw).unwrap();
    repository.add_source(&renamed).unwrap();
    assert_eq!(repository.manifest.sources[0].names.len(), 2);
    repository
        .confirm_unit(&id, Some(LengthUnit::Millimetre))
        .unwrap();
    assert_eq!(
        repository.manifest.sources[0].confirmed_unit,
        Some(LengthUnit::Millimetre)
    );
    assert_eq!(LengthUnit::Millimetre.metres(), 0.001);
    let model_hash = repository.manifest.model.sha256.clone();
    let working = repository.working_model_copy().unwrap();
    assert_ne!(working, repository.model_path().unwrap());
    let mut edited = Editor::from_stored_document(&working).unwrap();
    edited
        .place_column(atelier_bim::model::Point::new(9., 9.))
        .unwrap();
    storage::save_document(&edited.project, &working, edited.project_identity()).unwrap();
    assert_eq!(
        Repository::open(&root).unwrap().manifest.model.sha256,
        model_hash
    );
    let moved = fixture.0.join("moved");
    fs::rename(&root, &moved).unwrap();
    let repository = Repository::open(&moved).unwrap();
    let restored = repository.load_model().unwrap();
    assert_eq!(restored.project, model);
    assert_eq!(restored.project_identity(), editor.project_identity());
    assert_eq!(editor.project, model);
    assert_eq!(
        context,
        (
            editor.revision,
            editor.undo.len(),
            editor.dirty(),
            editor.project_identity().to_owned()
        )
    );
    assert_eq!(fs::read(&source).unwrap(), raw);
}
#[test]
fn failed_and_stale_updates_do_not_replace_committed_manifest() {
    let fixture = Fixture::new();
    let root = fixture.0.join("project");
    let editor = Editor::new(Project::default());
    let mut first = Repository::create_for_editor(&root, &editor).unwrap();
    let mut stale = Repository::open(&root).unwrap();
    let source = fixture.0.join("broken.dxf");
    fs::write(&source, b"malformed").unwrap();
    let id = first.add_source(&source).unwrap();
    assert!(first.manifest.sources[0].inventory_error.is_some());
    assert!(first.manifest.sources[0].summary.is_none());
    let manifest = fs::read(root.join("project.json")).unwrap();
    assert!(stale.add_source(&source).is_err());
    assert_eq!(fs::read(root.join("project.json")).unwrap(), manifest);
    let lock = root.join(".openarchi-repository.lock");
    fs::write(&lock, b"external-writer").unwrap();
    assert!(first.confirm_unit(&id, Some(LengthUnit::Metre)).is_err());
    assert_eq!(fs::read(&lock).unwrap(), b"external-writer");
    fs::remove_file(&lock).unwrap();
    assert!(Repository::create_for_editor(&root, &editor).is_err());
    assert_eq!(fs::read(root.join("project.json")).unwrap(), manifest);
    fs::write(root.join(&first.manifest.sources[0].blob.path), b"tampered").unwrap();
    assert!(Repository::open(&root).is_err());
    assert!(first.confirm_unit(&id, None).is_err());
    assert_eq!(fs::read(root.join("project.json")).unwrap(), manifest);
}
#[test]
fn missing_resources_and_unsafe_or_future_manifests_are_refused() {
    let fixture = Fixture::new();
    let root = fixture.0.join("project");
    let editor = Editor::new(Project::default());
    let repository = Repository::create_for_editor(&root, &editor).unwrap();
    let path = root.join("project.json");
    let original = fs::read(&path).unwrap();
    for (key, value) in [
        ("schema_version", serde_json::json!(77)),
        ("document_id", serde_json::json!("bad")),
    ] {
        let mut manifest: serde_json::Value = serde_json::from_slice(&original).unwrap();
        manifest[key] = value;
        fs::write(&path, serde_json::to_vec(&manifest).unwrap()).unwrap();
        assert!(Repository::open(&root).is_err());
    }
    let mut manifest: serde_json::Value = serde_json::from_slice(&original).unwrap();
    manifest["model"]["path"] = serde_json::json!("../outside.json");
    fs::write(&path, serde_json::to_vec(&manifest).unwrap()).unwrap();
    assert!(Repository::open(&root).is_err());
    fs::write(&path, &original).unwrap();
    fs::remove_file(repository.model_path().unwrap()).unwrap();
    assert!(Repository::open(&root).is_err());
}
#[cfg(unix)]
#[test]
fn resource_directory_and_manifest_symlinks_never_redirect_writes() {
    use std::os::unix::fs::symlink;
    let fixture = Fixture::new();
    let root = fixture.0.join("project");
    let outside = fixture.0.join("outside");
    fs::create_dir(&outside).unwrap();
    let editor = Editor::new(Project::default());
    let mut repository = Repository::create_for_editor(&root, &editor).unwrap();
    symlink(&outside, root.join("sources")).unwrap();
    let source = fixture.0.join("raw.dwg");
    fs::write(&source, b"original DWG").unwrap();
    assert!(repository.add_source(&source).is_err());
    assert_eq!(fs::read_dir(&outside).unwrap().count(), 0);
    fs::remove_file(root.join("sources")).unwrap();
    let manifest = root.join("project.json");
    let original = fs::read(&manifest).unwrap();
    let target = outside.join("target.json");
    fs::write(&target, &original).unwrap();
    fs::remove_file(&manifest).unwrap();
    symlink(&target, &manifest).unwrap();
    assert!(Repository::open(&root).is_err());
    assert!(repository.add_source(&source).is_err());
    assert_eq!(fs::read(target).unwrap(), original);
}

#[test]
fn model_checkpoint_is_atomic_and_keeps_previous_snapshot_sources_and_editor_history() {
    let fixture = Fixture::new();
    let root = fixture.0.join("project");
    let mut editor = Editor::new(Project::default());
    let mut repository = Repository::create_for_editor(&root, &editor).unwrap();
    let first_path = repository.model_path().unwrap();
    let first_bytes = fs::read(&first_path).unwrap();
    let source = fixture.0.join("source.dxf");
    fs::write(&source, dxf(6)).unwrap();
    let id = repository.add_source(&source).unwrap();
    repository
        .confirm_unit(&id, Some(LengthUnit::Metre))
        .unwrap();
    let sources = serde_json::to_value(&repository.manifest.sources).unwrap();
    editor
        .place_column(atelier_bim::model::Point::new(10., 4.))
        .unwrap();
    let model = editor.project.clone();
    let history = editor.undo.len();
    let mut stale = Repository::open(&root).unwrap();
    repository
        .checkpoint(&editor.project, editor.project_identity(), editor.revision)
        .unwrap();
    assert_eq!(repository.manifest.model_revision, editor.revision);
    assert_ne!(repository.model_path().unwrap(), first_path);
    assert_eq!(fs::read(first_path).unwrap(), first_bytes);
    assert_eq!(
        serde_json::to_value(&repository.manifest.sources).unwrap(),
        sources
    );
    let reopened = Repository::open(&root).unwrap().load_model().unwrap();
    assert_eq!(reopened.project, model);
    assert_eq!(reopened.project_identity(), editor.project_identity());
    assert_eq!(editor.undo.len(), history);
    assert!(editor.dirty());
    let committed = fs::read(root.join("project.json")).unwrap();
    assert!(
        stale
            .checkpoint(&model, editor.project_identity(), editor.revision)
            .is_err()
    );
    assert!(
        repository
            .checkpoint(&model, &storage::new_document_identity(), editor.revision)
            .is_err()
    );
    assert_eq!(fs::read(root.join("project.json")).unwrap(), committed);
    editor.undo();
    assert!(editor.project.elements.is_empty());
    assert_eq!(reopened.project, model);
}

#[test]
fn repository_carries_verified_drawing_sidecars_after_original_cache_is_gone() {
    use atelier_bim::{command::ModelCommand, drawings};
    let fixture = Fixture::new();
    let image_path = fixture.0.join("original.png");
    image::RgbaImage::from_pixel(40, 20, image::Rgba([230u8, 220, 210, 255]))
        .save(&image_path)
        .unwrap();
    let cache = fixture.0.join("outside-cache");
    let page = drawings::import(&image_path, &cache, 1).unwrap();
    let expected = drawings::verified_bytes(&page).unwrap();
    let mut editor = Editor::new(Project::default());
    editor
        .apply(ModelCommand::SetDrawings {
            document: drawings::DrawingDocument {
                pages: vec![page],
                ..Default::default()
            },
        })
        .unwrap();
    let root = fixture.0.join("project");
    Repository::create_for_editor(&root, &editor).unwrap();
    fs::remove_file(&image_path).unwrap();
    fs::remove_dir_all(cache).unwrap();
    let moved = fixture.0.join("moved");
    fs::rename(root, &moved).unwrap();
    let repository = Repository::open(&moved).unwrap();
    let reopened = repository.load_model().unwrap();
    assert_eq!(reopened.project_identity(), editor.project_identity());
    assert_eq!(
        drawings::verified_bytes(&reopened.project.drawings.pages[0]).unwrap(),
        expected
    );
    assert!(
        reopened.project.drawings.pages[0]
            .path
            .starts_with(fs::canonicalize(&moved).unwrap())
    );
    fs::write(&reopened.project.drawings.pages[0].path, b"corrupt").unwrap();
    assert!(Repository::open(&moved).is_err());
}
