use atelier_bim::{model::Point, roof_contour::RoofContour};
fn ring(p: &[(f64, f64)]) -> Vec<Point> {
    p.iter().map(|&(x, y)| Point::new(x, y)).collect()
}
fn sample() -> RoofContour {
    RoofContour {
        outer: ring(&[(0., 0.), (6., 0.), (6., 2.), (2., 2.), (2., 6.), (0., 6.)]),
        holes: vec![ring(&[(0.5, 0.5), (1.5, 0.5), (1.5, 1.5), (0.5, 1.5)])],
    }
}
#[test]
fn concave_roof_with_hole_preserves_area_hit_triangulation_and_json_in_both_windings() {
    for reversed in [false, true] {
        let mut c = sample();
        if reversed {
            c.outer.reverse();
            c.holes[0].reverse();
        }
        c.validate().unwrap();
        assert!((c.area() - 19.).abs() < 1e-8);
        assert!(!c.contains(Point::new(4., 4.)));
        assert!(!c.contains(Point::new(1., 1.)));
        assert!(c.contains(Point::new(1., 3.)));
        let area: f64 = c
            .triangles()
            .iter()
            .map(|t| {
                ((t[1].x - t[0].x) * (t[2].y - t[0].y) - (t[2].x - t[0].x) * (t[1].y - t[0].y))
                    .abs()
                    / 2.
            })
            .sum();
        assert!((area - 19.).abs() < 1e-5);
        for t in c.triangles() {
            assert!(c.contains(Point::new(
                (t[0].x + t[1].x + t[2].x) / 3.,
                (t[0].y + t[1].y + t[2].y) / 3.
            )));
        }
        let saved = serde_json::to_string(&c).unwrap();
        assert_eq!(serde_json::from_str::<RoofContour>(&saved).unwrap(), c);
        c.translate(Point::new(10., -2.));
        c.validate().unwrap();
        assert_eq!(
            c.bounds(),
            Some((Point::new(10., -2.), Point::new(16., 4.)))
        );
        assert!(!c.contains(Point::new(11., -1.)));
        assert!(c.contains(Point::new(11., 1.)));
        assert!((c.area() - 19.).abs() < 1e-8);
    }
}
#[test]
fn roof_contour_rejects_invalid_outer_and_hole_topologies() {
    let mut cases = vec![];
    for outer in [
        vec![],
        ring(&[(0., 0.), (2., 2.), (0., 2.), (2., 0.)]),
        ring(&[(0., 0.), (0., 0.), (2., 2.)]),
        ring(&[(0., 0.), (100000., 0.), (0., 2.)]),
    ] {
        cases.push(RoofContour {
            outer,
            holes: vec![],
        });
    }
    for hole in [
        ring(&[(3., 3.), (4., 3.), (4., 4.), (3., 4.)]),
        ring(&[(0., 0.), (1., 0.), (1., 1.), (0., 1.)]),
    ] {
        let mut c = sample();
        c.holes = vec![hole];
        cases.push(c);
    }
    let mut duplicate = sample();
    duplicate.holes.push(duplicate.holes[0].clone());
    cases.push(duplicate);
    let mut nested = sample();
    nested
        .holes
        .push(ring(&[(0.7, 0.7), (0.9, 0.7), (0.9, 0.9), (0.7, 0.9)]));
    cases.push(nested);
    let mut nan = sample();
    nan.outer[0].x = f64::NAN;
    cases.push(nan);
    for (i, c) in cases.iter().enumerate() {
        assert!(c.validate().is_err(), "case {i}");
    }
}

