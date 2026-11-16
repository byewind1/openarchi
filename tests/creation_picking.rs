use atelier_bim::{
    editor::Editor,
    geometry::Camera,
    model::{OpeningKind, Point, Project},
    stairs::StairSpec,
};
use glam::DVec3;
fn camera(x: f64, y: f64, z: f64) -> Camera {
    Camera {
        yaw: -std::f64::consts::FRAC_PI_2,
        pitch: 0.,
        span: 10.,
        target: DVec3::new(x, y, z),
    }
}
#[test]
fn stair_pick_uses_treads_and_soffit_instead_of_full_height_box() {
    for story in [0, 1] {
        let mut e = Editor::new(Project::default());
        e.story = story;
        let spec = StairSpec::default();
        let id = e.place_stair(spec).unwrap();
        let base = e.project.elevation(story);
        for (x, z, expected) in [
            (0.2, 2., None),
            (0.05, -0.04, Some(id)),
            (2.3, 1.5, Some(id)),
        ] {
            let c = camera(x, 0.5, base + z);
            assert_eq!(
                c.pick(&e.project, [0., 0.], 1., Some(story)),
                expected,
                "x={x} z={z} story={story}"
            );
            assert_eq!(c.pick(&e.project, [0., 0.], 1., Some(1 - story)), None);
        }
    }
}
#[test]
fn opening_3d_pick_is_not_occluded_by_its_straight_host() {
    for kind in [OpeningKind::Door, OpeningKind::Window] {
        let mut e = Editor::new(Project::default());
        let wall = e
            .place_wall_segments(vec![(Point::new(0., 0.), Point::new(6., 0.))])
            .unwrap()[0];
        let opening = e
            .place_opening_on_wall(kind, wall, 3., kind.defaults())
            .unwrap();
        for reverse in [false, true] {
            if reverse {
                e.project.elements.reverse();
            }
            assert_eq!(
                camera(3., 0., 1.2).pick(&e.project, [0., 0.], 1., Some(0)),
                Some(opening)
            );
            assert_eq!(
                camera(0.5, 0., 1.2).pick(&e.project, [0., 0.], 1., Some(0)),
                Some(wall)
            );
            assert_eq!(
                camera(3., 0., 2.8).pick(&e.project, [0., 0.], 1., Some(0)),
                Some(wall)
            );
        }
    }
}

#[test]
fn stair_bounds_include_generated_soffit_and_handrails() {
    for story in [0, 1] {
        for run in [0.3, 4.8] {
            for rotated in [false, true] {
                let mut e = Editor::new(Project::default());
                e.story = story;
                let spec = StairSpec {
                    start: Point::new(7., -5.),
                    end: if rotated {
                        Point::new(7., -5. + run)
                    } else {
                        Point::new(7. + run, -5.)
                    },
                    total_rise: 10.,
                    slab_thickness: 0.5,
                    handrail_height: 1.5,
                    ..StairSpec::default()
                };
                e.place_stair(spec.clone()).unwrap();
                let mesh =
                    atelier_bim::stairs::generate(&spec, e.project.elevation(story)).unwrap();
                let bounds = atelier_bim::geometry::corners(&e.project, &e.project.elements[0]);
                let lo = bounds
                    .iter()
                    .copied()
                    .fold(DVec3::splat(f64::INFINITY), DVec3::min);
                let hi = bounds
                    .iter()
                    .copied()
                    .fold(DVec3::splat(f64::NEG_INFINITY), DVec3::max);
                for p in mesh
                    .triangles
                    .iter()
                    .flatten()
                    .chain(mesh.handrails.iter().flatten())
                {
                    assert!(
                        p.cmpge(lo - DVec3::splat(1e-5)).all()
                            && p.cmple(hi + DVec3::splat(1e-5)).all(),
                        "{p:?} outside {lo:?}..{hi:?}"
                    );
                }
                assert!(lo.z < e.project.elevation(story));
                assert!(hi.z > e.project.elevation(story) + spec.total_rise);
            }
        }
    }
}

