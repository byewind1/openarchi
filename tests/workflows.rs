use atelier_bim::{
    editor::{Editor, Tool},
    geometry::{self, Camera, SnapKind},
    model::*,
    storage::{self, JsonAdapter},
    wall::{WallGeometryMethod, WallReferenceLine},
};
fn p(x: f32, y: f32) -> Point {
    Point::new(x, y)
}
fn empty() -> Editor {
    Editor::new(Project::default())
}
#[test]
fn wall_creation_undo_redo_and_branch() {
    let mut e = empty();
    e.set_tool(Tool::Wall);
    e.click(p(0., 0.), 0.);
    assert!(e.project.elements.is_empty());
    e.click(p(4., 0.), 0.);
    assert_eq!(e.project.elements.len(), 1);
    assert_eq!(e.project.elements[0].area(), 12.);
    e.undo();
    assert!(e.project.elements.is_empty());
    e.redo();
    assert_eq!(e.project.elements.len(), 1);
    e.undo();
    e.click(p(0., 0.), 0.);
    e.click(p(6., 0.), 0.);
    assert!(e.redo.is_empty());
    assert_eq!(e.project.elements[0].area(), 18.);
}

fn wall_segment(element: &Element) -> (Point, Point) {
    match element.shape {
        Shape::Wall { start, end, .. } => (start, end),
        _ => panic!("expected a wall"),
    }
}

#[test]
fn continuous_wall_method_builds_a_connected_chain_in_one_undo() {
    let mut editor = empty();
    editor.set_tool(Tool::Wall);
    editor.wall_settings.geometry.geometry_method = WallGeometryMethod::Continuous;
    for point in [p(0., 0.), p(4., 0.), p(4., 3.)] {
        editor.click(point, 0.);
    }
    assert!(editor.project.elements.is_empty());
    assert_eq!(editor.wall_points.len(), 3);
    let ids = editor.finish_wall(None).unwrap();
    assert_eq!(ids.len(), 2);
    assert_eq!(editor.project.elements.len(), 2);
    assert_eq!(
        wall_segment(&editor.project.elements[0]),
        (p(0., 0.), p(4., 0.))
    );
    assert_eq!(
        wall_segment(&editor.project.elements[1]),
        (p(4., 0.), p(4., 3.))
    );
    assert_eq!(editor.selected_ids, vec![ids[1]]);
    assert_eq!(editor.undo.len(), 1);
    assert!(editor.project.validate().is_ok());

    editor.undo();
    assert!(editor.project.elements.is_empty());
    editor.redo();
    assert_eq!(editor.project.elements.len(), 2);
}

#[test]
fn continuous_wall_backspace_removes_only_the_last_input_node() {
    let mut editor = empty();
    editor.set_tool(Tool::Wall);
    editor.wall_settings.geometry.geometry_method = WallGeometryMethod::Continuous;
    for point in [p(0., 0.), p(4., 0.), p(4., 3.)] {
        editor.click(point, 0.);
    }
    assert!(editor.backspace_wall());
    assert_eq!(editor.wall_points, vec![p(0., 0.), p(4., 0.)]);
    assert_eq!(editor.anchor, Some(p(4., 0.)));
    assert!(editor.backspace_wall());
    assert_eq!(editor.wall_points, vec![p(0., 0.)]);
    assert!(editor.backspace_wall());
    assert!(editor.wall_points.is_empty());
    assert!(editor.anchor.is_none());
    assert!(editor.project.elements.is_empty());
}

#[test]
fn rectangular_wall_method_builds_four_world_aligned_segments() {
    let mut editor = empty();
    editor.set_tool(Tool::Wall);
    editor.wall_settings.geometry.geometry_method = WallGeometryMethod::Rectangular;
    editor.click(p(1., 2.), 0.);
    assert!(editor.project.elements.is_empty());
    editor.click(p(5., 5.), 0.);
    assert_eq!(editor.project.elements.len(), 4);
    for element in &editor.project.elements {
        let (start, end) = wall_segment(element);
        assert!(
            (start.x - end.x).abs() < 1e-5 || (start.y - end.y).abs() < 1e-5,
            "rectangle edges must follow the world axes"
        );
    }
    assert_eq!(editor.undo.len(), 1);
    editor.undo();
    assert!(editor.project.elements.is_empty());
}

