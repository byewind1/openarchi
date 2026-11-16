use atelier_bim::{
    editor::Editor,
    model::{Point, Project},
    slab_geometry::{polygon, regularize_creation_path, validate_contour},
};
use geo::{Area, BooleanOps, LineString, Polygon};

fn points(values: &[(f64, f64)]) -> Vec<Point> {
    values.iter().map(|&(x, y)| Point::new(x, y)).collect()
}

#[test]
fn nonzero_regions_match_actual_ac29_star_and_opposite_overlap() {
    let cases = [
        (
            include_str!("fixtures/geometry/slab-multicross--star-points.json"),
            include_str!("fixtures/geometry/slab-multicross--star-api.json"),
            1,
        ),
        (
            include_str!("fixtures/geometry/slab-overlap--loops-points.json"),
            include_str!("fixtures/geometry/slab-overlap--loops-api.json"),
            2,
        ),
    ];
    for (input, evidence, count) in cases {
        let screen: Vec<[f64; 2]> = serde_json::from_str(input).unwrap();
        // The unchanged observed plan view: derive input coordinates from its
        // actual API endpoints, not from assumed equal screen/world lengths.
        let path: Vec<_> = screen
            .iter()
            .map(|p| {
                Point::new(
                    (6.2067574 + (p[0] - 450.) * 0.19172970401) as f64,
                    (45.031127967 - (p[1] - 350.) * 0.19172970401) as f64,
                )
            })
            .collect();
        let json: serde_json::Value = serde_json::from_str(evidence).unwrap();
        let outline = json["response"]["structuredContent"]["detailsOfElements"][0]["details"]["polygonOutline"]
            .as_array().unwrap();
        let expected = Polygon::new(
            LineString::from(
                outline
                    .iter()
                    .map(|p| (p["x"].as_f64().unwrap(), p["y"].as_f64().unwrap()))
                    .collect::<Vec<_>>(),
            ),
            vec![],
        );
        for path in [path.clone(), path.into_iter().rev().collect()] {
            let regions = regularize_creation_path(&path, 1.25).unwrap();
            assert_eq!(regions.len(), count);
            let mut editor = Editor::new(Project::default());
            for contour in regions {
                validate_contour(&contour).unwrap();
                assert_eq!(contour.elevation, 1.25);
                editor.place_slab_contour(contour).unwrap();
            }
            let largest = editor
                .project
                .elements
                .iter()
                .map(polygon)
                .max_by(|a, b| a.unsigned_area().total_cmp(&b.unsigned_area()))
                .unwrap();
            assert!(largest.xor(&expected).unsigned_area() < 0.001);
            assert!(largest.interiors().is_empty());
        }
    }
}

#[test]
fn regularization_preserves_holes_and_typed_history_and_serialization() {
    // Outer square and an oppositely oriented inner square joined by a
    // retraced bridge: bridge cancels, inner loop becomes a real hole.
    let path = points(&[
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
    ]);
    let mut regions = regularize_creation_path(&path, -0.75).unwrap();
    assert_eq!(regions.len(), 1);
    let contour = regions.remove(0);
    assert_eq!(contour.holes.len(), 1);
    let mut editor = Editor::new(Project::default());
    let before = editor.project.clone();
    editor.place_slab_contour(contour).unwrap();
    assert!((polygon(&editor.project.elements[0]).unsigned_area() - 64.).abs() < 1e-5);
    assert_eq!(editor.undo.len(), 1);
    let created = editor.project.clone();
    let restored: Project =
        serde_json::from_str(&serde_json::to_string(&created).unwrap()).unwrap();
    assert_eq!(restored, created);
    editor.undo();
    assert_eq!(editor.project, before);
    editor.redo();
    assert_eq!(editor.project, created);
}