#[test]
fn hidden_layer_does_not_occlude_visible_created_stair() {
    let mut e = Editor::new(Project::default());
    let visible = e.place_stair(StairSpec::default()).unwrap();
    let hidden = e
        .place_stair(StairSpec {
            start: Point::new(0., -2.),
            end: Point::new(4.8, -2.),
            ..Default::default()
        })
        .unwrap();
    let layer = e.project.layers.new_layer("Hidden front stair", None);
    e.project
        .elements
        .iter_mut()
        .find(|x| x.id == hidden)
        .unwrap()
        .layer = atelier_bim::layers::ElementLayer(layer);
    e.project
        .layers
        .layers
        .iter_mut()
        .find(|x| x.id == layer)
        .unwrap()
        .status
        .visible = false;
    assert_eq!(
        camera(2.3, -1., 1.5).pick(&e.project, [0., 0.], 1., Some(0)),
        Some(visible)
    );
}

#[test]
fn opening_proxy_requires_visible_host_but_visible_locked_objects_remain_selectable() {
    let mut e = Editor::new(Project::default());
    let wall = e
        .place_wall_segments(vec![(Point::new(0., 0.), Point::new(6., 0.))])
        .unwrap()[0];
    let opening = e
        .place_opening_on_wall(OpeningKind::Door, wall, 3., OpeningKind::Door.defaults())
        .unwrap();
    let layer = e.project.layers.new_layer("Host visibility", None);
    e.project
        .elements
        .iter_mut()
        .find(|x| x.id == wall)
        .unwrap()
        .layer = atelier_bim::layers::ElementLayer(layer);
    e.project.layers.layer_mut(layer).unwrap().status.visible = false;
    assert_eq!(
        camera(3., 0., 1.2).pick(&e.project, [0., 0.], 1., Some(0)),
        None
    );
    e.project.layers.layer_mut(layer).unwrap().status.visible = true;
    e.project.layers.layer_mut(layer).unwrap().status.locked = true;
    assert_eq!(
        camera(3., 0., 1.2).pick(&e.project, [0., 0.], 1., Some(0)),
        Some(opening)
    );
    assert_eq!(
        camera(0.5, 0., 1.2).pick(&e.project, [0., 0.], 1., Some(0)),
        Some(wall)
    );
}

#[test]
fn viewport_candidates_match_selection_and_inside_outside_marquee() {
    use atelier_bim::{
        editor::{MarqueeArea, MarqueeScope},
        geometry::ModelViewFilter,
    };
    let mut e = Editor::new(Project::default());
    let rear = e.place_stair(StairSpec::default()).unwrap();
    let front = e
        .place_stair(StairSpec {
            start: Point::new(0., -2.),
            end: Point::new(4.8, -2.),
            ..Default::default()
        })
        .unwrap();
    let c = camera(2.3, -1., 1.5);
    let selected = [rear];
    let area = MarqueeArea::new(
        vec![
            Point::new(-1., -0.5),
            Point::new(6., -0.5),
            Point::new(6., 2.),
            Point::new(-1., 2.),
        ],
        MarqueeScope::CurrentStory,
        0,
    )
    .unwrap();
    let base = ModelViewFilter::default();
    for (filter, expected) in [
        (base, Some(front)),
        (
            ModelViewFilter {
                selected: &selected,
                selection_only: true,
                ..base
            },
            Some(rear),
        ),
        (
            ModelViewFilter {
                selection_only: true,
                ..base
            },
            None,
        ),
        (
            ModelViewFilter {
                marquee: Some(&area),
                ..base
            },
            Some(rear),
        ),
        (
            ModelViewFilter {
                marquee: Some(&area),
                outside_marquee: true,
                ..base
            },
            Some(front),
        ),
        (
            ModelViewFilter {
                story: Some(1),
                ..base
            },
            None,
        ),
    ] {
        assert_eq!(c.pick_in_view(&e.project, [0., 0.], 1., filter), expected);
    }
    let mut above = area.clone();
    above.z_range = Some((10., 11.));
    assert_eq!(
        c.pick_in_view(
            &e.project,
            [0., 0.],
            1.,
            ModelViewFilter {
                marquee: Some(&above),
                ..base
            }
        ),
        None
    );
}

