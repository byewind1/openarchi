//! Decimal minimum dimensions through real typed creation, not standalone epsilon arithmetic.
use atelier_bim::{
    column::ColumnSection,
    command::ModelCommand,
    editor::Editor,
    model::{ModelAdapter, OpeningKind, Point, Project, SlabContour},
    roof::RoofSpec,
    stairs::StairSpec,
    storage::JsonAdapter,
};
fn create(kind: &str, origin: f64, delta: f64) -> Result<Editor, String> {
    let mut e = Editor::new(Project::default());
    let p = |x, y| Point::new(origin + x, origin + y);
    match kind {
        "wall" => {
            e.place_wall_segments(vec![(p(0., 0.), p(0.01 + delta, 0.))])?;
        }
        "slab" => {
            let w = 0.01 + delta;
            e.place_slab_contour(SlabContour {
                points: vec![p(0., 0.), p(w, 0.), p(w, 0.01), p(0., 0.01)],
                ..Default::default()
            })?;
        }
        "column" => {
            e.place_column_section(
                p(0., 0.),
                ColumnSection::Rectangular {
                    width: 0.01 + delta,
                    depth: 0.01,
                },
                0.01,
            )?;
        }
        "beam" => {
            e.beam_size = [0.01, 0.01, 0.];
            e.place_beam(p(0., 0.), p(0.01 + delta, 0.))?;
        }
        "roof" => {
            let w = 0.01 + delta;
            e.place_roof(RoofSpec {
                contour: None,
                pivot_start: p(0., 0.),
                pivot_end: p(w, 0.),
                rise_left: true,
                min: p(0., 0.),
                max: p(w, 0.01),
                pitch: 20.,
                thickness: 0.01,
                pivot_offset: 3.,
            })?;
        }
        "stair" => {
            e.place_stair(StairSpec {
                start: p(0., 0.),
                end: p(0.3 + delta, 0.),
                ..Default::default()
            })?;
        }
        "opening" => {
            e.place_wall_segments(vec![(p(0., 0.), p(1., 0.))])?;
            let mut defaults = OpeningKind::Door.defaults();
            defaults.width = 0.05 + delta;
            defaults.height = 0.05;
            e.place_opening_on_wall(OpeningKind::Door, 1, 0.025, defaults)?;
        }
        "polyline" => {
            e.place_polyline(vec![p(0., 0.), p(0.01 + delta, 0.)], vec![])?;
        }
        _ => unreachable!(),
    }
    Ok(e)
}
#[test]
fn decimal_minima_survive_translation_history_and_storage_for_all_eight_types() {
    let mut failures = Vec::new();
    for kind in [
        "wall", "slab", "column", "beam", "roof", "stair", "opening", "polyline",
    ] {
        for origin in [0., 4., 1000., -1000., 99900., -99900.] {
            match create(kind, origin, 0.) {
                Ok(mut e) => {
                    let created = e.project.clone();
                    let ids = e.project.elements.iter().map(|x| x.id).collect::<Vec<_>>();
                    for offset in [Point::new(-origin, -origin), Point::new(origin, origin)] {
                        e.apply(ModelCommand::MoveElements {
                            ids: ids.clone(),
                            offset,
                        })
                        .unwrap();
                    }
                    assert_eq!(e.project, created, "translation {kind} origin={origin}");
                    let bytes = JsonAdapter.export(&created).unwrap();
                    assert_eq!(JsonAdapter.import(&bytes).unwrap(), created);
                    e.undo();
                    e.redo();
                    assert_eq!(e.project, created);
                }
                Err(error) => failures.push(format!("{kind} origin={origin}: {error}")),
            }
            assert!(
                create(kind, origin, -1e-6).is_err(),
                "undersized {kind} origin={origin}"
            );
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn existing_rotation_mirror_boundaries_preserve_minimum_models_and_real_history() {
    use atelier_bim::{editing::EditCommand, precision::TolerancePolicy};
    for kind in [
        "wall", "slab", "column", "beam", "roof", "stair", "opening", "polyline",
    ] {
        for origin in [0., 99900., -99900.] {
            for mirror in [false, true] {
                let mut e = create(kind, origin, 0.).unwrap();
                e.set_selection(e.project.elements.iter().map(|x| x.id).collect());
                let before = e.project.clone();
                let history = e.undo.len();
                let revision = e.revision;
                let command = EditCommand::Transform {
                    offset: Point::default(),
                    origin: Point::new(origin, origin),
                    angle: if mirror { 0. } else { 30. },
                    scale: 1.,
                    mirror,
                    copies: 0,
                };
                let result = e.edit(command);
                if ["column", "beam", "roof"].contains(&kind) {
                    assert!(result.is_err(), "unsupported {kind}");
                    assert_eq!(e.project, before);
                    assert_eq!(e.undo.len(), history);
                    assert_eq!(e.revision, revision);
                    continue;
                }
                result.unwrap_or_else(|error| {
                    panic!("{kind} origin={origin} mirror={mirror}: {error}")
                });
                let transformed = e.project.clone();
                assert_eq!(transformed.elements.len(), before.elements.len());
                for (a, b) in before.elements.iter().zip(&transformed.elements) {
                    assert!(
                        TolerancePolicy::default()
                            .area_m2
                            .accepts(b.area(), a.area()),
                        "{kind} origin={origin} mirror={mirror}"
                    );
                }
                assert_eq!(e.undo.len(), history + 1);
                e.undo();
                assert_eq!(e.project, before);
                e.redo();
                assert_eq!(e.project, transformed);
                e.set_selection(e.project.elements.iter().map(|x| x.id).collect());
                e.edit(EditCommand::Transform {
                    offset: Point::default(),
                    origin: Point::new(origin, origin),
                    angle: if mirror { 0. } else { -30. },
                    scale: 1.,
                    mirror,
                    copies: 0,
                })
                .unwrap();
                e.project.validate().unwrap();
                assert_eq!(e.project.elements.len(), before.elements.len());
                for (a, b) in before.elements.iter().zip(&e.project.elements) {
                    assert!(
                        TolerancePolicy::default()
                            .area_m2
                            .accepts(b.area(), a.area()),
                        "roundtrip {kind}"
                    );
                }
            }
        }
    }
}

#[test]
fn seeded_supported_transform_roundtrips_keep_eight_families_valid() {
    use atelier_bim::{editing::EditCommand, precision::TolerancePolicy};
    const INITIAL_SEED: u64 = 0x4f41_4b33_2026_1107;
    let mut seed = INITIAL_SEED;
    for case in 0..16 {
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let angle = ((seed >> 16) % 35000) as f64 / 100. - 175.;
        let dx = ((seed >> 32) % 1000) as f64 / 100. - 5.;
        let origin = [0., 99900., -99900.][case % 3];
        for kind in [
            "wall", "slab", "column", "beam", "roof", "stair", "opening", "polyline",
        ] {
            let context = format!(
                "seed={INITIAL_SEED:x} case={case} kind={kind} origin={origin} angle={angle}"
            );
            let mut e = create(kind, origin, 0.).unwrap();
            let before = e.project.clone();
            let ids: Vec<_> = before.elements.iter().map(|e| e.id).collect();
            e.set_selection(ids.clone());
            let supported_rotation = !["column", "beam", "roof"].contains(&kind);
            if supported_rotation {
                e.edit(EditCommand::Transform {
                    offset: Point::default(),
                    origin: Point::new(origin, origin),
                    angle,
                    scale: 1.,
                    mirror: false,
                    copies: 0,
                })
                .unwrap_or_else(|m| panic!("{context}: {m}"));
                e.edit(EditCommand::Transform {
                    offset: Point::default(),
                    origin: Point::new(origin, origin),
                    angle: -angle,
                    scale: 1.,
                    mirror: false,
                    copies: 0,
                })
                .unwrap_or_else(|m| panic!("{context}: {m}"));
            }
            for offset in [Point::new(dx, -dx), Point::new(-dx, dx)] {
                e.apply(ModelCommand::MoveElements {
                    ids: ids.clone(),
                    offset,
                })
                .unwrap_or_else(|m| panic!("{context}: {m}"));
            }
            let after = e.project.clone();
            assert!(e.geometry_report().first_error().is_none(), "{context}");
            for (a, b) in before.elements.iter().zip(&after.elements) {
                assert!(
                    TolerancePolicy::default()
                        .area_m2
                        .accepts(b.area(), a.area()),
                    "{context}"
                );
                assert_eq!(a.id, b.id);
                assert_eq!(a.kind(), b.kind());
                for (x, y) in atelier_bim::geometry::element_plan_outline(&before, a)
                    .iter()
                    .zip(atelier_bim::geometry::element_plan_outline(&after, b))
                {
                    assert!(
                        x.distance(y) <= 0.0001,
                        "{context} distance={}",
                        x.distance(y)
                    );
                }
            }
            assert_eq!(
                JsonAdapter
                    .import(&JsonAdapter.export(&after).unwrap())
                    .unwrap(),
                after,
                "{context}"
            );
            e.undo();
            e.redo();
            assert_eq!(e.project, after, "{context}");
        }
    }
}

#[test]
fn invalid_updates_identify_each_family_and_preserve_real_redo_and_ids() {
    use atelier_bim::model::Shape;
    for kind in [
        "wall", "slab", "column", "beam", "roof", "stair", "opening", "polyline",
    ] {
        let mut e = create(kind, 0., 0.).unwrap();
        let ids = e.project.elements.iter().map(|e| e.id).collect();
        e.apply(ModelCommand::MoveElements {
            ids,
            offset: Point::new(1., 1.),
        })
        .unwrap();
        let redone = e.project.clone();
        e.undo();
        let before = e.project.clone();
        let revision = e.revision;
        let history = e.undo.len();
        let redo = e.redo.len();
        let selection = e.selection_ids();
        let mut bad = e.project.elements[if kind == "opening" { 1 } else { 0 }].clone();
        match &mut bad.shape {
            Shape::ConceptMass { spec } => spec.height = 0.,
            Shape::Zone { spec } => spec.height = 0.,
            Shape::Wall { height, .. }
            | Shape::Column { height, .. }
            | Shape::Opening { height, .. }
            | Shape::Beam { height, .. } => *height = 0.,
            Shape::Slab { thickness, .. } => *thickness = 0.,
            Shape::Roof { spec } => spec.thickness = 0.,
            Shape::Stair { spec } => spec.end = spec.start,
            Shape::Polyline { points, .. } => points[1] = points[0],
        }
        let command = ModelCommand::UpdateElement {
            element: bad.clone(),
        };
        let error = match e.prepare_change(revision, vec![command.clone()]) {
            Ok(_) => panic!("{kind}"),
            Err(error) => error,
        };
        assert_eq!(error.diagnostics[0].element_id, bad.id, "{kind}");
        assert!(e.apply(command).is_err(), "{kind}");
        assert_eq!(e.project, before, "{kind}");
        assert_eq!(e.revision, revision);
        assert_eq!(e.undo.len(), history);
        assert_eq!(e.redo.len(), redo);
        assert_eq!(e.selection_ids(), selection);
        e.redo();
        assert_eq!(e.project, redone, "{kind}");
    }
}