#[test]
fn small_contour_area_and_creation_are_stable_under_large_translations() {
    for offset in [0., 10000., 90000., -90000.] {
        for reversed in [false, true] {
            let point = |x: f64, y: f64| Point::new(x + offset, y - offset);
            let mut outer = vec![point(0., 0.), point(8., 0.), point(8., 6.), point(0., 6.)];
            if reversed {
                outer.reverse();
            }
            assert_eq!(atelier_bim::slab_geometry::area(&outer), 48.);
            let contour = atelier_bim::model::SlabContour {
                points: outer.clone(),
                holes: vec![atelier_bim::model::SlabContour {
                    points: vec![point(2., 2.), point(4., 2.), point(4., 4.), point(2., 4.)],
                    ..Default::default()
                }],
                ..Default::default()
            };
            validate_contour(&contour).unwrap();
            let roof_contour = atelier_bim::roof_contour::RoofContour {
                outer: contour.points.clone(),
                holes: contour.holes.iter().map(|h| h.points.clone()).collect(),
            };
            let roof = atelier_bim::roof::RoofSpec {
                contour: Some(roof_contour),
                pivot_start: point(0., -1.),
                pivot_end: point(8., -1.),
                rise_left: true,
                min: point(0., 0.),
                max: point(8., 6.),
                pitch: 30.,
                thickness: 0.2,
                pivot_offset: 1.,
            };
            roof.validate().unwrap();
            assert_eq!(roof.footprint_contour().area(), 44.);
            let mut e = Editor::new(Project::default());
            let before = e.project.clone();
            e.place_slab_contour(contour).unwrap();
            assert_eq!(e.project.elements[0].area(), 44.);
            let regions = regularize_creation_path(&outer, 0.).unwrap();
            assert_eq!(regions.len(), 1);
            assert_eq!(atelier_bim::slab_geometry::area(&regions[0].points), 48.);
            let after = e.project.clone();
            let loaded: Project =
                serde_json::from_str(&serde_json::to_string(&after).unwrap()).unwrap();
            loaded.validate().unwrap();
            assert_eq!(loaded, after);
            e.undo();
            assert_eq!(e.project, before);
            e.redo();
            assert_eq!(e.project, after);
        }
    }
}

#[test]
fn triangulation_and_closed_offset_preserve_translation_and_winding() {
    for offset in [0., 10000., 90000., -90000.] {
        for reversed in [false, true] {
            let point = |x: f64, y: f64| Point::new(x + offset, y - offset);
            let mut concave = vec![
                point(0., 0.),
                point(6., 0.),
                point(6., 2.),
                point(2., 2.),
                point(2., 6.),
                point(0., 6.),
            ];
            if reversed {
                concave.reverse();
            }
            let triangles = atelier_bim::geometry::triangulate_polygon(&concave);
            assert_eq!(triangles.len(), 4, "offset={offset}, reversed={reversed}");
            assert_eq!(
                triangles
                    .iter()
                    .map(|t| atelier_bim::slab_geometry::area(t))
                    .sum::<f64>(),
                20.
            );
            let mut closed = vec![point(0., 0.), point(8., 0.), point(8., 6.), point(0., 6.)];
            if reversed {
                closed.reverse();
            }
            closed.push(closed[0]);
            let inset =
                atelier_bim::geometry::offset_polyline(&closed, if reversed { -1. } else { 1. })
                    .unwrap();
            assert_eq!(inset.first(), inset.last());
            assert_eq!(atelier_bim::slab_geometry::area(&inset), 24.);
            for p in &inset {
                assert!((1. ..=7.).contains(&(p.x - offset)));
                assert!((1. ..=5.).contains(&(p.y + offset)));
            }
        }
    }
}

