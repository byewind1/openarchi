use atelier_bim::{
    editor::{Editor, Tool},
    model::{ArcSegment, Point, Project},
    storage,
    view_settings::RenovationStatus,
};
#[test]
fn failed_polyline_finish_preserves_nodes_and_redo_then_retry_commits_once() {
    let mut e = Editor::new(Project::default());
    e.place_column(Point::new(-2., -2.)).unwrap();
    let created = e.project.clone();
    e.undo();
    e.set_tool(Tool::Polyline);
    for i in 0..10_001 {
        e.click(Point::new(i as f64 * 0.02, 0.), 0.01);
    }
    let nodes = e.polyline_points.clone();
    let before = e.project.clone();
    let revision = e.revision;
    assert!(e.finish_polyline(None).is_err());
    assert_eq!(e.polyline_points, nodes);
    assert_eq!(e.project, before);
    assert_eq!(e.revision, revision);
    assert!(e.undo.is_empty());
    e.redo();
    assert_eq!(e.project, created);
    e.undo();
    // History does not consume the retained polyline input; correct it and retry.
    e.polyline_points.truncate(3);
    let id = e.finish_polyline(None).unwrap().unwrap();
    assert!(e.polyline_points.is_empty());
    assert_eq!(e.selected, Some(id));
    assert_eq!(e.undo.len(), 1);
    let placed = e.project.clone();
    e.undo();
    assert_eq!(e.project, before);
    e.redo();
    assert_eq!(e.project, placed);
}
#[test]
fn typed_polyline_preserves_legacy_defaults_and_exact_arcs_in_history_and_file() {
    let points = vec![Point::default(), Point::new(4., 0.), Point::new(4., 3.)];
    let mut direct = Editor::new(Project::default());
    direct.wall_renovation = RenovationStatus::Existing;
    direct.place_polyline(points.clone(), vec![]).unwrap();
    let mut pointer = Editor::new(Project::default());
    pointer.wall_renovation = RenovationStatus::Existing;
    pointer.set_tool(Tool::Polyline);
    for p in &points {
        pointer.click(*p, 0.01);
    }
    pointer.finish_polyline(None).unwrap();
    assert_eq!(pointer.project, direct.project);
    assert_eq!(direct.project.elements[0].renovation, RenovationStatus::New);
    let mut e = Editor::new(Project::default());
    let arc = ArcSegment {
        radius: 3.,
        clockwise: true,
        large_arc: true,
    };
    e.place_polyline(points.clone(), vec![Some(arc), None])
        .unwrap();
    let created = e.project.clone();
    e.undo();
    let before = e.project.clone();
    assert!(
        e.place_polyline(points, vec![Some(ArcSegment { radius: 0.1, ..arc }), None])
            .is_err()
    );
    assert_eq!(e.project, before);
    e.redo();
    assert_eq!(e.project, created);
    let path = std::env::temp_dir().join(format!("oa-polyline-typed-{}.json", std::process::id()));
    storage::save(&created, &path).unwrap();
    assert_eq!(storage::load(&path).unwrap(), created);
    std::fs::remove_file(path).unwrap();
}
