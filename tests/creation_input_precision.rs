use atelier_bim::{
    creation_input::{CreationPoint, PolygonCreationInput},
    model::Point,
    roof::RoofDraft,
    roof_creation::RoofPolygonDraft,
    slab_creation::SlabPolygonDraft,
    slab_geometry::{regularize_creation_regions, regularize_precise_creation_regions},
};
fn roof() -> RoofPolygonDraft {
    RoofPolygonDraft::from_contour_start(RoofDraft::ContourStart {
        start: Point::new(0., 0.),
        end: Point::new(10., 0.),
        rise_left: true,
    })
    .unwrap()
}
fn shared_start() -> Vec<CreationPoint> {
    [
        (450., 450.),
        (350., 350.),
        (550., 350.),
        (350., 550.),
        (550., 550.),
        (450., 450.),
    ]
    .map(|(x, y)| CreationPoint {
        x: 6.2067574 + (x - 450.) * 0.19172970401,
        y: 45.031127967 - (y - 350.) * 0.19172970401,
    })
    .to_vec()
}
#[test]
fn shared_start_symmetry_survives_input_until_regularization() {
    let input = shared_start();
    let quantized: Vec<_> = input
        .iter()
        .map(|p| Point::new(f64::from(p.x as f32), f64::from(p.y as f32)))
        .collect();
    let rounded = regularize_creation_regions(&quantized, 0.).unwrap();
    assert!((rounded[0].net_area - rounded[1].net_area).abs() > 8e-6);
    for points in [input.clone(), input.into_iter().rev().collect()] {
        let mut slab = SlabPolygonDraft::default();
        let mut roof = roof();
        for p in points.iter().copied() {
            slab.accept_precise_vertex(p).unwrap();
            roof.accept_precise_vertex(p).unwrap();
        }
        assert_eq!(slab.precise_points(), points);
        assert_eq!(roof.precise_points(), points);
        let before = slab.clone();
        let regions = slab.regions(None, 1.25).unwrap();
        assert_eq!(regions.len(), 2);
        assert!((regions[0].net_area - regions[1].net_area).abs() < 1e-10);
        assert_eq!(
            slab.regions(slab.points().last().copied(), 1.25).unwrap(),
            regions
        );
        assert_eq!(slab, before, "display cursor must not mutate precise input");
        let roofs = roof.regularized_candidates(None, [30., 0.3, 0.]).unwrap();
        assert_eq!(roofs.len(), 2);
    }
}
#[test]
fn precise_numeric_targets_retain_sub_float_offsets_for_both_drafts() {
    let start = CreationPoint {
        x: 12345.000001,
        y: 23456.000001,
    };
    let mut slab = SlabPolygonDraft::default();
    let mut roof = roof();
    slab.accept_precise_vertex(start).unwrap();
    roof.accept_precise_vertex(start).unwrap();
    let target = slab.precise_numeric_target(1.000001, 0.).unwrap();
    assert!((target.x - 12346.000002).abs() < 1e-10);
    assert_eq!(target.y, start.y);
    assert_eq!(target, roof.precise_numeric_target(1.000001, 0.).unwrap());
    assert_eq!(
        slab.precise_points(),
        [start],
        "numeric preview does not accept a node"
    );
    slab.accept_precise_vertex(target).unwrap();
    roof.accept_precise_vertex(target).unwrap();
    assert_eq!(slab.points(), roof.points());
    assert_eq!(target.x, target.model_point().x);
    assert_ne!(target.x, f64::from(target.x as f32));
}
#[test]
fn invalid_precise_inputs_preserve_both_caches_and_drafts() {
    let mut input = PolygonCreationInput::default();
    input.accept(CreationPoint { x: 1., y: 2. }).unwrap();
    let before = input.clone();
    for x in [f64::NAN, f64::INFINITY, 100_000., -100_000.] {
        assert!(input.accept(CreationPoint { x, y: 0. }).is_err());
        assert_eq!(input, before);
    }
    for (length, angle) in [
        (0., 0.),
        (-1., 0.),
        (f64::INFINITY, 0.),
        (1., f64::NAN),
        (200_000., 0.),
    ] {
        assert!(input.numeric_target(length, angle).is_err());
        assert_eq!(input, before);
    }
    let mut boundary = PolygonCreationInput::default();
    boundary
        .accept(CreationPoint {
            x: 99_999.999999,
            y: 0.,
        })
        .unwrap();
    assert_eq!(boundary.points()[0].x, 99_999.999999);
    let mut slab = SlabPolygonDraft::default();
    let mut roof = roof();
    slab.accept_vertex(Point::new(1., 2.)).unwrap();
    roof.accept_vertex(Point::new(1., 2.)).unwrap();
    let old_slab = slab.clone();
    let old_roof = roof.clone();
    assert!(
        slab.accept_precise_vertex(CreationPoint { x: f64::NAN, y: 0. })
            .is_err()
    );
    assert!(
        roof.accept_precise_vertex(CreationPoint { x: f64::NAN, y: 0. })
            .is_err()
    );
    assert_eq!(slab, old_slab);
    assert_eq!(roof, old_roof);
}
#[test]
fn precise_micro_difference_and_legacy_geometry_are_both_preserved() {
    let (x, y) = (-12.966213002, 45.031127967);
    let precise = [(x, y), (x + 10.000001, y - 10.), (x, y - 10.), (x + 10., y)]
        .map(|(x, y)| CreationPoint { x, y });
    let regions = regularize_precise_creation_regions(&precise, 0.).unwrap();
    assert_eq!(regions.len(), 2);
    let difference = (regions[0].net_area - regions[1].net_area).abs();
    assert!((4.8e-6..5.2e-6).contains(&difference));
    let points: Vec<_> = precise.iter().map(|p| p.model_point()).collect();
    let promoted: Vec<_> = points.iter().copied().map(CreationPoint::from).collect();
    assert_eq!(
        regularize_creation_regions(&points, 0.).unwrap(),
        regularize_precise_creation_regions(&promoted, 0.).unwrap()
    );
}

