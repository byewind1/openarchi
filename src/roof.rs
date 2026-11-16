//! Single-plane rectangular roofs; the pivot belongs to the bottom surface.
use crate::model::Point;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RoofSpec {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contour: Option<crate::roof_contour::RoofContour>,
    pub pivot_start: Point,
    pub pivot_end: Point,
    pub rise_left: bool,
    pub min: Point,
    pub max: Point,
    pub pitch: f64,
    pub thickness: f64,
    pub pivot_offset: f64,
}
impl RoofSpec {
    pub fn validate(&self) -> Result<(), String> {
        if let Some(c) = &self.contour {
            c.validate()?;
            if c.bounds() != Some((self.min, self.max)) {
                return Err("Invalid roof geometry or parameters".into());
            }
        }
        let point_ok = |p: Point| p.finite() && p.x.abs() < 100000. && p.y.abs() < 100000.;
        // A single-plane height is affine over the footprint. Its extrema
        // occur at actual outer vertices, not at unoccupied bounding corners.
        let rectangle = self.outline();
        let height_vertices = self
            .contour
            .as_ref()
            .map_or(rectangle.as_slice(), |contour| contour.outer.as_slice());
        if ![self.pivot_start, self.pivot_end, self.min, self.max]
            .into_iter()
            .all(point_ok)
            || !crate::precision::segment_meets_minimum(
                self.pivot_start,
                self.pivot_end,
                crate::precision::GeometryContext::CONSTRUCTION.minimum_edge_m,
            )
            || !crate::precision::axis_meets_minimum(
                self.min.x,
                self.max.x,
                crate::precision::GeometryContext::CONSTRUCTION.minimum_edge_m,
            )
            || !crate::precision::axis_meets_minimum(
                self.min.y,
                self.max.y,
                crate::precision::GeometryContext::CONSTRUCTION.minimum_edge_m,
            )
            || !self.pitch.is_finite()
            || !(-80. ..=80.).contains(&self.pitch)
            || !self.thickness.is_finite()
            || !(0.01..=5.).contains(&self.thickness)
            || !self.pivot_offset.is_finite()
            || !(-100. ..=100.).contains(&self.pivot_offset)
            || height_vertices
                .iter()
                .any(|&p| !self.bottom(p, 0.).is_finite() || self.bottom(p, 0.).abs() > 100000.)
        {
            return Err("Invalid roof geometry or parameters".into());
        }
        Ok(())
    }
    pub fn outline(&self) -> [Point; 4] {
        [
            self.min,
            Point::new(self.max.x, self.min.y),
            self.max,
            Point::new(self.min.x, self.max.y),
        ]
    }
    pub fn footprint_contour(&self) -> crate::roof_contour::RoofContour {
        self.contour
            .clone()
            .unwrap_or_else(|| crate::roof_contour::RoofContour {
                outer: self.outline().to_vec(),
                holes: vec![],
            })
    }
    pub fn bottom(&self, p: Point, story: f64) -> f64 {
        let a = self.pivot_start;
        let b = self.pivot_end;
        let len = a.distance(b).max(0.001);
        let distance = ((b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x)) / len;
        story
            + self.pivot_offset
            + distance * if self.rise_left { 1. } else { -1. } * self.pitch.to_radians().tan()
    }
    pub fn translate(&mut self, d: Point) {
        if let Some(c) = &mut self.contour {
            c.translate(d);
        }
        for p in [
            &mut self.pivot_start,
            &mut self.pivot_end,
            &mut self.min,
            &mut self.max,
        ] {
            p.x += d.x;
            p.y += d.y;
        }
    }
    pub fn area(&self) -> f64 {
        self.footprint_contour().area() as f64 / self.pitch.to_radians().cos()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RoofGeometryMethod {
    Polygonal,
    #[default]
    Rectangular,
    RotatedRectangle,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RoofDraft {
    RotatedContour {
        start: Point,
        end: Point,
        rise_left: bool,
        rectangle: crate::slab_creation::SlabDraft,
    },
    PivotEnd {
        start: Point,
    },
    RiseSide {
        start: Point,
        end: Point,
    },
    ContourStart {
        start: Point,
        end: Point,
        rise_left: bool,
    },
    ContourEnd {
        start: Point,
        end: Point,
        rise_left: bool,
        first: Point,
    },
}
impl RoofDraft {
    pub fn numeric_values(self, point: Point) -> Option<(f64, f64)> {
        match self {
            Self::PivotEnd { start } => Some((
                start.distance(point),
                (point.y - start.y).atan2(point.x - start.x).to_degrees(),
            )),
            Self::RotatedContour { rectangle, .. } => Some(rectangle.numeric_values(point)),
            Self::ContourEnd { first, .. } => Some((point.x - first.x, point.y - first.y)),
            _ => None,
        }
    }

    pub fn numeric_target(self, a: f64, b: f64) -> Result<Point, String> {
        let invalid = || "Invalid roof geometry or parameters".to_owned();
        if !a.is_finite() || !b.is_finite() {
            return Err(invalid());
        }
        let point = match self {
            Self::RotatedContour { rectangle, .. } => {
                return rectangle.numeric_target(a, b).map_err(|_| invalid());
            }
            Self::PivotEnd { start } => {
                if a < 0.01 {
                    return Err("Roof pivot line is too short".into());
                }
                let angle = b.to_radians();
                Point::new(start.x + a * angle.cos(), start.y + a * angle.sin())
            }
            Self::ContourEnd { first, .. } => {
                if a.abs() < 0.01 || b.abs() < 0.01 {
                    return Err(invalid());
                }
                Point::new(first.x + a, first.y + b)
            }
            _ => return Err(invalid()),
        };
        if !point.finite() || point.x.abs() >= 100000. || point.y.abs() >= 100000. {
            return Err("Coordinates must be finite and within the local modeling range".into());
        }
        Ok(point)
    }
    pub fn pivot(self) -> (Point, Option<Point>) {
        match self {
            Self::PivotEnd { start } => (start, None),
            Self::RiseSide { start, end }
            | Self::ContourStart { start, end, .. }
            | Self::ContourEnd { start, end, .. }
            | Self::RotatedContour { start, end, .. } => (start, Some(end)),
        }
    }
}

pub use crate::sloped_prism::PrismMesh as RoofMesh;

/// Compatibility mesh entry over the same validated snapshot used by queries.
pub fn mesh(
    spec: &RoofSpec,
    story: f64,
    clip: Option<&[Point]>,
    z_range: Option<(f64, f64)>,
) -> Result<RoofMesh, String> {
    crate::roof_geometry::geometry(spec, story).and_then(|g| g.clipped(clip, z_range))
}

/// Explicit contour replacement keeps legacy bounds-override behavior without
/// mutating the domain input. All consumers still use the canonical derivation.
pub fn mesh_contour(
    spec: &RoofSpec,
    contour: &crate::roof_contour::RoofContour,
    story: f64,
    clip: Option<&[Point]>,
    z_range: Option<(f64, f64)>,
) -> Result<RoofMesh, String> {
    let Some((min, max)) = contour.bounds() else {
        return Err("Invalid roof contour bounds".into());
    };
    let mut bounded = spec.clone();
    bounded.contour = Some(contour.clone());
    bounded.min = min;
    bounded.max = max;
    mesh(&bounded, story, clip, z_range)
}