#[test]
fn clipped_stair_surface_pick_rejects_removed_body_and_keeps_cut_cap() {
    use atelier_bim::{
        editor::{MarqueeArea, MarqueeScope},
        geometry::ModelViewFilter,
    };
    let mut e = Editor::new(Project::default());
    let id = e.place_stair(StairSpec::default()).unwrap();
    let area = MarqueeArea::new(
        vec![
            Point::new(3., -1.),
            Point::new(6., -1.),
            Point::new(6., 2.),
            Point::new(3., 2.),
        ],
        MarqueeScope::CurrentStory,
        0,
    )
    .unwrap();
    let filter = ModelViewFilter {
        marquee: Some(&area),
        ..Default::default()
    };
    assert_eq!(
        camera(2.3, 0.5, 1.5).pick_in_view(&e.project, [0., 0.], 1., filter),
        None
    );
    let cap = Camera {
        yaw: std::f64::consts::PI,
        pitch: 0.,
        span: 10.,
        target: DVec3::new(3., 0.5, 2.),
    };
    assert_eq!(cap.pick_in_view(&e.project, [0., 0.], 1., filter), Some(id));
    let mut z = area.clone();
    z.z_range = Some((2.3, 3.));
    assert_eq!(
        cap.pick_in_view(
            &e.project,
            [0., 0.],
            1.,
            ModelViewFilter {
                marquee: Some(&z),
                ..filter
            }
        ),
        None
    );
}

#[test]
fn clipped_wall_preserves_opening_proxy_and_rejects_removed_wall_body() {
    use atelier_bim::{
        editor::{MarqueeArea, MarqueeScope},
        geometry::ModelViewFilter,
    };
    let mut e = Editor::new(Project::default());
    let wall = e
        .place_wall_segments(vec![(Point::new(0., 0.), Point::new(6., 0.))])
        .unwrap()[0];
    let door = e
        .place_opening_on_wall(OpeningKind::Door, wall, 4., OpeningKind::Door.defaults())
        .unwrap();
    let area = MarqueeArea::new(
        vec![
            Point::new(3., -1.),
            Point::new(6., -1.),
            Point::new(6., 1.),
            Point::new(3., 1.),
        ],
        MarqueeScope::CurrentStory,
        0,
    )
    .unwrap();
    let filter = ModelViewFilter {
        marquee: Some(&area),
        ..Default::default()
    };
    assert_eq!(
        camera(0.5, 0., 1.).pick_in_view(&e.project, [0., 0.], 1., filter),
        None
    );
    assert_eq!(
        camera(4., 0., 1.).pick_in_view(&e.project, [0., 0.], 1., filter),
        Some(door)
    );
    assert_eq!(
        camera(4., 0., 2.8).pick_in_view(&e.project, [0., 0.], 1., filter),
        Some(wall)
    );
    let cap = Camera {
        yaw: std::f64::consts::PI,
        pitch: 0.,
        span: 10.,
        target: DVec3::new(3., 0., 2.8),
    };
    assert_eq!(
        cap.pick_in_view(&e.project, [0., 0.], 1., filter),
        Some(wall)
    );
}

#[test]
fn clipped_column_and_beam_exclude_removed_half_and_pick_exposed_cap() {
    use atelier_bim::{
        editor::{MarqueeArea, MarqueeScope},
        geometry::{self, ModelViewFilter},
    };
    for beam in [false, true] {
        let mut e = Editor::new(Project::default());
        let id = if beam {
            e.place_beam(Point::new(-2., 0.), Point::new(2., 0.))
                .unwrap()
        } else {
            e.place_column(Point::new(0., 0.)).unwrap()
        };
        let bounds = geometry::corners(&e.project, &e.project.elements[0]);
        let z = (bounds[0].z + bounds[4].z) / 2.;
        let area = MarqueeArea::new(
            vec![
                Point::new(0., -2.),
                Point::new(5., -2.),
                Point::new(5., 2.),
                Point::new(0., 2.),
            ],
            MarqueeScope::CurrentStory,
            0,
        )
        .unwrap();
        let filter = ModelViewFilter {
            marquee: Some(&area),
            ..Default::default()
        };
        assert_eq!(
            camera(-0.05, 0., z).pick(&e.project, [0., 0.], 1., Some(0)),
            Some(id)
        );
        assert_eq!(
            camera(-0.05, 0., z).pick_in_view(&e.project, [0., 0.], 1., filter),
            None
        );
        let cap = Camera {
            yaw: std::f64::consts::PI,
            pitch: 0.,
            span: 10.,
            target: DVec3::new(0., 0., z),
        };
        assert_eq!(cap.pick_in_view(&e.project, [0., 0.], 1., filter), Some(id));
    }
}