#[test]
fn sloped_contour_mesh_has_closed_hole_volume_and_clips_without_using_stale_bounds() {
    use atelier_bim::roof::{RoofSpec, mesh_contour};
    let spec = RoofSpec {
        contour: None,
        pivot_start: Point::new(0., 0.),
        pivot_end: Point::new(6., 0.),
        rise_left: true,
        min: Point::new(50., 50.),
        max: Point::new(51., 51.),
        pitch: 30.,
        thickness: 0.2,
        pivot_offset: 3.,
    };
    let volume = |m: &atelier_bim::roof::RoofMesh| {
        m.triangles
            .iter()
            .map(|t| t[0].dot(t[1].cross(t[2])) / 6.)
            .sum::<f64>()
            .abs()
    };
    for reversed in [false, true] {
        let mut c = sample();
        if reversed {
            c.outer.reverse();
            c.holes[0].reverse();
        }
        let original = c.clone();
        let before = spec.clone();
        let m = mesh_contour(&spec, &c, 3.2, None, None).unwrap();
        assert!((volume(&m) - 3.8).abs() < 1e-4, "volume {}", volume(&m));
        let hole = ring(&[(0.7, 0.7), (1.3, 0.7), (1.3, 1.3), (0.7, 1.3)]);
        assert!(
            mesh_contour(&spec, &c, 3.2, Some(&hole), None)
                .unwrap()
                .triangles
                .is_empty()
        );
        let solid = ring(&[(0.5, 2.5), (1.5, 2.5), (1.5, 3.5), (0.5, 3.5)]);
        let clipped = mesh_contour(&spec, &c, 3.2, Some(&solid), None).unwrap();
        assert!((volume(&clipped) - 0.2).abs() < 1e-4);
        let z = mesh_contour(&spec, &c, 3.2, Some(&solid), Some((7.9, 8.0))).unwrap();
        assert!(!z.triangles.is_empty());
        assert!((volume(&z) - 0.02 / 30_f64.to_radians().tan()).abs() < 1e-4);
        assert!(
            z.triangles
                .iter()
                .flatten()
                .all(|p| p.z >= 7.9 - 1e-5 && p.z <= 8.0 + 1e-5)
        );
        assert_eq!(c, original);
        assert_eq!(spec, before);
    }
}

#[test]
fn persisted_roof_contour_creation_history_picking_and_legacy_json() {
    use atelier_bim::{
        command::ModelCommand,
        editor::Editor,
        geometry,
        model::{Project, Shape},
        roof::RoofSpec,
        storage,
    };
    let c = sample();
    let (min, max) = c.bounds().unwrap();
    let spec = RoofSpec {
        contour: Some(c),
        pivot_start: Point::default(),
        pivot_end: Point::new(6., 0.),
        rise_left: true,
        min,
        max,
        pitch: 30.,
        thickness: 0.2,
        pivot_offset: 3.,
    };
    let mut e = Editor::new(Project::default());
    let empty = e.project.clone();
    let id = e.place_roof(spec.clone()).unwrap();
    let created = e.project.clone();
    assert!((created.elements[0].area() - 19. / 30_f64.to_radians().cos()).abs() < 1e-4);
    for p in [Point::new(1., 1.), Point::new(4., 4.)] {
        assert_eq!(geometry::hit_plan(&created, 0, p, 0.01), None);
    }
    assert_eq!(
        geometry::hit_plan(&created, 0, Point::new(1., 3.), 0.01),
        Some(id)
    );
    let hole = ring(&[(0.7, 0.7), (1.3, 0.7), (1.3, 1.3), (0.7, 1.3)]);
    assert!(!geometry::element_intersects_polygon(
        &created,
        &created.elements[0],
        &hole
    ));
    e.undo();
    assert_eq!(e.project, empty);
    e.redo();
    assert_eq!(e.project, created);
    let mut invalid = spec.clone();
    invalid.max.x += 1.;
    let revision = e.revision;
    assert!(e.place_roof(invalid).is_err());
    assert_eq!(e.project, created);
    assert_eq!(e.revision, revision);
    e.apply(ModelCommand::MoveElements {
        ids: vec![id],
        offset: Point::new(10., -2.),
    })
    .unwrap();
    if let Shape::Roof { spec } = &e.project.elements[0].shape {
        assert_eq!(
            spec.contour.as_ref().unwrap().bounds(),
            Some((spec.min, spec.max))
        );
        assert_eq!(spec.pivot_start, Point::new(10., -2.));
    } else {
        panic!()
    }
    let path = std::env::temp_dir().join(format!("oa-roof-contour-{}.json", std::process::id()));
    storage::save(&e.project, &path).unwrap();
    assert_eq!(storage::load(&path).unwrap(), e.project);
    std::fs::remove_file(path).unwrap();
    e.undo();
    assert_eq!(e.project, created);
    let mut legacy = serde_json::to_value(&spec).unwrap();
    legacy.as_object_mut().unwrap().remove("contour");
    let old: RoofSpec = serde_json::from_value(legacy).unwrap();
    assert!(old.contour.is_none());
    old.validate().unwrap();
    assert!(serde_json::to_value(old).unwrap().get("contour").is_none());
}

