use atelier_bim::{
    command::ModelCommand,
    derivation_cache::{DerivationCache, GeometryHandle},
    editor::Editor,
    model::{Material, ModelAdapter, OpeningKind, Point, Project, Shape},
    storage::JsonAdapter,
};
use std::{collections::BTreeMap, sync::Arc};
fn scene() -> Project {
    let mut p = Project::default();
    let q = Point::new;
    for (start, end) in [
        (q(0., 0.), q(4., 0.)),
        (q(4., 0.), q(4., 3.)),
        (q(50., 0.), q(54., 0.)),
    ] {
        p.add(
            0,
            Material::Concrete,
            Shape::Wall {
                start,
                end,
                thickness: 0.3,
                height: 3.,
            },
        );
    }
    p.add(
        0,
        Material::Concrete,
        Shape::Opening {
            host_wall: 1,
            kind: OpeningKind::Window,
            width: 1.,
            height: 1.,
            sill: 1.,
            offset: 2.,
        },
    );
    p.add(
        0,
        Material::Concrete,
        Shape::Slab {
            min: q(0., 0.),
            max: q(4., 4.),
            thickness: 0.3,
        },
    );
    p.add(
        0,
        Material::Concrete,
        Shape::Beam {
            start: q(0., 0.),
            end: q(4., 0.),
            width: 0.3,
            height: 0.5,
            top_offset: 2.,
            slant_angle: 0.,
            arc: None,
        },
    );
    let column = Shape::Column {
        center: q(2., 2.),
        section: atelier_bim::column::ColumnSection::Circular { diameter: 0.4 },
        height: 3.,
        bottom_offset: 0.,
        axis_rotation: 0.,
        inclination: None,
        plan: Default::default(),
    };
    p.add(0, Material::Concrete, column.clone());
    p.add(
        0,
        Material::Concrete,
        Shape::Roof {
            spec: atelier_bim::roof::RoofSpec {
                contour: None,
                pivot_start: q(0., 0.),
                pivot_end: q(4., 0.),
                rise_left: true,
                min: q(0., 0.),
                max: q(4., 4.),
                pitch: 25.,
                thickness: 0.2,
                pivot_offset: 3.,
            },
        },
    );
    p.add(
        0,
        Material::Concrete,
        Shape::Stair {
            spec: Default::default(),
        },
    );
    p.add(
        0,
        Material::Concrete,
        Shape::Polyline {
            points: vec![q(0., 0.), q(2., 2.)],
            arcs: vec![],
        },
    );
    p.add(1, Material::Concrete, column);
    p.validate().unwrap();
    p
}
fn all(
    cache: &mut DerivationCache,
    p: &Project,
    revision: u64,
    session: u64,
) -> BTreeMap<u64, GeometryHandle> {
    let mut read = cache.read(p, revision, session).unwrap();
    p.elements
        .iter()
        .map(|e| (e.id, read.source(e.id).unwrap()))
        .collect()
}
fn compare_full(cache: &mut DerivationCache, p: &Project, revision: u64, session: u64) {
    let incremental = all(cache, p, revision, session);
    let mut disabled = DerivationCache::default();
    disabled.set_enabled(false);
    let reference = all(&mut disabled, p, revision, session);
    assert_eq!(incremental, reference);
}
#[test]
fn eight_families_cold_warm_and_disabled_are_exact_and_old_context_is_immutable() {
    let p = scene();
    let before = p.clone();
    let mut cache = DerivationCache::default();
    let cold = all(&mut cache, &p, 1, 7);
    assert_eq!(cache.stats().source_derivations, p.elements.len());
    let warm = all(&mut cache, &p, 2, 7);
    assert_eq!(cache.stats().source_derivations, 0);
    assert_eq!(cache.stats().context_rebuilds, 0);
    for (id, old) in &cold {
        let new = &warm[id];
        assert!(Arc::ptr_eq(&old.source, &new.source));
        assert_eq!(old.identity.input_revision, Some(1));
        assert_eq!(new.identity.input_revision, Some(2));
        assert_eq!(old.identity.document_session, Some(7));
        assert_eq!(old.source.identity().input_revision, None);
    }
    compare_full(&mut cache, &p, 2, 7);
    assert_eq!(p, before);
    let foreign = all(&mut cache, &p, 0, 8);
    assert_eq!(cache.stats().source_derivations, p.elements.len());
    assert!(!Arc::ptr_eq(&cold[&1].source, &foreign[&1].source));
    assert_eq!(cold[&1].identity.document_session, Some(7));
    assert_eq!(foreign[&1].identity.document_session, Some(8));
}
#[test]
fn wall_neighbor_change_invalidates_connected_wall_and_hosted_child_but_not_far_objects() {
    let mut p = scene();
    let mut cache = DerivationCache::default();
    let old = all(&mut cache, &p, 0, 1);
    if let Shape::Wall { thickness, .. } = &mut p.elements[1].shape {
        *thickness = 0.6;
    }
    p.validate().unwrap();
    let new = all(&mut cache, &p, 1, 1);
    assert_eq!(cache.stats().invalidated_ids, vec![1, 2, 4]);
    assert_eq!(cache.stats().context_rebuilds, 1);
    for id in [1, 2, 4] {
        assert!(!Arc::ptr_eq(&old[&id].source, &new[&id].source));
    }
    for id in [3, 5, 6, 7, 8, 9, 10, 11] {
        assert!(Arc::ptr_eq(&old[&id].source, &new[&id].source), "id {id}");
    }
    compare_full(&mut cache, &p, 1, 1);
    assert_eq!(cache.dependencies().host_openings[&1], vec![4]);
    assert_eq!(cache.dependencies().story_elements[&1], vec![11]);
}
#[test]
fn adding_an_unrelated_wall_keeps_existing_wall_identity_and_payload() {
    let mut p = scene();
    let mut cache = DerivationCache::default();
    let old = all(&mut cache, &p, 0, 1);
    p.add(
        0,
        Material::Concrete,
        Shape::Wall {
            start: Point::new(100., 0.),
            end: Point::new(104., 0.),
            thickness: 0.2,
            height: 3.,
        },
    );
    let new = all(&mut cache, &p, 1, 1);
    assert!(cache.stats().invalidated_ids.is_empty());
    for (id, g) in &old {
        assert!(Arc::ptr_eq(&g.source, &new[id].source));
        assert_eq!(
            g.identity.input_fingerprint,
            new[id].identity.input_fingerprint
        );
    }
    compare_full(&mut cache, &p, 1, 1);
}
#[test]
fn color_names_visibility_and_revision_do_not_invalidate_physical_geometry() {
    let mut p = scene();
    let mut cache = DerivationCache::default();
    let old = all(&mut cache, &p, 0, 1);
    p.name = "presentation only".into();
    p.stories[0].name = "renamed".into();
    for e in &mut p.elements {
        e.name = "renamed element".into();
        e.material = Material::Timber;
    }
    p.layers.set_all_visible(false);
    let new = all(&mut cache, &p, 100, 1);
    assert!(cache.stats().invalidated_ids.is_empty());
    assert_eq!(cache.stats().context_rebuilds, 0);
    assert_eq!(cache.stats().source_derivations, 0);
    for (id, g) in old {
        assert!(Arc::ptr_eq(&g.source, &new[&id].source));
    }
    compare_full(&mut cache, &p, 100, 1);
}
#[test]
fn story_elevation_and_legacy_direct_edits_are_detected_without_trusting_revision() {
    let mut p = scene();
    let mut cache = DerivationCache::default();
    let old = all(&mut cache, &p, 0, 1);
    p.stories[0].elevation = 1.;
    let new = all(&mut cache, &p, 0, 1);
    assert_eq!(cache.stats().invalidated_ids, (1..=10).collect::<Vec<_>>());
    assert!(Arc::ptr_eq(&old[&11].source, &new[&11].source));
    compare_full(&mut cache, &p, 0, 1);
    // The same numeric base at another/missing story cannot reuse a valid host.
    p.elements[3].story = 1;
    let mut read = cache.read(&p, 0, 1).unwrap();
    assert!(read.source(4).is_err());
    drop(read);
    p.elements[4].story = 999;
    let mut read = cache.read(&p, 0, 1).unwrap();
    assert_eq!(read.source(5).unwrap_err().code, "GEOMETRY_STORY_MISSING");
}
#[test]
fn host_delete_undo_redo_copy_and_reopen_match_full_derivation_without_mutating_history() {
    let mut editor = Editor::new(scene());
    let mut cache = DerivationCache::default();
    all(
        &mut cache,
        &editor.project,
        editor.revision,
        editor.document_session(),
    );
    let baseline = editor.project.clone();
    let saved = editor.saved.clone();
    editor
        .apply(ModelCommand::RemoveElements { ids: vec![1] })
        .unwrap();
    assert!(
        !editor
            .project
            .elements
            .iter()
            .any(|e| e.id == 1 || e.id == 4)
    );
    compare_full(
        &mut cache,
        &editor.project,
        editor.revision,
        editor.document_session(),
    );
    editor.undo();
    assert_eq!(editor.project, baseline);
    compare_full(
        &mut cache,
        &editor.project,
        editor.revision,
        editor.document_session(),
    );
    editor.redo();
    compare_full(
        &mut cache,
        &editor.project,
        editor.revision,
        editor.document_session(),
    );
    editor.undo();
    editor.set_selection(vec![1]);
    editor.copy_selection();
    editor.paste(Point::new(20., 20.));
    compare_full(
        &mut cache,
        &editor.project,
        editor.revision,
        editor.document_session(),
    );
    let history = editor.undo.len();
    let revision = editor.revision;
    let selection = editor.selection_ids();
    let dirty = editor.dirty();
    compare_full(
        &mut cache,
        &editor.project,
        revision,
        editor.document_session(),
    );
    assert_eq!(editor.undo.len(), history);
    assert_eq!(editor.revision, revision);
    assert_eq!(editor.selection_ids(), selection);
    assert_eq!(editor.dirty(), dirty);
    assert_eq!(editor.saved, saved);
    let project = JsonAdapter
        .import(&JsonAdapter.export(&editor.project).unwrap())
        .unwrap();
    let reopened = Editor::new(project);
    compare_full(
        &mut cache,
        &reopened.project,
        reopened.revision,
        reopened.document_session(),
    );
}
#[test]
fn dangling_and_cyclic_opening_hosts_return_errors_without_cached_success_or_recursion() {
    let mut p = scene();
    let mut cache = DerivationCache::default();
    all(&mut cache, &p, 0, 1);
    p.elements.retain(|e| e.id != 1);
    let mut read = cache.read(&p, 0, 1).unwrap();
    assert_eq!(read.source(4).unwrap_err().code, "OPENING_HOST_MISSING");
    drop(read);
    if let Shape::Opening { host_wall, .. } =
        &mut p.elements.iter_mut().find(|e| e.id == 4).unwrap().shape
    {
        *host_wall = 4;
    }
    let mut read = cache.read(&p, 0, 1).unwrap();
    assert_eq!(read.source(4).unwrap_err().code, "OPENING_HOST_MISSING");
}