#[test]
fn positive_thin_slab_clip_remains_pickable() {
    use atelier_bim::{
        editor::{MarqueeArea, MarqueeScope},
        geometry::ModelViewFilter,
        model::SlabContour,
    };
    let mut e = Editor::new(Project::default());
    let points = vec![
        Point::new(0., 0.),
        Point::new(4., 0.),
        Point::new(4., 4.),
        Point::new(0., 4.),
    ];
    let id = e
        .place_slab_contour(SlabContour {
            points: points.clone(),
            ..Default::default()
        })
        .unwrap();
    let mut area = MarqueeArea::new(points, MarqueeScope::CurrentStory, 0).unwrap();
    area.z_range = Some((-0.100005, -0.1));
    let c = Camera {
        yaw: 0.,
        pitch: 1.55,
        span: 10.,
        target: DVec3::new(2., 2., -0.1),
    };
    assert_eq!(
        c.pick_in_view(
            &e.project,
            [0., 0.],
            1.,
            ModelViewFilter {
                marquee: Some(&area),
                ..Default::default()
            }
        ),
        Some(id)
    );
}

#[test]
fn clipped_slab_and_roof_preserve_holes_and_concave_clip_voids() {
    use atelier_bim::{
        editor::{MarqueeArea, MarqueeScope},
        geometry::ModelViewFilter,
        model::SlabContour,
        roof::RoofSpec,
        roof_contour::RoofContour,
    };
    let p = Point::new;
    let outer = vec![p(0., 0.), p(6., 0.), p(6., 6.), p(0., 6.)];
    let hole = vec![p(1., 1.), p(2., 1.), p(2., 2.), p(1., 2.)];
    let clip = vec![
        p(0., 0.),
        p(6., 0.),
        p(6., 3.),
        p(3., 3.),
        p(3., 6.),
        p(0., 6.),
    ];
    for roof in [false, true] {
        let mut e = Editor::new(Project::default());
        let id = if roof {
            e.place_roof(RoofSpec {
                contour: Some(RoofContour {
                    outer: outer.clone(),
                    holes: vec![hole.clone()],
                }),
                pivot_start: p(0., 0.),
                pivot_end: p(6., 0.),
                rise_left: true,
                min: p(0., 0.),
                max: p(6., 6.),
                pitch: 10.,
                thickness: 0.3,
                pivot_offset: 0.,
            })
            .unwrap()
        } else {
            e.place_slab_contour(SlabContour {
                points: outer.clone(),
                holes: vec![SlabContour {
                    points: hole.clone(),
                    ..Default::default()
                }],
                ..Default::default()
            })
            .unwrap()
        };
        let area = MarqueeArea::new(clip.clone(), MarqueeScope::CurrentStory, 0).unwrap();
        for (x, y, expected) in [
            (1.5, 1.5, None),
            (4., 4., None),
            (4., 1., Some(id)),
            (1., 4., Some(id)),
        ] {
            let z = if roof {
                y * 10_f64.to_radians().tan() + 0.15
            } else {
                -0.1
            };
            let c = Camera {
                yaw: 0.,
                pitch: 1.55,
                span: 10.,
                target: DVec3::new(x, y, z),
            };
            assert_eq!(
                c.pick_in_view(
                    &e.project,
                    [0., 0.],
                    1.,
                    ModelViewFilter {
                        marquee: Some(&area),
                        ..Default::default()
                    }
                ),
                expected,
                "roof={roof}, x={x}, y={y}"
            );
        }
    }
}

