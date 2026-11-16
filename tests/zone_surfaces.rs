use atelier_bim::{
    automation_registry::{NativeBatch, describe},
    editor::Editor,
    model::{ModelAdapter, Project, Shape},
    storage::JsonAdapter,
    view_settings::Model3dStyle,
    zones::surfaces::{ZoneSurface, random_commands},
};
use serde_json::json;
fn commit(e: &mut Editor, id: &str, value: serde_json::Value) {
    let request: NativeBatch = serde_json::from_value(value).unwrap();
    let change = e
        .prepare_batch_change(e.revision, &request.lower(e).unwrap())
        .unwrap();
    e.commit_change(id, &change).unwrap();
}
fn fixture() -> Editor {
    let mut e = Editor::new(Project::default());
    commit(
        &mut e,
        "create",
        json!({"version":1,"creates":[{"name":"room","story":0,"layer":0,"material":"plaster","geometry":{"type":"zone","space_name":"Editable courtyard space","purpose":"concept","outline":[{"x":0,"y":0},{"x":10,"y":0},{"x":10,"y":8},{"x":0,"y":8}],"holes":[[{"x":2,"y":2},{"x":4,"y":2},{"x":4,"y":4},{"x":2,"y":4}]]}}]}),
    );
    e
}
#[test]
fn zone_surfaces_atomic_cancel_undo_roundtrip_and_marker() {
    let mut e = fixture();
    let old = e.project.clone();
    let id = e.project.elements[0].id;
    let commands = random_commands(&e, &[id], "one-request").unwrap();
    let change = e.prepare_change(e.revision, commands).unwrap();
    assert_eq!(e.project, old);
    drop(change);
    assert_eq!(e.project, old);
    let c = e
        .prepare_change(
            e.revision,
            random_commands(&e, &[id], "one-request").unwrap(),
        )
        .unwrap();
    e.commit_change("color-once", &c).unwrap();
    let mut restored = e.project.elements[0].clone();
    let Shape::Zone { spec } = &mut restored.shape else {
        panic!()
    };
    assert!(!spec.surface.is_inherit());
    spec.surface = Default::default();
    assert_eq!(restored, old.elements[0]);
    let colored = e.project.clone();
    e.undo();
    assert_eq!(e.project, old);
    e.redo();
    assert_eq!(e.project, colored);
    let bytes = JsonAdapter.export(&e.project).unwrap();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v["schema_version"], 34);
    assert_eq!(JsonAdapter.import(&bytes).unwrap(), e.project);
    for version in [2, 3, 26, 33] {
        let mut v = v.clone();
        v["schema_version"] = json!(version);
        assert!(
            JsonAdapter
                .import(&serde_json::to_vec(&v).unwrap())
                .is_err()
        );
    }
    assert!(random_commands(&e, &[id, id], "bad").is_err());
    e.project.edit_state.locked.insert(id);
    assert!(
        e.prepare_change(e.revision, random_commands(&e, &[id], "locked").unwrap())
            .is_err()
    );
}
#[test]
fn zone_surfaces_omitted_update_preserves_explicit_reset_and_styles_use_native_path() {
    let mut e = fixture();
    let id = e.project.elements[0].id;
    let c = e
        .prepare_change(e.revision, random_commands(&e, &[id], "paint").unwrap())
        .unwrap();
    e.commit_change("paint", &c).unwrap();
    let original = e.project.elements[0].shape.clone();
    let mut g = json!({"type":"zone","space_name":"Revised","purpose":"concept","outline":[{"x":0,"y":0},{"x":10,"y":0},{"x":10,"y":8},{"x":0,"y":8}],"holes":[[{"x":2,"y":2},{"x":4,"y":2},{"x":4,"y":4},{"x":2,"y":4}]],"height":4});
    commit(
        &mut e,
        "height",
        json!({"version":1,"edits":[{"operation":"update","target":{"kind":"existing","value":id},"geometry":g}]}),
    );
    let (Shape::Zone { spec: a }, Shape::Zone { spec: b }) =
        (&original, &e.project.elements[0].shape)
    else {
        panic!()
    };
    assert_eq!(a.surface, b.surface);
    assert_eq!(b.height, 4.);
    for (n, style) in [
        Model3dStyle::Wireframe,
        Model3dStyle::Shaded,
        Model3dStyle::ShadedWithEdges,
    ]
    .into_iter()
    .enumerate()
    {
        let before = e.project.elements.clone();
        commit(
            &mut e,
            &format!("style-{n}"),
            json!({"version":1,"edits":[{"operation":"set3d_style","style":style}]}),
        );
        assert_eq!(e.project.elements, before);
        assert_eq!(e.project.plan_options.model_3d_style, style);
        assert_eq!(
            JsonAdapter
                .import(&JsonAdapter.export(&e.project).unwrap())
                .unwrap(),
            e.project
        );
    }
    assert!(describe("3d_style").is_some());
    g["surface"] = json!({"type":"inherit"});
    commit(
        &mut e,
        "reset",
        json!({"version":1,"edits":[{"operation":"update","target":{"kind":"existing","value":id},"geometry":g}]}),
    );
    let Shape::Zone { spec } = &e.project.elements[0].shape else {
        panic!()
    };
    assert_eq!(spec.surface, ZoneSurface::Inherit);
}
#[test]
fn zone_surfaces_real_example_keeps_all_other_data_and_view_cache_modes() {
    let mut e = Editor::new(atelier_bim::storage::startup_example());
    let old = e.project.clone();
    let ids: Vec<_> = old
        .elements
        .iter()
        .filter(|e| matches!(e.shape, Shape::Zone { .. }))
        .map(|e| e.id)
        .collect();
    assert!(!ids.is_empty());
    let c = e
        .prepare_change(
            e.revision,
            random_commands(&e, &ids, "persistent-example").unwrap(),
        )
        .unwrap();
    e.commit_change("example-paint", &c).unwrap();
    let mut round = JsonAdapter
        .import(&JsonAdapter.export(&e.project).unwrap())
        .unwrap();
    for item in &mut round.elements {
        if let Shape::Zone { spec } = &mut item.shape {
            spec.surface = Default::default();
        }
    }
    assert_eq!(round, old);
    let mut cache = atelier_bim::derivation_cache::DerivationCache::default();
    let mut scenes = atelier_bim::render::CpuSceneCache::default();
    for style in [
        Model3dStyle::ShadedWithEdges,
        Model3dStyle::Wireframe,
        Model3dStyle::Shaded,
    ] {
        e.project.plan_options.model_3d_style = style;
        e.project.grid_settings.show_construction_grid = false;
        let mut read = cache.read(&e.project, 0, 1).unwrap();
        let view = atelier_bim::render::SceneView {
            selected: &[],
            hovered: None,
            story: None,
            marquee: None,
            outside_marquee: false,
            selection_only: false,
        };
        let scene = scenes.prepare(&e.project, view, &mut read);
        assert!(scene.diagnostics.is_empty());
        if style == Model3dStyle::Wireframe {
            assert_eq!(scene.triangle_count(), 0);
            assert!(scene.line_count() > 0);
        } else {
            assert!(scene.triangle_count() > 0);
            if style == Model3dStyle::Shaded {
                assert_eq!(scene.line_count(), 0);
            }
        }
    }
}

#[test]
fn zone_surfaces_retained_figure_snapshot_requires_marker_after_current_reset() {
    let mut e = fixture();
    let id = e.project.elements[0].id;
    let change = e
        .prepare_change(e.revision, random_commands(&e, &[id], "snapshot").unwrap())
        .unwrap();
    e.commit_change("paint-snapshot", &change).unwrap();
    commit(
        &mut e,
        "figure",
        json!({"version":1,"edits":[{"operation":"diagram","edit":{"action":"create","name":"Retained surface snapshot","definition":{"story":0,"option":null,"domain":"zones"}}}]}),
    );
    assert!(!e.project.drawings.diagrams.is_empty());
    let mut element = e.project.elements[0].clone();
    let Shape::Zone { spec } = &mut element.shape else {
        panic!()
    };
    spec.surface = Default::default();
    e.replace_element(element).unwrap();
    let bytes = JsonAdapter.export(&e.project).unwrap();
    let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(value["schema_version"], 34);
    assert_eq!(JsonAdapter.import(&bytes).unwrap(), e.project);
    value["schema_version"] = json!(11);
    assert!(
        JsonAdapter
            .import(&serde_json::to_vec(&value).unwrap())
            .is_err()
    );
}