#[test]
fn polygonal_wall_method_closes_a_counter_clockwise_ring() {
    let mut editor = empty();
    editor.set_tool(Tool::Wall);
    editor.wall_settings.geometry.geometry_method = WallGeometryMethod::Polygonal;
    editor.wall_settings.geometry.reference_line = WallReferenceLine::Inside;
    for point in [p(0., 0.), p(0., 3.), p(4., 3.), p(4., 0.)] {
        editor.click(point, 0.);
    }
    assert!(editor.project.elements.is_empty());
    // Clicking the first node closes the ring.
    editor.click(p(0., 0.), 0.1);
    assert_eq!(editor.project.elements.len(), 4);
    let segments: Vec<_> = editor.project.elements.iter().map(wall_segment).collect();
    for index in 0..segments.len() {
        assert_eq!(segments[index].1, segments[(index + 1) % segments.len()].0);
    }
    let signed_area = editor
        .project
        .elements
        .iter()
        .map(|element| {
            let (start, end) = wall_segment(element);
            start.x * end.y - end.x * start.y
        })
        .sum::<f32>()
        * 0.5;
    assert!(
        signed_area > 0.,
        "polygon walls should run counter-clockwise so Inside stays inside"
    );
    assert_eq!(editor.undo.len(), 1);
    assert!(editor.project.validate().is_ok());
}

