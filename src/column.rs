//! Exact column sections and derived vertical solids. Tessellation is not model data.
use crate::model::Point;
use glam::DVec3;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ColumnSection {
    Rectangular { width: f64, depth: f64 },
    Circular { diameter: f64 },
}

// Flat adapter preserves the original rectangular project representation.
// Circular files have an explicit discriminator and one authoritative diameter.
impl Serialize for ColumnSection {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let mut map = serializer.serialize_map(Some(2))?;
        match *self {
            Self::Rectangular { width, depth } => {
                map.serialize_entry("width", &width)?;
                map.serialize_entry("depth", &depth)?;
            }
            Self::Circular { diameter } => {
                map.serialize_entry("section", "Circular")?;
                map.serialize_entry("diameter", &diameter)?;
            }
        }
        map.end()
    }
}
impl<'de> Deserialize<'de> for ColumnSection {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Wire {
            section: Option<String>,
            width: Option<f64>,
            depth: Option<f64>,
            diameter: Option<f64>,
        }
        let w = Wire::deserialize(deserializer)?;
        match (w.section.as_deref(), w.width, w.depth, w.diameter) {
            (None | Some("Rectangular"), Some(width), Some(depth), None) => {
                Ok(Self::Rectangular { width, depth })
            }
            (Some("Circular"), None, None, Some(diameter)) => Ok(Self::Circular { diameter }),
            _ => Err(serde::de::Error::custom(
                "Invalid or conflicting column section fields",
            )),
        }
    }
}

/// Return the one world-space solid derived from the persistent column model.
pub fn solid(
    project: &crate::model::Project,
    element: &crate::model::Element,
) -> Option<ColumnSolid> {
    if let crate::model::Shape::Column {
        center,
        section,
        height,
        bottom_offset,
        axis_rotation,
        inclination: None,
        ..
    } = element.shape
    {
        Some(ColumnSolid {
            section,
            center,
            bottom: project.elevation(element.story) + bottom_offset,
            height,
            angle_degrees: axis_rotation,
        })
    } else {
        None
    }
}

/// One shared world-space derivation for both persistent vertical and inclined columns.
pub fn inclined_solid(
    project: &crate::model::Project,
    element: &crate::model::Element,
) -> Option<crate::column_inclination::InclinedColumnSolid> {
    let crate::model::Shape::Column {
        center,
        section,
        height,
        bottom_offset,
        axis_rotation,
        inclination,
        ..
    } = element.shape
    else {
        return None;
    };
    let orientation = inclination.unwrap_or(crate::column_inclination::ColumnInclination {
        slant_degrees: 90.,
        direction_degrees: 0.,
    });
    Some(crate::column_inclination::InclinedColumnSolid {
        column: ColumnSolid {
            section,
            center,
            bottom: project.elevation(element.story) + bottom_offset,
            height,
            angle_degrees: axis_rotation,
        },
        slant_degrees: orientation.slant_degrees,
        direction_degrees: orientation.direction_degrees,
    })
}

/// Only confirmed rectangular control nodes; circular editing nodes remain unsupported.
pub fn edit_nodes(element: &crate::model::Element) -> Vec<Point> {
    match element.shape {
        crate::model::Shape::Column {
            section: ColumnSection::Rectangular { .. },
            inclination: None,
            ..
        } => crate::geometry::footprint(element).to_vec(),
        _ => Vec::new(),
    }
}

impl ColumnSection {
    /// Creation-default transition, observed in AC29 INT5101 default settings.
    pub fn with_circular(self, circular: bool) -> Self {
        match (self, circular) {
            (Self::Rectangular { depth, .. }, true) => Self::Circular { diameter: depth },
            (Self::Circular { diameter }, false) => Self::Rectangular {
                width: diameter,
                depth: diameter,
            },
            _ => self,
        }
    }
    pub fn is_circular(self) -> bool {
        matches!(self, Self::Circular { .. })
    }

    pub fn validate(self) -> Result<(), String> {
        let valid = |v: f64| v.is_finite() && (0.01..=100.).contains(&v);
        let ok = match self {
            Self::Rectangular { width, depth } => valid(width) && valid(depth),
            Self::Circular { diameter } => valid(diameter),
        };
        ok.then_some(())
            .ok_or_else(|| "Invalid column section dimensions".into())
    }

