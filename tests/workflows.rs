use atelier_bim::{
    editor::{Editor, Tool},
    geometry::{self, Camera, SnapKind},
    model::*,
    storage::{self, JsonAdapter},
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
    let wall = project.add(0, Material::Concrete, Shape::Wall {
        start: p(0., 0.), end: p(4., 0.), thickness: 0.2, height: 3.,
    });
    project.add(1, Material::Timber, Shape::Slab {
        min: p(0., 0.), max: p(2., 2.), thickness: 0.2,
    });
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
    assert!(matches!(editor.project.elements[0].shape, Shape::Wall { start, .. } if start == p(1., 2.)));
    assert!(matches!(editor.project.elements[1].shape, Shape::Slab { min, .. } if min == p(1., 2.)));
    editor.duplicate();
    assert_eq!(editor.project.elements.len(), 4);
    assert_eq!(editor.selection_ids().len(), 2);
    editor.undo();
    assert_eq!(editor.project.elements.len(), 2);
    assert_eq!(editor.project.elements[0].shape, Shape::Wall { start: p(1., 2.), end: p(5., 2.), thickness: 0.2, height: 3. });
}

#[test]
fn marquee_stretch_moves_only_enclosed_endpoints_and_is_atomic() {
    let mut editor = empty();
    editor.project.add(0, Material::Concrete, Shape::Wall {
        start: p(0., 0.), end: p(4., 0.), thickness: 0.2, height: 3.,
    });
    editor.marquee = atelier_bim::editor::MarqueeArea::new(
        vec![p(-1., -1.), p(1., -1.), p(1., 1.), p(-1., 1.)],
        atelier_bim::editor::MarqueeScope::CurrentStory,
        0,
    );
    assert_eq!(editor.stretch_nodes_in_marquee(p(0., 2.)).unwrap(), 1);
    assert!(matches!(editor.project.elements[0].shape, Shape::Wall { start, end, .. } if start == p(0., 2.) && end == p(4., 0.)));
    editor.undo();
    assert!(matches!(editor.project.elements[0].shape, Shape::Wall { start, end, .. } if start == p(0., 0.) && end == p(4., 0.)));
}

#[test]
fn marquee_clipboard_transform_and_story_commands_work_on_groups() {
    let mut editor = empty();
    let id = editor.project.add(0, Material::Concrete, Shape::Wall {
        start: p(0., 0.), end: p(4., 0.), thickness: 0.2, height: 3.,
    });
    editor.set_selection(vec![id]);
    assert_eq!(editor.copy_selection(), 1);
    assert_eq!(editor.paste(p(0., 2.)), 1);
    assert_eq!(editor.project.elements.len(), 2);
    let copies = editor.selection_ids();
    editor.set_selection(vec![id, copies[0]]);
    editor.rotate_selection_quarter_turn(false);
    assert!(editor.project.elements.iter().all(|e| matches!(e.shape, Shape::Wall { start, end, .. } if (start.x-end.x).abs() < 1e-5)));
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
    let low = project.add(0, Material::Concrete, Shape::Wall {
        start: p(0., 0.), end: p(4., 0.), thickness: 0.2, height: 3.,
    });
    let high = project.add(1, Material::Concrete, Shape::Wall {
        start: p(0., 0.), end: p(4., 0.), thickness: 0.2, height: 3.,
    });
    atelier_bim::editor::MarqueeArea::new(
        vec![p(-1., -1.), p(1., -1.), p(1., 1.), p(-1., 1.)],
        atelier_bim::editor::MarqueeScope::AllStories,
        0,
    ).unwrap();
    assert!(geometry::element_overlaps_z_range(&project, &project.elements[0], (0., 3.)));
    assert!(!geometry::element_overlaps_z_range(&project, &project.elements[1], (0., 3.)));
    let narrow = [p(1.5, -1.), p(2.5, -1.), p(2.5, 1.), p(1.5, 1.)];
    assert!(geometry::element_intersects_polygon(&project.elements[0], &narrow));
    assert!(!geometry::element_has_node_in_polygon(&project.elements[0], &narrow));
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