#[test]
fn zero_area_is_empty_but_invalid_inputs_are_errors_and_bowtie_keeps_both_regions() {
    assert!(
        regularize_creation_path(&points(&[(0., 0.), (1., 0.), (2., 0.)]), 0.)
            .unwrap()
            .is_empty()
    );
    let cancelled = points(&[
        (0., 0.),
        (2., 0.),
        (2., 2.),
        (0., 2.),
        (0., 0.),
        (0., 2.),
        (2., 2.),
        (2., 0.),
        (0., 0.),
    ]);
    assert!(regularize_creation_path(&cancelled, 0.).unwrap().is_empty());
    let bowtie = points(&[(0., 0.), (4., 4.), (0., 4.), (4., 0.)]);
    let regions = regularize_creation_path(&bowtie, 0.).unwrap();
    assert_eq!(
        regions.len(),
        2,
        "core must not silently choose a UI region"
    );
    for region in regions {
        validate_contour(&region).unwrap();
    }
    for value in [f64::NAN, f64::INFINITY, 100_000.] {
        let invalid = points(&[(value, 0.), (2., 0.), (0., 2.)]);
        assert!(regularize_creation_path(&invalid, 0.).is_err());
    }
    assert!(regularize_creation_path(&bowtie, f64::NAN).is_err());
    assert!(regularize_creation_path(&bowtie[..2], 0.).is_err());
}

#[test]
fn creation_region_metrics_preserve_equal_areas_in_double_model() {
    use atelier_bim::slab_geometry::regularize_creation_regions;
    let (x, y, w) = (-12.966213002_f64, 45.031127967_f64, 38.345940803_f64);
    let input: Vec<_> = [(x, y), (x + w, y - w), (x, y - w), (x + w, y)]
        .into_iter()
        .map(|(x, y)| Point::new(x as f64, y as f64))
        .collect();
    for path in [input.clone(), input.into_iter().rev().collect()] {
        let regions = regularize_creation_regions(&path, 1.25).unwrap();
        assert_eq!(regions.len(), 2);
        assert!((regions[0].net_area - regions[1].net_area).abs() < 1e-10);
        let restored_area = |index: usize| {
            let c = &regions[index].contour;
            Polygon::new(
                LineString::from(
                    c.points
                        .iter()
                        .map(|p| (p.x as f64, p.y as f64))
                        .collect::<Vec<_>>(),
                ),
                vec![],
            )
            .unsigned_area()
        };
        assert!(
            (restored_area(0) - restored_area(1)).abs() < 1e-10,
            "the double model must preserve equal source areas"
        );
        assert_eq!(
            regularize_creation_path(&path, 1.25).unwrap(),
            regions.into_iter().map(|r| r.contour).collect::<Vec<_>>()
        );
    }
}

#[test]
fn creation_region_metrics_retain_real_micrometre_width_difference() {
    use atelier_bim::slab_geometry::regularize_creation_regions;
    let (x, y) = (-12.966213002_f64, 45.031127967_f64);
    let path: Vec<_> = [(x, y), (x + 10.000001, y - 10.), (x, y - 10.), (x + 10., y)]
        .into_iter()
        .map(|(x, y)| Point::new(x as f64, y as f64))
        .collect();
    let regions = regularize_creation_regions(&path, 0.).unwrap();
    assert_eq!(regions.len(), 2);
    let upper = regions
        .iter()
        .find(|r| r.contour.points.iter().any(|p| p.y > y - 1.))
        .unwrap();
    let lower = regions
        .iter()
        .find(|r| r.contour.points.iter().all(|p| p.y < y - 1.))
        .unwrap();
    let difference = lower.net_area - upper.net_area;
    assert!(
        (4e-6..6e-6).contains(&difference),
        "actual difference: {difference}"
    );
    assert!(
        difference < lower.net_area * 1e-6,
        "the rejected relative tolerance would erase this real difference"
    );
}

#[test]
fn creation_region_metrics_subtract_holes_and_preserve_elevation() {
    use atelier_bim::slab_geometry::regularize_creation_regions;
    let path = points(&[
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
    ]);
    let regions = regularize_creation_regions(&path, -2.).unwrap();
    assert_eq!(regions.len(), 1);
    assert_eq!(regions[0].contour.holes.len(), 1);
    assert!((regions[0].net_area - 64.).abs() < 1e-9);
    assert_eq!(regions[0].contour.elevation, -2.);
    assert_eq!(
        regularize_creation_path(&path, -2.).unwrap(),
        regions.into_iter().map(|r| r.contour).collect::<Vec<_>>()
    );
}
