use atelier_bim::{
    editor::{Editor, Tool},
    model::{Point, Project, Shape},
    slab_creation::SlabPolygonDraft,
};
fn p(x: f64, y: f64) -> Point {
    Point::new(x, y)
}
fn session() -> Editor {
    let mut e = Editor::new(Project::default());
    e.set_tool(Tool::Slab);
    e.begin_slab_polygon().unwrap();
    e
}
#[test]
fn explicit_regions_preserve_all_crossing_results_and_commit_only_the_chosen_one() {
    for chosen in 0..2 {
        let mut e = session();
        e.slab_elevation = 1.25;
        let before = e.project.clone();
        for point in [p(0., 0.), p(4., 4.), p(0., 4.), p(4., 0.)] {
            e.click(point, 0.01);
        }
        let frozen = e.slab_polygon_draft().unwrap().clone();
        e.begin_slab_polygon().unwrap();
        assert_eq!(e.slab_polygon_draft(), Some(&frozen));
        let regions = e.slab_polygon_candidates(None).unwrap();
        assert_eq!(regions.len(), 2);
        assert_eq!(e.project, before);
        assert!(e.undo.is_empty());
        let mut stale = regions[chosen].clone();
        stale.elevation = 2.;
        assert!(e.commit_slab_polygon_region(&stale).is_err());
        assert_eq!(e.slab_polygon_draft(), Some(&frozen));
        assert_eq!(e.project, before);
        let id = e.commit_slab_polygon_region(&regions[chosen]).unwrap();
        assert_eq!(e.selected, Some(id));
        assert!(e.anchor.is_none() && e.slab_polygon_draft().is_none());
        assert_eq!(e.project.elements.len(), 1);
        assert_eq!(e.undo.len(), 1);
        assert_eq!(
            e.project.elements[0].slab_contour.as_ref(),
            Some(&regions[chosen])
        );
        assert!(matches!(e.project.elements[0].shape, Shape::Slab { .. }));
        let after = e.project.clone();
        let loaded: Project =
            serde_json::from_str(&serde_json::to_string(&after).unwrap()).unwrap();
        loaded.validate().unwrap();
        assert_eq!(loaded, after);
        e.undo();
        assert_eq!(e.project, before);
        e.redo();
        assert_eq!(e.project, after);
    }
}
#[test]
fn polygon_numeric_preview_and_locked_commit_recovery_keep_same_draft() {
    let mut e = session();
    let layer = e.project.layers.new_layer("Slab test", None);
    e.slab_layer = atelier_bim::layers::ElementLayer(layer);
    e.accept_slab_polygon_vertex(p(0., 0.)).unwrap();
    for (len, angle) in [(6., 0.), (2., 90.), (4., 180.), (4., 90.)] {
        e.accept_slab_polygon_numeric_vertex(len, angle).unwrap();
    }
    let frozen = e.slab_polygon_draft().unwrap().clone();
    let q = frozen.numeric_target(2., 180.).unwrap();
    let preview = e.slab_polygon_candidates(Some(q)).unwrap();
    assert_eq!(e.slab_polygon_draft(), Some(&frozen));
    e.accept_slab_polygon_numeric_vertex(2., 180.).unwrap();
    assert_eq!(e.slab_polygon_candidates(None).unwrap(), preview);
    let region = preview[0].clone();
    let frozen = e.slab_polygon_draft().unwrap().clone();
    e.project
        .layers
        .layers
        .iter_mut()
        .find(|l| l.id == layer)
        .unwrap()
        .status
        .locked = true;
    let before = e.project.clone();
    assert!(e.commit_slab_polygon_region(&region).is_err());
    assert_eq!(e.project, before);
    assert_eq!(e.slab_polygon_draft(), Some(&frozen));
    assert!(e.undo.is_empty());
    e.project
        .layers
        .layers
        .iter_mut()
        .find(|l| l.id == layer)
        .unwrap()
        .status
        .locked = false;
    e.commit_slab_polygon_region(&region).unwrap();
    assert_eq!(e.undo.len(), 1);
}
#[test]
fn numeric_node_rejection_preserves_draft_anchor_and_real_redo() {
    let mut e = session();
    for point in [p(0., 0.), p(4., 0.), p(0., 4.)] {
        e.accept_slab_polygon_vertex(point).unwrap();
    }
    let region = e.slab_polygon_candidates(None).unwrap().remove(0);
    e.commit_slab_polygon_region(&region).unwrap();
    let committed = e.project.clone();
    e.undo();
    let baseline = e.project.clone();
    assert!(e.accept_slab_polygon_numeric_vertex(2., 0.).is_err());
    e.begin_slab_polygon().unwrap();
    assert!(e.accept_slab_polygon_numeric_vertex(2., 0.).is_err());
    assert!(e.slab_polygon_draft().unwrap().points().is_empty());
    assert!(e.anchor.is_none());
    e.accept_slab_polygon_vertex(p(2., 3.)).unwrap();
    let frozen = e.slab_polygon_draft().unwrap().clone();
    for (length, angle) in [
        (0., 0.),
        (-1., 0.),
        (f64::NAN, 0.),
        (1., f64::INFINITY),
        (100000., 0.),
    ] {
        assert!(e.accept_slab_polygon_numeric_vertex(length, angle).is_err());
        assert_eq!(e.slab_polygon_draft(), Some(&frozen));
        assert_eq!(e.anchor, Some(p(2., 3.)));
        assert_eq!(e.project, baseline);
        assert!(e.undo.is_empty());
    }
    e.accept_slab_polygon_numeric_vertex(2., 0.).unwrap();
    assert_eq!(e.anchor, Some(p(4., 3.)));
    assert_eq!(
        e.slab_polygon_draft().unwrap().points(),
        &[p(2., 3.), p(4., 3.)]
    );
    e.redo();
    assert_eq!(e.project, committed);
    assert!(e.slab_polygon_draft().is_none() && e.anchor.is_none());
}

#[test]
fn polygon_sessions_clear_on_every_existing_exit_and_reject_bad_input() {
    for action in 0..6 {
        let mut e = session();
        e.accept_slab_polygon_vertex(p(1., 2.)).unwrap();
        let before = e.project.clone();
        match action {
            0 => e.cancel_wall_draft(),
            1 => e.set_tool(Tool::Wall),
            2 => e.switch_story(1),
            3 => e.switch_worksheet(None),
            4 => e.undo(),
            _ => e.redo(),
        }
        assert!(e.slab_polygon_draft().is_none() && e.anchor.is_none());
        assert_eq!(e.project, before);
    }
    let mut draft = SlabPolygonDraft::default();
    assert!(draft.numeric_target(1., 0.).is_err());
    draft.accept_vertex(p(2., 3.)).unwrap();
    let frozen = draft.clone();
    for q in [p(f64::NAN, 0.), p(100000., 0.)] {
        assert!(draft.accept_vertex(q).is_err());
        assert_eq!(draft, frozen);
    }
    assert!(draft.numeric_target(0., 0.).is_err());
    assert!(draft.numeric_target(1., f64::INFINITY).is_err());
}
