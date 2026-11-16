use atelier_bim::{
    column::ColumnSection,
    geometry,
    model::{Material, Point, Project, Shape},
};

#[test]
fn preview_paths_preserve_round_columns_and_open_polylines_without_mutation() {
    let mut project = Project::default();
    let circle = project.add(
        0,
        Material::Concrete,
        Shape::Column {
            center: Point::new(2., 3.),
            section: ColumnSection::Circular { diameter: 2. },
            height: 3.,
            bottom_offset: 0.,
            axis_rotation: 0.,
            inclination: None,
            plan: Default::default(),
        },
    );
    let points = vec![Point::new(0., 0.), Point::new(2., 0.), Point::new(2., 1.)];
    let line = project.add(
        0,
        Material::Concrete,
        Shape::Polyline {
            points: points.clone(),
            arcs: vec![],
        },
    );
    let before = project.clone();
    let paths = geometry::element_plan_paths(
        &project,
        project.elements.iter().find(|e| e.id == circle).unwrap(),
    );
    assert_eq!(paths.len(), 1);
    assert!(paths[0].len() > 32);
    assert_eq!(paths[0].first(), paths[0].last());
    for p in &paths[0] {
        assert!((p.distance(Point::new(2., 3.)) - 1.).abs() < 1e-5);
    }
    let paths = geometry::element_plan_paths(
        &project,
        project.elements.iter().find(|e| e.id == line).unwrap(),
    );
    assert_eq!(paths, vec![points]);
    assert_eq!(project, before);
}

#[test]
fn inclined_column_preview_paths_use_view_story_and_keep_cut_projection_rings_separate() {
    use atelier_bim::{column_inclination::ColumnInclination, column_plan, editor::Editor};
    for section in [
        ColumnSection::Circular { diameter: 0.6 },
        ColumnSection::Rectangular {
            width: 0.4,
            depth: 0.6,
        },
    ] {
        let mut editor = Editor::new(Project::default());
        editor.column_plan.display = column_plan::ColumnPlanDisplay::ProjectedWithOverhead;
        editor
            .place_column_with_inclination(
                Point::default(),
                section,
                6.,
                Some(ColumnInclination {
                    slant_degrees: 60.,
                    direction_degrees: 30.,
                }),
            )
            .unwrap();
        let before = editor.project.clone();
        let element = &before.elements[0];
        let upper = column_plan::for_element(&before, element, 1)
            .unwrap()
            .unwrap();
        let mut expected = vec![upper.cut, upper.uncut, upper.overhead];
        expected.retain(|path| !path.is_empty());
        assert_eq!(expected.len(), 3);
        for path in &mut expected {
            if path.first() != path.last() {
                path.push(path[0]);
            }
        }
        let actual = geometry::element_plan_paths_on_story(&before, element, 1);
        assert_eq!(actual, expected);
        assert_ne!(actual, geometry::element_plan_paths(&before, element));
        assert_eq!(
            geometry::element_plan_paths(&before, element),
            geometry::element_plan_paths_on_story(&before, element, element.story)
        );
        let mut invalid_range = before.clone();
        invalid_range.plan_options.floor_plan_cut.upper_story_offset = 50;
        assert!(
            geometry::element_plan_paths_on_story(&invalid_range, &invalid_range.elements[0], 1)
                .is_empty()
        );
        assert_eq!(editor.project, before);
    }
}