#[test]
fn explicit_mesh_contour_overrides_existing_spec_contour_without_mutation() {
    use atelier_bim::roof::{RoofSpec, mesh_contour};
    let old = sample();
    let (min, max) = old.bounds().unwrap();
    let spec = RoofSpec {
        contour: Some(old),
        pivot_start: Point::default(),
        pivot_end: Point::new(6., 0.),
        rise_left: true,
        min,
        max,
        pitch: 30.,
        thickness: 0.2,
        pivot_offset: 3.,
    };
    let before = spec.clone();
    let mut new = sample();
    new.translate(Point::new(10., 0.));
    let mesh = mesh_contour(&spec, &new, 0., None, None).unwrap();
    assert!(!mesh.triangles.is_empty());
    assert!(mesh.triangles.iter().flatten().all(|p| p.x >= 10.));
    assert_eq!(spec, before);
}

#[test]
fn diagonal_roof_validation_uses_real_vertices_instead_of_empty_bounds_corners() {
    use atelier_bim::{
        command::ModelCommand,
        editor::Editor,
        model::{Point, Project},
        roof::RoofSpec,
        roof_contour::RoofContour,
    };
    for pitch in [-80., 80.] {
        for rise_left in [false, true] {
            let contour = RoofContour {
                outer: vec![
                    Point::new(0., 0.),
                    Point::new(30000., 30000.),
                    Point::new(30000., 30001.),
                    Point::new(0., 1.),
                ],
                holes: vec![],
            };
            let (min, max) = contour.bounds().unwrap();
            let spec = RoofSpec {
                contour: Some(contour),
                pivot_start: Point::default(),
                pivot_end: Point::new(30000., 30000.),
                rise_left,
                min,
                max,
                pitch,
                thickness: 0.2,
                pivot_offset: 3.,
            };
            assert!(
                spec.outline()
                    .iter()
                    .any(|&p| spec.bottom(p, 0.).abs() > 100000.),
                "fixture exposes phantom corner height"
            );
            assert!(
                spec.contour
                    .as_ref()
                    .unwrap()
                    .outer
                    .iter()
                    .all(|&p| spec.bottom(p, 0.).abs() < 10.)
            );
            assert!(
                spec.validate().is_ok(),
                "actual roof is within the existing range"
            );
            let mut editor = Editor::new(Project::default());
            let before = editor.project.clone();
            let id = editor.place_roof(spec).unwrap();
            let created = editor.project.clone();
            let corners = atelier_bim::geometry::corners(&created, &created.elements[0]);
            assert!(corners.iter().all(|p| p.z.abs() < 10.));
            let atelier_bim::model::Shape::Roof { spec } = &created.elements[0].shape else {
                panic!("roof")
            };
            let mesh = atelier_bim::roof::mesh(spec, 0., None, None).unwrap();
            let mesh_low = mesh
                .triangles
                .iter()
                .flatten()
                .map(|p| p.z)
                .fold(f64::INFINITY, f64::min);
            let mesh_high = mesh
                .triangles
                .iter()
                .flatten()
                .map(|p| p.z)
                .fold(f64::NEG_INFINITY, f64::max);
            assert!((corners[0].z - mesh_low).abs() < 1e-4);
            assert!((corners[4].z - mesh_high).abs() < 1e-4);
            editor
                .apply(ModelCommand::MoveElements {
                    ids: vec![id],
                    offset: Point::new(2., -1.),
                })
                .unwrap();
            let moved = editor.project.clone();
            assert_eq!(
                serde_json::from_str::<Project>(&serde_json::to_string(&moved).unwrap()).unwrap(),
                moved
            );
            editor.undo();
            assert_eq!(editor.project, created);
            editor.undo();
            assert_eq!(editor.project, before);
            editor.redo();
            editor.redo();
            assert_eq!(editor.project, moved);
        }
    }
}

