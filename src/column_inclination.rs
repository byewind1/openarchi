//! Horizontal end cuts of a constant section swept along an inclined axis.
//! This geometry core has no persistent model or UI state. Vertical columns keep
//! their existing representation; section topology and analytic picking are reused.
use crate::{
    column::{ColumnMesh, ColumnSection, ColumnSolid},
    model::Point,
};
use glam::{DVec2, DVec3};

/// Persistent orientation only. All geometry remains derived from the column.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ColumnInclination {
    pub slant_degrees: f64,
    pub direction_degrees: f64,
}

impl ColumnInclination {
    /// Resolve an accepted, non-degenerate plan axis using the fixed vertical height.
    /// Coincident-point gestures must be resolved by the caller's input contract.
    pub fn from_plan_axis(base: Point, top: Point, height: f64) -> Result<Self, String> {
        if ![base.x, base.y, top.x, top.y, height]
            .into_iter()
            .all(f64::is_finite)
            || height <= 0.
        {
            return Err(invalid());
        }
        let dx = f64::from(top.x) - f64::from(base.x);
        let dy = f64::from(top.y) - f64::from(base.y);
        let distance = dx.hypot(dy);
        if distance == 0. {
            return Err(invalid());
        }
        Ok(Self {
            slant_degrees: f64::from(height).atan2(distance).to_degrees(),
            direction_degrees: (-dx).atan2(dy).to_degrees(),
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InclinedColumnSolid {
    /// Height is vertical; angle_degrees is the rotation around the column axis.
    pub column: ColumnSolid,
    /// Angle above the horizontal plane: 90 is vertical.
    pub slant_degrees: f64,
    /// Rotation from +Y: -90 points toward +X.
    pub direction_degrees: f64,
}

#[derive(Clone, Copy)]
struct Frame {
    origin: DVec3,
    u: DVec2,
    v: DVec2,
    sin: f64,
    cot: f64,
}
fn invalid() -> String {
    "Invalid inclined column geometry".into()
}
impl Frame {
    fn point(self, p: DVec3) -> DVec3 {
        let xy = self.u * f64::from(p.x)
            + self.v * (f64::from(p.y) / self.sin + f64::from(p.z) * self.cot);
        self.origin + DVec3::new(xy.x, xy.y, f64::from(p.z))
    }
    fn inverse_vector(self, p: DVec3) -> DVec3 {
        let xy = p.truncate() - self.v * p.z * self.cot;
        DVec3::new(xy.dot(self.u), xy.dot(self.v) * self.sin, p.z)
    }
}
impl InclinedColumnSolid {
    fn frame(self) -> Result<Frame, String> {
        self.column.validate()?;
        if !self.slant_degrees.is_finite()
            || !(0. < self.slant_degrees && self.slant_degrees <= 90.)
            || !self.direction_degrees.is_finite()
        {
            return Err(invalid());
        }
        let (sin, cot) = if self.slant_degrees == 90. {
            (1., 0.)
        } else {
            let a = self.slant_degrees.to_radians();
            (a.sin(), 1. / a.tan())
        };
        let (s, c) = self
            .direction_degrees
            .rem_euclid(360.)
            .to_radians()
            .sin_cos();
        Ok(Frame {
            origin: DVec3::new(
                self.column.center.x.into(),
                self.column.center.y.into(),
                self.column.bottom.into(),
            ),
            u: DVec2::new(c, s),
            v: DVec2::new(-s, c),
            sin,
            cot,
        })
    }
    fn unchanged_vertical(self) -> bool {
        self.slant_degrees == 90. && self.direction_degrees.rem_euclid(360.) == 0.
    }
    fn local(self) -> ColumnSolid {
        ColumnSolid {
            center: Point::default(),
            bottom: 0.,
            ..self.column
        }
    }
    pub fn bounds(self) -> Result<[DVec3; 2], String> {
        let f = self.frame()?;
        let extent = match self.column.section {
            ColumnSection::Circular { diameter } => {
                let r = f64::from(diameter) / 2.;
                DVec2::new(f.u.x.hypot(f.v.x / f.sin), f.u.y.hypot(f.v.y / f.sin)) * r
            }
            ColumnSection::Rectangular { width, depth } => {
                let (s, c) = f64::from(self.column.angle_degrees)
                    .rem_euclid(360.)
                    .to_radians()
                    .sin_cos();
                let u = f.u * c + f.v * (s / f.sin);
                let v = -f.u * s + f.v * (c / f.sin);
                (u.abs() * f64::from(width) + v.abs() * f64::from(depth)) / 2.
            }
        };
        let shift = f.v * (f64::from(self.column.height) * f.cot);
        let lo = f.origin + DVec3::new(shift.x.min(0.) - extent.x, shift.y.min(0.) - extent.y, 0.);
        let hi = f.origin
            + DVec3::new(
                shift.x.max(0.) + extent.x,
                shift.y.max(0.) + extent.y,
                self.column.height.into(),
            );
        if !lo.is_finite()
            || !hi.is_finite()
            || lo.abs().max_element() >= 100000.
            || hi.abs().max_element() >= 100000.
        {
            return Err(invalid());
        }
        let lo = lo;
        let hi = hi;
        if !lo.cmplt(hi).all() {
            return Err(invalid());
        }
        if self.unchanged_vertical() {
            return self.column.bounds();
        }
        Ok([lo, hi])
    }
    pub fn volume(self) -> Result<f64, String> {
        self.bounds()?;
        let area = match self.column.section {
            ColumnSection::Rectangular { width, depth } => f64::from(width) * f64::from(depth),
            ColumnSection::Circular { diameter } => {
                std::f64::consts::PI * (f64::from(diameter) / 2.).powi(2)
            }
        };
        Ok(area * f64::from(self.column.height) / self.frame()?.sin)
    }
    /// A real section at a supplied world elevation; no floor-plan default is inferred.
    pub fn section_at(self, elevation: f64) -> Result<Vec<Point>, String> {
        self.bounds()?;
        if !elevation.is_finite()
            || elevation < self.column.bottom
            || elevation > self.column.bottom + self.column.height
        {
            return Err(invalid());
        }
        // The representable world top may round upward relative to the stored
        // height; accept that same endpoint used by bounds and mesh.
        let z = (elevation - self.column.bottom).clamp(0., self.column.height);
        if self.unchanged_vertical() {
            return self.column.outline();
        }
        let f = self.frame()?;
        self.column
            .section
            .outline_with_error(
                Point::default(),
                self.column.angle_degrees,
                ColumnSection::CHORD_ERROR * f.sin,
            )?
            .into_iter()
            .map(|p| {
                let q = f.point(DVec3::new(p.x, p.y, z));
                if !q.is_finite() {
                    return Err(invalid());
                }
                Ok(Point::new(q.x, q.y))
            })
            .collect()
    }
    pub fn mesh(self) -> Result<ColumnMesh, String> {
        crate::column_geometry::snapshot(crate::column_geometry::ColumnSource::Inclined(self))
            .map(|g| g.mesh)
    }
    pub(crate) fn mesh_source(self) -> Result<ColumnMesh, String> {
        self.bounds()?;
        if self.unchanged_vertical() {
            return self.column.mesh_with_error(ColumnSection::CHORD_ERROR);
        }
        let f = self.frame()?;
        let mut mesh = self
            .local()
            .mesh_with_error(ColumnSection::CHORD_ERROR * f.sin)?;
        for triangle in &mut mesh.triangles {
            for point in triangle {
                *point = f.point(*point);
            }
        }
        for edge in &mut mesh.edges {
            for point in edge {
                *point = f.point(*point);
            }
        }
        if mesh.triangles.iter().any(|[a, b, c]| {
            let n = (*b - *a).cross(*c - *a);
            !n.is_finite() || n.length_squared() == 0.
        }) {
            return Err(invalid());
        }
        Ok(mesh)
    }
    /// The inverse affine map preserves the original ray parameter, including
    /// non-unit directions. The existing exact section solver handles caps/sides.
    pub fn ray_hit(self, origin: DVec3, direction: DVec3) -> Option<f64> {
        self.bounds().ok()?;
        if !origin.is_finite() || !direction.is_finite() || direction == DVec3::ZERO {
            return None;
        }
        if self.unchanged_vertical() {
            return self.column.ray_hit(origin, direction);
        }
        let f = self.frame().ok()?;
        self.local().ray_hit(
            f.inverse_vector(origin - f.origin),
            f.inverse_vector(direction),
        )
    }
}