#[test]
fn reference_clip_preserves_disjoint_segments_and_boundary_anchors() {
    use atelier_bim::{
        editor::{MarqueeArea, MarqueeScope},
        geometry::ModelViewFilter,
    };
    let p = Point::new;
    let mut area = MarqueeArea::new(
        vec![
            p(0.5, -1.),
            p(1., -1.),
            p(1., 2.),
            p(3., 2.),
            p(3., -1.),
            p(4., -1.),
            p(4., 3.),
            p(0.5, 3.),
        ],
        MarqueeScope::CurrentStory,
        0,
    )
    .unwrap();
    area.z_range = Some((1., 3.));
    let filter = ModelViewFilter {
        marquee: Some(&area),
        ..Default::default()
    };
    let segments = filter.reference_segments(p(0., 0.), p(5., 0.), 3.);
    assert_eq!(segments.len(), 2);
    for [a, b] in segments {
        assert!((a.x >= 0.5 && b.x <= 1.) || (a.x >= 3. && b.x <= 4.));
    }
    assert!(filter.contains_anchor(p(0.5, 0.), 3.));
    assert!(!filter.contains_anchor(p(2., 0.), 3.));
    assert!(!filter.contains_anchor(p(0., 0.), 3.));
    assert!(
        filter
            .reference_segments(p(0., 0.), p(5., 0.), 4.)
            .is_empty()
    );
    let outside = ModelViewFilter {
        outside_marquee: true,
        ..filter
    };
    assert_eq!(
        outside.reference_segments(p(0., 0.), p(5., 0.), 4.),
        vec![[p(0., 0.), p(5., 0.)]]
    );
}

#[test]
fn viewport_z_interval_order_matches_clipping_for_inside_and_outside() {
    use atelier_bim::{
        editor::{MarqueeArea, MarqueeScope},
        geometry::{ModelViewFilter, element_fully_inside_volume, element_overlaps_z_range},
    };
    let mut e = Editor::new(Project::default());
    let id = e.place_stair(StairSpec::default()).unwrap();
    let before = e.project.clone();
    let history = e.undo.len();
    let mut area = MarqueeArea::new(
        vec![
            Point::new(-1., -1.),
            Point::new(6., -1.),
            Point::new(6., 2.),
            Point::new(-1., 2.),
        ],
        MarqueeScope::CurrentStory,
        0,
    )
    .unwrap();
    let c = camera(2.3, 0.5, 1.5);
    for range in [(-5., 10.), (10., -5.)] {
        area.z_range = Some(range);
        let element = &e.project.elements[0];
        assert!(
            element_overlaps_z_range(&e.project, element, range),
            "{range:?}"
        );
        assert!(element_fully_inside_volume(
            &e.project,
            element,
            &area.vertices,
            Some(range)
        ));
        let filter = ModelViewFilter {
            marquee: Some(&area),
            ..Default::default()
        };
        assert_eq!(c.pick_in_view(&e.project, [0., 0.], 1., filter), Some(id));
        assert_eq!(
            c.pick_in_view(
                &e.project,
                [0., 0.],
                1.,
                ModelViewFilter {
                    outside_marquee: true,
                    ..filter
                }
            ),
            None
        );
    }
    for range in [(10., 11.), (11., 10.)] {
        area.z_range = Some(range);
        assert_eq!(
            c.pick_in_view(
                &e.project,
                [0., 0.],
                1.,
                ModelViewFilter {
                    marquee: Some(&area),
                    ..Default::default()
                }
            ),
            None
        );
    }
    for range in [(f64::NAN, 10.), (-5., f64::NAN)] {
        let element = &e.project.elements[0];
        assert!(!element_overlaps_z_range(&e.project, element, range));
        assert!(!element_fully_inside_volume(
            &e.project,
            element,
            &area.vertices,
            Some(range)
        ));
    }
    assert_eq!(e.project, before);
    assert_eq!(e.undo.len(), history);
}

