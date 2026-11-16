use atelier_bim::{
    column::{ColumnSection, ColumnSolid},
    column_inclination::InclinedColumnSolid,
    column_plan::{ColumnPlanDisplay as Display, ColumnProjectionRange as Range},
    model::Point,
};
use geo::{Area, Contains, LineString, Polygon};
fn solid() -> InclinedColumnSolid {
    InclinedColumnSolid {
        column: ColumnSolid {
            section: ColumnSection::Rectangular {
                width: 2.,
                depth: 1.,
            },
            center: Point::new(10., 20.),
            bottom: 4.,
            height: 3.,
            angle_degrees: 0.,
        },
        slant_degrees: 30.,
        direction_degrees: 0.,
    }
}
fn poly(points: &[Point]) -> Polygon<f64> {
    Polygon::new(
        LineString::from(
            points
                .iter()
                .map(|p| (p.x as f64, p.y as f64))
                .collect::<Vec<_>>(),
        ),
        vec![],
    )
}
fn area(points: &[Point]) -> f64 {
    if points.is_empty() {
        0.
    } else {
        poly(points).unsigned_area()
    }
}
fn near(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < 0.0001, "{actual} != {expected}");
}
#[test]
fn projected_parts_use_real_cut_and_each_vertical_interval() {
    let s = solid();
    let p = s
        .plan_geometry(5., Display::ProjectedWithOverhead, Range::EntireElement)
        .unwrap();
    near(area(&p.cut), 4.);
    near(area(&p.uncut), 4. + 2. * 3_f64.sqrt());
    near(area(&p.overhead), 4. + 4. * 3_f64.sqrt());
    near(
        area(&s.projected_outline(4., 7.).unwrap()),
        4. + 6. * 3_f64.sqrt(),
    );
    let projected = s
        .plan_geometry(5., Display::Projected, Range::EntireElement)
        .unwrap();
    assert_eq!(projected.cut, p.cut);
    assert_eq!(projected.uncut, p.uncut);
    assert!(projected.overhead.is_empty());
    let cut = s
        .plan_geometry(5., Display::CutOnly, Range::EntireElement)
        .unwrap();
    assert_eq!(cut.cut, p.cut);
    assert!(cut.uncut.is_empty() && cut.overhead.is_empty());
}
#[test]
fn cut_above_below_and_on_caps_has_no_invented_section() {
    let s = solid();
    for (cut, below) in [(3., true), (8., false)] {
        let p = s
            .plan_geometry(cut, Display::ProjectedWithOverhead, Range::EntireElement)
            .unwrap();
        assert!(p.cut.is_empty());
        assert_eq!(p.uncut.is_empty(), below);
        assert_eq!(p.overhead.is_empty(), !below);
        near(
            area(if below { &p.overhead } else { &p.uncut }),
            4. + 6. * 3_f64.sqrt(),
        );
    }
    for cut in [4., 7.] {
        let p = s
            .plan_geometry(cut, Display::ProjectedWithOverhead, Range::EntireElement)
            .unwrap();
        near(area(&p.cut), 4.);
    }
    assert!(
        s.plan_geometry(3., Display::Projected, Range::EntireElement)
            .unwrap()
            .uncut
            .is_empty()
    );
    assert!(
        s.plan_geometry(8., Display::CutOnly, Range::EntireElement)
            .unwrap()
            .cut
            .is_empty()
    );
}
#[test]
fn projection_range_clamps_floor_cut_but_absolute_limit_does_not() {
    let s = solid();
    let range = Range::FloorPlan {
        lower: 5.,
        upper: 6.,
    };
    let p = s
        .plan_geometry(8., Display::ProjectedWithOverhead, range)
        .unwrap();
    assert_eq!(p.cut, s.section_at(6.).unwrap());
    assert!(p.overhead.is_empty());
    near(area(&p.uncut), 4. + 2. * 3_f64.sqrt());
    let p = s
        .plan_geometry(
            4.,
            Display::ProjectedWithOverhead,
            Range::FloorPlan {
                lower: 6.,
                upper: 5.,
            },
        )
        .unwrap();
    assert_eq!(p.cut, s.section_at(6.).unwrap());
    assert!(p.uncut.is_empty() && p.overhead.is_empty());
    let p = s
        .plan_geometry(
            4.,
            Display::ProjectedWithOverhead,
            Range::Above { lower: 5. },
        )
        .unwrap();
    assert!(p.cut.is_empty() && p.uncut.is_empty());
    near(area(&p.overhead), 4. + 4. * 3_f64.sqrt());
    let p = s
        .plan_geometry(5., Display::CutOnly, Range::Above { lower: 6. })
        .unwrap();
    assert_eq!(p.cut, s.section_at(5.).unwrap());
    let p = s
        .plan_geometry(
            5.,
            Display::ProjectedWithOverhead,
            Range::Above { lower: 8. },
        )
        .unwrap();
    assert!(p.cut.is_empty() && p.uncut.is_empty() && p.overhead.is_empty());
}
#[test]
fn circular_projection_preserves_curved_ends_and_vertical_limit() {
    let mut s = solid();
    s.column.section = ColumnSection::Circular { diameter: 2. };
    let p = s
        .plan_geometry(5., Display::ProjectedWithOverhead, Range::EntireElement)
        .unwrap();
    // Ellipse area + sweep length times width perpendicular to sweep.
    assert!((area(&p.cut) - 2. * std::f64::consts::PI).abs() < 0.02);
    assert!((area(&p.uncut) - (2. * std::f64::consts::PI + 2. * 3_f64.sqrt())).abs() < 0.02);
    assert!(!poly(&p.uncut).contains(&geo::Point::new(10.95, 18.05)));
    s.slant_degrees = 90.;
    let p = s
        .plan_geometry(5., Display::ProjectedWithOverhead, Range::EntireElement)
        .unwrap();
    near(area(&p.cut), area(&p.uncut));
    near(area(&p.cut), area(&p.overhead));
}
#[test]
fn finite_height_validation_and_rounded_top_remain_consistent() {
    let mut s = solid();
    s.column.bottom = 10.;
    s.column.height = 2.8;
    assert!(s.section_at(s.column.bottom + s.column.height).is_ok());
    assert!(
        s.plan_geometry(11., Display::ProjectedWithOverhead, Range::EntireElement)
            .is_ok()
    );
    assert!(s.projected_outline(7., 4.).is_err());
    assert!(s.projected_outline(f64::NAN, 7.).is_err());
    assert!(
        s.plan_geometry(f64::NAN, Display::Projected, Range::EntireElement)
            .is_err()
    );
    assert!(
        s.plan_geometry(
            5.,
            Display::Projected,
            Range::FloorPlan {
                lower: f64::NAN,
                upper: 7.
            }
        )
        .is_err()
    );
    assert!(
        s.plan_geometry(
            5.,
            Display::Projected,
            Range::Above {
                lower: f64::INFINITY
            }
        )
        .is_err()
    );
}
