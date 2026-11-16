//! Captured AC29 INT 5101 samples, not a claim of global equivalence.
//! Source: docs/development/autonomous-loop/runs/20261103-ac29-basic-comparison/.
use atelier_bim::{
    editor::Editor,
    geometry,
    model::{Point, Project},
    roof::RoofSpec,
};

fn compare(editor: &Editor, expected: [f64; 6]) {
    let corners = geometry::corners(&editor.project, &editor.project.elements[0]);
    let mut actual = [
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
    ];
    for p in corners {
        for (axis, value) in [p.x, p.y, p.z].into_iter().enumerate() {
            actual[2 * axis] = actual[2 * axis].min(value);
            actual[2 * axis + 1] = actual[2 * axis + 1].max(value);
        }
    }
    for (axis, (a, b)) in actual.into_iter().zip(expected).enumerate() {
        assert!(
            (a - b).abs() < 0.0001,
            "bound {axis}: OpenArchi {a}, AC29 {b}"
        );
    }
}

#[test]
fn ac29_centered_rectangular_column_sample() {
    let mut e = Editor::new(Project::default());
    e.column_section = atelier_bim::column::ColumnSection::Rectangular {
        width: 0.4,
        depth: 0.4,
    };
    e.column_height = 3.;
    e.place_column(Point::new(6.2067574, 32.952156614)).unwrap();
    compare(
        &e,
        [6.0067574, 6.4067574, 32.752156614, 33.152156614, 0., 3.],
    );
}

#[test]
fn ac29_top_center_horizontal_beam_sample() {
    let mut e = Editor::new(Project::default());
    e.beam_size = [0.3, 0.5, 3.];
    e.place_beam(
        Point::new(6.2067574, 17.613780293),
        Point::new(44.552698202, 17.613780293),
    )
    .unwrap();
    compare(
        &e,
        [6.2067574, 44.552698202, 17.463780293, 17.763780293, 2.5, 3.],
    );
}

#[test]
fn ac29_vertical_thickness_bottom_pivot_rectangular_roof_sample() {
    let mut e = Editor::new(Project::default());
    e.place_roof(RoofSpec {
        contour: None,
        pivot_start: Point::new(6.2067574, 2.275403972),
        pivot_end: Point::new(25.379727801, 2.275403972),
        rise_left: true,
        min: Point::new(2.372163319, -1.559190108),
        max: Point::new(29.214321881, 11.861889172),
        pitch: 20.,
        thickness: 0.2,
        pivot_offset: 3.,
    })
    .unwrap();
    compare(
        &e,
        [
            2.372163296,
            29.214323044,
            -1.559190154,
            11.861889839,
            1.604321837,
            6.689195633,
        ],
    );
}