#[test]
fn outside_marquee_keeps_wall_crossing_a_concave_notch() {
    use atelier_bim::{
        editor::{MarqueeArea, MarqueeScope},
        geometry::{self, ModelViewFilter},
    };
    let mut e = Editor::new(Project::default());
    let id = e
        .place_wall_segments(vec![(Point::new(0., 1.), Point::new(6., 1.))])
        .unwrap()[0];
    let notch = vec![
        Point::new(-1., -1.),
        Point::new(7., -1.),
        Point::new(7., 3.),
        Point::new(4., 3.),
        Point::new(4., 0.),
        Point::new(2., 0.),
        Point::new(2., 3.),
        Point::new(-1., 3.),
    ];
    let wall = &e.project.elements[0];
    assert!(
        geometry::element_plan_outline(&e.project, wall)
            .iter()
            .all(|p| geometry::contains(*p, &notch))
    );
    for vertices in [notch.clone(), notch.into_iter().rev().collect()] {
        assert!(!geometry::element_fully_inside_polygon(
            &e.project, wall, &vertices
        ));
        let area = MarqueeArea::new(vertices, MarqueeScope::CurrentStory, 0).unwrap();
        assert_eq!(
            camera(3., 1., 1.).pick_in_view(
                &e.project,
                [0., 0.],
                1.,
                ModelViewFilter {
                    marquee: Some(&area),
                    outside_marquee: true,
                    ..Default::default()
                }
            ),
            Some(id)
        );
    }
    let cover = vec![
        Point::new(-1., -1.),
        Point::new(7., -1.),
        Point::new(7., 3.),
        Point::new(-1., 3.),
    ];
    assert!(geometry::element_fully_inside_polygon(
        &e.project, wall, &cover
    ));
}

#[test]
fn full_containment_checks_open_curved_paths_and_slanted_column_projection() {
    use atelier_bim::{column_inclination::ColumnInclination, geometry, model::Point};
    let p = Point::new;
    let notch = vec![
        p(-1., -1.),
        p(7., -1.),
        p(7., 3.),
        p(4., 3.),
        p(4., 0.),
        p(2., 0.),
        p(2., 3.),
        p(-1., 3.),
    ];
    let mut e = Editor::new(Project::default());
    let straight = e
        .place_polyline(vec![p(0., 1.), p(6., 1.)], vec![])
        .unwrap();
    let open = e
        .place_polyline(vec![p(1., 2.), p(1., -0.5), p(5., -0.5), p(5., 2.)], vec![])
        .unwrap();
    let arc = geometry::arc_segment_through(p(0., 1.), p(6., 1.), p(3., 4.)).unwrap();
    let curved = e
        .place_polyline(vec![p(0., 1.), p(6., 1.)], vec![Some(arc)])
        .unwrap();
    e.column_slanted = true;
    e.column_height = 6.;
    e.column_inclination = ColumnInclination {
        slant_degrees: 45.,
        direction_degrees: 0.,
    };
    let column = e.place_column(p(0., 1.)).unwrap();
    let get = |id| {
        e.project
            .elements
            .iter()
            .find(|element| element.id == id)
            .unwrap()
    };
    assert!(!geometry::element_fully_inside_polygon(
        &e.project,
        get(straight),
        &notch
    ));
    assert!(geometry::element_fully_inside_polygon(
        &e.project,
        get(open),
        &notch
    ));
    assert!(!geometry::element_fully_inside_polygon(
        &e.project,
        get(column),
        &notch
    ));
    let cover = vec![p(-1., -1.), p(7., -1.), p(7., 2.), p(-1., 2.)];
    assert!(geometry::element_fully_inside_polygon(
        &e.project,
        get(straight),
        &cover
    ));
    assert!(!geometry::element_fully_inside_polygon(
        &e.project,
        get(curved),
        &cover
    ));
    // Boundary-coincident segments and the existing point tolerance stay accepted.
    let edge = e
        .project
        .elements
        .iter()
        .find(|element| element.id == straight)
        .unwrap();
    for top in [1., 1. - 0.000005] {
        let cover = vec![p(-1., -1.), p(7., -1.), p(7., top), p(-1., top)];
        assert!(geometry::element_fully_inside_polygon(
            &e.project, edge, &cover
        ));
    }
    assert!(!geometry::element_fully_inside_polygon(
        &e.project,
        edge,
        &[]
    ));
}

