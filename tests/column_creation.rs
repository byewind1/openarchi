use atelier_bim::{
    column::ColumnSection,
    column_inclination::ColumnInclination,
    column_plan::{ColumnPlanDisplay, ColumnProjection},
    editor::{Editor, Tool},
    geometry,
    model::{Point, Project, Shape},
};
#[test]
fn single_inclined_creation_preview_commit_history_and_defaults_agree() {
    for section in [
        ColumnSection::Rectangular {
            width: 0.3,
            depth: 0.6,
        },
        ColumnSection::Circular { diameter: 0.6 },
    ] {
        let mut e = Editor::new(Project::default());
        e.set_tool(Tool::Column);
        e.column_section = section;
        e.column_slanted = true;
        e.column_inclination = ColumnInclination {
            slant_degrees: 60.,
            direction_degrees: -90.,
        };
        e.column_axis_rotation = 30.;
        let before = e.project.clone();
        let preview = e.column_creation_preview(Point::new(2., 3.)).unwrap();
        assert_eq!(e.project, before);
        assert!(e.undo.is_empty());
        let outline = geometry::element_plan_outline(&e.project, &preview);
        assert!(!outline.is_empty());
        e.click(Point::new(2., 3.), 0.01);
        assert_eq!(e.project.elements.len(), 1);
        assert_eq!(e.project.elements[0], preview);
        assert_eq!(e.undo.len(), 1);
        let placed = e.project.clone();
        e.undo();
        assert_eq!(e.project, before);
        e.redo();
        assert_eq!(e.project, placed);
        let loaded: Project =
            serde_json::from_str(&serde_json::to_string(&placed).unwrap()).unwrap();
        loaded.validate().unwrap();
        assert_eq!(loaded, placed);
    }
}
#[test]
fn invalid_or_unresolved_preview_and_click_preserve_history_and_defaults() {
    let mut e = Editor::new(Project::default());
    e.set_tool(Tool::Column);
    e.column_slanted = true;
    e.column_inclination.slant_degrees = 0.;
    let before = e.project.clone();
    assert!(e.column_creation_preview(Point::default()).is_err());
    e.click(Point::default(), 0.01);
    assert_eq!(e.project, before);
    assert!(e.undo.is_empty());
    e.column_inclination.slant_degrees = 60.;
    e.story = 2;
    assert!(e.column_creation_preview(Point::default()).is_err());
    e.click(Point::default(), 0.01);
    assert_eq!(e.project, before);
    e.column_plan.projection = ColumnProjection::EntireElement;
    assert!(e.column_creation_preview(Point::default()).is_ok());
}
#[test]
fn toggling_creation_defaults_preserves_direction_and_projection() {
    let mut e = Editor::new(Project::default());
    e.column_slanted = true;
    e.column_inclination = ColumnInclination {
        slant_degrees: 60.,
        direction_degrees: -90.,
    };
    e.column_plan.display = ColumnPlanDisplay::CutOnly;
    e.column_slanted = false;
    let vertical = e.column_creation_preview(Point::default()).unwrap();
    assert!(matches!(
        vertical.shape,
        Shape::Column {
            inclination: None,
            ..
        }
    ));
    e.column_slanted = true;
    let slanted = e.column_creation_preview(Point::default()).unwrap();
    assert!(matches!(
        slanted.shape,
        Shape::Column {
            inclination: Some(ColumnInclination {
                slant_degrees: 60.,
                direction_degrees: -90.
            }),
            ..
        }
    ));
    assert_eq!(e.column_plan.display, ColumnPlanDisplay::CutOnly);
    assert!(e.project.elements.is_empty());
}

#[test]
fn two_point_candidate_commit_preserves_defaults_and_roundtrips_history() {
    for section in [
        ColumnSection::Rectangular {
            width: 0.3,
            depth: 0.6,
        },
        ColumnSection::Circular { diameter: 0.6 },
    ] {
        let mut e = Editor::new(Project::default());
        e.column_section = section;
        e.column_height = 2.8;
        e.column_bottom_offset = -0.4;
        e.column_axis_rotation = 30.;
        e.column_plan.projection = ColumnProjection::EntireElement;
        let defaults = (e.column_slanted, e.column_inclination, e.column_plan);
        let before = e.project.clone();
        let base = Point::new(2., 3.);
        let top = Point::new(5., 7.);
        let preview = e.column_axis_preview(base, top).unwrap();
        assert_eq!(e.project, before);
        assert!(e.undo.is_empty());
        let id = e.place_column_axis(base, top).unwrap();
        assert_eq!(id, preview.id);
        assert_eq!(e.project.elements[0], preview);
        assert_eq!(e.undo.len(), 1);
        assert_eq!(
            (e.column_slanted, e.column_inclination, e.column_plan),
            defaults
        );
        let created = e.project.clone();
        let loaded: Project =
            serde_json::from_str(&serde_json::to_string(&created).unwrap()).unwrap();
        loaded.validate().unwrap();
        assert_eq!(loaded, created);
        e.undo();
        assert_eq!(e.project, before);
        e.redo();
        assert_eq!(e.project, created);
    }
}

#[test]
fn rejected_two_point_creation_preserves_real_redo() {
    let mut e = Editor::new(Project::default());
    let base = Point::new(2., 3.);
    let top = Point::new(5., 7.);
    e.place_column_axis(base, top).unwrap();
    let created = e.project.clone();
    e.undo();
    let before = e.project.clone();
    for (a, b) in [
        (base, base),
        (Point::new(f64::NAN, 0.), top),
        (base, Point::new(200000., 3.)),
    ] {
        assert!(e.column_axis_preview(a, b).is_err());
        assert!(e.place_column_axis(a, b).is_err());
        assert_eq!(e.project, before);
        assert!(e.undo.is_empty());
    }
    e.story = 2;
    assert!(e.place_column_axis(base, top).is_err());
    assert_eq!(e.project, before);
    e.redo();
    assert_eq!(e.project, created);
}

#[test]
fn two_click_draft_cancel_switch_and_empty_history_cleanup() {
    let mut e = Editor::new(Project::default());
    e.column_two_click = true;
    e.set_tool(Tool::Column);
    let before = e.project.clone();
    let base = Point::new(2., 3.);
    for action in 0..5 {
        e.set_tool(Tool::Column);
        e.click(base, 0.01);
        assert_eq!(e.anchor, Some(base));
        assert_eq!(e.project, before);
        match action {
            0 => e.cancel_wall_draft(),
            1 => e.undo(),
            2 => e.redo(),
            3 => e.set_tool(Tool::Select),
            _ => e.switch_story(1),
        }
        assert!(e.anchor.is_none());
        assert_eq!(e.project, before);
    }
    e.switch_story(0);
    e.set_tool(Tool::Column);
    e.click(base, 0.01);
    e.click(base, 0.01);
    assert_eq!(e.anchor, Some(base));
    assert_eq!(e.project, before);
    let top = Point::new(5., 7.);
    let expected = e.column_axis_preview(base, top).unwrap();
    e.click(top, 0.01);
    assert_eq!(e.project.elements, vec![expected]);
    assert!(e.anchor.is_none());
    assert_eq!(e.undo.len(), 1);
}