#[test]
fn editor_owns_one_cache_and_batch_scope_preserves_real_history_and_rejects_nested_borrows() {
    let mut e = Editor::new(scene());
    e.apply(ModelCommand::MoveElements {
        ids: vec![5],
        offset: Point::new(1., 1.),
    })
    .unwrap();
    let redone = e.project.clone();
    e.undo();
    let before = e.project.clone();
    let revision = e.revision;
    let selection = e.selection_ids();
    let undo = e.undo.len();
    let redo = e.redo.len();
    let saved = e.saved.clone();
    let cold = e.geometry_source(1).unwrap();
    let batch = e
        .with_geometry_read(|read| {
            let values = before
                .elements
                .iter()
                .map(|x| read.source(x.id).unwrap())
                .collect::<Vec<_>>();
            assert_eq!(e.geometry_source(1).unwrap_err().code, "GEOMETRY_READ_BUSY");
            values
        })
        .unwrap();
    assert!(Arc::ptr_eq(&cold.source, &batch[0].source));
    assert_eq!(
        e.wall_geometry(1).unwrap().identity.input_revision,
        Some(revision)
    );
    assert_eq!(
        e.opening_geometry(4)
            .unwrap()
            .host_identity
            .document_session,
        Some(e.document_session())
    );
    assert_eq!(e.project, before);
    assert_eq!(e.revision, revision);
    assert_eq!(e.selection_ids(), selection);
    assert_eq!(e.undo.len(), undo);
    assert_eq!(e.redo.len(), redo);
    assert_eq!(e.saved, saved);
    e.redo();
    assert_eq!(e.project, redone);
}