    pub fn area(self) -> Result<f64, String> {
        self.validate()?;
        Ok(match self {
            Self::Rectangular { width, depth } => width * depth,
            Self::Circular { diameter } => std::f64::consts::PI * (diameter / 2.).powi(2),
        })
    }

    /// Maximum radial chord error in metres, independent of zoom.
    pub const CHORD_ERROR: f64 =
        crate::precision::GeometryContext::SAMPLING.circular_column_chord_error_m;

    pub fn outline(self, center: Point, angle_degrees: f64) -> Result<Vec<Point>, String> {
        self.outline_with_error(center, angle_degrees, Self::CHORD_ERROR)
    }

    pub(crate) fn outline_with_error(
        self,
        center: Point,
        angle_degrees: f64,
        chord_error: f64,
    ) -> Result<Vec<Point>, String> {
        self.validate()?;
        if !chord_error.is_finite() || chord_error <= 0. {
            return Err("Invalid column section dimensions".into());
        }
        if !center.finite()
            || center.x.abs() >= 100000.
            || center.y.abs() >= 100000.
            || !angle_degrees.is_finite()
        {
            return Err("Invalid column position or angle".into());
        }
        let outline = match self {
            Self::Rectangular { width, depth } => {
                crate::geometry::rotated_column_footprint(center, width, depth, angle_degrees)
                    .to_vec()
            }
            Self::Circular { diameter } => {
                let radius = f64::from(diameter) / 2.;
                let step = 2. * (1. - chord_error / radius).clamp(-1., 1.).acos();
                let count = ((std::f64::consts::TAU / step).ceil() as usize)
                    .max(32)
                    .div_ceil(4)
                    * 4;
                (0..count)
                    .map(|i| {
                        let angle = i as f64 * std::f64::consts::TAU / count as f64;
                        Point::new(
                            center.x + (radius * angle.cos()) as f64,
                            center.y + (radius * angle.sin()) as f64,
                        )
                    })
                    .collect()
            }
        };
        if outline
            .iter()
            .enumerate()
            .any(|(i, p)| !p.finite() || *p == outline[(i + 1) % outline.len()])
        {
            return Err("Column outline cannot be represented at this coordinate precision".into());
        }
        Ok(outline)
    }
}

/// Derived world-space solid, never stored as a second project geometry.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ColumnSolid {
    pub section: ColumnSection,
    pub center: Point,
    pub bottom: f64,
    pub height: f64,
    pub angle_degrees: f64,
}

pub use crate::derived_geometry::SurfaceMesh as ColumnMesh;

impl ColumnSolid {
    pub fn validate(self) -> Result<(), String> {
        self.section.validate()?;
        if !self.center.finite()
            || self.center.x.abs() >= 100000.
            || self.center.y.abs() >= 100000.
            || !self.bottom.is_finite()
            || !self.height.is_finite()
            || !(0.01..=100.).contains(&self.height)
            || !self.angle_degrees.is_finite()
            || !(self.bottom + self.height).is_finite()
            || self.bottom + self.height <= self.bottom
        {
            return Err("Invalid column solid position or height".into());
        }
        Ok(())
    }

    pub fn outline(self) -> Result<Vec<Point>, String> {
        self.validate()?;
        self.section.outline(self.center, self.angle_degrees)
    }

    pub fn bounds(self) -> Result<[DVec3; 2], String> {
        self.validate()?;
        let (x, y) = match self.section {
            ColumnSection::Circular { diameter } => (diameter / 2., diameter / 2.),
            ColumnSection::Rectangular { width, depth } => {
                let (sin, cos) = self.angle_degrees.rem_euclid(360.).to_radians().sin_cos();
                (
                    (width * cos.abs() + depth * sin.abs()) / 2.,
                    (width * sin.abs() + depth * cos.abs()) / 2.,
                )
            }
        };
        Ok([
            DVec3::new(self.center.x - x, self.center.y - y, self.bottom),
            DVec3::new(
                self.center.x + x,
                self.center.y + y,
                self.bottom + self.height,
            ),
        ])
    }

    pub fn mesh(self) -> Result<ColumnMesh, String> {
        crate::column_geometry::snapshot(crate::column_geometry::ColumnSource::Vertical(self))
            .map(|g| g.mesh)
    }

