use atelier_bim::{
    automation::{Query, ReadService, SpaceRequest},
    automation_registry::{NativeBatch, describe},
    editor::{Editor, Tool},
    geometry,
    model::{Material, ModelAdapter, Point, Project, Shape},
    storage::JsonAdapter,
    zones::{
        ZoneSpec,
        analysis::{AnalysisOptions, analyze},
    },
};
use serde_json::json;
fn p(x: f64, y: f64) -> Point {
    Point::new(x, y)
}
fn rect(x: f64, y: f64, w: f64, h: f64) -> Vec<Point> {
    vec![p(x, y), p(x + w, y), p(x + w, y + h), p(x, y + h)]
}
fn spec(outline: Vec<Point>) -> ZoneSpec {
    ZoneSpec {
        surface: Default::default(),
        purpose: "living".into(),
        outline,
        holes: vec![],
        height: 3.,
        bottom_offset: 0.,
        label_position: None,
    }
}
fn zone(project: &mut Project, story: u32, s: ZoneSpec) -> u64 {
    project.add(story, Material::Plaster, Shape::Zone { spec: s })
}
fn near(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-7, "{a} != {b}");
}

#[test]
fn automatic_rectangle_label_is_centered_and_hole_centroid_is_not_used() {
    let s = spec(rect(7.66, 8.56, 3.08, 1.24));
    let q = s.label_point();
    near(q.x, 9.2);
    near(q.y, 9.18);
    let mut s = spec(rect(0., 0., 4., 4.));
    s.holes.push(rect(1., 1., 2., 2.));
    let q = s.label_point();
    assert!(!geometry::contains(q, &s.holes[0]));
}
#[test]
fn analysis_reports_gross_wall_overlap_and_refuses_unbounded_requests() {
    let mut project = Project::default();
    project.add(
        0,
        Material::Plaster,
        Shape::Wall {
            start: p(0., 2.),
            end: p(4., 2.),
            thickness: 0.2,
            height: 3.,
        },
    );
    zone(&mut project, 0, spec(rect(0., 0., 4., 4.)));
    let r = analyze(
        &project,
        &AnalysisOptions {
            story: Some(0),
            ..Default::default()
        },
    )
    .unwrap();
    near(r.spaces[0].wall_footprint_overlap_m2, 0.8);
    for _ in 0..256 {
        zone(&mut project, 0, spec(rect(10., 10., 1., 1.)));
    }
    assert!(analyze(&project, &Default::default()).is_err());
    let r = analyze(
        &project,
        &AnalysisOptions {
            story: Some(1),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(r.spaces.is_empty());
}
#[test]
fn space_holes_height_bounds_and_material_are_independent() {
    let mut project = Project::default();
    let mut s = spec(rect(0., 0., 4., 4.));
    s.holes.push(rect(1., 1., 2., 2.));
    s.height = 6.;
    s.bottom_offset = 0.4;
    let id = zone(&mut project, 0, s.clone());
    project.validate().unwrap();
    let e = &project.elements[0];
    let g = atelier_bim::zones::derive(e, 3.2).unwrap();
    near(s.area(), 12.);
    near(g.volume_m3, 72.);
    near(g.bounds[0].z, 3.6);
    near(g.bounds[1].z, 9.6);
    let editor = Editor::new(project);
    let source = atelier_bim::derivation_cache::GeometrySource::Zone(g);
    assert_eq!(source.material_volume_m3(), None);
    assert!(
        source
            .ray_hit(
                glam::DVec3::new(2., 2., 20.),
                glam::DVec3::NEG_Z,
                None,
                None
            )
            .unwrap()
            .is_none()
    );
    assert!(geometry::hit_plan(&editor.project, 0, p(2., 2.), 0.01).is_none());
    assert!(!geometry::element_intersects_polygon(
        &editor.project,
        e_in(&editor.project, id),
        &rect(1.2, 1.2, 0.2, 0.2)
    ));
    let q = s.label_point();
    assert!(geometry::contains(q, &s.outline) && !geometry::contains(q, &s.holes[0]));
}
fn e_in(project: &Project, id: u64) -> &atelier_bim::model::Element {
    project.elements.iter().find(|e| e.id == id).unwrap()
}
#[test]
fn concave_large_translated_and_reversed_boundaries_have_true_area() {
    let mut s = spec(vec![
        p(0., 0.),
        p(4., 0.),
        p(4., 1.),
        p(1., 1.),
        p(1., 4.),
        p(0., 4.),
    ]);
    near(s.area(), 7.);
    s.transform(|q| p(q.x + 99_000., q.y - 99_000.));
    s.outline.reverse();
    s.validate().unwrap();
    near(s.area(), 7.);
    let mut project = Project::default();
    zone(&mut project, 0, s);
    project.validate().unwrap();
}
#[test]
fn invalid_space_boundaries_and_parameters_are_refused_without_mutation() {
    let mut editor = Editor::new(Project::default());
    let before = editor.project.clone();
    let mut cases = vec![
        spec(vec![p(0., 0.), p(2., 2.), p(0., 2.), p(2., 0.)]),
        spec(rect(0., 0., 0., 2.)),
    ];
    let mut bad = spec(rect(0., 0., 4., 4.));
    bad.holes.push(rect(3., 3., 2., 2.));
    cases.push(bad);
    for height in [0., f64::NAN, 101.] {
        let mut s = spec(rect(0., 0., 2., 2.));
        s.height = height;
        cases.push(s);
    }
    let mut bad = spec(rect(0., 0., 2., 2.));
    bad.label_position = Some(p(f64::INFINITY, 0.));
    cases.push(bad);
    for s in cases {
        assert!(editor.place_zone(s).is_err());
        assert_eq!(editor.project, before);
        assert!(editor.undo.is_empty());
    }
}
#[test]
fn shared_wire_create_update_move_undo_and_json_roundtrip() {
    let mut editor = Editor::new(Project::default());
    let before = editor.project.clone();
    let g =
        json!({"type":"zone","space_name":"主卧","purpose":"bedroom","outline":rect(0.,0.,5.,4.)});
    let batch:NativeBatch=serde_json::from_value(json!({"version":1,"creates":[{"name":"bedroom","story":0,"layer":0,"material":"plaster","geometry":g}]})).unwrap();
    let prepared = editor
        .prepare_batch_change(editor.revision, &batch.lower(&editor).unwrap())
        .unwrap();
    assert_eq!(editor.project, before);
    let receipt = editor.commit_change("space-create", &prepared).unwrap();
    let id = receipt.changed_ids[0];
    let original = editor.project.clone();
    let g = json!({"type":"zone","space_name":"主卧","purpose":"bedroom","outline":rect(0.,0.,6.,4.),"height":3.2,"label_position":{"x":2.,"y":2.}});
    let batch:NativeBatch=serde_json::from_value(json!({"version":1,"edits":[{"operation":"update","target":{"kind":"existing","value":id},"geometry":g},{"operation":"move","targets":[{"kind":"existing","value":id}],"offset":{"x":10.,"y":1.}}]})).unwrap();
    let prepared = editor
        .prepare_batch_change(editor.revision, &batch.lower(&editor).unwrap())
        .unwrap();
    editor.commit_change("space-edit", &prepared).unwrap();
    let e = e_in(&editor.project, id);
    assert_eq!(e.name, "主卧");
    near(e.area(), 24.);
    let Shape::Zone { spec: s } = &e.shape else {
        panic!()
    };
    assert_eq!(s.label_position, Some(p(12., 3.)));
    let saved = JsonAdapter.export(&editor.project).unwrap();
    assert_eq!(JsonAdapter.import(&saved).unwrap(), editor.project);
    editor.undo();
    assert_eq!(editor.project, original);
    editor.redo();
    assert_eq!(JsonAdapter.import(&saved).unwrap(), editor.project);
    editor.undo();
    editor.undo();
    assert_eq!(editor.project, before);
    let d = describe("zone").unwrap();
    assert!(d["zone_semantics"].is_object());
    assert!(
        !d["parameters_schema"]["allOf"][0]["properties"]
            .as_object()
            .unwrap()
            .contains_key("type")
    );
}
#[test]
fn analysis_measures_union_overlap_unassigned_and_outside_on_one_story() {
    let mut project = Project::default();
    zone(&mut project, 0, spec(rect(0., 0., 4., 4.)));
    zone(&mut project, 0, spec(rect(2., 0., 4., 4.)));
    zone(&mut project, 1, spec(rect(0., 0., 4., 4.)));
    let r = analyze(
        &project,
        &AnalysisOptions {
            story: Some(0),
            boundary: Some(rect(0., 0., 8., 4.)),
            holes: vec![],
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(r.spaces.len(), 2);
    near(r.by_story[&0].area_m2, 32.);
    near(r.assigned_union_m2.unwrap(), 24.);
    near(r.unassigned_m2.unwrap(), 8.);
    near(r.relations[0].overlap_m2, 8.);
    let r = analyze(
        &project,
        &AnalysisOptions {
            story: Some(0),
            boundary: Some(rect(0., 0., 4., 4.)),
            holes: vec![],
            ..Default::default()
        },
    )
    .unwrap();
    near(r.spaces[1].outside_boundary_m2.unwrap(), 8.);
    assert!(
        analyze(
            &project,
            &AnalysisOptions {
                story: None,
                boundary: Some(rect(0., 0., 4., 4.)),
                holes: vec![],
                ..Default::default()
            }
        )
        .is_err()
    );
}
#[test]
fn near_relation_does_not_claim_door_connectivity_or_bridge_stories() {
    let mut project = Project::default();
    zone(&mut project, 0, spec(rect(0., 0., 2., 2.)));
    zone(&mut project, 0, spec(rect(2.12, 0., 2., 2.)));
    zone(&mut project, 1, spec(rect(0., 0., 2., 2.)));
    let r = analyze(&project, &Default::default()).unwrap();
    assert_eq!(r.relations.len(), 1);
    assert_eq!(r.relations[0].relation, "near_boundary");
    near(r.relations[0].boundary_distance_m, 0.12);
    near(r.relations[0].overlap_m2, 0.);
    assert!(r.limitations.iter().any(|l| l.contains("door")));
}
#[test]
fn analysis_target_revision_read_projection_and_failed_request_do_not_mutate() {
    let mut project = Project::default();
    zone(&mut project, 0, spec(rect(0., 0., 3., 4.)));
    let editor = Editor::new(project);
    let service = ReadService::new("space-test".into());
    let mut request = SpaceRequest {
        target: service.target(&editor),
        expected_revision: editor.revision,
        options: Default::default(),
    };
    assert_eq!(
        service.analyze_spaces(&editor, &request).unwrap()["analysis"]["spaces"][0]["area_m2"],
        12.
    );
    request.expected_revision += 1;
    assert_eq!(
        service.analyze_spaces(&editor, &request).unwrap_err().code,
        "REVISION_CONFLICT"
    );
    request.expected_revision = editor.revision;
    request.target.project_id = "different".into();
    assert_eq!(
        service.analyze_spaces(&editor, &request).unwrap_err().code,
        "TARGET_MISMATCH"
    );
    let q = Query {
        design_scope: Default::default(),
        target: service.target(&editor),
        expected_revision: editor.revision,
        ids: vec![],
        kind: Some("Zone".into()),
        story: None,
        region: None,
        fields: vec!["kind".into(), "shape".into(), "quantities".into()],
        limit: 100,
        cursor: None,
    };
    let r = service.query(&editor, &q).unwrap();
    assert_eq!(r["elements"][0]["kind"], "Zone");
    assert_eq!(
        r["elements"][0]["quantities"]["material_volume_m3"],
        serde_json::Value::Null
    );
    assert!(editor.undo.is_empty());
}
#[test]
fn space_creation_rectangle_polygon_cancel_and_default_villa_are_independent() {
    let mut editor = Editor::new(Project::default());
    editor.set_tool(Tool::Zone);
    editor.click(p(0., 0.), 0.01);
    assert_eq!(editor.project.elements.len(), 0);
    editor.click(p(4., 3.), 0.01);
    assert_eq!(editor.project.elements.len(), 1);
    near(editor.project.elements[0].area(), 12.);
    editor.zone_polygon = true;
    for p in [p(10., 0.), p(12., 0.), p(11., 2.)] {
        editor.click(p, 0.01);
    }
    editor.finish_zone().unwrap();
    assert_eq!(editor.project.elements.len(), 2);
    near(editor.project.elements[1].area(), 2.);
    editor.click(p(20., 0.), 0.01);
    editor.set_tool(Tool::Select);
    assert!(editor.polyline_points.is_empty());
    assert_eq!(editor.project.elements.len(), 2);
    assert_eq!(atelier_bim::storage::startup_example().elements.len(), 130);
}

#[test]
fn design_option_space_scope_is_shared_with_visibility_and_never_mixes_candidates() {
    use atelier_bim::view_settings::DesignOptionScope;
    let mut project = Project::default();
    let a = project.design_options.create("A".into());
    let b = project.design_options.create("B".into());
    zone(&mut project, 0, spec(rect(0., 0., 2., 2.)));
    let ai = zone(&mut project, 0, spec(rect(4., 0., 3., 3.)));
    let bi = zone(&mut project, 0, spec(rect(4., 0., 4., 4.)));
    project
        .elements
        .iter_mut()
        .find(|e| e.id == ai)
        .unwrap()
        .design_option = Some(a);
    project
        .elements
        .iter_mut()
        .find(|e| e.id == bi)
        .unwrap()
        .design_option = Some(b);
    let default: AnalysisOptions = serde_json::from_value(json!({})).unwrap();
    assert_eq!(default.design_scope, DesignOptionScope::All {});
    near(
        analyze(&project, &default).unwrap().by_story[&0].area_m2,
        29.,
    );
    for (active, total) in [(None, 4.), (Some(a), 13.), (Some(b), 20.)] {
        let scope = DesignOptionScope::active(active);
        project.plan_options.active_design_option = active;
        let report = analyze(
            &project,
            &AnalysisOptions {
                design_scope: scope,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(report.design_scope, scope);
        near(report.by_story[&0].area_m2, total);
        let expected = project
            .elements
            .iter()
            .filter(|e| project.plan_options.element_visible(e, &project.layers))
            .map(|e| e.id)
            .collect::<Vec<_>>();
        assert_eq!(
            report.spaces.iter().map(|e| e.id).collect::<Vec<_>>(),
            expected
        );
        assert!(report.relations.iter().all(|r| r.overlap_m2 <= 1e-6));
    }
    let editor = Editor::new(project);
    let original = editor.project.clone();
    let service = ReadService::new("scope-test".into());
    let mut request = SpaceRequest {
        target: service.target(&editor),
        expected_revision: editor.revision,
        options: AnalysisOptions {
            design_scope: DesignOptionScope::Option { id: a },
            ..Default::default()
        },
    };
    let report = service.analyze_spaces(&editor, &request).unwrap();
    assert_eq!(report["analysis"]["by_story"]["0"]["area_m2"], 13.);
    assert_eq!(
        report["analysis"]["design_scope"],
        json!({"type":"option","id":a})
    );
    request.options.design_scope = DesignOptionScope::Option { id: 999 };
    assert_eq!(
        service.analyze_spaces(&editor, &request).unwrap_err().code,
        "INVALID_DESIGN_OPTION_SCOPE"
    );
    assert_eq!(editor.project, original);
    assert!(editor.undo.is_empty() && editor.redo.is_empty());
    assert!(
        serde_json::from_value::<AnalysisOptions>(json!({"design_scope":{"type":"main","id":a}}))
            .is_err()
    );
}
#[test]
fn scoped_space_analysis_rejects_unisolated_alternative_wall_join_context() {
    use atelier_bim::view_settings::DesignOptionScope;
    let mut project = Project::default();
    let a = project.design_options.create("A".into());
    zone(&mut project, 0, spec(rect(0., 0., 2., 2.)));
    let id = project.add(
        0,
        Material::Plaster,
        Shape::Wall {
            start: p(0., 0.),
            end: p(2., 0.),
            thickness: 0.2,
            height: 3.,
        },
    );
    assert!(
        analyze(
            &project,
            &AnalysisOptions {
                design_scope: DesignOptionScope::Main {},
                ..Default::default()
            }
        )
        .is_ok()
    );
    project
        .elements
        .iter_mut()
        .find(|e| e.id == id)
        .unwrap()
        .design_option = Some(a);
    let editor = Editor::new(project);
    let service = ReadService::new("scope-walls".into());
    let request = SpaceRequest {
        target: service.target(&editor),
        expected_revision: editor.revision,
        options: AnalysisOptions {
            design_scope: DesignOptionScope::Option { id: a },
            ..Default::default()
        },
    };
    assert_eq!(
        service.analyze_spaces(&editor, &request).unwrap_err().code,
        "UNSUPPORTED_OPTION_WALL_CONTEXT"
    );
    assert!(
        analyze(&editor.project, &Default::default()).is_ok(),
        "legacy explicit all retains its declared mixed scope"
    );
}
