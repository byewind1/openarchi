//! Single rectangular beam geometry with a top-center axis and vertical ends.
//! Section height is measured normal to the inclined axis, not vertically.
use crate::{
    geometry,
    model::{ArcSegment, Point},
};
use glam::DVec3;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BeamGeometry {
    pub start: Point,
    pub end: Point,
    pub start_top: f64,
    pub width: f64,
    pub height: f64,
    pub slant_angle: f64,
    pub arc: Option<ArcSegment>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BeamSolid {
    /// Straight beams: actual bottom-four/top-four corners.
    /// Curved beams: conservative axis-aligned bounds ONLY; use mesh for surfaces.
    pub corners: [DVec3; 8],
    pub axis_length: f64,
    pub volume: f64,
}

impl BeamGeometry {
    pub fn solid(self) -> Result<BeamSolid, String> {
        let invalid = || "Invalid beam geometry or parameters".to_owned();
        if ![self.start, self.end]
            .iter()
            .all(|p| p.finite() && p.x.abs() < 100000. && p.y.abs() < 100000.)
            || !self.start_top.is_finite()
            || ![self.width, self.height]
                .iter()
                .all(|v| v.is_finite() && (0.01..=100.).contains(v))
            || !self.slant_angle.is_finite()
            || !(-89. ..=89.).contains(&self.slant_angle)
        {
            return Err(invalid());
        }
        if let Some(arc) = self.arc {
            if self.slant_angle != 0. || arc.radius <= self.width * 0.5 {
                return Err(invalid());
            }
            let (_, sweep) =
                geometry::arc_center_sweep(self.start, self.end, arc).ok_or_else(invalid)?;
            let outline = self.outline()?;
            let (min, max) = crate::slab_geometry::bounds(&outline);
            let rect = [min, Point::new(max.x, min.y), max, Point::new(min.x, max.y)];
            let corners = std::array::from_fn(|i| {
                DVec3::new(
                    rect[i % 4].x,
                    rect[i % 4].y,
                    self.start_top - if i < 4 { self.height } else { 0. },
                )
            });
            if corners
                .iter()
                .any(|p| !p.is_finite() || p.abs().max_element() >= 100000.)
            {
                return Err(invalid());
            }
            let axis_length = arc.radius as f64 * sweep.abs() as f64;
            return Ok(BeamSolid {
                corners,
                axis_length,
                volume: axis_length * self.width as f64 * self.height as f64,
            });
        }
        let dx = self.end.x as f64 - self.start.x as f64;
        let dy = self.end.y as f64 - self.start.y as f64;
        let length = dx.hypot(dy);
        if !crate::precision::segment_meets_minimum(
            self.start,
            self.end,
            crate::precision::GeometryContext::CONSTRUCTION.minimum_edge_m,
        ) {
            return Err(invalid());
        }
        let angle = (self.slant_angle as f64).to_radians();
        let outline = geometry::wall_footprint(self.start, self.end, self.width, 0.);
        let corners: [DVec3; 8] = std::array::from_fn(|i| {
            let p = outline[i % 4];
            let bottom = self.bottom_at(p);
            DVec3::new(
                p.x,
                p.y,
                bottom + if i < 4 { 0. } else { self.height / angle.cos() },
            )
        });
        if corners
            .iter()
            .any(|p| !p.is_finite() || p.abs().max_element() >= 100000.)
        {
            return Err(invalid());
        }
        // Finite inputs can still collapse after conversion to model precision.
        for face in geometry::FACES {
            let a = corners[face[0]];
            let b = corners[face[1]];
            let c = corners[face[2]];
            let d = corners[face[3]];
            if (b - a).cross(c - a).length_squared()
                <= crate::precision::GeometryContext::TOPOLOGY.beam_face_double_area_squared_m4
                || (c - a).cross(d - a).length_squared()
                    <= crate::precision::GeometryContext::TOPOLOGY.beam_face_double_area_squared_m4
            {
                return Err(invalid());
            }
        }
        let axis_length = length / angle.cos();
        Ok(BeamSolid {
            corners,
            axis_length,
            volume: axis_length * self.width as f64 * self.height as f64,
        })
    }
}

/// Domain adapter: story-relative model parameters become absolute geometry.
pub fn geometry(
    project: &crate::model::Project,
    element: &crate::model::Element,
) -> Option<BeamGeometry> {
    let crate::model::Shape::Beam {
        start,
        end,
        width,
        height,
        top_offset,
        slant_angle,
        arc,
    } = element.shape
    else {
        return None;
    };
    Some(BeamGeometry {
        start,
        end,
        width,
        height,
        slant_angle,
        arc,
        start_top: project.elevation(element.story) + top_offset,
    })
}

/// Exact projected area, shared by model quantities and derived snapshots.
pub fn plan_area(start: Point, end: Point, width: f64, arc: Option<ArcSegment>) -> f64 {
    arc.map_or_else(
        || start.distance(end),
        |arc| geometry::arc_length(start, end, arc),
    ) * width
}

impl BeamGeometry {
    fn bottom_at(self, p: Point) -> f64 {
        let dx = self.end.x - self.start.x;
        let dy = self.end.y - self.start.y;
        let angle = self.slant_angle.to_radians();
        self.start_top
            + ((p.x - self.start.x) * dx + (p.y - self.start.y) * dy) / dx.hypot(dy) * angle.tan()
            - self.height / angle.cos()
    }
}

pub use crate::sloped_prism::PrismMesh as BeamMesh;
impl BeamGeometry {
    pub(crate) fn plan_polygon(self) -> Result<geo::Polygon<f64>, String> {
        use geo::{Orient, orient::Direction};
        let outline = self.outline()?;
        Ok(geo::Polygon::new(
            geo::LineString::from(
                outline
                    .iter()
                    .chain(outline.first())
                    .map(|p| (p.x, p.y))
                    .collect::<Vec<_>>(),
            ),
            vec![],
        )
        .orient(Direction::Default))
    }
    /// All display and picking callers consume this same validated snapshot.
    pub fn mesh(
        self,
        clip: Option<&[Point]>,
        z_range: Option<(f64, f64)>,
    ) -> Result<BeamMesh, String> {
        crate::beam_geometry::snapshot(self)?.clipped(clip, z_range)
    }

    pub(crate) fn mesh_source(
        self,
        clip: Option<&[Point]>,
        z_range: Option<(f64, f64)>,
    ) -> Result<BeamMesh, String> {
        self.solid()?;
        let outline = self.outline()?;
        let (min, max) = crate::slab_geometry::bounds(&outline);
        let polygon = self.plan_polygon()?;
        let mut mesh = crate::sloped_prism::mesh(
            geo::MultiPolygon(vec![polygon]),
            (min, max),
            self.height / self.slant_angle.to_radians().cos(),
            |p| self.bottom_at(p),
            clip,
            z_range,
        );
        if self.arc.is_some() {
            // The prism tessellator outlines every facet. Circular side facets
            // approximate one smooth surface: their vertical joins are not edges.
            // Keep the four radial-end corners and any genuine clipping corner.
            let half = outline.len() / 2;
            mesh.edges.retain(|[a, b]| {
                let point = Point::new(a.x, a.y);
                if point.distance(Point::new(b.x, b.y)) > 1e-6 {
                    return true;
                }
                if clip.is_some_and(|ring| {
                    (0..ring.len()).any(|i| {
                        geometry::point_on_segment(point, ring[i], ring[(i + 1) % ring.len()], 2e-5)
                    })
                }) {
                    return true;
                }
                !outline.iter().enumerate().any(|(i, sample)| {
                    ![0, half - 1, half, outline.len() - 1].contains(&i)
                        && point.distance(*sample) < 2e-5
                })
            });
        }
        Ok(mesh)
    }
}

pub(crate) fn is_zero(value: &f64) -> bool {
    *value == 0.
}

impl BeamGeometry {
    /// Shared plan boundary; arc subdivisions include cardinal extrema for bounds.
    pub fn outline(self) -> Result<Vec<Point>, String> {
        let Some(arc) = self.arc else {
            return Ok(geometry::wall_footprint(self.start, self.end, self.width, 0.).to_vec());
        };
        let invalid = || "Invalid beam geometry or parameters".to_owned();
        let (center, sweep) =
            geometry::arc_center_sweep(self.start, self.end, arc).ok_or_else(invalid)?;
        if !self.width.is_finite() || self.width <= 0. || arc.radius <= self.width * 0.5 {
            return Err(invalid());
        }
        let start = (self.start.y - center.y).atan2(self.start.x - center.x);
        let steps = (sweep.abs()
            / crate::precision::GeometryContext::SAMPLING.beam_arc_max_step_rad)
            .ceil()
            .max(2.) as usize;
        let mut fractions: Vec<f64> = (0..=steps).map(|i| i as f64 / steps as f64).collect();
        for i in 0..4 {
            let angle = i as f64 * std::f64::consts::FRAC_PI_2;
            let travel = if sweep > 0. {
                (angle - start).rem_euclid(std::f64::consts::TAU)
            } else {
                (start - angle).rem_euclid(std::f64::consts::TAU)
            };
            let t = travel / sweep.abs();
            if t > 0. && t < 1. {
                fractions.push(t);
            }
        }
        fractions.sort_by(f64::total_cmp);
        fractions.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
        let point = |t: f64, radius: f64| {
            let a = start + sweep * t;
            Point::new(center.x + radius * a.cos(), center.y + radius * a.sin())
        };
        let mut out: Vec<_> = fractions
            .iter()
            .map(|&t| point(t, arc.radius + self.width * 0.5))
            .collect();
        out.extend(
            fractions
                .iter()
                .rev()
                .map(|&t| point(t, arc.radius - self.width * 0.5)),
        );
        Ok(out)
    }
}