#[test]
fn production_scene_cached_and_full_paths_match_exact_vertices_colors_lines_and_diagnostics() {
    use atelier_bim::{
        editor::{MarqueeArea, MarqueeScope},
        render::{CpuSceneCache, SceneView, prepare_scene},
    };
    let p = scene();
    let mut cache = DerivationCache::default();
    let mut scene_cache = CpuSceneCache::default();
    let mut area = MarqueeArea::new(
        vec![
            Point::new(0.5, -1.),
            Point::new(3., -1.),
            Point::new(3., 2.),
            Point::new(1.5, 2.),
            Point::new(1.5, 3.5),
            Point::new(0.5, 3.5),
        ],
        MarqueeScope::AllStories,
        0,
    )
    .unwrap();
    area.z_range = Some((-0.1, 2.5));
    for (story, selected, hovered, marquee, outside, selection_only) in [
        (None, vec![], None, None, false, false),
        (Some(0), vec![1, 4], Some(2), None, false, false),
        (Some(1), vec![11], None, None, false, true),
        (None, vec![], None, Some(&area), false, false),
        (None, vec![1], None, Some(&area), true, false),
    ] {
        let view = SceneView {
            story,
            selected: &selected,
            hovered,
            marquee,
            outside_marquee: outside,
            selection_only,
        };
        let full = prepare_scene(&p, view, None);
        assert!(full.diagnostics.is_empty(), "{:?}", full.diagnostics);
        let mut read = cache.read(&p, 10, 1).unwrap();
        assert_eq!(&full, scene_cache.prepare(&p, view, &mut read));
        drop(read);
        let builds = scene_cache.stats().builds;
        let mut read = cache.read(&p, 11, 1).unwrap();
        assert_eq!(&full, scene_cache.prepare(&p, view, &mut read));
        assert_eq!(read.stats().source_derivations, 0);
        assert_eq!(scene_cache.stats().builds, builds);
        drop(read);
        let mut disabled = DerivationCache::default();
        disabled.set_enabled(false);
        let mut read = disabled.read(&p, 11, 1).unwrap();
        scene_cache.set_enabled(false);
        assert_eq!(&full, scene_cache.prepare(&p, view, &mut read));
        scene_cache.set_enabled(true);
    }
}
#[test]
fn scene_cache_rebuilds_style_and_clip_without_changing_source_solids_or_retaining_stale_geometry()
{
    use atelier_bim::{
        editor::{MarqueeArea, MarqueeScope},
        render::{CpuSceneCache, SceneView, prepare_scene},
    };
    let mut p = scene();
    let mut cache = DerivationCache::default();
    let mut scenes = CpuSceneCache::default();
    let view = SceneView {
        story: None,
        selected: &[],
        hovered: None,
        marquee: None,
        outside_marquee: false,
        selection_only: false,
    };
    let mut read = cache.read(&p, 0, 1).unwrap();
    let original = scenes.prepare(&p, view, &mut read).clone();
    drop(read);
    p.elements[0].material = Material::Timber;
    let mut read = cache.read(&p, 1, 1).unwrap();
    let recolored = scenes.prepare(&p, view, &mut read).clone();
    assert_eq!(read.stats().source_derivations, 0);
    assert_ne!(original, recolored);
    assert_eq!(recolored, prepare_scene(&p, view, None));
    drop(read);
    let invalid = MarqueeArea {
        vertices: vec![
            Point::new(0., 0.),
            Point::new(4., 4.),
            Point::new(0., 3.),
            Point::new(4., 0.),
        ],
        scope: MarqueeScope::AllStories,
        story: 0,
        z_range: None,
    };
    let invalid_view = SceneView {
        marquee: Some(&invalid),
        ..view
    };
    let mut read = cache.read(&p, 1, 1).unwrap();
    let broken = scenes.prepare(&p, invalid_view, &mut read);
    assert_eq!(broken.triangle_count(), 0);
    assert_eq!(broken.line_count(), 0);
    assert_eq!(broken.diagnostics[0].code, "INVALID_VIEW_CLIP");
    assert_eq!(&recolored, scenes.prepare(&p, view, &mut read));
    drop(read);
    let other = p.clone();
    let mut read = cache.read(&p, 1, 1).unwrap();
    let foreign = scenes.prepare(&other, view, &mut read);
    assert_eq!(foreign.triangle_count(), 0);
    assert_eq!(
        foreign.diagnostics[0].code,
        "GEOMETRY_READ_PROJECT_MISMATCH"
    );
    assert_eq!(&recolored, scenes.prepare(&p, view, &mut read));
}