#[test]
fn wall_geometry_methods_are_not_just_labels() {
    let mut single = empty();
    single.set_tool(Tool::Wall);
    single.click(p(0., 0.), 0.);
    single.click(p(4., 0.), 0.);
    assert_eq!(single.project.elements.len(), 1);

    let mut continuous = empty();
    continuous.set_tool(Tool::Wall);
    continuous.wall_settings.geometry.geometry_method = WallGeometryMethod::Continuous;
    for point in [p(0., 0.), p(4., 0.), p(4., 3.)] {
        continuous.click(point, 0.);
    }
    continuous.finish_wall(None).unwrap();
    assert_eq!(continuous.project.elements.len(), 2);

    let mut rectangular = empty();
    rectangular.set_tool(Tool::Wall);
    rectangular.wall_settings.geometry.geometry_method = WallGeometryMethod::Rectangular;
    rectangular.click(p(0., 0.), 0.);
    rectangular.click(p(4., 3.), 0.);
    assert_eq!(rectangular.project.elements.len(), 4);

    let mut polygonal = empty();
    polygonal.set_tool(Tool::Wall);
    polygonal.wall_settings.geometry.geometry_method = WallGeometryMethod::Polygonal;
    for point in [p(0., 0.), p(4., 0.), p(4., 3.), p(0., 3.)] {
        polygonal.click(point, 0.);
    }
    polygonal.finish_wall(None).unwrap();
    assert_eq!(polygonal.project.elements.len(), 4);
}
#[test]
fn reverse_slab_corner_normalization() {
    let mut e = empty();
    e.set_tool(Tool::Slab);
    e.click(p(5., 4.), 0.);
    e.click(p(1., 1.), 0.);
    assert_eq!(e.project.elements[0].area(), 12.);
    assert!(
        matches!(e.project.elements[0].shape,Shape::Slab{min,max,..} if min==p(1.,1.)&&max==p(5.,4.))
    );
}
#[test]
fn zero_geometry_does_not_create_history() {
    let mut e = empty();
    e.set_tool(Tool::Wall);
    e.click(p(1., 1.), 0.);
    e.click(p(1., 1.), 0.);
    assert!(e.undo.is_empty());
    assert!(e.anchor.is_some());
    e.set_tool(Tool::Slab);
    e.click(p(1., 1.), 0.);
    e.click(p(1., 8.), 0.);
    assert!(e.project.elements.is_empty());
}
#[test]
fn polyline_domain_validation_hit_snap_persistence_and_undo() {
    let mut editor = empty();
    editor.set_tool(Tool::Polyline);
    for point in [p(0., 0.), p(2., 2.), p(4., 0.)] {
        editor.click(point, 0.);
    }
    assert!(editor.project.elements.is_empty());
    let id = editor.finish_polyline(None).unwrap().unwrap();
    assert_eq!(
        editor.project.elements[0].area(),
        2. * 2_f32.sqrt() + 2. * 2_f32.sqrt()
    );
    assert_eq!(
        geometry::hit_plan(&editor.project, 0, p(1., 1.), 0.02),
        Some(id)
    );
    assert_eq!(
        geometry::hit_plan(&editor.project, 0, p(2., 0.), 0.02),
        None
    );
    assert_eq!(
        geometry::snap(&editor.project, 0, p(2.01, 2.), 0.05, true),
        (p(2., 2.), SnapKind::Endpoint)
    );
    let mut invalid = editor.project.clone();
    if let Shape::Polyline { points, .. } = &mut invalid.elements[0].shape {
        points[1] = points[0];
    }
    assert!(invalid.validate().is_err());
    let bytes = JsonAdapter.export(&editor.project).unwrap();
    assert_eq!(JsonAdapter.import(&bytes).unwrap(), editor.project);
    editor.move_selection(p(1., -1.));
    assert!(
        matches!(editor.project.elements[0].shape, Shape::Polyline { ref points , .. } if points[0] == p(1., -1.))
    );
    editor.undo();
    assert!(
        matches!(editor.project.elements[0].shape, Shape::Polyline { ref points , .. } if points[0] == p(0., 0.))
    );
    editor.redo();
    assert!(
        matches!(editor.project.elements[0].shape, Shape::Polyline { ref points , .. } if points[2] == p(5., -1.))
    );
}
#[test]
fn tool_and_story_change_cancel_drawing() {
    let mut e = empty();
    e.set_tool(Tool::Wall);
    e.click(p(0., 0.), 0.);
    e.switch_story(1);
    assert!(e.anchor.is_none());
    e.click(p(0., 0.), 0.);
    e.set_tool(Tool::Slab);
    assert!(e.anchor.is_none());
    e.switch_story(999);
    assert_eq!(e.story, 1);
}
#[test]
fn selection_prefers_wall_over_slab_and_filters_story() {
    let p = Project::demo();
    assert_eq!(geometry::hit_plan(&p, 0, Point::new(4., 0.), 0.01), Some(2));
    assert_eq!(geometry::hit_plan(&p, 0, Point::new(4., 2.), 0.01), Some(1));
    assert_eq!(geometry::hit_plan(&p, 1, Point::new(4., 0.), 0.01), None);
}
#[test]
fn snapping_endpoint_midpoint_grid_and_free() {
    let mut e = empty();
    e.set_tool(Tool::Wall);
    e.click(p(0., 0.), 0.);
    e.click(p(4., 0.), 0.);
    let proj = &e.project;
    assert_eq!(
        geometry::snap(proj, 0, p(0.01, 0.), 0.08, true),
        (p(0., 0.), SnapKind::Endpoint)
    );
    assert_eq!(
        geometry::snap(proj, 0, p(2.01, 0.), 0.08, true),
        (p(2., 0.), SnapKind::Midpoint)
    );
    assert_eq!(
        geometry::snap(proj, 0, p(8.04, 8.07), 0.08, true),
        (p(8., 8.1), SnapKind::Grid)
    );
    assert_eq!(
        geometry::snap(proj, 0, p(8.04, 8.07), 0.08, false),
        (p(8.04, 8.07), SnapKind::Free)
    );
}
#[test]
fn property_edit_updates_shared_geometry_and_is_undoable() {
    let mut e = Editor::new(Project::demo());
    let mut wall = e.project.elements[1].clone();
    if let Shape::Wall {
        height, thickness, ..
    } = &mut wall.shape
    {
        *height = 4.;
        *thickness = 0.4;
    }
    wall.story = 1;
    e.replace_element(wall.clone()).unwrap();
    let corners = geometry::corners(&e.project, &wall);
    assert!((corners[0].z - 3.2).abs() < 1e-6);
    assert!((corners[4].z - 7.2).abs() < 1e-6);
    assert!((geometry::footprint(&wall)[0].y - 0.2).abs() < 1e-6);
    e.undo();
    assert_eq!(e.project, Project::demo());
}
#[test]
fn invalid_property_edit_is_atomic() {
    let mut e = Editor::new(Project::demo());
    let before = e.project.clone();
    let mut wall = before.elements[1].clone();
    if let Shape::Wall { height, .. } = &mut wall.shape {
        *height = -1.;
    }
    assert!(e.replace_element(wall).is_err());
    assert_eq!(before, e.project);
    assert!(e.undo.is_empty());
}
#[test]
fn duplicate_and_delete_roundtrip() {
    let mut e = Editor::new(Project::demo());
    e.selected = Some(2);
    let n = e.project.elements.len();
    e.duplicate();
    assert_eq!(e.project.elements.len(), n + 1);
    let id = e.selected.unwrap();
    assert_ne!(id, 2);
    e.delete();
    assert_eq!(e.project.elements.len(), n);
    e.undo();
    assert_eq!(e.project.elements.last().unwrap().id, id);
    e.undo();
    assert_eq!(e.project, Project::demo());
}
#[test]
fn json_roundtrip_and_version_rejection() {
    let p = Project::demo();
    let bytes = JsonAdapter.export(&p).unwrap();
    assert_eq!(JsonAdapter.import(&bytes).unwrap(), p);
    let mut invalid = p.clone();
    invalid.schema_version = 99;
    assert!(
        JsonAdapter
            .import(&serde_json::to_vec(&invalid).unwrap())
            .is_err()
    );
    assert!(JsonAdapter.import(b"not json").is_err());
}
#[test]
fn invalid_ids_and_stories_rejected() {
    let mut p = Project::demo();
    p.elements[1].id = p.elements[0].id;
    assert!(p.validate().is_err());
    let mut p = Project::demo();
    p.elements[0].story = 999;
    assert!(p.validate().is_err());
    let mut p = Project::demo();
    p.stories[0].elevation = f32::NAN;
    assert!(p.validate().is_err());
}
#[test]
fn disk_save_load_and_failed_save_leave_project_intact() {
    let dir = std::env::temp_dir().join(format!("openarchi-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("project.json");
    let p = Project::demo();
    storage::save(&p, &path).unwrap();
    assert_eq!(storage::load(&path).unwrap(), p);
    assert!(!path.with_extension("json.tmp").exists());
    assert!(storage::save(&p, &dir.join("missing/project.json")).is_err());
    assert_eq!(storage::load(&path).unwrap(), p);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn marquee_scope_selection_group_move_duplicate_and_undo() {
    let mut project = Project::default();
    let wall = project.add(
        0,
        Material::Concrete,
        Shape::Wall {
            start: p(0., 0.),
            end: p(4., 0.),
            thickness: 0.2,
            height: 3.,
        },
    );
    project.add(
        1,
        Material::Timber,
        Shape::Slab {
            min: p(0., 0.),
            max: p(2., 2.),
            thickness: 0.2,
        },
    );
    let mut editor = Editor::new(project);
    editor.marquee = atelier_bim::editor::MarqueeArea::new(
        vec![p(-1., -1.), p(5., -1.), p(5., 3.), p(-1., 3.)],
        atelier_bim::editor::MarqueeScope::CurrentStory,
        0,
    );
    assert_eq!(editor.select_all_in_marquee(), 1);
    assert_eq!(editor.selection_ids(), vec![wall]);
    editor.marquee.as_mut().unwrap().scope = atelier_bim::editor::MarqueeScope::AllStories;
    assert_eq!(editor.select_all_in_marquee(), 2);
    editor.move_selection(p(1., 2.));
    assert!(
        matches!(editor.project.elements[0].shape, Shape::Wall { start, .. } if start == p(1., 2.))
    );
    assert!(
        matches!(editor.project.elements[1].shape, Shape::Slab { min, .. } if min == p(1., 2.))
    );
    editor.duplicate();
    assert_eq!(editor.project.elements.len(), 4);
    assert_eq!(editor.selection_ids().len(), 2);
    editor.undo();
    assert_eq!(editor.project.elements.len(), 2);
    assert_eq!(
        editor.project.elements[0].shape,
        Shape::Wall {
            start: p(1., 2.),
            end: p(5., 2.),
            thickness: 0.2,
            height: 3.
        }
    );
}

#[test]
fn marquee_stretch_moves_only_enclosed_endpoints_and_is_atomic() {
    let mut editor = empty();
    editor.project.add(
        0,
        Material::Concrete,
        Shape::Wall {
            start: p(0., 0.),
            end: p(4., 0.),
            thickness: 0.2,
            height: 3.,
        },
    );
    editor.marquee = atelier_bim::editor::MarqueeArea::new(
        vec![p(-1., -1.), p(1., -1.), p(1., 1.), p(-1., 1.)],
        atelier_bim::editor::MarqueeScope::CurrentStory,
        0,
    );
    assert_eq!(editor.stretch_nodes_in_marquee(p(0., 2.)).unwrap(), 1);
    assert!(
        matches!(editor.project.elements[0].shape, Shape::Wall { start, end, .. } if start == p(0., 2.) && end == p(4., 0.))
    );
    editor.undo();
    assert!(
        matches!(editor.project.elements[0].shape, Shape::Wall { start, end, .. } if start == p(0., 0.) && end == p(4., 0.))
    );
}

#[test]
fn marquee_clipboard_transform_and_story_commands_work_on_groups() {
    let mut editor = empty();
    let id = editor.project.add(
        0,
        Material::Concrete,
        Shape::Wall {
            start: p(0., 0.),
            end: p(4., 0.),
            thickness: 0.2,
            height: 3.,
        },
    );
    editor.set_selection(vec![id]);
    assert_eq!(editor.copy_selection(), 1);
    assert_eq!(editor.paste(p(0., 2.)), 1);
    assert_eq!(editor.project.elements.len(), 2);
    let copies = editor.selection_ids();
    editor.set_selection(vec![id, copies[0]]);
    editor.rotate_selection_quarter_turn(false);
    assert!(editor.project.elements.iter().all(
        |e| matches!(e.shape, Shape::Wall { start, end, .. } if (start.x-end.x).abs() < 1e-5)
    ));
    editor.mirror_selection(true);
    editor.move_selection_to_story(1);
    assert!(editor.project.elements.iter().all(|e| e.story == 1));
    editor.undo();
    assert!(editor.project.elements.iter().all(|e| e.story == 0));
    editor.set_selection(vec![id, copies[0]]);
    assert_eq!(editor.cut_selection(), 2);
    assert!(editor.project.elements.is_empty());
    assert_eq!(editor.paste(p(0., 0.)), 2);
}

#[test]
fn marquee_volume_filters_by_story_xy_and_height() {
    let mut project = Project::default();
    let low = project.add(
        0,
        Material::Concrete,
        Shape::Wall {
            start: p(0., 0.),
            end: p(4., 0.),
            thickness: 0.2,
            height: 3.,
        },
    );
    let high = project.add(
        1,
        Material::Concrete,
        Shape::Wall {
            start: p(0., 0.),
            end: p(4., 0.),
            thickness: 0.2,
            height: 3.,
        },
    );
    atelier_bim::editor::MarqueeArea::new(
        vec![p(-1., -1.), p(1., -1.), p(1., 1.), p(-1., 1.)],
        atelier_bim::editor::MarqueeScope::AllStories,
        0,
    )
    .unwrap();
    assert!(geometry::element_overlaps_z_range(
        &project,
        &project.elements[0],
        (0., 3.)
    ));
    assert!(!geometry::element_overlaps_z_range(
        &project,
        &project.elements[1],
        (0., 3.)
    ));
    let narrow = [p(1.5, -1.), p(2.5, -1.), p(2.5, 1.), p(1.5, 1.)];
    assert!(geometry::element_intersects_polygon(
        &project.elements[0],
        &narrow
    ));
    assert!(!geometry::element_has_node_in_polygon(
        &project.elements[0],
        &narrow
    ));
    assert_ne!(low, high);
}
#[test]
fn ray_pick_matches_projected_wall_and_story_visibility() {
    let mut p = Project::default();
    let id = p.add(
        1,
        Material::Concrete,
        Shape::Wall {
            start: Point::new(0., 0.),
            end: Point::new(4., 0.),
            thickness: 0.3,
            height: 3.,
        },
    );
    let c = Camera::default();
    let ndc = c.matrix(1.).project_point3(glam::Vec3::new(2., 0., 4.5));
    assert_eq!(c.pick(&p, [ndc.x, ndc.y], 1., None), Some(id));
    assert_eq!(c.pick(&p, [ndc.x, ndc.y], 1., Some(0)), None);
    assert_eq!(c.pick(&p, [0.99, 0.99], 1., None), None);
}
#[test]
fn savepoint_dirty_flag_tracks_undo() {
    let mut e = empty();
    assert!(!e.dirty());
    e.set_tool(Tool::Wall);
    e.click(p(0., 0.), 0.);
    e.click(p(4., 0.), 0.);
    assert!(e.dirty());
    e.saved = e.project.clone();
    assert!(!e.dirty());
    e.undo();
    assert!(e.dirty());
    e.redo();
    assert!(!e.dirty());
}
#[test]
fn demo_valid_and_all_mesh_corners_finite() {
    let p = Project::demo();
    p.validate().unwrap();
    assert!(
        p.elements.iter().any(|element| element.story == 1),
        "the demo needs upper-floor geometry so the default Trace reference is visible"
    );
    for e in &p.elements {
        assert!(geometry::corners(&p, e).iter().all(|v| v.is_finite()));
    }
}

#[test]
fn non_finite_creation_is_rejected_without_mutation() {
    let mut e = empty();
    e.set_tool(Tool::Wall);
    e.click(p(f32::NAN, 0.), 0.);
    assert!(e.anchor.is_none());
    e.click(p(0., 0.), 0.);
    e.click(p(f32::INFINITY, 3.), 0.);
    assert!(e.project.elements.is_empty());
    e.height = -1.;
    e.click(p(4., 0.), 0.);
    assert!(e.project.elements.is_empty());
    assert!(e.undo.is_empty());
    assert_eq!(e.project.next_id, 1);
}

#[test]
fn default_camera_fits_demo_in_narrow_and_wide_viewports() {
    let p = Project::demo();
    let camera = Camera::default();
    for aspect in [0.35, 0.65, 1.0, 1.8] {
        let matrix = camera.matrix(aspect);
        for e in &p.elements {
            for corner in geometry::corners(&p, e) {
                let point = matrix.project_point3(corner);
                assert!(
                    point.x.abs() < 0.95 && point.y.abs() < 0.95,
                    "{aspect}: {point:?}"
                );
            }
        }
    }
}

#[test]
fn grid_settings_roundtrip_and_legacy_defaults() {
    let mut p = Project::demo();
    p.grid_settings.aux_spacing_x = 0.75;
    p.grid_settings.aux_steps_x = 2;
    p.grid_settings.snap_mode = atelier_bim::grids::GridSnapMode::ConstructionGrid;
    p.grid_settings.rotation_deg = -30.0;
    p.grid_settings.rotated_display = true;
    p.grid_settings.rotate_enabled = true;
    let bytes = JsonAdapter.export(&p).unwrap();
    let loaded = JsonAdapter.import(&bytes).unwrap();
    assert_eq!(loaded.grid_settings, p.grid_settings);

    let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    value.as_object_mut().unwrap().remove("grid_settings");
    let legacy = JsonAdapter
        .import(&serde_json::to_vec(&value).unwrap())
        .unwrap();
    assert_eq!(
        legacy.grid_settings,
        atelier_bim::grids::GridSettings::default()
    );
}

#[test]
fn wall_settings_edit_geometry_undo_persistence_and_new_wall_defaults() {
    let mut editor = empty();
    editor.set_tool(Tool::Wall);
    editor.click(p(0., 0.), 0.);
    editor.click(p(4., 0.), 0.);
    let id = editor.project.elements[0].id;
    let mut element = editor.project.elements[0].clone();
    let mut draft = atelier_bim::wall_ui::WallSettingsDraft::from_element(&element).unwrap();
    draft.height = 4.5;
    draft.thickness = 0.3;
    draft.settings.geometry.reference_offset = 0.1;
    draft.settings.geometry.top_offset = 0.6;
    draft.settings.geometry.bottom_offset = -0.2;
    draft.settings.classification.property_id = "SW - 099".into();
    draft.apply_to_element(&mut element).unwrap();
    editor.replace_element(element).unwrap();

    let wall = editor
        .project
        .elements
        .iter()
        .find(|element| element.id == id)
        .unwrap();
    let footprint = geometry::footprint(wall);
    let center_y = footprint.iter().map(|point| point.y).sum::<f32>() / 4.;
    assert!((center_y - 0.1).abs() < 1e-5);
    let base = editor.project.elevation(0);
    let corners = geometry::corners(&editor.project, wall);
    assert!((corners[0].z - (base - 0.2)).abs() < 1e-5);
    assert!((corners[4].z - (base + 4.5 + 0.6)).abs() < 1e-5);
    assert_eq!(
        wall.wall_settings
            .as_ref()
            .unwrap()
            .classification
            .property_id,
        "SW - 099"
    );

    editor.undo();
    let restored = editor
        .project
        .elements
        .iter()
        .find(|element| element.id == id)
        .unwrap();
    let Shape::Wall { height, .. } = restored.shape else {
        panic!("wall expected");
    };
    assert_eq!(height, 3.);
    editor.redo();
    let bytes = JsonAdapter.export(&editor.project).unwrap();
    let imported = JsonAdapter.import(&bytes).unwrap();
    assert_eq!(imported, editor.project);
    assert_eq!(
        imported.elements[0]
            .wall_settings
            .as_ref()
            .unwrap()
            .classification
            .property_id,
        "SW - 099"
    );

    let mut defaults = atelier_bim::wall::WallSettings::default();
    defaults.classification.property_id = "SW - 777".into();
    editor.wall_settings = defaults;
    editor.thickness = 0.35;
    editor.height = 2.8;
    editor.click(p(0., 3.), 0.);
    editor.click(p(2., 3.), 0.);
    let new_wall = editor.project.elements.last().unwrap();
    let Shape::Wall {
        thickness, height, ..
    } = new_wall.shape
    else {
        panic!("wall expected");
    };
    assert!((thickness - 0.35).abs() < 1e-6);
    assert!((height - 2.8).abs() < 1e-6);
    assert_eq!(
        new_wall
            .wall_settings
            .as_ref()
            .unwrap()
            .classification
            .property_id,
        "SW - 777"
    );
}
