use atelier_bim::{
    column::ColumnSection,
    column_inclination::ColumnInclination,
    column_plan::{
        self, ColumnPlanDisplay as Display, ColumnPlanSettings, ColumnProjection as Projection,
    },
    editor::Editor,
    geometry,
    model::{Point, Project, Shape},
};
fn scene() -> (Editor, u64) {
    let mut e = Editor::new(Project::default());
    let id = e
        .place_column_with_inclination(
            Point::default(),
            ColumnSection::Circular { diameter: 0.6 },
            3.,
            Some(ColumnInclination {
                slant_degrees: 30.,
                direction_degrees: 0.,
            }),
        )
        .unwrap();
    (e, id)
}
fn display(e: &mut Editor, value: Display) {
    if let Shape::Column { plan, .. } = &mut e.project.elements[0].shape {
        plan.display = value;
    }
}
#[test]
fn real_cut_and_visible_projection_drive_hits_instead_of_bottom_circle() {
    let (mut e, id) = scene();
    display(&mut e, Display::CutOnly);
    let cy = 1.1 * 3_f64.sqrt();
    assert_eq!(
        geometry::hit_plan(&e.project, 0, Point::new(0., cy), 0.001),
        Some(id)
    );
    assert_eq!(
        geometry::hit_plan(&e.project, 0, Point::default(), 0.001),
        None
    );
    assert_eq!(
        geometry::hit_plan(&e.project, 0, Point::new(0.29, cy + 0.55), 0.001),
        None
    );
    display(&mut e, Display::Projected);
    assert_eq!(
        geometry::hit_plan(&e.project, 0, Point::new(0., 0.5), 0.001),
        Some(id)
    );
    assert_eq!(
        geometry::hit_plan(&e.project, 0, Point::new(0., 4.), 0.001),
        None
    );
    display(&mut e, Display::ProjectedWithOverhead);
    assert_eq!(
        geometry::hit_plan(&e.project, 0, Point::new(0., 4.), 0.001),
        Some(id)
    );
}
#[test]
fn cut_settings_roundtrip_and_do_not_modify_solid_or_element_orientation() {
    let (mut e, _) = scene();
    let element = e.project.elements[0].clone();
    let before = atelier_bim::column::inclined_solid(&e.project, &element)
        .unwrap()
        .mesh()
        .unwrap()
        .triangles;
    e.project.plan_options.floor_plan_cut.height = 2.;
    display(&mut e, Display::CutOnly);
    let plan = column_plan::for_element(&e.project, &e.project.elements[0], 0)
        .unwrap()
        .unwrap();
    assert!(plan.cut.iter().all(|p| p.y > 2.8));
    let json = serde_json::to_string(&e.project).unwrap();
    let loaded: Project = serde_json::from_str(&json).unwrap();
    loaded.validate().unwrap();
    assert_eq!(loaded, e.project);
    assert_eq!(
        atelier_bim::column::inclined_solid(&loaded, &loaded.elements[0])
            .unwrap()
            .mesh()
            .unwrap()
            .triangles,
        before
    );
    e.project.plan_options.floor_plan_cut.height = f64::NAN;
    assert!(e.project.validate().is_err());
}
#[test]
fn story_range_uses_elevations_and_missing_references_are_explicit() {
    let (mut e, _) = scene();
    e.project.stories[0].id = 90;
    e.project.stories[1].id = 12;
    e.project.stories[2].id = 3;
    let cut = e.project.plan_options.floor_plan_cut;
    let (z, range) = cut
        .resolve(&e.project, 90, ColumnPlanSettings::default())
        .unwrap();
    assert_eq!(z, 1.1);
    assert_eq!(
        range,
        column_plan::ColumnProjectionRange::FloorPlan {
            lower: 0.,
            upper: 3.2
        }
    );
    assert!(
        cut.resolve(&e.project, 3, ColumnPlanSettings::default())
            .is_err()
    );
    let entire = ColumnPlanSettings {
        display: Display::Projected,
        projection: Projection::EntireElement,
    };
    assert!(cut.resolve(&e.project, 3, entire).is_ok());
    let only = ColumnPlanSettings {
        display: Display::CutOnly,
        ..ColumnPlanSettings::default()
    };
    assert!(cut.resolve(&e.project, 3, only).is_ok());
}
#[test]
fn inclination_replacement_preserves_plan_settings_and_legacy_defaults_stay_omitted() {
    let (mut e, _) = scene();
    display(&mut e, Display::CutOnly);
    let original = e.project.clone();
    let mut changed = e.project.elements[0].clone();
    if let Shape::Column { inclination, .. } = &mut changed.shape {
        *inclination = None;
    }
    e.replace_element(changed).unwrap();
    e.undo();
    assert_eq!(e.project, original);
    e.redo();
    assert!(matches!(
        e.project.elements[0].shape,
        Shape::Column {
            plan: ColumnPlanSettings {
                display: Display::CutOnly,
                ..
            },
            inclination: None,
            ..
        }
    ));
    let mut old = Editor::new(Project::default());
    old.place_column(Point::default()).unwrap();
    let wire = serde_json::to_value(&old.project).unwrap();
    assert!(wire["plan_options"].get("floor_plan_cut").is_none());
    assert!(wire["elements"][0]["shape"].get("plan").is_none());
}