    pub(crate) fn mesh_with_error(self, chord_error: f64) -> Result<ColumnMesh, String> {
        self.validate()?;
        let outline =
            self.section
                .outline_with_error(self.center, self.angle_degrees, chord_error)?;
        let n = outline.len();
        let bottom = DVec3::new(self.center.x, self.center.y, self.bottom);
        let top = bottom + DVec3::Z * self.height;
        let mut mesh = ColumnMesh::default();
        for i in 0..n {
            let a = DVec3::new(outline[i].x, outline[i].y, self.bottom);
            let b = DVec3::new(outline[(i + 1) % n].x, outline[(i + 1) % n].y, self.bottom);
            let upper_a = a + DVec3::Z * self.height;
            let upper_b = b + DVec3::Z * self.height;
            mesh.triangles.extend([
                [bottom, b, a],
                [top, upper_a, upper_b],
                [a, b, upper_b],
                [a, upper_b, upper_a],
            ]);
            mesh.edges.extend([[a, b], [upper_a, upper_b]]);
            if matches!(self.section, ColumnSection::Rectangular { .. }) {
                mesh.edges.push([a, upper_a]);
            }
        }
        if mesh.triangles.iter().any(|[a, b, c]| {
            let normal = (*b - *a).cross(*c - *a);
            !normal.is_finite() || normal.length_squared() == 0.
        }) {
            return Err("Column mesh cannot be represented at this coordinate precision".into());
        }
        Ok(mesh)
    }

    /// Nearest nonnegative parameter t of origin + t * direction.
    /// Uses the analytic section, including caps, rather than its bounding box.
    pub fn ray_hit(self, origin: DVec3, direction: DVec3) -> Option<f64> {
        self.validate().ok()?;
        if !origin.is_finite() || !direction.is_finite() || direction == DVec3::ZERO {
            return None;
        }
        let o = [
            f64::from(origin.x) - f64::from(self.center.x),
            f64::from(origin.y) - f64::from(self.center.y),
            f64::from(origin.z) - f64::from(self.bottom),
        ];
        let d = [
            f64::from(direction.x),
            f64::from(direction.y),
            f64::from(direction.z),
        ];
        let height = f64::from(self.height);
        let answer = match self.section {
            ColumnSection::Rectangular { width, depth } => {
                let angle = f64::from(self.angle_degrees).rem_euclid(360.).to_radians();
                let (s, c) = angle.sin_cos();
                let local_o = [o[0] * c + o[1] * s, -o[0] * s + o[1] * c, o[2]];
                let local_d = [d[0] * c + d[1] * s, -d[0] * s + d[1] * c, d[2]];
                let lo = [-f64::from(width) / 2., -f64::from(depth) / 2., 0.];
                let hi = [-lo[0], -lo[1], height];
                let (mut enter, mut leave) = (f64::NEG_INFINITY, f64::INFINITY);
                for axis in 0..3 {
                    if local_d[axis] == 0. {
                        if local_o[axis] < lo[axis] || local_o[axis] > hi[axis] {
                            return None;
                        }
                    } else {
                        let a = (lo[axis] - local_o[axis]) / local_d[axis];
                        let b = (hi[axis] - local_o[axis]) / local_d[axis];
                        enter = enter.max(a.min(b));
                        leave = leave.min(a.max(b));
                        if enter > leave {
                            return None;
                        }
                    }
                }
                if enter >= 0. { enter } else { leave }
            }
            ColumnSection::Circular { diameter } => {
                let r = f64::from(diameter) / 2.;
                let mut closest = f64::INFINITY;
                let a = d[0] * d[0] + d[1] * d[1];
                if a > 0. {
                    // Avoid cancellation of large discriminant terms for distant rays.
                    let middle = -(o[0] * d[0] + o[1] * d[1]) / a;
                    let x = o[0] + middle * d[0];
                    let y = o[1] + middle * d[1];
                    let radial = r * r - x * x - y * y;
                    let delta = (radial / a).sqrt();
                    for t in [middle - delta, middle + delta] {
                        let z = o[2] + t * d[2];
                        if t >= 0. && z >= 0. && z <= height {
                            closest = closest.min(t);
                        }
                    }
                }
                if d[2] != 0. {
                    for z in [0., height] {
                        let t = (z - o[2]) / d[2];
                        let x = o[0] + t * d[0];
                        let y = o[1] + t * d[1];
                        if t >= 0. && x * x + y * y <= r * r {
                            closest = closest.min(t);
                        }
                    }
                }
                closest
            }
        };
        (answer.is_finite() && answer >= 0. && answer <= f64::from(f64::MAX))
            .then_some(answer as f64)
    }
}
