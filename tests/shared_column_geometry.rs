use atelier_bim::{
    column::{ColumnSection, ColumnSolid},
    column_geometry::{self, ColumnSource},
    column_inclination::{ColumnInclination, InclinedColumnSolid},
    derived_geometry::inspect_closed_surface,
    editor::Editor,
    model::{ModelAdapter, Point, Project, Shape},
    storage::JsonAdapter,
};
fn p(x: f64, y: f64) -> Point {
    Point::new(x, y)
}
fn source(offset: f64, circle: bool, slant: f64) -> ColumnSource {
    let column = ColumnSolid {
        center: p(offset, offset),
        bottom: 1.,
        height: 3.,
        angle_degrees: 35.,
        section: if circle {
            ColumnSection::Circular { diameter: 1. }
        } else {
            ColumnSection::Rectangular {
                width: 0.4,
                depth: 0.6,
            }
        },
    };
    if slant == 90. {
        ColumnSource::Vertical(column)
    } else {
        ColumnSource::Inclined(InclinedColumnSolid {
            column,
            slant_degrees: slant,
            direction_degrees: -37.,
        })
    }
}
#[test]
fn column_snapshots_preserve_analytic_bounds_section_volume_and_closed_sampled_surfaces() {
    for offset in [0., 99900., -99900.] {
        for circle in [false, true] {
            for slant in [30., 60., 90.] {
                let src = source(offset, circle, slant);
                let g = column_geometry::snapshot(src).unwrap();
                let area = if circle {
                    std::f64::consts::PI / 4.
                } else {
                    0.24
                };
                let volume = area * 3. / slant.to_radians().sin();
                assert!((g.normal_section_area_m2 - area).abs() < 1e-12);
                assert!((g.volume_m3 - volume).abs() < 1e-12);
                assert!(
                    (g.horizontal_section_area_m2 - area / slant.to_radians().sin()).abs() < 1e-12
                );
                let metrics = inspect_closed_surface(&g.mesh.triangles).unwrap();
                assert_eq!(metrics.bounds, g.sampled_bounds);
                if circle {
                    assert!((metrics.signed_volume_m3 - volume).abs() / volume < 0.0015);
                } else {
                    assert!((metrics.signed_volume_m3 - volume).abs() < 1e-8);
                }
                for v in g.mesh.triangles.iter().flatten() {
                    assert!(v.cmpge(g.bounds[0] - 1e-9).all() && v.cmple(g.bounds[1] + 1e-9).all());
                }
                assert_eq!(g, column_geometry::snapshot(src).unwrap());
                assert!(g.section_at(0.).is_err());
                assert!(!g.section_at(2.).unwrap().is_empty());
            }
        }
    }
}
#[test]
fn shared_column_clips_remain_closed_across_rectangular_circular_and_inclined_sources() {
    for offset in [0., 99900., -99900.] {
        for circle in [false, true] {
            for slant in [60., 90.] {
                let g = column_geometry::snapshot(source(offset, circle, slant)).unwrap();
                let clip = [
                    p(offset, offset - 2.),
                    p(offset + 2., offset - 2.),
                    p(offset + 2., offset + 2.),
                    p(offset, offset + 2.),
                ];
                let mesh = g
                    .clipped(Some(&clip), Some((1.7, 2.8)))
                    .unwrap_or_else(|e| {
                        panic!("offset={offset} circle={circle} slant={slant}: {e}")
                    });
                inspect_closed_surface(&mesh.triangles).unwrap();
                assert!(
                    g.clipped(None, Some((10., 11.)))
                        .unwrap()
                        .triangles
                        .is_empty()
                );
                assert!(g.clipped(Some(&[]), None).is_err());
            }
        }
    }
}
#[test]
fn concave_and_disconnected_column_clips_remain_closed_without_partition_faces() {
    let clips = [
        (
            vec![
                p(-2., -2.),
                p(2., -2.),
                p(2., 0.),
                p(0., 0.),
                p(0., 2.),
                p(-2., 2.),
            ],
            12.,
        ),
        (
            vec![
                p(-3., -3.),
                p(-1., -3.),
                p(-1., 3.),
                p(1., 3.),
                p(1., -3.),
                p(3., -3.),
                p(3., 4.),
                p(-3., 4.),
            ],
            8.,
        ),
    ];
    for offset in [0., 99900., -99900.] {
        for (clip, area) in &clips {
            let src = ColumnSource::Vertical(ColumnSolid {
                center: p(offset, offset),
                bottom: 0.,
                height: 3.,
                angle_degrees: 0.,
                section: ColumnSection::Rectangular {
                    width: 4.,
                    depth: 4.,
                },
            });
            let g = column_geometry::snapshot(src).unwrap();
            let clip: Vec<_> = clip
                .iter()
                .map(|p| Point::new(p.x + offset, p.y + offset))
                .collect();
            let result = g
                .clipped_with_report(Some(&clip), Some((0.5, 2.5)))
                .unwrap_or_else(|e| panic!("offset={offset}, area={area}: {e}"));
            assert!(
                result.normalization.max_displacement_m <= result.normalization.merge_distance_m
            );
            assert!(
                result.normalization.max_edge_deviation_m <= result.normalization.merge_distance_m
            );
            assert_eq!(
                result.normalization.algorithm_version,
                atelier_bim::column_clip::CLIP_ALGORITHM_VERSION
            );
            let m = result.mesh;
            let volume = inspect_closed_surface(&m.triangles)
                .unwrap()
                .signed_volume_m3;
            assert!((volume - area * 2.).abs() < 1e-8);
        }
    }
}
#[test]
fn column_plan_queries_bind_view_context_separately_from_physical_identity() {
    use atelier_bim::{column_geometry::ColumnPlanView, command::ModelCommand};
    let mut e = Editor::new(Project::default());
    let id = e
        .place_column_with_inclination(
            p(0., 0.),
            ColumnSection::Circular { diameter: 0.4 },
            3.,
            Some(ColumnInclination {
                slant_degrees: 60.,
                direction_degrees: 20.,
            }),
        )
        .unwrap();
    let initial = e.column_plan_geometry(id, 0).unwrap();
    assert!(matches!(initial.view, ColumnPlanView::Projected { .. }));
    assert_eq!(initial.identity.input_revision, Some(e.revision));
    assert_eq!(
        initial.source_identity.document_session,
        Some(e.document_session())
    );
    let mut plan = e.project.plan_options.clone();
    plan.floor_plan_cut.height = 1.7;
    e.apply(ModelCommand::SetOptions {
        plan: Some(plan),
        design: None,
        guide: None,
        grid: None,
    })
    .unwrap();
    let changed = e.column_plan_geometry(id, 0).unwrap();
    assert_eq!(
        initial.source_identity.input_fingerprint,
        changed.source_identity.input_fingerprint
    );
    assert_ne!(
        initial.identity.input_fingerprint,
        changed.identity.input_fingerprint
    );
    assert_ne!(initial.view, changed.view);
    e.undo();
    assert_eq!(e.column_plan_geometry(id, 0).unwrap().view, initial.view);
    e.redo();
    let vertical = e
        .place_column_section(
            p(3., 0.),
            ColumnSection::Rectangular {
                width: 0.4,
                depth: 0.6,
            },
            0.2,
        )
        .unwrap();
    assert!(matches!(
        e.column_plan_geometry(vertical, 0).unwrap().view,
        ColumnPlanView::SymbolicFootprint(_)
    ));
    assert_eq!(
        e.column_plan_geometry(vertical, 1).unwrap().view,
        ColumnPlanView::NotOnStory
    );
    assert_eq!(
        e.column_plan_geometry(id, 999).unwrap_err().code,
        "INVALID_COLUMN_PLAN_CONTEXT"
    );
}
#[test]
fn physical_circle_ray_keeps_exact_section_at_points_outside_display_chords() {
    let c = ColumnSolid {
        center: p(0., 0.),
        bottom: 0.,
        height: 3.,
        angle_degrees: 0.,
        section: ColumnSection::Circular { diameter: 1. },
    };
    let g = column_geometry::snapshot(ColumnSource::Vertical(c)).unwrap();
    // Radius 0.49975 at the midpoint of a 5-degree chord lies outside the
    // regular 72-sided display polygon, but inside the exact 0.5 m radius.
    let angle = 2.5_f64.to_radians();
    let radius = 0.49975;
    assert!(radius > 0.5 * angle.cos());
    let ray = glam::DVec3::new(radius * angle.cos(), radius * angle.sin(), 5.);
    assert_eq!(g.ray_hit(ray, -glam::DVec3::Z), Some(2.));
}
#[test]
fn column_minimum_mesh_and_analytic_ray_are_valid_at_large_origins() {
    for offset in [0., 99900., -99900.] {
        for circle in [false, true] {
            let mut c = source(offset, circle, 90.).column();
            c.height = 0.01;
            c.angle_degrees = 0.;
            c.section = if circle {
                ColumnSection::Circular { diameter: 0.01 }
            } else {
                ColumnSection::Rectangular {
                    width: 0.01,
                    depth: 0.01,
                }
            };
            let g = column_geometry::snapshot(ColumnSource::Vertical(c)).unwrap();
            assert!(
                g.ray_hit(glam::DVec3::new(offset, offset, 2.), -glam::DVec3::Z)
                    .is_some()
            );
            assert!(
                g.ray_hit(
                    glam::DVec3::new(offset + 0.02, offset + 0.02, 2.),
                    -glam::DVec3::Z
                )
                .is_none()
            );
            inspect_closed_surface(&g.mesh.triangles).unwrap();
        }
    }
}
#[test]
fn editor_column_query_keeps_physical_identity_across_plan_changes_and_restores_geometry_exactly() {
    let mut e = Editor::new(Project::default());
    let id = e
        .place_column_section(p(0., 0.), ColumnSection::Circular { diameter: 0.4 }, 3.)
        .unwrap();
    let mut el = e.project.elements[0].clone();
    if let Shape::Column { inclination, .. } = &mut el.shape {
        *inclination = Some(ColumnInclination {
            slant_degrees: 60.,
            direction_degrees: 20.,
        });
    }
    e.replace_element(el).unwrap();
    let state = format!(
        "{:?}/{:?}/{:?}/{:?}/{:?}",
        e.project,
        e.undo,
        e.redo,
        e.revision,
        e.selection_ids()
    );
    let g = e.column_geometry(id).unwrap();
    assert_eq!(g.identity.input_revision, Some(e.revision));
    assert_eq!(g.identity.document_session, Some(e.document_session()));
    assert_eq!(
        state,
        format!(
            "{:?}/{:?}/{:?}/{:?}/{:?}",
            e.project,
            e.undo,
            e.redo,
            e.revision,
            e.selection_ids()
        )
    );
    let mut el = e.project.elements[0].clone();
    if let Shape::Column { plan, .. } = &mut el.shape {
        plan.display = atelier_bim::column_plan::ColumnPlanDisplay::CutOnly;
    }
    e.replace_element(el).unwrap();
    assert_eq!(
        g.identity.input_fingerprint,
        e.column_geometry(id).unwrap().identity.input_fingerprint
    );
    assert_ne!(
        g.identity.input_revision,
        e.column_geometry(id).unwrap().identity.input_revision
    );
    let mut el = e.project.elements[0].clone();
    if let Shape::Column { height, .. } = &mut el.shape {
        *height = 4.;
    }
    e.replace_element(el).unwrap();
    assert_ne!(
        g.identity.input_fingerprint,
        e.column_geometry(id).unwrap().identity.input_fingerprint
    );
    e.undo();
    assert_eq!(g.geometry, e.column_geometry(id).unwrap().geometry);
    e.redo();
    let reopened = Editor::new(
        JsonAdapter
            .import(&JsonAdapter.export(&e.project).unwrap())
            .unwrap(),
    );
    assert_eq!(
        reopened.column_geometry(id).unwrap().geometry,
        e.column_geometry(id).unwrap().geometry
    );
    assert_ne!(reopened.document_session(), e.document_session());
    assert_eq!(
        e.column_geometry(999).unwrap_err().code,
        "ELEMENT_NOT_FOUND"
    );
}