#[test]
fn actual_roof_height_overflow_still_rejects_atomic_creation_and_preserves_redo() {
    use atelier_bim::{
        editor::Editor,
        model::{Point, Project},
        roof::RoofSpec,
        roof_contour::RoofContour,
    };
    let contour = RoofContour {
        outer: vec![
            Point::new(0., 0.),
            Point::new(30000., 30000.),
            Point::new(30000., 30001.),
            Point::new(0., 1.),
        ],
        holes: vec![],
    };
    let (min, max) = contour.bounds().unwrap();
    for explicit in [false, true] {
        let spec = RoofSpec {
            contour: explicit.then_some(contour.clone()),
            pivot_start: Point::default(),
            pivot_end: Point::new(30000., 0.),
            rise_left: true,
            min,
            max,
            pitch: 80.,
            thickness: 0.2,
            pivot_offset: 3.,
        };
        assert!(spec.bottom(max, 0.) > 100000.);
        let mut editor = Editor::new(Project::default());
        editor.place_column(Point::new(1., 1.)).unwrap();
        let redo = editor.project.clone();
        editor.undo();
        let before = editor.project.clone();
        let revision = editor.revision;
        assert!(spec.validate().is_err());
        assert!(editor.place_roof(spec).is_err());
        assert_eq!(editor.project, before);
        assert_eq!(editor.revision, revision);
        editor.redo();
        assert_eq!(editor.project, redo);
    }
}

