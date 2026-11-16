use atelier_bim::{
    beam_creation::BeamArcDraft,
    editor::Editor,
    geometry,
    model::{Point, Project},
};
fn polar(center: Point, radius: f64, degrees: f64) -> Point {
    Point::new(
        center.x + radius * degrees.to_radians().cos() as f64,
        center.y + radius * degrees.to_radians().sin() as f64,
    )
}
fn draft(center: Point, radius: f64, bearing: f64) -> BeamArcDraft {
    let mut d = BeamArcDraft::new(center).unwrap();
    d.accept_radius(polar(center, radius, bearing)).unwrap();
    d
}
#[test]
fn first_motion_uses_actual_radius_bearing_and_preserves_large_arc() {
    for bearing in [0., 90., 37.0600827307] {
        for (first, expected) in [
            (-5., -90.),
            (-60., -90.),
            (-89., -90.),
            (-91., 270.),
            (-120., 270.),
        ] {
            let center = Point::new(6., 16.);
            let mut d = draft(center, 5., bearing);
            assert!(d.update_cursor(d.start().unwrap()).unwrap().is_none());
            d.update_cursor(polar(center, 5., bearing + first)).unwrap();
            let c = d
                .update_cursor(polar(center, 8., bearing - 90.))
                .unwrap()
                .unwrap();
            assert!((d.sweep_degrees().unwrap() - expected).abs() < 0.0001);
            assert!((center.distance(c.end) - 5.).abs() < 1e-5);
            assert_eq!(c.arc.large_arc, expected > 180.);
            assert_eq!(c.arc.clockwise, expected < 0.);
        }
    }
    let mut d = draft(Point::default(), 5., 0.);
    d.update_cursor(d.start().unwrap()).unwrap();
    d.update_cursor(Point::new(0., -5.)).unwrap();
    assert!((d.sweep_degrees().unwrap() - 270.).abs() < 1e-8);
}
#[test]
fn continuous_path_can_reverse_through_start_and_preview_projects_to_radius() {
    let center = Point::default();
    let mut d = draft(center, 5., 0.);
    for angle in [5., 30., 90., 30., 5., 0., -5., -30., -90.] {
        d.update_cursor(polar(center, 12., angle)).unwrap();
    }
    let c = d.candidate().unwrap();
    assert!(c.arc.clockwise);
    assert!(!c.arc.large_arc);
    assert!(c.end.distance(Point::new(0., -5.)) < 1e-5);
    let mut major = draft(center, 5., 0.);
    for angle in [5., 90., 180., 270.] {
        major.update_cursor(polar(center, 5., angle)).unwrap();
    }
    let c = major.candidate().unwrap();
    assert!(c.arc.large_arc);
    assert!(!c.arc.clockwise);
    major.update_cursor(Point::new(5., 0.)).unwrap();
    assert!(major.candidate().is_none());
    major.update_cursor(polar(center, 5., 270.)).unwrap();
    assert!(major.candidate().is_some());
}
#[test]
fn numeric_radius_and_sweep_feed_existing_typed_creation_once() {
    let center = Point::new(6.2067574, 16.271672365);
    let mut d = BeamArcDraft::new(center).unwrap();
    d.accept_numeric_radius(Point::new(25.379727801, 16.271672365), 5.)
        .unwrap();
    let c = d.set_sweep_degrees(60.).unwrap();
    assert!(c.start.distance(Point::new(11.2067574, 16.271672365)) < 1e-5);
    assert!(c.end.distance(Point::new(8.7067574, 20.601799384)) < 1e-5);
    let mut e = Editor::new(Project::default());
    e.beam_size = [0.3, 0.4, 3.];
    let before = e.project.clone();
    let id = e.place_curved_beam(c.start, c.end, c.arc).unwrap();
    let after = e.project.clone();
    assert_eq!(after.elements.len(), 1);
    assert_eq!(e.undo.len(), 1);
    assert!(
        (geometry::arc_length(c.start, c.end, c.arc) - 5. * std::f64::consts::PI / 3.).abs() < 1e-5
    );
    assert_eq!(after.elements[0].id, id);
    e.undo();
    assert_eq!(e.project, before);
    e.redo();
    assert_eq!(e.project, after);
}
#[test]
fn invalid_input_does_not_destroy_accepted_draft() {
    let mut d = BeamArcDraft::new(Point::default()).unwrap();
    let before = d;
    for p in [
        Point::default(),
        Point::new(f64::NAN, 0.),
        Point::new(100000., 0.),
    ] {
        assert!(d.accept_radius(p).is_err());
        assert_eq!(d, before);
    }
    assert!(d.accept_numeric_radius(Point::new(1., 0.), -5.).is_err());
    assert_eq!(d, before);
    d.accept_radius(Point::new(5., 0.)).unwrap();
    d.set_sweep_degrees(60.).unwrap();
    let accepted = d;
    for angle in [0., 360., f64::NAN, f64::INFINITY] {
        assert!(d.set_sweep_degrees(angle).is_err());
        assert_eq!(d, accepted);
    }
    assert!(d.update_cursor(Point::default()).is_err());
    assert_eq!(d, accepted);
    assert!(d.accept_radius(Point::new(10., 0.)).is_err());
    assert_eq!(d, accepted);
    let fresh = BeamArcDraft::new(Point::new(20., 20.)).unwrap();
    assert!(fresh.start().is_none());
    assert!(fresh.candidate().is_none());
}

