use atelier_bim::{
    editor::{Editor, Tool},
    model::{Material, Point, Project},
    stairs::StairSpec,
    storage,
    view_settings::RenovationStatus,
};

fn spec() -> StairSpec {
    StairSpec {
        start: Point::new(2., -1.),
        end: Point::new(5., 3.),
        width: 1.25,
        total_rise: 3.4,
        risers: 20,
        ..Default::default()
    }
}
#[test]
fn typed_and_pointer_stair_creation_match_full_legacy_element_and_history() {
    let mut api = Editor::new(Project::default());
    api.material = Material::Concrete;
    api.wall_renovation = RenovationStatus::New;
    let before = api.project.clone();
    let candidate = api.stair_creation_candidate(spec()).unwrap();
    assert_eq!(api.project, before);
    assert_eq!(api.revision, 0);
    assert!(api.undo.is_empty());
    let id = api.place_stair(spec()).unwrap();
    assert_eq!(api.project.elements[0], candidate);
    let mut legacy = before.clone();
    legacy.add_with_status(
        0,
        Material::Concrete,
        atelier_bim::model::Shape::Stair { spec: spec() },
        RenovationStatus::New,
    );
    assert_eq!(api.project, legacy);
    let mut pointer = Editor::new(before.clone());
    pointer.material = api.material;
    pointer.wall_renovation = api.wall_renovation;
    pointer.stair_spec = spec();
    pointer.set_tool(Tool::Stair);
    pointer.click(spec().start, 0.01);
    assert_eq!(pointer.project, before);
    pointer.click(spec().end, 0.01);
    assert_eq!(pointer.project, api.project);
    assert_eq!(pointer.selected, Some(id));
    assert!(pointer.anchor.is_none());
    assert_eq!(api.undo.len(), 1);
    assert_eq!(pointer.undo.len(), 1);
    let created = api.project.clone();
    api.undo();
    assert_eq!(api.project, before);
    api.redo();
    assert_eq!(api.project, created);
    let path = std::env::temp_dir().join(format!("oa-stair-typed-{}.json", std::process::id()));
    storage::save(&created, &path).unwrap();
    assert_eq!(storage::load(&path).unwrap(), created);
    std::fs::remove_file(path).unwrap();
}
#[test]
fn stair_candidate_and_failures_preserve_real_redo_identity_and_pointer_anchor() {
    let mut e = Editor::new(Project::default());
    e.place_stair(spec()).unwrap();
    let created = e.project.clone();
    e.undo();
    let before = e.project.clone();
    let revision = e.revision;
    let mut invalid = spec();
    invalid.width = f64::NAN;
    assert!(e.stair_creation_candidate(invalid.clone()).is_err());
    assert!(e.place_stair(invalid).is_err());
    assert_eq!(e.project, before);
    assert_eq!(e.revision, revision);
    assert!(e.undo.is_empty());
    e.project.next_id = u64::MAX;
    let exhausted = e.project.clone();
    assert!(e.place_stair(spec()).is_err());
    assert_eq!(e.project, exhausted);
    e.project.next_id = before.next_id;
    e.set_tool(Tool::Stair);
    e.click(spec().start, 0.01);
    e.stair_spec.width = 0.;
    e.click(spec().end, 0.01);
    assert_eq!(e.anchor, Some(spec().start));
    assert_eq!(e.project, before);
    assert!(e.undo.is_empty());
    e.redo();
    assert_eq!(e.project, created);
}

#[test]
fn public_stair_geometry_rejects_finite_endpoints_with_overflowing_run() {
    for (start, end) in [
        (Point::new(-f64::MAX, 0.), Point::new(f64::MAX, 0.)),
        (Point::new(0., -f64::MAX), Point::new(0., f64::MAX)),
        (Point::new(0., 0.), Point::new(f64::MAX, f64::MAX)),
    ] {
        assert!(start.finite() && end.finite());
        let bad = StairSpec {
            start,
            end,
            ..spec()
        };
        assert!(
            bad.validate().is_err(),
            "unrepresentable run accepted: {start:?} {end:?}"
        );
        assert!(atelier_bim::stairs::generate(&bad, 0.).is_err());
    }
    let good = spec();
    let geometry = atelier_bim::stairs::generate(&good, -3.2).unwrap();
    assert!(geometry.triangles.iter().flatten().all(|p| p.is_finite()));
    assert_eq!(geometry.tread_lines.len(), good.risers as usize);
}