#[test]
fn concave_holed_roof_copy_preview_and_paste_keep_geometry_at_last_legal_identity() {
    use atelier_bim::{
        editing::EditCommand,
        editor::Editor,
        model::{Project, Shape},
        reshape::{self, ReshapeCommand},
        roof::RoofSpec,
    };
    for paste in [false, true] {
        let contour = sample();
        let (min, max) = contour.bounds().unwrap();
        let spec = RoofSpec {
            contour: Some(contour),
            pivot_start: Point::new(-1., 0.),
            pivot_end: Point::new(7., 1.),
            rise_left: false,
            min,
            max,
            pitch: 30.,
            thickness: 0.3,
            pivot_offset: 2.,
        };
        let mut editor = Editor::new(Project::default());
        let id = editor.place_roof(spec.clone()).unwrap();
        editor.set_selection(vec![id]);
        editor.project.next_id = u64::MAX - 1;
        editor.project.validate().unwrap();
        let before = editor.project.clone();
        let offset = Point::new(10., -2.);
        let command = ReshapeCommand::SelectionEdit {
            ids: vec![id],
            command: EditCommand::Transform {
                offset,
                origin: Point::default(),
                angle: 0.,
                scale: 1.,
                mirror: false,
                copies: 1,
            },
        };
        let (preview, selected) = reshape::candidate(&before, &command).unwrap();
        assert_eq!(editor.project, before);
        assert_eq!(selected, vec![u64::MAX - 1]);
        if paste {
            assert_eq!(editor.copy_selection(), 1);
            assert_eq!(editor.paste(offset), 1);
        } else {
            editor.reshape(command).unwrap();
        }
        assert_eq!(editor.project, preview);
        assert_eq!(editor.project.next_id, u64::MAX);
        assert_eq!(editor.selection_ids(), selected);
        let copy = editor
            .project
            .elements
            .iter()
            .find(|e| e.id == u64::MAX - 1)
            .unwrap();
        let Shape::Roof { spec: copied } = &copy.shape else {
            panic!("roof")
        };
        let mut expected = spec;
        expected.translate(offset);
        assert_eq!(copied, &expected);
        assert_eq!(copied.contour.as_ref().unwrap().holes.len(), 1);
        assert!(
            !copied
                .contour
                .as_ref()
                .unwrap()
                .contains(Point::new(11., -1.)),
            "copied hole remains empty"
        );
        assert!(
            !copied
                .contour
                .as_ref()
                .unwrap()
                .contains(Point::new(14., 2.)),
            "concave notch remains empty"
        );
        assert!(
            copied
                .contour
                .as_ref()
                .unwrap()
                .contains(Point::new(11., 1.))
        );
        let after = editor.project.clone();
        assert_eq!(
            serde_json::from_str::<Project>(&serde_json::to_string(&after).unwrap()).unwrap(),
            after
        );
        editor.undo();
        assert_eq!(editor.project, before);
        editor.redo();
        assert_eq!(editor.project, after);
        // Capacity is now exhausted: clipboard and shared copy must fail atomically.
        editor.copy_selection();
        let revision = editor.revision;
        assert_eq!(editor.paste(offset), 0);
        assert_eq!(editor.project, after);
        assert_eq!(editor.revision, revision);
    }
}

#[test]
fn roof_marquee_translation_uses_whole_real_outline_not_bounds_vertices() {
    use atelier_bim::{
        editor::{Editor, MarqueeArea, MarqueeScope},
        model::{Project, Shape},
        roof::RoofSpec,
    };
    for concave in [true, false] {
        let mut e = Editor::new(Project::default());
        let spec = RoofSpec {
            contour: concave.then(sample),
            pivot_start: Point::default(),
            pivot_end: Point::new(6., 0.),
            rise_left: true,
            min: Point::default(),
            max: Point::new(6., 6.),
            pitch: 30.,
            thickness: 0.2,
            pivot_offset: 3.,
        };
        e.place_roof(spec).unwrap();
        let before = e.project.clone();
        let region = if concave {
            ring(&[
                (-0.2, -0.2),
                (6.2, -0.2),
                (6.2, 2.2),
                (2.2, 2.2),
                (2.2, 6.2),
                (-0.2, 6.2),
            ])
        } else {
            ring(&[
                (-1., -1.),
                (7., -1.),
                (7., 7.),
                (4., 7.),
                (4., 5.),
                (2., 5.),
                (2., 7.),
                (-1., 7.),
            ])
        };
        e.marquee = MarqueeArea::new(region, MarqueeScope::CurrentStory, 0);
        let undo = e.undo.len();
        let revision = e.revision;
        let moved = e.stretch_nodes_in_marquee(Point::new(10., -2.)).unwrap();
        assert_eq!(moved, usize::from(concave));
        if concave {
            let mut expected = before.clone();
            let Shape::Roof { spec } = &mut expected.elements[0].shape else {
                panic!()
            };
            spec.translate(Point::new(10., -2.));
            assert_eq!(e.project, expected);
            assert_eq!(e.undo.len(), undo + 1);
            let restored: Project =
                serde_json::from_str(&serde_json::to_string(&e.project).unwrap()).unwrap();
            assert_eq!(restored, expected);
            e.undo();
            assert_eq!(e.project, before);
            e.redo();
            assert_eq!(e.project, expected);
        } else {
            assert_eq!(e.project, before);
            assert_eq!(e.undo.len(), undo);
            assert_eq!(e.revision, revision);
        }
    }
}
