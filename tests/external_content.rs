use atelier_bim::{
    command::ModelCommand,
    drawings::{DrawingDocument, DrawingPlacement},
    editor::Editor,
    external::{self, ReferenceEdit, ReferenceKind},
    model::{Material, Point, Project, Shape},
    storage,
};
fn placement(x: f64) -> DrawingPlacement {
    DrawingPlacement {
        worksheet: None,
        story: 0,
        origin: Point::new(x, 0.),
        angle: 0.,
        visible: true,
        opacity: 255,
        frame: None,
    }
}
fn fixture() -> (std::path::PathBuf, Project) {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let dir = std::env::temp_dir().join(format!(
        "oa-external-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let mut p = Project::default();
    p.elements.clear();
    p.next_id = 1;
    p.add(
        0,
        Material::Plaster,
        Shape::Polyline {
            points: vec![Point::new(0., 0.), Point::new(2., 0.)],
            arcs: vec![],
        },
    );
    (dir, p)
}
#[test]
fn linked_instances_update_atomically_lock_direct_edits_and_break_individually() {
    let (dir, p) = fixture();
    let path = dir.join("module.json");
    storage::save(&p, &path).unwrap();
    let mut e = Editor::new(Project::default());
    let before = e.project.clone();
    let source = external::source(&path, ReferenceKind::Hotlink).unwrap();
    e.apply(ModelCommand::External(ReferenceEdit::Place {
        source: source.clone(),
        placement: placement(3.),
    }))
    .unwrap();
    e.apply(ModelCommand::External(ReferenceEdit::Place {
        source,
        placement: placement(10.),
    }))
    .unwrap();
    assert_eq!(e.project.drawings.reference_sources.len(), 1);
    assert_eq!(e.project.drawings.reference_instances.len(), 2);
    let first = e.project.drawings.reference_instances[0].clone();
    let second = e.project.drawings.reference_instances[1].clone();
    assert!(e.locked_element(first.elements[0]));
    let accepted = e.project.clone();
    assert!(
        e.apply(ModelCommand::MoveElements {
            ids: first.elements.clone(),
            offset: Point::new(1., 0.)
        })
        .is_err()
    );
    assert_eq!(e.project, accepted);
    let baseline = e.project.clone();
    e.project
        .translate_many(&first.elements, Point::new(1., 0.));
    e.commit(baseline);
    assert_eq!(e.project, accepted);
    let mut changed = p.clone();
    changed.elements[0].translate(Point::new(0., 5.));
    storage::save(&changed, &path).unwrap();
    let mut source = external::source(&path, ReferenceKind::Hotlink).unwrap();
    source.id = first.source;
    e.apply(ModelCommand::External(ReferenceEdit::Update { source }))
        .unwrap();
    for v in &e.project.drawings.reference_instances {
        let element = e
            .project
            .elements
            .iter()
            .find(|e| e.id == v.elements[0])
            .unwrap();
        if let Shape::Polyline { points, .. } = &element.shape {
            assert_eq!(points[0], Point::new(v.placement.origin.x, 5.));
        } else {
            panic!();
        }
    }
    e.undo();
    assert_eq!(e.project, accepted);
    e.redo();
    let updated = e.project.clone();
    e.apply(ModelCommand::External(ReferenceEdit::Break {
        instance: first.id,
    }))
    .unwrap();
    let member = updated.drawings.reference_instances[0].elements[0];
    assert!(!e.locked_element(member));
    assert_eq!(e.project.drawings.reference_instances[0].id, second.id);
    e.undo();
    assert_eq!(e.project, updated);
    let host = dir.join("host.json");
    storage::save(&e.project, &host).unwrap();
    assert_eq!(storage::load(&host).unwrap(), e.project);
    e.undo();
    e.undo();
    e.undo();
    assert_eq!(e.project, before);
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn xref_unload_reload_bind_and_invalid_update_preserve_history() {
    let (dir, p) = fixture();
    let path = dir.join("external.json");
    storage::save(&p, &path).unwrap();
    let mut e = Editor::new(Project::default());
    let source = external::source(&path, ReferenceKind::Xref).unwrap();
    e.apply(ModelCommand::External(ReferenceEdit::Place {
        source,
        placement: placement(0.),
    }))
    .unwrap();
    let source_id = e.project.drawings.reference_sources[0].id;
    let loaded = e.project.clone();
    e.apply(ModelCommand::External(ReferenceEdit::Loaded {
        source: source_id,
        loaded: false,
    }))
    .unwrap();
    assert!(e.project.elements.is_empty());
    assert!(!e.project.drawings.reference_instances[0].loaded);
    let unloaded = e.project.clone();
    let mut source = external::source(&path, ReferenceKind::Xref).unwrap();
    source.id = source_id;
    e.apply(ModelCommand::External(ReferenceEdit::Reload { source }))
        .unwrap();
    assert_eq!(e.project.elements.len(), 1);
    e.undo();
    assert_eq!(e.project, unloaded);
    e.undo();
    assert_eq!(e.project, loaded);
    e.redo();
    e.redo();
    e.apply(ModelCommand::External(ReferenceEdit::Bind {
        source: source_id,
    }))
    .unwrap();
    assert!(e.project.drawings.reference_instances.is_empty());
    assert!(e.project.drawings.reference_sources.is_empty());
    assert_eq!(e.project.elements.len(), 1);
    assert!(!e.locked_element(e.project.elements[0].id));
    e.undo();
    let before = e.project.clone();
    let mut bad = before.drawings.reference_sources[0].clone();
    bad.elements[0].story = 999;
    assert!(
        e.apply(ModelCommand::External(ReferenceEdit::Update {
            source: bad
        }))
        .is_err()
    );
    assert_eq!(e.project, before);
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn independent_images_search_and_transform_bindings_roundtrip() {
    let (dir, mut p) = fixture();
    let image = dir.join("floor-a.png");
    image::RgbaImage::from_pixel(100, 50, image::Rgba([1, 2, 3, 255]))
        .save(&image)
        .unwrap();
    let mut doc = DrawingDocument::default();
    for x in [0., 10.] {
        let mut page =
            atelier_bim::drawings::import(&image, &dir.join("cache"), doc.next_id().unwrap())
                .unwrap();
        page.placement = Some(placement(x));
        let id = page.id;
        doc.pages.push(page);
        doc.add_region(id, 0, [0., 0., 100., 50.]).unwrap();
    }
    assert_eq!(doc.search_sources("floor-a").len(), 2);
    assert_eq!(doc.pages[0].sha256, doc.pages[1].sha256);
    assert_ne!(doc.pages[0].id, doc.pages[1].id);
    let v = DrawingPlacement {
        angle: 37.,
        ..placement(5.)
    };
    let pixel = [27., 18.];
    let back = v.pixel(v.world(pixel, 0.01), 0.01);
    assert!((back[0] - pixel[0]).abs() < 0.001 && (back[1] - pixel[1]).abs() < 0.001);
    p.drawings = doc;
    storage::save(&p, &dir.join("images.json")).unwrap();
    let reopened = storage::load(&dir.join("images.json")).unwrap();
    assert_eq!(
        reopened.drawings.pages[1].placement,
        p.drawings.pages[1].placement
    );
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn dxf_adapter_respects_units_and_rejects_unknown_entities_without_partial_import() {
    let line = "0\nSECTION\n2\nHEADER\n9\n$INSUNITS\n70\n4\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n0\nLINE\n8\nWalls\n10\n1000\n20\n2000\n11\n3000\n21\n2000\n0\nENDSEC\n0\nEOF\n";
    let p = atelier_bim::dxf::parse(line.as_bytes()).unwrap();
    if let Shape::Polyline { points, .. } = &p.elements[0].shape {
        assert!((points[0].x - 1.).abs() < 1e-5);
        assert!((points[1].x - 3.).abs() < 1e-5);
    } else {
        panic!();
    }
    assert!(atelier_bim::dxf::parse(line.replace("LINE\n", "TEXT\n").as_bytes()).is_err());
    assert!(atelier_bim::dxf::parse(line.replace("$INSUNITS", "$OTHER").as_bytes()).is_err());
    assert!(
        atelier_bim::dxf::parse(line.replace("8\nWalls", "30\n1\n8\nWalls").as_bytes()).is_err()
    );
}

#[test]
fn binding_an_unloaded_xref_embeds_cached_contents_instead_of_losing_them() {
    let (dir, p) = fixture();
    let path = dir.join("unloaded.json");
    storage::save(&p, &path).unwrap();
    let mut e = Editor::new(Project::default());
    let source = external::source(&path, ReferenceKind::Xref).unwrap();
    e.apply(ModelCommand::External(ReferenceEdit::Place {
        source,
        placement: placement(0.),
    }))
    .unwrap();
    let id = e.project.drawings.reference_sources[0].id;
    e.apply(ModelCommand::External(ReferenceEdit::Loaded {
        source: id,
        loaded: false,
    }))
    .unwrap();
    let before = e.project.clone();
    assert!(e.project.elements.is_empty());
    std::fs::remove_file(path).unwrap();
    e.apply(ModelCommand::External(ReferenceEdit::Bind { source: id }))
        .unwrap();
    assert_eq!(e.project.elements.len(), 1);
    assert!(e.project.drawings.reference_instances.is_empty());
    e.undo();
    assert_eq!(e.project, before);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn real_reference_in_mixed_batches_rejects_atomically_and_preserves_redo() {
    let (dir, source_project) = fixture();
    let path = dir.join("mixed-source.json");
    storage::save(&source_project, &path).unwrap();
    let source = external::source(&path, ReferenceKind::Hotlink).unwrap();
    let mut editor = Editor::new(source_project);
    let native = editor.project.elements[0].id;
    editor
        .apply(ModelCommand::External(ReferenceEdit::Place {
            source,
            placement: placement(10.),
        }))
        .unwrap();
    let referenced = editor.project.drawings.reference_instances[0].elements[0];
    assert_ne!(native, referenced);
    assert!(!editor.locked_element(native));
    assert!(editor.locked_element(referenced));
    editor.set_selection(vec![native, referenced]);
    editor
        .apply(ModelCommand::MoveElements {
            ids: vec![native],
            offset: Point::new(1., 0.),
        })
        .unwrap();
    let moved = editor.project.clone();
    editor.undo();
    let before = editor.project.clone();
    let revision = editor.revision;
    let selection = editor.selection_ids();
    let history = (editor.undo.len(), editor.redo.len());
    let dirty = editor.dirty();
    assert!(history.1 > 0);
    for ids in [vec![native, referenced], vec![referenced, native]] {
        for command in [
            ModelCommand::RemoveElements { ids: ids.clone() },
            ModelCommand::MoveElements {
                ids,
                offset: Point::new(2., 0.),
            },
        ] {
            assert_eq!(
                editor.apply(command).unwrap_err(),
                format!("Element {referenced} is locked or read-only")
            );
            assert_eq!(editor.project, before);
            assert_eq!(editor.revision, revision);
            assert_eq!(editor.selection_ids(), selection);
            assert_eq!((editor.undo.len(), editor.redo.len()), history);
            assert_eq!(editor.dirty(), dirty);
        }
    }
    let member = editor
        .project
        .elements
        .iter()
        .find(|e| e.id == referenced)
        .unwrap()
        .clone();
    assert_eq!(
        editor
            .apply(ModelCommand::UpdateElement { element: member })
            .unwrap_err(),
        format!("Element {referenced} is locked or read-only")
    );
    assert_eq!(editor.project, before);
    assert_eq!(editor.revision, revision);
    assert_eq!((editor.undo.len(), editor.redo.len()), history);
    assert_eq!(editor.selection_ids(), selection);
    assert_eq!(editor.dirty(), dirty);
    editor.redo();
    assert_eq!(editor.project, moved);
    std::fs::remove_dir_all(dir).unwrap();
}
