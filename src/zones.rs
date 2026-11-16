//! Authored spaces. Their boundary is explicit and never masquerades as a slab
//! or claims to follow walls. Shared prism derivation supplies the geometry.
use crate::model::{Element, Point, Shape, SlabContour};
use serde::{Deserialize, Serialize};
pub mod analysis;
pub mod surfaces;

impl crate::editor::Editor {
    pub fn place_zone(&mut self, spec: ZoneSpec) -> Result<u64, String> {
        spec.validate()?;
        let mut element =
            self.creation_candidate(self.material, self.wall_layer, Shape::Zone { spec });
        if !self.zone_name.trim().is_empty() {
            element.name = self.zone_name.clone();
        }
        let id = element.id;
        self.apply(crate::command::ModelCommand::AddElement { element })?;
        self.set_selection(vec![id]);
        Ok(id)
    }
    pub fn zone_candidate(&self, outline: Vec<Point>) -> ZoneSpec {
        ZoneSpec {
            surface: Default::default(),
            purpose: self.zone_purpose.clone(),
            outline,
            holes: vec![],
            height: self.zone_height,
            bottom_offset: 0.,
            label_position: None,
        }
    }
    pub fn finish_zone(&mut self) -> Result<u64, String> {
        let spec = self.zone_candidate(self.polyline_points.clone());
        let id = self.place_zone(spec)?;
        self.polyline_points.clear();
        self.anchor = None;
        self.status = "Space created".into();
        Ok(id)
    }
    pub fn zone_preview(&self, p: Point) -> Vec<Point> {
        if self.zone_polygon {
            let mut v = self.polyline_points.clone();
            v.push(p);
            v
        } else if let Some(a) = self.anchor {
            vec![a, Point::new(p.x, a.y), p, Point::new(a.x, p.y)]
        } else {
            vec![]
        }
    }
    pub(crate) fn click_zone(&mut self, p: Point, tolerance: f64) {
        if self.zone_polygon {
            if self.polyline_points.len() >= 3 && self.polyline_points[0].distance(p) <= tolerance {
                if let Err(e) = self.finish_zone() {
                    self.status = e;
                }
            } else {
                self.polyline_points.push(p);
            }
        } else if self.anchor.is_some() {
            let spec = self.zone_candidate(self.zone_preview(p));
            match self.place_zone(spec) {
                Ok(_) => {
                    self.anchor = None;
                    self.status = "Space created".into();
                }
                Err(e) => self.status = e,
            }
        } else {
            self.anchor = Some(p);
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ZoneSpec {
    #[serde(default, skip_serializing_if = "surfaces::ZoneSurface::is_inherit")]
    pub surface: surfaces::ZoneSurface,
    pub purpose: String,
    pub outline: Vec<Point>,
    #[serde(default)]
    pub holes: Vec<Vec<Point>>,
    #[serde(default = "default_height")]
    pub height: f64,
    #[serde(default)]
    pub bottom_offset: f64,
    #[serde(default)]
    pub label_position: Option<Point>,
}
pub fn default_height() -> f64 {
    3.
}
impl ZoneSpec {
    pub fn contour(&self) -> SlabContour {
        SlabContour {
            points: self.outline.clone(),
            holes: self
                .holes
                .iter()
                .map(|p| SlabContour {
                    points: p.clone(),
                    ..Default::default()
                })
                .collect(),
            elevation: self.bottom_offset + self.height,
            ..Default::default()
        }
    }
    pub fn validate(&self) -> Result<(), String> {
        self.surface.validate()?;
        if self.purpose.trim().is_empty()
            || self.purpose.len() > 256
            || !self.height.is_finite()
            || !(0.01..=100.).contains(&self.height)
            || !self.bottom_offset.is_finite()
            || !(-100. ..=100.).contains(&self.bottom_offset)
            || self
                .label_position
                .is_some_and(|p| !crate::precision::GeometryContext::point_in_range(p))
        {
            return Err("Invalid space parameters".into());
        }
        crate::slab_geometry::validate_contour(&self.contour())
    }
    pub fn area(&self) -> f64 {
        crate::geometry::polygon_area(&self.outline).abs()
            - self
                .holes
                .iter()
                .map(|p| crate::geometry::polygon_area(p).abs())
                .sum::<f64>()
    }
    pub fn transform(&mut self, f: impl Fn(Point) -> Point) {
        for p in self
            .outline
            .iter_mut()
            .chain(self.holes.iter_mut().flatten())
        {
            *p = f(*p);
        }
        self.label_position = self.label_position.map(f);
    }
    pub fn label_point(&self) -> Point {
        if let Some(p) = self.label_position {
            return p;
        }
        use geo::{Centroid, Contains, InteriorPoint};
        let polygon = self.polygon();
        if let Some(point) = polygon.centroid() {
            if polygon.contains(&point) {
                return Point::new(point.x(), point.y());
            }
        }
        let p = polygon.interior_point();
        p.map(|p| Point::new(p.x(), p.y())).unwrap_or_default()
    }
    pub fn polygon(&self) -> geo::Polygon<f64> {
        fn ring(p: &[Point]) -> geo::LineString<f64> {
            let mut v: Vec<_> = p.iter().map(|p| (p.x, p.y)).collect();
            if v.first() != v.last() {
                if let Some(p) = v.first().copied() {
                    v.push(p);
                }
            }
            v.into()
        }
        geo::Polygon::new(
            ring(&self.outline),
            self.holes.iter().map(|p| ring(p)).collect(),
        )
    }
}
/// Adapt only an ephemeral prism input, never the authoritative model type.
pub fn derive(
    e: &Element,
    base: f64,
) -> Result<crate::slab_geometry::DerivedSlab, crate::derived_geometry::GeometryDiagnostic> {
    let Shape::Zone { spec } = &e.shape else {
        return Err(crate::derived_geometry::GeometryDiagnostic::error(
            e.id,
            "NOT_A_ZONE",
            "space",
            "Element is not a space",
        ));
    };
    spec.validate().map_err(|m| {
        crate::derived_geometry::GeometryDiagnostic::error(e.id, "INVALID_ZONE", "space", m)
    })?;
    let mut input = e.clone();
    let (min, max) = crate::slab_geometry::bounds(&spec.outline);
    input.shape = Shape::Slab {
        min,
        max,
        thickness: spec.height,
    };
    input.slab_contour = Some(spec.contour());
    let mut g = crate::slab_geometry::derive_space_prism(&input, base)?;
    g.identity = crate::derived_geometry::identity(e.id, spec, base, 1).map_err(|m| {
        crate::derived_geometry::GeometryDiagnostic::error(e.id, "INVALID_ZONE", "space", m)
    })?;
    Ok(g)
}