#[test]
fn precise_editor_entry_commits_through_existing_history_and_storage() {
    use atelier_bim::{
        editor::{Editor, Tool},
        model::Project,
    };
    for tool in [Tool::Slab, Tool::Roof] {
        let mut editor = Editor::new(Project::default());
        let before = editor.project.clone();
        editor.set_tool(tool);
        if tool == Tool::Roof {
            for p in [Point::new(-2., 1.), Point::new(8., 1.), Point::new(0., 3.)] {
                editor.click(p, 0.01);
            }
            editor.begin_roof_polygon().unwrap();
        } else {
            editor.begin_slab_polygon().unwrap();
        }
        let start = CreationPoint {
            x: 2.00000001,
            y: 3.00000001,
        };
        if tool == Tool::Slab {
            editor.accept_slab_polygon_precise_vertex(start).unwrap();
            editor
                .accept_slab_polygon_precise_numeric_vertex(2.000001, 0.)
                .unwrap();
            editor
                .accept_slab_polygon_precise_numeric_vertex(2., 90.)
                .unwrap();
            assert_eq!(
                editor.slab_polygon_draft().unwrap().precise_points()[0],
                start
            );
            assert_eq!(editor.project, before);
            let region = editor.slab_polygon_candidates(None).unwrap().remove(0);
            editor.commit_slab_polygon_region(&region).unwrap();
        } else {
            editor.accept_roof_polygon_precise_vertex(start).unwrap();
            editor
                .accept_roof_polygon_precise_numeric_vertex(2.000001, 0.)
                .unwrap();
            editor
                .accept_roof_polygon_precise_numeric_vertex(2., 90.)
                .unwrap();
            assert_eq!(
                editor.roof_polygon_draft().unwrap().precise_points()[0],
                start
            );
            assert_eq!(editor.project, before);
            let region = editor.roof_polygon_candidates(None).unwrap().remove(0);
            editor.commit_roof_polygon_region(&region).unwrap();
        }
        let committed = editor.project.clone();
        assert_eq!(
            serde_json::from_str::<Project>(&serde_json::to_string(&committed).unwrap()).unwrap(),
            committed
        );
        editor.undo();
        assert_eq!(editor.project, before);
        editor.redo();
        assert_eq!(editor.project, committed);
    }
}

