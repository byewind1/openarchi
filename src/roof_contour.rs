//! Straight-edged roof footprint data, independent of slab elevation/edge settings.
use crate::model::{Point, SlabContour};
use geo::{Area, Contains, Orient, orient::Direction};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RoofContour {
    pub outer: Vec<Point>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub holes: Vec<Vec<Point>>,
}

impl RoofContour {
    pub fn validate(&self) -> Result<(), String> {
        let ring = |points: &Vec<Point>| SlabContour {
            points: points.clone(),
            ..Default::default()
        };
        let mut topology = ring(&self.outer);
        topology.holes = self.holes.iter().map(ring).collect();
        crate::slab_geometry::validate_contour(&topology)
            .map_err(|_| "Invalid roof geometry or parameters".into())
    }

    pub fn rings(&self) -> impl Iterator<Item = &[Point]> {
        std::iter::once(self.outer.as_slice()).chain(self.holes.iter().map(Vec::as_slice))
    }

    /// Derivation only: normalize winding without rewriting persisted input.
    pub fn polygon(&self) -> geo::Polygon<f64> {
        let ring = |points: &[Point]| {
            let mut coords: Vec<_> = points.iter().map(|p| (p.x as f64, p.y as f64)).collect();
            if let Some(first) = coords.first().copied() {
                coords.push(first);
            }
            geo::LineString::from(coords)
        };
        geo::Polygon::new(
            ring(&self.outer),
            self.holes.iter().map(|p| ring(p)).collect(),
        )
        .orient(Direction::Default)
    }

    pub fn area(&self) -> f64 {
        self.polygon().unsigned_area()
    }

    pub fn contains(&self, point: Point) -> bool {
        self.polygon()
            .contains(&geo::Point::new(point.x as f64, point.y as f64))
    }

    pub fn bounds(&self) -> Option<(Point, Point)> {
        let first = *self.outer.first()?;
        Some(self.outer.iter().fold((first, first), |(a, b), p| {
            (
                Point::new(a.x.min(p.x), a.y.min(p.y)),
                Point::new(b.x.max(p.x), b.y.max(p.y)),
            )
        }))
    }

    pub fn triangles(&self) -> Vec<[Point; 3]> {
        crate::derived_geometry::triangulate_with_boundary(&self.polygon())
            .into_iter()
            .map(|t| t.map(|p| Point::new(p.x, p.y)))
            .collect()
    }

    pub fn translate(&mut self, offset: Point) {
        for p in self.outer.iter_mut().chain(self.holes.iter_mut().flatten()) {
            p.x += offset.x;
            p.y += offset.y;
        }
    }
}
