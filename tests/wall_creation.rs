use atelier_bim::{
    command::ModelCommand,
    editor::{Editor, Tool},
    model::{Point, Project, Shape},
    wall::WallGeometryMethod,
};
fn square() -> Vec<(Point, Point)> {
    let p = [
        Point::new(0., 0.),
        Point::new(4., 0.),
        Point::new(4., 3.),
        Point::new(0., 3.),
    ];
    (0..4).map(|i| (p[i], p[(i + 1) % 4])).collect()
}
#[test]
fn typed_wall_batch_matches_rectangle_parameters_autogroup_and_single_history() {
    for group in [false, true] {
        let mut api = Editor::new(Project::default());
        api.project.edit_state.autogroup = group;
        api.set_wall_geometry_method(WallGeometryMethod::Rectangular);
        let before = api.project.clone();
        let ids = api.place_wall_segments(square()).unwrap();
        let created = api.project.clone();
        let mut legacy = before.clone();
        for (start, end) in square() {
            legacy.add_with_status(
                api.story,
                api.material,
                Shape::Wall {
                    start,
                    end,
                    thickness: api.thickness,
                    height: api.height,
                },
                api.wall_renovation,
            );
            let element = legacy.elements.last_mut().unwrap();
            element.layer = api.wall_layer;
            element.wall_settings = Some(api.wall_settings.clone());
        }
        if group {
            legacy.edit_state.groups.insert(0, ids.clone());
        }
        assert_eq!(created, legacy);

        assert_eq!(api.undo.len(), 1);
        assert_eq!(ids, vec![1, 2, 3, 4]);
        assert_eq!(
            created.edit_state.groups,
            if group { vec![ids.clone()] } else { vec![] }
        );
        let mut ui = Editor::new(before.clone());
        ui.set_tool(Tool::Wall);
        ui.set_wall_geometry_method(WallGeometryMethod::Rectangular);
        ui.click(Point::default(), 0.01);
        ui.click(Point::new(4., 3.), 0.01);
        assert_eq!(ui.project, created);
        assert_eq!(ui.selected, Some(4));
        assert_eq!(ui.selected_ids, vec![4]);
        assert_eq!(
            ui.selection_ids(),
            if group { ids.clone() } else { vec![4] }
        );
        assert_eq!(ui.undo.len(), 1);
        assert!(matches!(
            ui.undo[0].command,
            ModelCommand::AddElements { .. }
        ));
        api.undo();
        assert_eq!(api.project, before);
        api.redo();
        assert_eq!(api.project, created);
        let path =
            std::env::temp_dir().join(format!("oa-wall-batch-{}-{group}.json", std::process::id()));
        atelier_bim::storage::save(&created, &path).unwrap();
        assert_eq!(atelier_bim::storage::load(&path).unwrap(), created);
        std::fs::remove_file(path).unwrap();
    }
}
#[test]
fn batch_rejects_duplicate_overflow_and_invalid_later_element_without_consuming_redo() {
    let mut source = Editor::new(Project::default());
    source.place_wall_segments(square()).unwrap();
    let walls = source.project.elements.clone();
    let mut e = Editor::new(Project::default());
    e.place_column(Point::new(-2., -2.)).unwrap();
    let redo = e.project.clone();
    e.undo();
    let before = e.project.clone();
    let revision = e.revision;
    for mode in 0..3 {
        let mut batch = walls.clone();
        match mode {
            0 => batch[3].id = batch[0].id,
            1 => batch[3].id = u64::MAX,
            _ => {
                if let Shape::Wall {
                    ref mut thickness, ..
                } = batch[3].shape
                {
                    *thickness = f64::NAN;
                }
            }
        }
        assert!(
            e.apply(ModelCommand::AddElements {
                elements: batch,
                group: true
            })
            .is_err()
        );
        assert_eq!(e.project, before);
        assert_eq!(e.revision, revision);
        assert!(e.undo.is_empty());
    }
    assert!(
        e.apply(ModelCommand::AddElements {
            elements: vec![],
            group: true
        })
        .unwrap()
        .is_empty()
    );
    assert_eq!(e.project, before);
    assert_eq!(e.revision, revision);
    e.redo();
    assert_eq!(e.project, redo);
}
#[test]
fn invalid_wall_batch_preserves_accepted_draft_and_last_legal_batch_undoes() {
    let mut e = Editor::new(Project::default());
    e.set_tool(Tool::Wall);
    e.set_wall_geometry_method(WallGeometryMethod::Continuous);
    e.click(Point::default(), 0.01);
    e.click(Point::new(4., 0.), 0.01);
    let nodes = e.wall_points.clone();
    let anchor = e.anchor;
    let before = e.project.clone();
    let mut segments = square();
    segments[3].1 = segments[3].0;
    assert!(e.place_wall_segments(segments).is_err());
    assert_eq!(e.project, before);
    assert_eq!(e.wall_points, nodes);
    assert_eq!(e.anchor, anchor);
    e.project.next_id = u64::MAX - 4;
    let before = e.project.clone();
    let ids = e.place_wall_segments(square()).unwrap();
    assert_eq!(ids.last(), Some(&(u64::MAX - 1)));
    assert_eq!(e.project.next_id, u64::MAX);
    e.undo();
    assert_eq!(e.project, before);
}