#[test]
fn exact_cursor_preview_matches_acceptance_without_rounding_the_closing_point() {
    let points = shared_start();
    let mut draft = SlabPolygonDraft::default();
    for p in &points[..points.len() - 1] {
        draft.accept_precise_vertex(*p).unwrap();
    }
    let closing = *points.last().unwrap();
    let frozen = draft.clone();
    let precise = draft.precise_regions(Some(closing), 0.).unwrap();
    let display = draft.regions(Some(closing.model_point()), 0.).unwrap();
    assert!((precise[0].net_area - precise[1].net_area).abs() < 1e-10);
    assert!(
        (display[0].net_area - display[1].net_area).abs() < 1e-10,
        "f64 model preview preserves exact closing input"
    );
    assert_eq!(draft, frozen);
    draft.accept_precise_vertex(closing).unwrap();
    assert_eq!(precise, draft.precise_regions(None, 0.).unwrap());
    assert_eq!(precise, draft.precise_regions(Some(closing), 0.).unwrap());
}

#[test]
fn exact_editor_preview_is_the_candidate_committed_after_acceptance() {
    use atelier_bim::{
        editor::{Editor, Tool},
        model::Project,
    };
    for tool in [Tool::Slab, Tool::Roof] {
        let mut e = Editor::new(Project::default());
        e.set_tool(tool);
        if tool == Tool::Roof {
            for p in [Point::new(-2., 1.), Point::new(8., 1.), Point::new(0., 3.)] {
                e.click(p, 0.01);
            }
            e.begin_roof_polygon().unwrap();
        } else {
            e.begin_slab_polygon().unwrap();
        }
        let before = e.project.clone();
        let nodes = shared_start();
        for p in &nodes[..nodes.len() - 1] {
            if tool == Tool::Slab {
                e.accept_slab_polygon_precise_vertex(*p).unwrap();
            } else {
                e.accept_roof_polygon_precise_vertex(*p).unwrap();
            }
        }
        let cursor = *nodes.last().unwrap();
        if tool == Tool::Slab {
            let frozen = e.slab_polygon_draft().unwrap().clone();
            assert!(
                e.slab_polygon_precise_candidates(Some(CreationPoint { x: f64::NAN, y: 0. }))
                    .is_err()
            );
            let preview = e.slab_polygon_precise_candidates(Some(cursor)).unwrap();
            assert_eq!(e.slab_polygon_draft(), Some(&frozen));
            assert_eq!(e.project, before);
            e.accept_slab_polygon_precise_vertex(cursor).unwrap();
            assert_eq!(preview, e.slab_polygon_candidates(None).unwrap());
            e.commit_slab_polygon_region(&preview[0]).unwrap();
        } else {
            let frozen = e.roof_polygon_draft().unwrap().clone();
            assert!(
                e.roof_polygon_precise_candidates(Some(CreationPoint { x: f64::NAN, y: 0. }))
                    .is_err()
            );
            let preview = e.roof_polygon_precise_candidates(Some(cursor)).unwrap();
            assert_eq!(e.roof_polygon_draft(), Some(&frozen));
            assert_eq!(e.project, before);
            e.accept_roof_polygon_precise_vertex(cursor).unwrap();
            assert_eq!(preview, e.roof_polygon_candidates(None).unwrap());
            e.commit_roof_polygon_region(&preview[0]).unwrap();
        }
        let committed = e.project.clone();
        e.undo();
        assert_eq!(e.project, before);
        e.redo();
        assert_eq!(e.project, committed);
    }
}