#[test]
fn inclined_column_cross_story_cut_hit_and_hover_share_view_elevation() {
    let (mut e, id) = scene();
    if let Shape::Column { height, plan, .. } = &mut e.project.elements[0].shape {
        *height = 6.;
        plan.display = Display::CutOnly;
    }
    let element = &e.project.elements[0];
    assert!(geometry::element_on_plan_story(&e.project, element, 0));
    assert!(geometry::element_on_plan_story(&e.project, element, 1));
    assert!(!geometry::element_on_plan_story(&e.project, element, 2));
    let center0 = Point::new(0., 1.1 * 3_f64.sqrt());
    let center1 = Point::new(0., 4.3 * 3_f64.sqrt());
    assert_eq!(geometry::hit_plan(&e.project, 0, center0, 0.001), Some(id));
    assert_eq!(geometry::hit_plan(&e.project, 1, center1, 0.001), Some(id));
    assert_eq!(geometry::hit_plan(&e.project, 1, center0, 0.001), None);
    assert_eq!(geometry::hit_plan(&e.project, 2, center1, 0.001), None);
    assert_eq!(
        geometry::plan_hover_kind(&e.project, 1, center1, 0.01, 0.01),
        Some(geometry::PlanHoverKind::Body)
    );
    let ring = geometry::element_plan_outline_on_story(&e.project, element, 1);
    assert!(ring.iter().all(|p| p.y > 6.));
    assert_ne!(ring, geometry::element_plan_outline(&e.project, element));
    let mut vertical = element.clone();
    if let Shape::Column { inclination, .. } = &mut vertical.shape {
        *inclination = None;
    }
    assert!(!geometry::element_on_plan_story(&e.project, &vertical, 1));
}