#[test]
fn later_locked_layer_blocks_entire_typed_batch_and_wall_api() {
    let mut source = Editor::new(Project::default());
    source.place_wall_segments(square()).unwrap();
    let mut e = Editor::new(Project::default());
    let layer = e.project.layers.new_layer("Batch locked", None);
    e.project
        .layers
        .layers
        .iter_mut()
        .find(|l| l.id == layer)
        .unwrap()
        .status
        .locked = true;
    let before = e.project.clone();
    let mut batch = source.project.elements.clone();
    batch[3].layer = atelier_bim::layers::ElementLayer(layer);
    assert!(
        e.apply(ModelCommand::AddElements {
            elements: batch,
            group: true
        })
        .is_err()
    );
    assert_eq!(e.project, before);
    assert!(e.undo.is_empty());
    assert_eq!(e.revision, 0);
    e.wall_layer = atelier_bim::layers::ElementLayer(layer);
    let status = e.status.clone();
    let error = e.wall_creation_candidates(square()).unwrap_err();
    assert_eq!(e.status, status);
    assert_eq!(e.project, before);
    assert_eq!(e.place_wall_segments(square()).unwrap_err(), error);
    assert_eq!(e.status, error);
    assert_eq!(e.project, before);
    assert!(e.undo.is_empty());
}

#[test]
fn wall_candidates_are_readonly_and_preserve_batch_defaults_and_failures() {
    for group in [false, true] {
        let mut e = Editor::new(Project::default());
        e.project.edit_state.autogroup = group;
        e.thickness = 0.35;
        e.height = 4.2;
        e.material = atelier_bim::model::Material::Timber;
        e.wall_renovation = atelier_bim::view_settings::RenovationStatus::Existing;
        e.place_column(Point::new(9., 9.)).unwrap();
        let redo = e.project.clone();
        e.undo();
        let before = e.project.clone();
        let revision = e.revision;
        let status = e.status.clone();
        let candidates = e.wall_creation_candidates(square()).unwrap();
        let preview = e.preview_add_elements(candidates.clone(), group).unwrap();
        assert_eq!(e.project, before);
        assert_eq!(e.status, status);
        assert_eq!(e.revision, revision);
        for mode in 0..2 {
            let mut segments = square();
            if mode == 0 {
                segments[3].1 = segments[3].0;
            } else {
                e.project.next_id = u64::MAX - 2;
            }
            let failure_before = e.project.clone();
            let error = e.wall_creation_candidates(segments.clone()).unwrap_err();
            assert_eq!(e.status, status);
            assert_eq!(e.project, failure_before);
            assert_eq!(e.place_wall_segments(segments).unwrap_err(), error);
            assert_eq!(e.status, error);
            assert_eq!(e.project, failure_before);
            assert_eq!(e.revision, revision);
            e.project.next_id = before.next_id;
            e.status = status.clone();
        }
        e.redo();
        assert_eq!(e.project, redo);
        e.undo();
        let ids = e.place_wall_segments(square()).unwrap();
        assert_eq!(ids, candidates.iter().map(|x| x.id).collect::<Vec<_>>());
        assert_eq!(e.project.elements, candidates);
        assert_eq!(e.project, preview);
        assert_eq!(e.undo.len(), 1);
        let json = serde_json::to_string(&e.project).unwrap();
        assert_eq!(serde_json::from_str::<Project>(&json).unwrap(), preview);
        e.undo();
        assert_eq!(e.project, before);
        e.redo();
        assert_eq!(e.project, preview);
    }
}