#[test]
fn region_membership_uses_precise_boundary_instead_of_model_rounding() {
    use geo::{Intersects, LineString, Polygon};
    let input =
        [(1.00001, 0.), (2., 0.), (2., 1.), (1.00001, 1.)].map(|(x, y)| CreationPoint { x, y });
    let region = regularize_precise_creation_regions(&input, 0.)
        .unwrap()
        .remove(0);
    let rounded_x = region
        .contour
        .points
        .iter()
        .map(|p| p.x as f64)
        .fold(f64::INFINITY, f64::min);
    assert_ne!(rounded_x, 1.00001);
    let probe = CreationPoint {
        x: (rounded_x + 1.00001) / 2.,
        y: 0.5,
    };
    let rendered = Polygon::new(
        LineString::from(
            region
                .contour
                .points
                .iter()
                .map(|p| (p.x as f64, p.y as f64))
                .collect::<Vec<_>>(),
        ),
        vec![],
    );
    let model_contains = rendered.intersects(&geo::Point::new(probe.x, probe.y));
    assert_ne!(
        region.contains_precise_point(probe),
        model_contains,
        "fixture must expose a real membership difference across the rounded boundary"
    );
    assert_eq!(region.contains_precise_point(probe), probe.x > 1.00001);
    assert!(!region.contains_precise_point(CreationPoint {
        x: f64::NAN,
        y: 0.5
    }));
}

#[test]
fn region_membership_distinguishes_shared_input_holes_and_their_boundaries() {
    let points = shared_start();
    let regions = regularize_precise_creation_regions(&points, 0.).unwrap();
    assert_eq!(
        regions
            .iter()
            .filter(|r| r.contains_precise_point(points[0]))
            .count(),
        2
    );
    assert_eq!(
        regions
            .iter()
            .filter(|r| r.contains_precise_point(points[1]))
            .count(),
        1
    );
    let input = [
        (0., 0.),
        (10., 0.),
        (10., 10.),
        (0., 10.),
        (0., 0.),
        (2., 2.),
        (2., 8.),
        (8., 8.),
        (8., 2.),
        (2., 2.),
        (0., 0.),
    ]
    .map(|(x, y)| CreationPoint { x, y });
    let region = regularize_precise_creation_regions(&input, 0.)
        .unwrap()
        .remove(0);
    for (x, y, expected) in [
        (1., 1., true),
        (5., 5., false),
        (2., 5., true),
        (0., 5., true),
        (-1., 5., false),
    ] {
        assert_eq!(
            region.contains_precise_point(CreationPoint { x, y }),
            expected
        );
    }
}

#[test]
fn region_choice_preserves_ambiguity_and_uses_first_discriminating_input() {
    use atelier_bim::slab_geometry::{
        CreationRegionChoice as Choice, inspect_creation_region_choice as inspect,
    };
    assert_eq!(inspect(&[], &[]), Choice::Empty);
    let points = shared_start();
    let regions = regularize_precise_creation_regions(&points, 0.).unwrap();
    assert_eq!(inspect(&regions, &[]), Choice::Ambiguous(vec![0, 1]));
    assert_eq!(
        inspect(&regions, &points[..1]),
        Choice::Ambiguous(vec![0, 1])
    );
    let expected = regions
        .iter()
        .position(|r| r.contains_precise_point(points[1]))
        .unwrap();
    assert_eq!(inspect(&regions, &points), Choice::Unique(expected));
    assert_eq!(
        inspect(
            &regions,
            &[CreationPoint { x: -999., y: -999. }, points[0], points[1]]
        ),
        Choice::Unique(expected)
    );
    let opposite = regions
        .iter()
        .position(|r| r.contains_precise_point(points[3]))
        .unwrap();
    assert_ne!(expected, opposite);
    assert_eq!(
        inspect(&regions, &[points[0], points[3], points[1]]),
        Choice::Unique(opposite)
    );
    let duplicate = vec![regions[expected].clone(), regions[expected].clone()];
    assert_eq!(
        inspect(&duplicate, &points),
        Choice::Ambiguous(vec![0, 1]),
        "no hidden order fallback"
    );
}

