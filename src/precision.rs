//! Engineering precision contract in SI units. These are OpenArchi policies,
//! not measurements of Archicad's internal tolerances. Construction thresholds
//! remain in their existing object contracts until explicitly audited.
use crate::model::Point;

pub const MODEL_SCHEMA_VERSION: u32 = 2;
pub const GEOMETRY_POLICY_VERSION: u32 = 2;
pub const LOCAL_COORDINATE_LIMIT_M: f64 = 100_000.;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ErrorBudget {
    pub absolute: f64,
    pub relative: f64,
}
impl ErrorBudget {
    pub fn accepts(self, actual: f64, expected: f64) -> bool {
        actual.is_finite()
            && expected.is_finite()
            && (actual - expected).abs() <= self.absolute + self.relative * expected.abs()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TolerancePolicy {
    /// Round-trip model length budget, metres; not a vertex-merging distance.
    pub length_m: ErrorBudget,
    pub area_m2: ErrorBudget,
    pub volume_m3: ErrorBudget,
    /// Comparison of explicit angular inputs, radians; not curve segmentation.
    pub angle_rad: ErrorBudget,
}
impl Default for TolerancePolicy {
    fn default() -> Self {
        Self {
            length_m: ErrorBudget {
                absolute: 0.0001,
                relative: 1e-12,
            },
            area_m2: ErrorBudget {
                absolute: 1e-8,
                relative: 1e-10,
            },
            volume_m3: ErrorBudget {
                absolute: 1e-8,
                relative: 1e-10,
            },
            angle_rad: ErrorBudget {
                absolute: 1e-10,
                relative: 1e-12,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ConstructionPolicy {
    pub minimum_edge_m: f64,
    pub minimum_path_area_m2: f64,
    pub minimum_contour_area_m2: f64,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TopologyPolicy {
    pub segment_collinear_double_area_m2: f64,
    pub segment_touch_distance_m: f64,
    pub triangle_double_area_m2: f64,
    pub clip_plane_distance_m: f64,
    pub clip_vertex_merge_m: f64,
    pub beam_face_double_area_squared_m4: f64,
    pub pick_ray_determinant_m2: f64,
    pub restored_ring_merge_m: f64,
    pub arc_endpoint_angle_rad: f64,
}
/// Existing specialised solver guards, separated by physical dimension.
/// Values are compatibility policy, not engineering acceptance tolerances.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SolverPolicy {
    pub segment_denominator_floor_m2: f64,
    pub raw_line_cross_m2: f64,
    pub normalized_parallel_sine: f64,
    pub offset_forward_dot_m2: f64,
    pub offset_area_product_m4: f64,
    pub circle_height_squared_m2: f64,
    pub arc_chord_overshoot_m: f64,
    pub arc_center_double_area_m2: f64,
    pub arc_through_angle_guard_rad: f64,
    pub arc_minimum_sweep_rad: f64,
    pub wall_station_merge_m: f64,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SamplingPolicy {
    pub polyline_arc_max_step_rad: f64,
    pub beam_arc_max_step_rad: f64,
    pub circular_column_chord_error_m: f64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GeometryContext {
    pub tolerance: TolerancePolicy,
}
impl GeometryContext {
    // Frozen numerical values; validation-policy identity is versioned separately.
    pub const CONSTRUCTION: ConstructionPolicy = ConstructionPolicy {
        minimum_edge_m: 0.01,
        minimum_path_area_m2: 1e-6,
        minimum_contour_area_m2: 0.0001,
    };
    pub const TOPOLOGY: TopologyPolicy = TopologyPolicy {
        segment_collinear_double_area_m2: 1e-5,
        segment_touch_distance_m: 1e-5,
        triangle_double_area_m2: 1e-6,
        clip_plane_distance_m: 1e-8,
        clip_vertex_merge_m: 1e-8,
        beam_face_double_area_squared_m4: 1e-14,
        pick_ray_determinant_m2: 1e-7,
        restored_ring_merge_m: 1e-5,
        arc_endpoint_angle_rad: 1e-5,
    };
    pub const SOLVER: SolverPolicy = SolverPolicy {
        segment_denominator_floor_m2: 1e-9,
        raw_line_cross_m2: 1e-6,
        normalized_parallel_sine: 1e-6,
        offset_forward_dot_m2: 1e-6,
        offset_area_product_m4: 1e-6,
        circle_height_squared_m2: 1e-4,
        arc_chord_overshoot_m: 1e-4,
        arc_center_double_area_m2: 1e-5,
        arc_through_angle_guard_rad: 1e-5,
        arc_minimum_sweep_rad: 1e-4,
        wall_station_merge_m: 1e-5,
    };
    pub const SAMPLING: SamplingPolicy = SamplingPolicy {
        polyline_arc_max_step_rad: std::f64::consts::PI / 36.,
        beam_arc_max_step_rad: std::f64::consts::PI / 180.,
        circular_column_chord_error_m: 0.0005,
    };

    /// Preserve the existing strict modelling range (endpoints excluded).
    pub fn point_in_range(point: Point) -> bool {
        point.finite()
            && point.x.abs() < LOCAL_COORDINATE_LIMIT_M
            && point.y.abs() < LOCAL_COORDINATE_LIMIT_M
    }
}

/// Arithmetic length uncertainty for the supported coordinate frame. This
/// never serves as a topology merge distance or an engineering acceptance budget.
pub fn arithmetic_length_roundoff_m(coordinate_scale_m: f64) -> f64 {
    16. * f64::EPSILON * coordinate_scale_m.abs().max(LOCAL_COORDINATE_LIMIT_M)
}
/// Compare construction lengths without treating coordinate subtraction roundoff
/// as an intentional undersize. The supported coordinate frame supplies a floor:
/// a short edge moved back to zero retains rounding from its previous large origin.
/// This is separate from the 0.1 mm acceptance budget and never merges vertices.
pub fn length_meets_minimum(measured: f64, minimum: f64, coordinate_scale: f64) -> bool {
    let roundoff = arithmetic_length_roundoff_m(coordinate_scale.abs().max(minimum.abs()));
    measured.is_finite()
        && minimum.is_finite()
        && coordinate_scale.is_finite()
        && measured + roundoff >= minimum
}

pub fn coordinate_scale(points: impl IntoIterator<Item = Point>) -> f64 {
    points
        .into_iter()
        .map(|p| p.x.abs().max(p.y.abs()))
        .fold(0., f64::max)
}

pub fn segment_meets_minimum(a: Point, b: Point, minimum_m: f64) -> bool {
    a.finite()
        && b.finite()
        && length_meets_minimum(a.distance(b), minimum_m, coordinate_scale([a, b]))
}

pub fn axis_meets_minimum(start: f64, end: f64, minimum_m: f64) -> bool {
    start.is_finite()
        && end.is_finite()
        && length_meets_minimum(end - start, minimum_m, start.abs().max(end.abs()))
}

/// A numerical area bound derived from the minimum area's characteristic length.
/// This does not use a length epsilon as an area tolerance or the UI error budget.
pub fn area_meets_minimum(area_m2: f64, minimum_m2: f64, coordinate_scale_m: f64) -> bool {
    area_m2.is_finite()
        && area_m2 >= 0.
        && minimum_m2.is_finite()
        && minimum_m2 > 0.
        && length_meets_minimum(area_m2.sqrt(), minimum_m2.sqrt(), coordinate_scale_m)
}

/// The sole 3D narrowing boundary. Subtraction precedes conversion; the model
/// and all CPU geometry, including picking, retain their global f64 values.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RenderOrigin(pub glam::DVec3);
impl RenderOrigin {
    pub fn position(self, world: glam::DVec3) -> [f32; 3] {
        (world - self.0).as_vec3().to_array()
    }
}