#[test]
fn editor_arc_session_commits_once_and_retries_invalid_parameters() {
    use atelier_bim::editor::Tool;
    let mut e = Editor::new(Project::default());
    e.set_tool(Tool::Beam);
    e.beam_slant_angle = 12.;
    let before = e.project.clone();
    assert_eq!(e.click_beam_arc(Point::default()).unwrap(), None);
    assert!(e.click_beam_arc(Point::default()).is_err());
    assert_eq!(e.click_beam_arc(Point::new(5., 0.)).unwrap(), None);
    assert_eq!(e.project, before);
    assert!(e.undo.is_empty());
    e.beam_size[0] = 20.; // Inner radius would be negative.
    assert!(e.click_beam_arc(Point::new(0., 8.)).is_err());
    assert_eq!(e.project, before);
    assert!(e.undo.is_empty());
    assert!(e.beam_arc_draft.unwrap().candidate().is_some());
    e.beam_size[0] = 0.3;
    let id = e.commit_beam_arc().unwrap();
    assert_eq!(e.selected, Some(id));
    assert!(e.beam_arc_draft.is_none());
    assert!(e.anchor.is_none());
    assert_eq!(e.beam_slant_angle, 12.);
    assert_eq!(e.undo.len(), 1);
    let after = e.project.clone();
    let atelier_bim::model::Shape::Beam {
        end,
        slant_angle,
        arc,
        ..
    } = after.elements[0].shape
    else {
        panic!()
    };
    assert!(end.distance(Point::new(0., 5.)) < 1e-5);
    assert_eq!(slant_angle, 0.);
    assert!(arc.is_some());
    assert!(e.commit_beam_arc().is_err());
    assert_eq!(e.undo.len(), 1);
    e.undo();
    assert_eq!(e.project, before);
    e.redo();
    assert_eq!(e.project, after);
}

#[test]
fn editor_arc_lifecycle_clears_center_and_radius_without_history() {
    use atelier_bim::editor::Tool;
    for stage in [1, 2] {
        for action in 0..6 {
            let mut e = Editor::new(Project::default());
            e.set_tool(Tool::Beam);
            e.click_beam_arc(Point::new(10., 10.)).unwrap();
            if stage == 2 {
                e.click_beam_arc(Point::new(15., 10.)).unwrap();
            }
            let before = e.project.clone();
            match action {
                0 => e.cancel_wall_draft(),
                1 => e.set_tool(Tool::Wall),
                2 => e.switch_story(e.story),
                3 => e.switch_worksheet(None),
                4 => e.undo(),
                _ => e.redo(),
            }
            assert!(e.beam_arc_draft.is_none(), "stage {stage}, action {action}");
            assert!(e.anchor.is_none());
            assert_eq!(e.project, before);
            assert!(e.undo.is_empty());
            e.set_tool(Tool::Beam);
            e.click_beam_arc(Point::new(20., 20.)).unwrap();
            assert_eq!(e.beam_arc_draft.unwrap().center(), Point::new(20., 20.));
            assert!(e.beam_arc_draft.unwrap().start().is_none());
        }
    }
    let mut e = Editor::new(Project::default());
    assert!(e.click_beam_arc(Point::default()).is_err());
    assert!(e.beam_arc_draft.is_none());
}

#[test]
fn shared_beam_preview_preserves_defaults_history_and_matches_commit() {
    use atelier_bim::{
        layers::ElementLayer,
        model::{ArcSegment, Material},
        view_settings::RenovationStatus,
    };
    for (slant, arc) in [
        (0., None),
        (25., None),
        (
            0.,
            Some(ArcSegment {
                radius: 5.,
                clockwise: false,
                large_arc: true,
            }),
        ),
    ] {
        let mut e = Editor::new(Project::default());
        e.place_column(Point::new(1., 1.)).unwrap();
        let redo_project = e.project.clone();
        e.undo();
        e.story = 1;
        e.material = Material::Plaster;
        e.beam_material = Material::Timber;
        e.beam_layer = ElementLayer(
            e.project
                .layers
                .layers
                .iter()
                .find(|l| l.id != 0 && !l.status.locked)
                .unwrap()
                .id,
        );
        e.beam_size = [0.4, 0.6, 2.5];
        e.wall_renovation = RenovationStatus::Existing;
        let a = Point::new(0., 0.);
        let b = Point::new(6., 0.);
        let before = e.project.clone();
        let revision = e.revision;
        let selected = e.selection_ids();
        let preview = e.beam_path_preview(a, b, slant, arc).unwrap();
        assert_eq!(preview.material, Material::Timber);
        assert_eq!(preview.layer, e.beam_layer);
        assert_eq!(preview.story, 1);
        assert_eq!(preview.renovation, RenovationStatus::Existing);
        assert_eq!(e.project, before);
        assert_eq!(e.revision, revision);
        assert_eq!(e.selection_ids(), selected);
        assert!(e.beam_path_preview(a, a, slant, arc).is_none());
        e.redo();
        assert_eq!(e.project, redo_project);
        e.undo();
        assert_eq!(e.project, before);
        let expected = e
            .preview_add_elements(vec![preview.clone()], false)
            .unwrap();
        let id = if let Some(arc) = arc {
            e.place_curved_beam(a, b, arc).unwrap()
        } else {
            e.place_slanted_beam(a, b, slant).unwrap()
        };
        assert_eq!(
            e.project.elements.iter().find(|x| x.id == id).unwrap(),
            &preview
        );
        assert_eq!(e.project, expected);
        e.undo();
        assert_eq!(e.project, before);
        e.redo();
        assert_eq!(e.project, expected);
    }
}