#[test]
fn column_plan_references_snap_and_marquee_use_real_current_cut() {
    use atelier_bim::editor::{MarqueeArea, MarqueeScope};
    for circular in [false, true] {
        let (mut e, id) = scene();
        if let Shape::Column {
            height,
            section,
            plan,
            ..
        } = &mut e.project.elements[0].shape
        {
            *height = 6.;
            if !circular {
                *section = ColumnSection::Rectangular {
                    width: 0.3,
                    depth: 0.6,
                };
            }
            plan.display = Display::CutOnly;
        }
        e.switch_story(1);
        let center = Point::new(0., 4.3 * 3_f64.sqrt());
        let points = column_plan::reference_points(&e.project, &e.project.elements[0], 1);
        assert_eq!(points.len(), if circular { 1 } else { 5 });
        assert!(points[0].distance(center) < 1e-5);
        let near = Point::new(center.x + 0.01, center.y + 0.01);
        let (snapped, kind) = geometry::snap(&e.project, 1, near, 0.05, true);
        assert_eq!(kind, geometry::SnapKind::SnapPoint);
        assert!(snapped.distance(center) < 1e-5);
        let (_, other) = geometry::snap(&e.project, 0, near, 0.05, true);
        assert_ne!(other, geometry::SnapKind::SnapPoint);
        let box_at = |c: Point| {
            vec![
                Point::new(c.x - 0.05, c.y - 0.05),
                Point::new(c.x + 0.05, c.y - 0.05),
                Point::new(c.x + 0.05, c.y + 0.05),
                Point::new(c.x - 0.05, c.y + 0.05),
            ]
        };
        let before = e.project.clone();
        let history = e.undo.len();
        e.marquee = MarqueeArea::new(box_at(center), MarqueeScope::CurrentStory, 1);
        assert_eq!(e.select_all_in_marquee(), 1);
        assert_eq!(e.selected, Some(id));
        e.marquee = MarqueeArea::new(box_at(Point::default()), MarqueeScope::CurrentStory, 1);
        assert_eq!(e.select_all_in_marquee(), 0);
        assert_eq!(e.project, before);
        assert_eq!(e.undo.len(), history);
        let layer = e
            .project
            .layers
            .layers
            .iter_mut()
            .find(|l| !l.protected)
            .unwrap();
        layer.status.visible = false;
        e.project.elements[0].layer = atelier_bim::layers::ElementLayer(layer.id);
        assert_ne!(
            geometry::snap(&e.project, 1, near, 0.05, true).1,
            geometry::SnapKind::SnapPoint
        );
        e.marquee = MarqueeArea::new(box_at(center), MarqueeScope::CurrentStory, 1);
        assert_eq!(e.select_all_in_marquee(), 0);
    }
}

#[test]
fn column_all_story_marquee_and_range_boundaries_do_not_invent_references() {
    use atelier_bim::editor::{MarqueeArea, MarqueeScope};
    let (mut e, _) = scene();
    if let Shape::Column { height, plan, .. } = &mut e.project.elements[0].shape {
        *height = 6.;
        plan.display = Display::CutOnly;
    }
    let center = column_plan::reference_points(&e.project, &e.project.elements[0], 1)[0];
    let polygon = vec![
        Point::new(center.x - 0.1, center.y - 0.1),
        Point::new(center.x + 0.1, center.y - 0.1),
        Point::new(center.x + 0.1, center.y + 0.1),
        Point::new(center.x - 0.1, center.y + 0.1),
    ];
    let before = e.project.clone();
    e.marquee = MarqueeArea::new(polygon.clone(), MarqueeScope::CurrentStory, 0);
    assert_eq!(e.select_all_in_marquee(), 0);
    e.marquee = MarqueeArea::new(polygon, MarqueeScope::AllStories, 0);
    assert_eq!(e.select_all_in_marquee(), 1);
    e.marquee.as_mut().unwrap().z_range = Some((10., 11.));
    assert_eq!(e.select_all_in_marquee(), 0);
    assert_eq!(e.project, before);
    e.project.plan_options.floor_plan_cut.height = 10.;
    assert!(column_plan::reference_points(&e.project, &e.project.elements[0], 1).is_empty());
    e.project.plan_options.floor_plan_cut.height = 1.1;
    if let Shape::Column { height, .. } = &mut e.project.elements[0].shape {
        *height = 3.2;
    }
    assert!(!geometry::element_on_plan_story(
        &e.project,
        &e.project.elements[0],
        1
    ));
    if let Shape::Column { height, plan, .. } = &mut e.project.elements[0].shape {
        *height = 10.;
        plan.display = Display::Projected;
        plan.projection = Projection::FloorPlanRange;
    }
    assert!(column_plan::reference_points(&e.project, &e.project.elements[0], 2).is_empty());
}