#[test]
fn region_choice_preserves_real_micro_area_priority_over_first_node() {
    use atelier_bim::slab_geometry::{CreationRegionChoice, inspect_creation_region_choice};
    let (x, y) = (-12.966213002, 45.031127967);
    let points = [(x, y), (x + 10.000001, y - 10.), (x, y - 10.), (x + 10., y)]
        .map(|(x, y)| CreationPoint { x, y });
    let regions = regularize_precise_creation_regions(&points, 0.).unwrap();
    let large = regions
        .iter()
        .position(|r| r.contains_precise_point(points[1]))
        .unwrap();
    assert!(!regions[large].contains_precise_point(points[0]));
    assert_eq!(
        inspect_creation_region_choice(&regions, &points),
        CreationRegionChoice::Unique(large)
    );
}

#[test]
fn roof_analysis_replays_observed_cross_with_independent_pivot() {
    use atelier_bim::slab_geometry::CreationRegionChoice;
    let mut draft = RoofPolygonDraft::from_contour_start(RoofDraft::ContourStart {
        start: Point::new(-12.966213, -2.901298),
        end: Point::new(25.379728, -2.901298),
        rise_left: true,
    })
    .unwrap();
    for (x, y) in [(350., 350.), (550., 550.), (350., 550.), (550., 350.)] {
        draft
            .accept_precise_vertex(CreationPoint {
                x: 6.2067574 + (x - 450.) * 0.19172970401,
                y: 45.031127967 - (y - 350.) * 0.19172970401,
            })
            .unwrap();
    }
    let frozen = draft.clone();
    let analysis = draft.precise_region_analysis(None, [45., 0.3, 3.]).unwrap();
    let CreationRegionChoice::Unique(index) = *analysis.choice() else {
        panic!("expected unique observed region")
    };
    assert_eq!(analysis.candidates().len(), 2);
    let chosen = &analysis.candidates()[index];
    let expected = [
        (-12.966213002, 45.031127967),
        (6.2067574, 25.858157565),
        (25.379727801, 45.031127967),
    ];
    let outer = &chosen.contour.as_ref().unwrap().outer;
    assert_eq!(outer.len(), 3);
    for (x, y) in expected {
        assert!(
            outer
                .iter()
                .any(|p| (p.x as f64 - x).abs() < 2e-5 && (p.y as f64 - y).abs() < 2e-5)
        );
    }
    assert_eq!(chosen.pivot_start, Point::new(-12.966213, -2.901298));
    assert_eq!(chosen.pivot_end, Point::new(25.379728, -2.901298));
    assert!(chosen.rise_left);
    assert_eq!(
        [chosen.pitch, chosen.thickness, chosen.pivot_offset],
        [45., 0.3, 3.]
    );
    assert_eq!(draft, frozen);
}

#[test]
fn roof_analysis_retains_precise_order_and_matches_accepted_cursor() {
    use atelier_bim::slab_geometry::CreationRegionChoice;
    for points in [shared_start(), shared_start().into_iter().rev().collect()] {
        let mut draft = roof();
        for p in &points[..points.len() - 1] {
            draft.accept_precise_vertex(*p).unwrap();
        }
        let frozen = draft.clone();
        let cursor = *points.last().unwrap();
        let preview = draft
            .precise_region_analysis(Some(cursor), [30., 0.25, 2.])
            .unwrap();
        assert_eq!(draft, frozen);
        draft.accept_precise_vertex(cursor).unwrap();
        assert_eq!(
            preview,
            draft
                .precise_region_analysis(None, [30., 0.25, 2.])
                .unwrap()
        );
        let CreationRegionChoice::Unique(i) = *preview.choice() else {
            panic!("expected discriminating second node")
        };
        let second = points[1].model_point();
        assert!(
            preview.candidates()[i]
                .contour
                .as_ref()
                .unwrap()
                .outer
                .iter()
                .any(|p| p.distance(second) < 1e-4)
        );
        assert_eq!(
            preview.candidates(),
            draft
                .precise_regularized_candidates(None, [30., 0.25, 2.])
                .unwrap()
        );
    }
}