#[test]
fn cached_view_filter_matches_full_reference_for_all_families_and_curved_holed_inclined_variants() {
    use atelier_bim::{
        editor::{MarqueeArea, MarqueeScope},
        geometry::ModelViewFilter,
        model::{ArcSegment, SlabContour, WallContour},
    };
    let q = Point::new;
    for variant in 0..3 {
        let mut p = scene();
        if variant > 0 {
            if let Shape::Column { inclination, .. } = &mut p.elements[6].shape {
                *inclination = Some(atelier_bim::column_inclination::ColumnInclination {
                    slant_degrees: 55.,
                    direction_degrees: -45.,
                });
            }
        }
        if variant == 2 {
            p.elements[0].wall_contour = Some(WallContour::Arc {
                arc: ArcSegment {
                    radius: 3.,
                    clockwise: false,
                    large_arc: false,
                },
            });
            if let Shape::Beam { arc, .. } = &mut p.elements[5].shape {
                *arc = Some(ArcSegment {
                    radius: 3.,
                    clockwise: false,
                    large_arc: false,
                });
            }
            atelier_bim::slab_geometry::sync(
                &mut p.elements[4],
                SlabContour {
                    points: vec![q(0., 0.), q(4., 0.), q(4., 4.), q(0., 4.)],
                    holes: vec![SlabContour {
                        points: vec![q(1., 1.), q(1., 2.), q(2., 2.), q(2., 1.)],
                        ..Default::default()
                    }],
                    ..Default::default()
                },
            )
            .unwrap();
            if let Shape::Roof { spec } = &mut p.elements[7].shape {
                spec.contour = Some(atelier_bim::roof_contour::RoofContour {
                    outer: vec![
                        q(0., 0.),
                        q(4., 0.),
                        q(4., 2.),
                        q(2., 2.),
                        q(2., 4.),
                        q(0., 4.),
                    ],
                    holes: vec![vec![q(0.5, 0.5), q(1.5, 0.5), q(1.5, 1.5), q(0.5, 1.5)]],
                });
            }
        }
        p.validate().unwrap();
        let mut cache = DerivationCache::default();
        let mut read = cache.read(&p, 0, 1).unwrap();
        for polygon in [
            vec![q(-1., -2.), q(55., -2.), q(55., 8.), q(-1., 8.)],
            vec![
                q(0.5, -0.1),
                q(3.5, -0.1),
                q(3.5, 2.),
                q(2., 2.),
                q(2., 3.5),
                q(0.5, 3.5),
            ],
            vec![q(1.1, 1.1), q(1.9, 1.1), q(1.9, 1.9), q(1.1, 1.9)],
        ] {
            let mut area = MarqueeArea::new(polygon, MarqueeScope::AllStories, 0).unwrap();
            for range in [None, Some((-0.1, 2.5)), Some((100., 101.))] {
                area.z_range = range;
                for outside in [false, true] {
                    for selection_only in [false, true] {
                        for story in [None, Some(0), Some(1)] {
                            let filter = ModelViewFilter {
                                story,
                                selected: &[1, 4, 7],
                                selection_only,
                                marquee: Some(&area),
                                outside_marquee: outside,
                            };
                            for e in &p.elements {
                                assert_eq!(
                                    read.model_includes(e.id, filter).unwrap(),
                                    filter.includes(&p, e),
                                    "variant={variant} type={} id={} outside={outside} selection={selection_only} story={story:?} range={range:?}",
                                    e.kind(),
                                    e.id
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn cached_camera_hits_match_reference_after_edits_clips_selection_and_history() {
    use atelier_bim::{
        editor::{MarqueeArea, MarqueeScope},
        geometry::{Camera, ModelViewFilter},
    };
    let mut e = Editor::new(scene());
    let baseline = e.project.clone();
    let area = MarqueeArea::new(
        vec![
            Point::new(0.5, -1.),
            Point::new(3.5, -1.),
            Point::new(3.5, 3.),
            Point::new(2., 3.),
            Point::new(2., 1.),
            Point::new(0.5, 1.),
        ],
        MarqueeScope::AllStories,
        0,
    )
    .unwrap();
    for edit in 0..3 {
        if edit == 1 {
            e.apply(ModelCommand::MoveElements {
                ids: vec![1, 2],
                offset: Point::new(1., 0.),
            })
            .unwrap();
        }
        if edit == 2 {
            e.undo();
            assert_eq!(e.project, baseline);
        }
        for (marquee, outside, selection_only) in [
            (None, false, false),
            (Some(&area), false, false),
            (Some(&area), true, false),
            (None, false, true),
        ] {
            let filter = ModelViewFilter {
                story: None,
                selected: &[1, 4, 7],
                selection_only,
                marquee,
                outside_marquee: outside,
            };
            for yaw in [-1.05, 0.3] {
                let camera = Camera {
                    target: glam::DVec3::new(3., 2., 1.5),
                    span: 12.,
                    yaw,
                    pitch: 0.83,
                };
                for x in [-0.6, -0.2, 0., 0.2, 0.6] {
                    for y in [-0.4, 0., 0.4] {
                        let full = camera.pick_in_view(&e.project, [x, y], 1.5, filter);
                        let cached = e.pick_model(&camera, [x, y], 1.5, filter).unwrap_or_else(|d|panic!("edit={edit} xy={x},{y} outside={outside} selection={selection_only} yaw={yaw}: {d:?}"));
                        assert_eq!(
                            cached, full,
                            "edit={edit} xy={x},{y} outside={outside} selection={selection_only} yaw={yaw}"
                        );
                    }
                }
            }
        }
    }
    assert_eq!(e.project, baseline);
}
#[test]
fn cached_ray_preserves_analytic_column_and_void_roles_and_rejects_invalid_queries() {
    use atelier_bim::derivation_cache::GeometrySource;
    use glam::DVec3;
    let e = Editor::new(scene());
    let column = e.geometry_source(7).unwrap();
    let GeometrySource::Column(g) = column.source.as_ref() else {
        panic!()
    };
    let radius = 0.2;
    let angle = 0.37_f64;
    let origin = DVec3::new(
        2. + radius * 0.999 * angle.cos(),
        2. + radius * 0.999 * angle.sin(),
        10.,
    );
    assert_eq!(
        column
            .source
            .ray_hit(origin, -DVec3::Z, None, None)
            .unwrap(),
        g.geometry.ray_hit(origin, -DVec3::Z)
    );
    assert!(
        column
            .source
            .ray_hit(origin, -DVec3::Z, None, None)
            .unwrap()
            .is_some()
    );
    assert!(
        column
            .source
            .ray_hit(origin, DVec3::ZERO, None, None)
            .is_err()
    );
    assert!(
        column
            .source
            .ray_hit(origin, DVec3::splat(f64::NAN), None, None)
            .is_err()
    );
    let line = e.geometry_source(10).unwrap();
    assert!(
        line.source
            .ray_hit(DVec3::new(1., 1., 10.), -DVec3::Z, None, None)
            .unwrap()
            .is_none()
    );
    let wall = e.geometry_source(1).unwrap();
    let opening = e.geometry_source(4).unwrap();
    let origin = DVec3::new(2., -1., 1.5);
    let direction = DVec3::Y;
    assert!(
        wall.source
            .ray_hit(origin, direction, None, None)
            .unwrap()
            .is_none()
    );
    assert!(
        opening
            .source
            .ray_hit(origin, direction, None, None)
            .unwrap()
            .is_some()
    );
}

#[test]
fn column_plan_contexts_reuse_physical_source_and_ignore_unrelated_story_inputs() {
    use atelier_bim::{column_geometry, column_plan::ColumnPlanDisplay};
    let mut p = scene();
    if let Shape::Column {
        inclination, plan, ..
    } = &mut p.elements[6].shape
    {
        *inclination = Some(atelier_bim::column_inclination::ColumnInclination {
            slant_degrees: 55.,
            direction_degrees: -45.,
        });
        plan.display = ColumnPlanDisplay::ProjectedWithOverhead;
    }
    p.validate().unwrap();
    let mut cache = DerivationCache::default();
    let mut read = cache.read(&p, 0, 1).unwrap();
    let source = read.source(7).unwrap();
    let initial = read.column_plan(7, 0).unwrap();
    assert_eq!(
        initial.identity.algorithm_version,
        column_geometry::COLUMN_PLAN_DERIVATION_VERSION
    );
    assert_eq!(
        initial.source_identity.algorithm_version,
        column_geometry::COLUMN_DERIVATION_VERSION
    );
    drop(read);
    if let Shape::Column { plan, .. } = &mut p.elements[6].shape {
        plan.display = ColumnPlanDisplay::CutOnly;
    }
    let mut read = cache.read(&p, 1, 1).unwrap();
    let changed_source = read.source(7).unwrap();
    let cut = read.column_plan(7, 0).unwrap();
    assert!(Arc::ptr_eq(&source.source, &changed_source.source));
    assert_eq!(read.stats().source_derivations, 0);
    assert_eq!(read.stats().plan_derivations, 1);
    assert_ne!(
        initial.identity.input_fingerprint,
        cut.identity.input_fingerprint
    );
    let mut expected = column_geometry::derive_plan(&p, &p.elements[6], 0).unwrap();
    for id in [&mut expected.identity, &mut expected.source_identity] {
        id.input_revision = Some(1);
        id.document_session = Some(1);
    }
    assert_eq!(cut, expected);
    drop(read);
    p.stories[2].elevation += 2.;
    let mut read = cache.read(&p, 2, 1).unwrap();
    let unrelated = read.column_plan(7, 0).unwrap();
    assert_eq!(
        unrelated.identity.input_fingerprint,
        cut.identity.input_fingerprint
    );
    assert_eq!(read.stats().plan_derivations, 0);
    assert_eq!(read.stats().plan_hits, 1);
    assert_eq!(initial.identity.input_revision, Some(0));
    drop(read);
    p.plan_options.floor_plan_cut.height = 1.7;
    let mut read = cache.read(&p, 3, 1).unwrap();
    let shifted = read.column_plan(7, 0).unwrap();
    assert_ne!(
        shifted.identity.input_fingerprint,
        cut.identity.input_fingerprint
    );
    assert_eq!(read.stats().source_derivations, 0);
    assert_eq!(read.stats().plan_derivations, 1);
    drop(read);
    if let Shape::Column { plan, .. } = &mut p.elements[6].shape {
        plan.display = ColumnPlanDisplay::Projected;
    }
    p.plan_options.floor_plan_cut.upper_story_offset = 999;
    let mut read = cache.read(&p, 4, 1).unwrap();
    assert_eq!(
        read.column_plan(7, 0).unwrap_err().code,
        "INVALID_COLUMN_PLAN_CONTEXT"
    );
}

#[test]
fn immutable_batch_returns_only_requested_results_and_keeps_prior_read_identity() {
    let mut editor = Editor::new(scene());
    let batch = editor.geometry_batch(&[4, 7, 999], Some(0)).unwrap();
    assert_eq!(batch.revision, 0);
    assert_eq!(batch.document_session, editor.document_session());
    assert_eq!(batch.opening(4).unwrap().identity.element_id, 4);
    assert_eq!(
        batch.column_plan(7).unwrap().identity.input_revision,
        Some(0)
    );
    assert_eq!(batch.source(999).unwrap_err().code, "ELEMENT_NOT_FOUND");
    assert_eq!(batch.source(1).unwrap_err().code, "ELEMENT_NOT_REQUESTED");
    let old = batch.source(4).unwrap().identity.clone();
    editor
        .apply(ModelCommand::MoveElements {
            ids: vec![1],
            offset: Point::new(1., 0.),
        })
        .unwrap();
    let new = editor.geometry_batch(&[4], Some(0)).unwrap();
    assert_eq!(batch.source(4).unwrap().identity, old);
    assert_ne!(
        new.source(4).unwrap().identity.input_revision,
        old.input_revision
    );
    assert_ne!(
        new.source(4).unwrap().identity.input_fingerprint,
        old.input_fingerprint
    );
}

#[test]
fn quantities_use_shared_nominal_units_and_keep_voids_separate_from_material() {
    use atelier_bim::derivation_cache::NominalMeasure;
    let p = scene();
    let mut cache = DerivationCache::default();
    let mut read = cache.read(&p, 0, 1).unwrap();
    for e in &p.elements {
        let handle = read.source(e.id).unwrap();
        let value = match handle.source.nominal_measure() {
            NominalMeasure::AreaM2(value) => {
                assert!(!matches!(e.shape, Shape::Polyline { .. }));
                value
            }
            NominalMeasure::LengthM(value) => {
                assert!(matches!(e.shape, Shape::Polyline { .. }));
                value
            }
        };
        assert!((value - e.area()).abs() < 1e-9, "{}", e.kind());
        let json = serde_json::to_value(handle.source.nominal_measure()).unwrap();
        assert_eq!(
            json["unit"],
            if matches!(e.shape, Shape::Polyline { .. }) {
                "length_m"
            } else {
                "area_m2"
            }
        );
        if matches!(e.shape, Shape::Opening { .. }) {
            assert!(handle.source.material_volume_m3().is_none());
            assert!((handle.source.void_volume_m3().unwrap() - 0.3).abs() < 1e-10);
        } else if matches!(e.shape, Shape::Polyline { .. }) {
            assert!(handle.source.material_volume_m3().is_none());
            assert!(handle.source.void_volume_m3().is_none());
        } else {
            assert!(handle.source.material_volume_m3().unwrap() > 0.);
            assert!(handle.source.void_volume_m3().is_none());
        }
    }
}

#[test]
fn cached_validation_matches_full_input_source_warning_and_failure_reports() {
    use atelier_bim::{geometry_validation::validate_cached, model::ArcSegment};
    let mut p = scene();
    let mut cache = DerivationCache::default();
    let full = |p: &Project| {
        let mut r = p.validation_report();
        r.input_revision = Some(9);
        r.document_session = Some(2);
        r
    };
    assert_eq!(validate_cached(&p, &mut cache, 9, 2), full(&p));
    assert_eq!(validate_cached(&p, &mut cache, 9, 2), full(&p));
    assert_eq!(cache.stats().source_derivations, 0);
    if let Shape::Opening { width, .. } = &mut p.elements[3].shape {
        *width = 0.8;
    }
    assert_eq!(validate_cached(&p, &mut cache, 9, 2), full(&p));
    assert_eq!(cache.stats().source_derivations, 2);
    let saved = p.clone();
    if let Shape::Polyline { points, arcs } = &mut p.elements[9].shape {
        *points = vec![Point::new(0., 0.), Point::new(2.00005, 0.)];
        *arcs = vec![Some(ArcSegment {
            radius: 1.,
            clockwise: false,
            large_arc: false,
        })];
    }
    let warning = validate_cached(&p, &mut cache, 9, 2);
    assert_eq!(warning, full(&p));
    assert!(warning.first_error().is_none());
    assert!(
        warning
            .diagnostics
            .iter()
            .any(|d| d.code == "ARC_ENDPOINT_RADIUS_TOLERANCE")
    );

    if let Shape::Polyline { arcs, .. } = &mut p.elements[9].shape {
        *arcs = vec![Some(ArcSegment {
            radius: 1e308,
            clockwise: true,
            large_arc: true,
        })];
    }
    let rejected = validate_cached(&p, &mut cache, 9, 2);
    assert_eq!(rejected, full(&p));
    assert!(rejected.first_error().is_some());
    p = saved;
    assert_eq!(validate_cached(&p, &mut cache, 9, 2), full(&p));
    p.schema_version = 999;
    let rejected = validate_cached(&p, &mut cache, 9, 2);
    assert_eq!(rejected, full(&p));
    assert!(!rejected.input_checked);
    assert_eq!(rejected.inspected_elements, 0);
    // Invalid structural inputs must be rejected before any join/index operation.
    let mut invalid = Project::default();
    for _ in 0..40 {
        invalid.add(
            0,
            Material::Concrete,
            Shape::Wall {
                start: Point::default(),
                end: Point::default(),
                thickness: 0.3,
                height: 3.,
            },
        );
    }
    assert_eq!(validate_cached(&invalid, &mut cache, 9, 2), full(&invalid));
}
#[test]
fn invalid_nonphysical_wall_settings_cannot_reuse_a_previously_valid_source() {
    let mut p = scene();
    p.elements[0].wall_settings = Some(Default::default());
    p.validate().unwrap();
    let mut cache = DerivationCache::default();
    {
        let mut read = cache.read(&p, 0, 1).unwrap();
        read.source(1).unwrap();
    }
    p.elements[0]
        .wall_settings
        .as_mut()
        .unwrap()
        .floor_plan
        .uncut_line_weight_mm = f64::NAN;
    let mut read = cache.read(&p, 0, 1).unwrap();
    let cached = read.source(1).unwrap_err();
    let full = atelier_bim::wall_solid::derive(&p, &p.elements[0]).unwrap_err();
    assert_eq!(cached, full);
}

#[test]
fn enabled_and_disabled_validation_keep_prepared_transactions_and_real_history_identical() {
    let mut cached = Editor::new(scene());
    let mut disabled = Editor::new(scene());
    disabled.set_source_cache_enabled(false);
    for editor in [&mut cached, &mut disabled] {
        editor
            .with_geometry_read(|read| {
                for id in 1..=11 {
                    read.source(id).unwrap();
                }
            })
            .unwrap();
    }
    let commands = vec![ModelCommand::MoveElements {
        ids: vec![1, 2],
        offset: Point::new(1., 0.),
    }];
    let candidate1 = cached
        .prepare_change(cached.revision, commands.clone())
        .unwrap();
    let candidate2 = disabled
        .prepare_change(disabled.revision, commands)
        .unwrap();
    assert_eq!(candidate1.project(), candidate2.project());
    cached.commit_change("cached-1", &candidate1).unwrap();
    disabled.commit_change("full-1", &candidate2).unwrap();
    assert_eq!(cached.project, disabled.project);
    assert_eq!(cached.undo.len(), disabled.undo.len());
    let committed = cached.project.clone();
    for editor in [&mut cached, &mut disabled] {
        editor.undo();
    }
    assert_eq!(cached.project, disabled.project);
    for editor in [&mut cached, &mut disabled] {
        let before = editor.project.clone();
        let mut invalid = before.elements[0].clone();
        if let Shape::Wall { height, .. } = &mut invalid.shape {
            *height = 0.;
        }
        assert!(
            editor
                .apply(ModelCommand::UpdateElement { element: invalid })
                .is_err()
        );
        assert_eq!(editor.project, before);
        editor.redo();
        assert_eq!(editor.project, committed);
    }
    assert_eq!(
        cached.geometry_report().diagnostics,
        disabled.geometry_report().diagnostics
    );
}

#[test]
fn dependency_candidates_follow_existing_physical_neighbour_rules_and_remove_old_edges() {
    use atelier_bim::layers::ElementLayer;
    let mut p = scene();
    // Three incident rays and a T endpoint, followed by a sparse indexed story.
    for (start, end) in [
        (Point::new(4., 0.), Point::new(6., -2.)),
        (Point::new(2., 0.), Point::new(2., -2.)),
    ] {
        p.add(
            0,
            Material::Concrete,
            Shape::Wall {
                start,
                end,
                thickness: 0.3,
                height: 3.,
            },
        );
    }
    for i in 0..40 {
        let x = 100. + 10. * f64::from(i);
        p.add(
            0,
            Material::Concrete,
            Shape::Wall {
                start: Point::new(x, 0.),
                end: Point::new(x + 2., 0.),
                thickness: 0.3,
                height: 3.,
            },
        );
    }
    let mut cache = DerivationCache::default();
    all(&mut cache, &p, 0, 1);
    let graph = cache.dependencies().wall_join_candidates.clone();
    assert_eq!(graph[&1], vec![2, 12, 13]);
    assert_eq!(graph[&2], vec![1, 12]);
    assert_eq!(graph[&12], vec![1, 2]);
    assert_eq!(graph[&13], vec![1]);
    assert!(graph[&3].is_empty());
    assert!((14..=53).all(|id| graph[&id].is_empty()));
    for (id, neighbours) in &graph {
        assert!(neighbours.iter().all(|n| graph[n].contains(id)));
    }
    // Hidden/locked layers remain physical. Use a normal layer, not the
    // immutable root layer whose status normalization overrides raw changes.
    let layer = p
        .layers
        .layers
        .iter()
        .find(|l| l.id > 1 && l.group == 1)
        .unwrap()
        .id;
    p.elements[1].layer = ElementLayer(layer);
    let status = &mut p
        .layers
        .layers
        .iter_mut()
        .find(|l| l.id == layer)
        .unwrap()
        .status;
    status.visible = false;
    status.locked = true;
    all(&mut cache, &p, 0, 1);
    assert_eq!(cache.dependencies().wall_join_candidates, graph);
    p.layers
        .layers
        .iter_mut()
        .find(|l| l.id == layer)
        .unwrap()
        .group = 0;
    all(&mut cache, &p, 0, 1);
    assert!(cache.dependencies().wall_join_candidates[&2].is_empty());
    assert_eq!(cache.dependencies().wall_join_candidates[&1], vec![12, 13]);
    p.elements[1].layer = ElementLayer(0);
    p.elements[1].story = 1;
    all(&mut cache, &p, 0, 1);
    assert!(cache.dependencies().wall_join_candidates[&2].is_empty());
    // Move that wall into a formerly isolated location on the original story.
    p.elements[1].story = 0;
    p.elements[1].shape = Shape::Wall {
        start: Point::new(54., 0.),
        end: Point::new(54., 3.),
        thickness: 0.3,
        height: 3.,
    };
    all(&mut cache, &p, 0, 1);
    assert_eq!(cache.dependencies().wall_join_candidates[&2], vec![3]);
    assert_eq!(cache.dependencies().wall_join_candidates[&3], vec![2]);
    assert!(!cache.dependencies().wall_join_candidates[&1].contains(&2));
    compare_full(&mut cache, &p, 0, 1);
}

#[test]
fn frame_plan_hits_and_cursor_priorities_match_legacy_wall_opening_queries() {
    use atelier_bim::{geometry, storage};
    for project in [scene(), storage::startup_example()] {
        let editor = Editor::new(project);
        let ids = editor
            .project
            .elements
            .iter()
            .map(|e| e.id)
            .collect::<Vec<_>>();
        let batch = editor.geometry_batch(&ids, Some(0)).unwrap();
        let before = editor
            .with_geometry_read(|read| read.stats().clone())
            .unwrap();
        for point in [
            Point::new(0., 0.),
            Point::new(2., 0.),
            Point::new(2., 0.15),
            Point::new(2., 1.),
            Point::new(4., 1.5),
            Point::new(8., 8.),
            Point::new(50., 0.),
            Point::new(100., 100.),
        ] {
            assert_eq!(
                geometry::hit_plan_with_batch(&editor.project, 0, point, 0.06, &batch),
                geometry::hit_plan(&editor.project, 0, point, 0.06),
                "hit {point:?}"
            );
            assert_eq!(
                geometry::plan_hover_kind_with_batch(&editor.project, 0, point, 0.08, 0.06, &batch),
                geometry::plan_hover_kind(&editor.project, 0, point, 0.08, 0.06),
                "hover {point:?}"
            );
        }
        let after = editor
            .with_geometry_read(|read| read.stats().clone())
            .unwrap();
        assert_eq!(after.source_derivations, before.source_derivations);
        assert_eq!(after.plan_derivations, before.plan_derivations);
        assert_eq!(
            geometry::plan_hover_kind_with_batch(
                &editor.project,
                0,
                Point::new(100., 100.),
                0.08,
                0.06,
                &batch
            ),
            None
        );
    }
}

/// Diagnostic only; the normal suite asserts semantics, not machine-dependent timing.
#[test]
#[ignore = "explicit plan hover CPU profile on a protected saved input or public example"]
fn plan_hover_frame_profile() {
    use atelier_bim::{geometry, storage};
    use std::time::Instant;
    fn rss_snapshot_kib() -> Option<u64> {
        let result = std::process::Command::new("ps")
            .args(["-p", &std::process::id().to_string(), "-o", "rss="])
            .output()
            .ok()?;
        std::str::from_utf8(&result.stdout)
            .ok()?
            .trim()
            .parse()
            .ok()
    }
    let input = std::env::var_os("OPENARCHI_HOVER_PROFILE_INPUT").map(std::path::PathBuf::from);
    let editor = if let Some(p) = &input {
        Editor::from_stored_document(p).unwrap()
    } else {
        Editor::new(storage::startup_example())
    };
    let revision = editor.revision;
    let geometry = editor.project.elements.clone();
    let points = [
        Point::new(-20., 40.),
        Point::new(38.65, 29.98),
        Point::new(40., 32.),
    ];
    let rss_before = rss_snapshot_kib();
    let start = Instant::now();
    let legacy = points.map(|point| {
        std::hint::black_box(geometry::plan_hover_kind(
            &editor.project,
            0,
            point,
            0.2,
            0.15,
        ))
    });
    let legacy_ms = start.elapsed().as_secs_f64() * 1000.;
    let start = Instant::now();
    let ids = editor
        .project
        .elements
        .iter()
        .map(|e| e.id)
        .collect::<Vec<_>>();
    let batch = editor.geometry_batch(&ids, Some(0)).unwrap();
    let cold_frame_ms = start.elapsed().as_secs_f64() * 1000.;
    let rss_cold = rss_snapshot_kib();
    let before = editor
        .with_geometry_read(|read| read.stats().clone())
        .unwrap();
    let start = Instant::now();
    for _ in 0..50 {
        let warm = points.map(|point| {
            std::hint::black_box(geometry::plan_hover_kind_with_batch(
                &editor.project,
                0,
                point,
                0.2,
                0.15,
                &batch,
            ))
        });
        assert_eq!(warm, legacy);
    }
    let warm_ms = start.elapsed().as_secs_f64() * 1000.;
    let after = editor
        .with_geometry_read(|read| read.stats().clone())
        .unwrap();
    assert_eq!(before.source_derivations, after.source_derivations);
    assert_eq!(editor.project.elements, geometry);
    assert_eq!(editor.revision, revision);
    println!(
        "PLAN_HOVER_PROFILE {}",
        serde_json::json!({"input":input,"elements":editor.project.elements.len(),"legacy_three_queries_ms":legacy_ms,"cold_frame_geometry_prepare_ms":cold_frame_ms,"warm_150_queries_ms":warm_ms,"legacy_per_query_ms":legacy_ms/3.,"warm_per_query_ms":warm_ms/150.,"rss_snapshots_kib":{"before":rss_before,"after_cold":rss_cold,"after_warm":rss_snapshot_kib()},"peak_memory":"not_measured_by_this_headless_probe","scope":"CPU hover queries only, excludes GPU/frame scheduling/AI; cold preparation separate; geometry/revision unchanged"})
    );
}

#[test]
#[ignore = "explicit read-only protected input for bounded comparison preparation measurement"]
fn candidate_comparison_cpu_call_chain_cold_and_hot() {
    use atelier_bim::render::CpuSceneCache;
    let path = std::path::PathBuf::from(
        std::env::var_os("OPENARCHI_COMPARISON_INPUT").expect("protected input"),
    );
    let editor = Editor::from_stored_document(&path).unwrap();
    let p = &editor.project;
    let compare = p.design_process.task().unwrap().compare(&p).unwrap();
    let ids = compare
        .candidates
        .each_ref()
        .map(|c| c.elements.iter().map(|e| e.id).collect::<Vec<_>>());
    let mut scenes = [CpuSceneCache::default(), CpuSceneCache::default()];
    let mut samples = vec![];
    for _ in 0..101 {
        let start = std::time::Instant::now();
        editor
            .with_geometry_read(|read| {
                for j in 0..2 {
                    let s = scenes[j].prepare_comparison(p, &ids[j], read);
                    assert!(s.diagnostics.is_empty());
                    assert!(s.triangle_count() > 0);
                    std::hint::black_box(s);
                }
            })
            .unwrap();
        samples.push(start.elapsed().as_nanos());
    }
    let cold = samples.remove(0);
    samples.sort();
    use sha2::{Digest, Sha256};
    let sha = format!("{:x}", Sha256::digest(std::fs::read(&path).unwrap()));
    eprintln!(
        "COMPARISON_CPU {}",
        serde_json::json!({"input_sha256":sha,"boundary":"GeometryRead preparation + two candidate CPU scenes; no GPU frame, no AI", "cold_ns":cold,"hot_median_ns":samples[50],"hot_max_ns":samples[99],"scene_stats":scenes.each_ref().map(|s|s.stats()),"objects":p.elements.len()})
    );
}