#[test]
fn roof_volume_filter_uses_actual_contour_elevations_not_virtual_box_corners() {
    use atelier_bim::{
        editor::{MarqueeArea, MarqueeScope},
        geometry::{self, ModelViewFilter},
        roof::{self, RoofSpec},
        roof_contour::RoofContour,
    };
    let p = Point::new;
    for story in [0, 1] {
        for pitch in [-45., 45.] {
            let mut e = Editor::new(Project::default());
            e.story = story;
            let spec = RoofSpec {
                contour: Some(RoofContour {
                    outer: vec![p(0., 2.), p(2., 0.), p(4., 2.), p(2., 4.)],
                    holes: vec![],
                }),
                pivot_start: p(0., 0.),
                pivot_end: p(4., 4.),
                rise_left: true,
                min: p(0., 0.),
                max: p(4., 4.),
                pitch,
                thickness: 0.2,
                pivot_offset: 0.,
            };
            e.place_roof(spec.clone()).unwrap();
            let mesh = roof::mesh(&spec, e.project.elevation(story), None, None).unwrap();
            let low = mesh
                .triangles
                .iter()
                .flatten()
                .map(|v| v.z)
                .fold(f64::INFINITY, f64::min);
            let high = mesh
                .triangles
                .iter()
                .flatten()
                .map(|v| v.z)
                .fold(f64::NEG_INFINITY, f64::max);
            let element = &e.project.elements[0];
            let mut area = MarqueeArea::new(
                vec![p(-1., -1.), p(5., -1.), p(5., 5.), p(-1., 5.)],
                MarqueeScope::CurrentStory,
                story,
            )
            .unwrap();
            area.z_range = Some((low - 0.01, high + 0.01));
            assert!(
                geometry::element_fully_inside_volume(
                    &e.project,
                    element,
                    &area.vertices,
                    area.z_range
                ),
                "story={story} pitch={pitch}"
            );
            assert!(!geometry::element_overlaps_z_range(
                &e.project,
                element,
                (high + 0.1, high + 0.2)
            ));
            let filter = ModelViewFilter {
                marquee: Some(&area),
                outside_marquee: true,
                ..Default::default()
            };
            assert!(!filter.includes(&e.project, element));
            let c = camera(2., 2., e.project.elevation(story) + 0.1);
            assert_eq!(c.pick_in_view(&e.project, [0., 0.], 1., filter), None);
            assert_eq!(
                c.pick_in_view(
                    &e.project,
                    [0., 0.],
                    1.,
                    ModelViewFilter {
                        outside_marquee: false,
                        ..filter
                    }
                ),
                Some(element.id)
            );

            assert!(
                ModelViewFilter {
                    outside_marquee: false,
                    ..filter
                }
                .includes(&e.project, element)
            );
        }
    }
}

#[test]
fn large_camera_depth_interval_keeps_finite_inverse_and_surface_picking() {
    use atelier_bim::{editor::Editor, model::Project};
    let mut e = Editor::new(Project::default());
    e.column_height = 20.;
    e.column_section = atelier_bim::column::ColumnSection::Rectangular {
        width: 100.,
        depth: 100.,
    };
    let id = e
        .place_column(atelier_bim::model::Point::new(30000., 30000.))
        .unwrap();
    for span in [1000., 30000., 100000.] {
        for aspect in [0.5, 1., 2.] {
            let camera = Camera {
                span,
                target: glam::DVec3::new(30000., 30000., 10.),
                ..Default::default()
            };
            let projected = camera.matrix(aspect).project_point3(camera.target);
            assert!(projected.z > 0. && projected.z < 1.);
            assert!(camera.matrix(aspect).inverse().is_finite());
            assert_eq!(camera.pick(&e.project, [0., 0.], aspect, Some(0)), Some(id));
        }
    }
}