#[test]
fn roof_analysis_empty_and_invalid_are_read_only_with_real_redo() {
    use atelier_bim::{
        editor::{Editor, Tool},
        model::Project,
        slab_geometry::CreationRegionChoice,
    };
    let mut editor = Editor::new(Project::default());
    editor.place_column(Point::new(5., 5.)).unwrap();
    let redo = editor.project.clone();
    editor.undo();
    editor.set_tool(Tool::Roof);
    for p in [Point::new(-2., 1.), Point::new(8., 1.), Point::new(0., 3.)] {
        editor.click(p, 0.01);
    }
    editor.begin_roof_polygon().unwrap();
    for (x, y) in [(0., 0.), (1., 0.), (2., 0.)] {
        editor
            .accept_roof_polygon_precise_vertex(CreationPoint { x, y })
            .unwrap();
    }
    let before = editor.project.clone();
    let frozen = editor.roof_polygon_draft().unwrap().clone();
    let empty = frozen
        .precise_region_analysis(None, editor.roof_size)
        .unwrap();
    assert_eq!(empty.choice(), &CreationRegionChoice::Empty);
    assert!(empty.candidates().is_empty());
    for (cursor, parameters) in [
        (CreationPoint { x: f64::NAN, y: 1. }, [30., 0.3, 0.]),
        (CreationPoint { x: 1., y: 2. }, [30., -1., 0.]),
    ] {
        assert!(
            editor
                .roof_polygon_draft()
                .unwrap()
                .precise_region_analysis(Some(cursor), parameters)
                .is_err()
        );
    }
    assert_eq!(editor.roof_polygon_draft(), Some(&frozen));
    assert_eq!(editor.project, before);
    editor.redo();
    assert_eq!(editor.project, redo);
}

#[test]
fn roof_analysis_matches_three_independent_native_region_readbacks() {
    use atelier_bim::slab_geometry::CreationRegionChoice;
    let shared = vec![
        (450., 450.),
        (350., 350.),
        (550., 350.),
        (350., 550.),
        (550., 550.),
        (450., 450.),
    ];
    let cases = [
        (
            vec![(350., 350.), (600., 550.), (350., 550.), (550., 350.)],
            include_str!("fixtures/geometry/roof-region-priority--api.json"),
        ),
        (
            shared.clone(),
            include_str!("fixtures/geometry/roof-region-priority--shared-forward--api.json"),
        ),
        (
            shared.into_iter().rev().collect(),
            include_str!("fixtures/geometry/roof-region-priority--shared-reverse--api.json"),
        ),
    ];
    for (screen, recorded) in cases {
        let mut draft = roof();
        for (x, y) in screen {
            draft
                .accept_precise_vertex(CreationPoint {
                    x: 6.2067574 + (x - 450.) * 0.19172970401,
                    y: 45.031127967 - (y - 350.) * 0.19172970401,
                })
                .unwrap();
        }
        let analysis = draft.precise_region_analysis(None, [45., 0.3, 3.]).unwrap();
        let CreationRegionChoice::Unique(index) = *analysis.choice() else {
            panic!("native produced one roof")
        };
        let actual = &analysis.candidates()[index].contour.as_ref().unwrap().outer;
        let evidence: serde_json::Value = serde_json::from_str(recorded).unwrap();
        let expected = evidence["details"]["structuredContent"]["detailsOfElements"][0]["details"]
            ["polygonOutline"]
            .as_array()
            .unwrap();
        assert_eq!(
            expected.first(),
            expected.last(),
            "API returns a closed ring"
        );
        assert_eq!(actual.len(), expected.len() - 1);
        for point in &expected[..expected.len() - 1] {
            let x = point["x"].as_f64().unwrap();
            let y = point["y"].as_f64().unwrap();
            assert!(
                actual
                    .iter()
                    .any(|p| (p.x as f64 - x).abs() < 2e-5 && (p.y as f64 - y).abs() < 2e-5),
                "expected native vertex {x}, {y}; got {actual:?}"
            );
        }
    }
}
