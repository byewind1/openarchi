//! Read-only orthographic expression of the existing checked prism kernel.
//! Positions here are display offsets, never edits to the shared model.
use super::{Diagram, Presentation};
use crate::model::Shape;
use glam::DVec3;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DisplayOffset {
    pub element_id: u64,
    /// Explicit model-axis display displacement in millimetres, not a measured location.
    pub xyz: [i32; 3],
}
pub fn validate_offsets(offsets: &[DisplayOffset]) -> Result<(), String> {
    if offsets.len() > 32
        || offsets
            .iter()
            .any(|v| v.element_id == 0 || v.xyz.iter().any(|n| !(-100_000..=100_000).contains(n)))
        || offsets
            .windows(2)
            .any(|w| w[0].element_id >= w[1].element_id)
    {
        return Err(
            "Axon offsets require unique ascending object IDs and each axis within ±100000 mm"
                .into(),
        );
    }
    Ok(())
}
/// Fixed equal-axis orthographic camera looking from +X,+Y,+Z towards the origin.
/// Screen Y points down. Subtract the local origin before calling this function.
pub fn project(v: DVec3) -> [f64; 2] {
    [
        (v.x - v.y) / 2_f64.sqrt(),
        (v.x + v.y - 2. * v.z) / 6_f64.sqrt(),
    ]
}
#[derive(Clone, Debug)]
pub struct Face {
    pub element_id: u64,
    pub points: [[f64; 2]; 3],
    pub color: [u8; 3],
    pub depth: f64,
}
#[derive(Clone, Debug)]
pub struct Edge {
    pub element_id: u64,
    pub points: [[f64; 2]; 2],
}
#[derive(Clone, Debug)]
pub struct Label {
    pub element_id: u64,
    pub point: [f64; 2],
}
pub struct Page {
    pub faces: Vec<Face>,
    pub edges: Vec<Edge>,
    pub labels: Vec<Label>,
    pub origin: DVec3,
    pub min: [f64; 2],
    pub max: [f64; 2],
    pub pixels_per_m: f64,
}
impl Page {
    pub fn new(d: &Diagram) -> Result<Self, String> {
        let Presentation::ExplodedAxon { offsets_mm } = &d.definition.presentation else {
            return Err("Figure is not an exploded axon".into());
        };
        validate_offsets(offsets_mm)?;
        if offsets_mm
            .iter()
            .any(|v| !d.snapshot.elements.iter().any(|e| e.id == v.element_id))
        {
            return Err("Axon offset refers to an object outside its source snapshot".into());
        }
        let mut solids = Vec::new();
        let mut origin = DVec3::splat(f64::INFINITY);
        for e in &d.snapshot.elements {
            let g = match e.shape {
                Shape::Zone { .. } => crate::zones::derive(e, d.snapshot.story.elevation),
                Shape::ConceptMass { .. } => {
                    crate::concept_mass::derive(e, d.snapshot.story.elevation)
                }
                _ => return Err("Unsupported axon source geometry".into()),
            }
            .map_err(|e| e.message)?;
            origin = origin.min(g.bounds[0]);
            solids.push((e, g));
        }
        if !origin.is_finite() {
            return Err("Axon requires source geometry".into());
        }
        let mut p = Self {
            faces: vec![],
            edges: vec![],
            labels: vec![],
            origin,
            min: [f64::INFINITY; 2],
            max: [f64::NEG_INFINITY; 2],
            pixels_per_m: 1.,
        };
        let camera = DVec3::ONE.normalize();
        for (e, g) in solids {
            let offset = offsets_mm
                .iter()
                .find(|v| v.element_id == e.id)
                .map(|v| DVec3::from_array(v.xyz.map(|n| f64::from(n) / 1000.)))
                .unwrap_or(DVec3::ZERO);
            let local = |v: DVec3| v - origin + offset;
            for tri in g.triangles() {
                let tri = tri.map(local);
                let n = (tri[1] - tri[0]).cross(tri[2] - tri[0]).normalize();
                // Translucent conceptual shells: keep both sides, sorted back to front.
                let shade = 0.62 + 0.38 * n.dot(camera).max(0.);
                let color = d
                    .element_color(e)
                    .map(|n| (f64::from(n) * shade).round() as u8);
                let points = tri.map(project);
                for q in points {
                    p.include(q);
                }
                p.faces.push(Face {
                    element_id: e.id,
                    points,
                    color,
                    depth: (tri[0] + tri[1] + tri[2]).dot(camera) / 3.,
                });
            }
            for edge in g.edges {
                p.edges.push(Edge {
                    element_id: e.id,
                    points: edge.map(|v| project(local(v))),
                });
            }
            let center = (g.bounds[0] + g.bounds[1]) / 2.;
            p.labels.push(Label {
                element_id: e.id,
                point: project(local(center)),
            });
        }
        p.faces.sort_by(|a, b| {
            a.depth
                .total_cmp(&b.depth)
                .then(a.element_id.cmp(&b.element_id))
        });
        p.pixels_per_m =
            (750. / (p.max[0] - p.min[0]).max(0.01)).min(580. / (p.max[1] - p.min[1]).max(0.01));
        Ok(p)
    }
    fn include(&mut self, q: [f64; 2]) {
        for (i, v) in q.into_iter().enumerate() {
            self.min[i] = self.min[i].min(v);
            self.max[i] = self.max[i].max(v);
        }
    }
    pub fn point(&self, q: [f64; 2]) -> [f64; 2] {
        [
            40. + (750. - (self.max[0] - self.min[0]) * self.pixels_per_m) / 2.
                + (q[0] - self.min[0]) * self.pixels_per_m,
            115. + (580. - (self.max[1] - self.min[1]) * self.pixels_per_m) / 2.
                + (q[1] - self.min[1]) * self.pixels_per_m,
        ]
    }
    /// Legend-only world-axis directions, at a fixed page size for legibility.
    /// These are explicitly direction guides, not a paper or page scale bar.
    pub fn axes() -> [(&'static str, [f64; 2]); 3] {
        [
            ("+X", project(DVec3::X)),
            ("+Y", project(DVec3::Y)),
            ("+Z", project(DVec3::Z)),
        ]
    }
}

pub(super) fn append_svg(d: &Diagram, s: &mut String) -> Result<(), String> {
    use std::fmt::Write as _;
    let page = Page::new(d)?;
    write!(s,"<metadata id=\"axon-projection\">{}</metadata>",super::xml(&serde_json::json!({"camera":"orthographic_isometric_from_positive_xyz","origin_m":page.origin.to_array(),"min":page.min,"max":page.max,"pixels_per_m":page.pixels_per_m,"units":"m","offsets":"explicit xyz millimetres in definition; unlisted = zero; does not move model","surfaces":"translucent conceptual prisms including back faces; no hidden-surface claim"}).to_string())).unwrap();
    for f in &page.faces {
        let p = f.points.map(|q| page.point(q));
        let c = f.color;
        write!(s,r#"<path data-element-id="{}" data-axon-face="true" d="M {:.8} {:.8} L {:.8} {:.8} L {:.8} {:.8} Z" fill="rgb({},{},{})" fill-opacity="0.45"/>"#,f.element_id,p[0][0],p[0][1],p[1][0],p[1][1],p[2][0],p[2][1],c[0],c[1],c[2]).unwrap();
    }
    for e in &page.edges {
        let p = e.points.map(|q| page.point(q));
        write!(s,r##"<path data-element-id="{}" data-axon-edge="true" d="M {:.8} {:.8} L {:.8} {:.8}" fill="none" stroke="#303b46" stroke-opacity="0.65" stroke-width="1"/>"##,e.element_id,p[0][0],p[0][1],p[1][0],p[1][1]).unwrap();
    }
    for label in &page.labels {
        let p = page.point(label.point);
        write!(s,r#"<text x="{:.8}" y="{:.8}" text-anchor="middle" font-family="sans-serif" font-size="12">#{}</text>"#,p[0],p[1],label.element_id).unwrap();
    }
    for (name, v) in Page::axes() {
        write!(
            s,
            r##"<path d="M 780 735 l {} {}" fill="none" stroke="#303b46"/>"##,
            v[0] * 35.,
            v[1] * 35.
        )
        .unwrap();
        super::svg_text(s, 780. + v[0] * 48., 735. + v[1] * 48., name, 11.);
    }
    for (x, y, text, size) in d.text_lines() {
        super::svg_text(s, x, y, &text, size);
    }
    Ok(())
}
