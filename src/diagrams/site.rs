//! Source-bound plan expression. A study inset is not a legal building line.
use super::{Diagram, Presentation, Snapshot};
use crate::{
    model::{Element, Point, Project, Shape},
    site::SiteContext,
};
use geo::{
    Area, Buffer, LineString, Polygon,
    algorithm::buffer::{BufferStyle, LineJoin},
};
use serde::{Deserialize, Serialize};

pub const ASSUMPTION: &str = "Concept study only; no approved setback requirement";
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub geometry_version: u32,
    /// Existing declarations, excluding terrain which is not drawn by this plan.
    pub context: SiteContext,
    pub boundary_element: Option<Element>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub control_element: Option<Element>,
}
impl Source {
    pub fn capture(p: &Project) -> Result<Self, String> {
        let mut context = p.site.clone();
        context.terrain = None;
        let boundary_element = context
            .boundary
            .as_ref()
            .map(|b| {
                p.elements
                    .iter()
                    .find(|e| e.id == b.element_id)
                    .cloned()
                    .ok_or("Site boundary source element is missing")
            })
            .transpose()?;
        let control_element = context
            .control_region
            .as_ref()
            .map(|b| {
                p.elements
                    .iter()
                    .find(|e| e.id == b.element_id)
                    .cloned()
                    .ok_or("Control region source element is missing")
            })
            .transpose()?;
        let s = Self {
            geometry_version: if control_element.is_some() { 2 } else { 1 },
            context,
            boundary_element,
            control_element,
        };
        s.validate()?;
        Ok(s)
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.geometry_version != if self.control_element.is_some() { 2 } else { 1 }
            || self.context.terrain.is_some()
        {
            return Err("Unsupported site figure source version".into());
        }
        self.context.validate()?;
        for (reference, element) in [
            (&self.context.boundary, &self.boundary_element),
            (&self.context.control_region, &self.control_element),
        ] {
            match (reference, element) {
                (None, None) => {}
                (Some(b), Some(e)) if b.element_id == e.id && e.worksheet.is_none() => {
                    if Self::element_outline(Some(e))?.len() > 2048 {
                        return Err("Site figure boundary exceeds 2048 vertices".into());
                    }
                }
                _ => return Err("Site figure boundary identity is inconsistent".into()),
            }
        }
        Ok(())
    }
    pub fn outline(&self) -> Result<Vec<Point>, String> {
        Self::element_outline(self.boundary_element.as_ref())
    }
    pub fn control_outline(&self) -> Result<Vec<Point>, String> {
        Self::element_outline(self.control_element.as_ref())
    }
    fn element_outline(element: Option<&Element>) -> Result<Vec<Point>, String> {
        let Some(e) = element else {
            return Ok(vec![]);
        };
        let Shape::Polyline { points, arcs } = &e.shape else {
            return Err("Site figure boundary must be a straight closed Polyline".into());
        };
        if !arcs.is_empty() || points.len() < 4 || points.first() != points.last() {
            return Err("Site figure boundary must be a straight closed Polyline".into());
        }
        let points = points[..points.len() - 1].to_vec();
        crate::slab_geometry::validate(&points)?;
        Ok(points)
    }
    pub fn user_modified(&self) -> bool {
        self.context.boundary.as_ref().is_some_and(|b| {
            self.outline().is_ok_and(|v| {
                v.iter().map(|p| [p.x, p.y]).collect::<Vec<_>>() != b.imported_outline_m
            })
        })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Model,
    CommonContext,
    Boundary,
    ControlRegion,
    Inset,
}
#[derive(Clone, Debug)]
pub struct Path {
    pub kind: Kind,
    pub element_id: Option<u64>,
    pub rings: Vec<Vec<Point>>,
    pub check_state: Option<crate::constraints::State>,
}
impl Path {
    pub fn color(&self) -> [u8; 3] {
        if let Some(state) = self.check_state {
            return super::checks::color(state);
        }
        match self.kind {
            Kind::Model => [105, 143, 168],
            Kind::CommonContext => [115, 115, 125],
            Kind::Boundary => [172, 112, 47],
            Kind::ControlRegion => [143, 91, 159],
            Kind::Inset => [71, 116, 182],
        }
    }
    pub fn label(&self) -> &'static str {
        match self.kind {
            Kind::Model => "Authored spaces / masses",
            Kind::CommonContext => "Common context · excluded from candidate quantities",
            Kind::Boundary => "Source boundary · approval unknown",
            Kind::ControlRegion => "Declared control region · approval unknown",
            Kind::Inset => "Uniform inset · design study",
        }
    }
}
pub struct Page {
    pub paths: Vec<Path>,
    pub layout: super::Layout,
    pub boundary_area_m2: Option<f64>,
    pub inset_area_m2: Option<f64>,
    pub inset_parts: usize,
    pub constraint_overlay: bool,
}
fn local_polygon(points: &[Point], o: Point) -> Polygon<f64> {
    Polygon::new(
        LineString::from(
            points
                .iter()
                .map(|p| (p.x - o.x, p.y - o.y))
                .collect::<Vec<_>>(),
        ),
        vec![],
    )
}
fn unclosed(line: &LineString<f64>, o: Point) -> Vec<Point> {
    let mut v: Vec<_> = line
        .0
        .iter()
        .map(|p| Point::new(p.x + o.x, p.y + o.y))
        .collect();
    if v.first() == v.last() {
        v.pop();
    }
    v
}
impl Page {
    pub fn labels(&self) -> Vec<(u64, [f64; 2], bool)> {
        use geo::{Centroid, Contains, InteriorPoint};
        let mut occupied = std::collections::BTreeMap::new();
        self.paths
            .iter()
            .filter_map(|p| {
                let id = p.element_id?;
                let centered = matches!(p.kind, Kind::Model | Kind::CommonContext);
                let world = if centered {
                    let ring = |v: &Vec<Point>| {
                        LineString::from(v.iter().map(|p| (p.x, p.y)).collect::<Vec<_>>())
                    };
                    let polygon =
                        Polygon::new(ring(&p.rings[0]), p.rings[1..].iter().map(ring).collect());
                    let q = polygon
                        .centroid()
                        .filter(|q| polygon.contains(q))
                        .or_else(|| polygon.interior_point());
                    q.map(|q| Point::new(q.x(), q.y())).unwrap_or(p.rings[0][0])
                } else {
                    p.rings[0][0]
                };
                let mut point = self.layout.point(world);
                if !centered {
                    point[0] += 4.;
                    point[1] -= 4.;
                }
                if self.constraint_overlay && centered {
                    let row = occupied
                        .entry((point[0].round() as i64, point[1].round() as i64))
                        .or_insert(0);
                    point[1] += f64::from(*row) * 14.;
                    *row += 1;
                }
                Some((id, point, centered))
            })
            .collect()
    }
    pub fn new(d: &Diagram) -> Result<Self, String> {
        Self::for_snapshot(&d.definition.presentation, &d.snapshot)
    }
    pub(super) fn for_snapshot(
        presentation: &Presentation,
        snapshot: &Snapshot,
    ) -> Result<Self, String> {
        if !presentation.is_site() {
            return Err("Figure is not a site plan".into());
        }
        let src = snapshot
            .site
            .as_ref()
            .ok_or("Site figure source snapshot is missing")?;
        src.validate()?;
        let mut page = Self {
            paths: vec![],
            layout: super::Layout {
                min: Point::new(f64::INFINITY, f64::INFINITY),
                max: Point::new(f64::NEG_INFINITY, f64::NEG_INFINITY),
                pixels_per_m: 1.,
            },
            boundary_area_m2: None,
            inset_area_m2: None,
            inset_parts: 0,
            constraint_overlay: matches!(presentation, Presentation::ConstraintOverlay),
        };
        let checks = if page.constraint_overlay {
            Some(super::checks::Source::read(snapshot)?)
        } else {
            None
        };
        let outline = src.outline()?;
        if !outline.is_empty() {
            let o = outline[0];
            let poly = local_polygon(&outline, o);
            page.boundary_area_m2 = Some(crate::slab_geometry::area(&outline));
            page.paths.push(Path {
                kind: Kind::Boundary,
                element_id: src.boundary_element.as_ref().map(|e| e.id),
                rings: vec![outline.clone()],
                check_state: None,
            });
            if let Presentation::SetbackStudy { distance_mm, .. } = presentation {
                // i_overlay's pinned Round parameter is max segment length/radius.
                // 0.04 bounds chord sagitta to ~0.0002*d, not a statutory tolerance.
                let result = poly.buffer_with_style(
                    BufferStyle::new(-f64::from(*distance_mm) / 1000.)
                        .line_join(LineJoin::Round(0.04)),
                );
                page.inset_area_m2 = Some(result.unsigned_area());
                page.inset_parts = result.0.len();
                let mut total = 0;
                for poly in result.0 {
                    let rings: Vec<_> = std::iter::once(poly.exterior())
                        .chain(poly.interiors())
                        .map(|l| unclosed(l, o))
                        .collect();
                    total += rings.iter().map(Vec::len).sum::<usize>();
                    if total > 32768
                        || rings
                            .iter()
                            .any(|v| v.len() < 3 || v.iter().any(|p| !p.finite()))
                    {
                        return Err(
                            "Study inset exceeds the 32768 vertex limit or has invalid output"
                                .into(),
                        );
                    }
                    page.paths.push(Path {
                        kind: Kind::Inset,
                        element_id: None,
                        rings,
                        check_state: None,
                    });
                }
            }
        } else if matches!(presentation, Presentation::SetbackStudy { .. }) {
            return Err("A source boundary is required for a setback study".into());
        }
        let control = src.control_outline()?;
        if !control.is_empty() {
            page.paths.push(Path {
                kind: Kind::ControlRegion,
                element_id: src.control_element.as_ref().map(|e| e.id),
                rings: vec![control],
                check_state: None,
            });
        }
        for e in checks.iter().flat_map(|c| &c.common_context) {
            let (r, holes) = e
                .authored_prism_rings()
                .ok_or("Unsupported common context footprint")?;
            page.paths.push(Path {
                kind: Kind::CommonContext,
                element_id: Some(e.id),
                rings: std::iter::once(r.clone())
                    .chain(holes.iter().cloned())
                    .collect(),
                check_state: None,
            });
        }
        for e in &snapshot.elements {
            let (r, h) = e
                .authored_prism_rings()
                .ok_or("Unsupported site plan object")?;
            page.paths.push(Path {
                kind: Kind::Model,
                element_id: Some(e.id),
                rings: std::iter::once(r.clone())
                    .chain(h.iter().cloned())
                    .collect(),
                check_state: checks.as_ref().and_then(|c| c.element_state(e.id)),
            });
        }
        if page.constraint_overlay {
            // Worst known state stays visible when a linked Zone and mass share
            // a footprint. This changes display order only, never geometry.
            page.paths.sort_by_key(|p| match p.check_state {
                None => 0,
                Some(crate::constraints::State::Pass) => 1,
                Some(crate::constraints::State::NotApplicable) => 2,
                Some(crate::constraints::State::Unknown) => 3,
                Some(crate::constraints::State::Fail) => 4,
            });
        }
        for point in page.paths.iter().flat_map(|p| p.rings.iter().flatten()) {
            page.layout.min.x = page.layout.min.x.min(point.x);
            page.layout.min.y = page.layout.min.y.min(point.y);
            page.layout.max.x = page.layout.max.x.max(point.x);
            page.layout.max.y = page.layout.max.y.max(point.y);
        }
        if !page.layout.min.finite() {
            return Err("Site plan needs a source boundary or authored geometry".into());
        }
        page.layout.pixels_per_m = (750. / (page.layout.max.x - page.layout.min.x).max(0.01))
            .min(540. / (page.layout.max.y - page.layout.min.y).max(0.01));
        Ok(page)
    }
}
pub fn summary(s: &Snapshot) -> serde_json::Value {
    let Some(source) = &s.site else {
        return serde_json::Value::Null;
    };
    serde_json::json!({"declarations":source.context,"boundary_element_id":source.boundary_element.as_ref().map(|e|e.id),"user_modified_boundary":source.user_modified(),"boundary_approval":"unknown","control_region_element_id":source.control_element.as_ref().map(|e|e.id),"control_region_approval":"unknown","compliance":"not_evaluated","source_reading":"identities are declarations, not verification of original bytes or approval","terrain_roads_neighbors":"not_drawn","geometry_version":source.geometry_version})
}
pub fn text_lines(d: &Diagram, t: &impl Fn(&str) -> String) -> Vec<(f64, f64, String, f32)> {
    if matches!(d.definition.presentation, Presentation::ConstraintOverlay) {
        return super::checks::text_lines(d, t);
    }

    let mut v = vec![
        (
            40.,
            38.,
            format!(
                "#{} · v{} · {}",
                d.id,
                d.generation,
                super::short(&d.name, 80)
            ),
            22.,
        ),
        (
            40.,
            66.,
            format!(
                "{} · {}",
                if d.definition.option.is_none() {
                    t("Main model")
                } else {
                    super::short(&d.snapshot.option_name, 45)
                },
                super::short(&d.snapshot.story.name, 30)
            ),
            15.,
        ),
        (
            40.,
            91.,
            t("Local model position · declared coordinates · no approval verdict"),
            14.,
        ),
    ];
    let Some(source) = &d.snapshot.site else {
        return v;
    };
    let mut lines = vec![t(if source.boundary_element.is_some() {
        "Boundary recorded; survey approval unknown"
    } else {
        "Boundary unknown; no boundary invented"
    })];
    if let Some(b) = &source.context.boundary {
        lines.push(format!(
            "#{} · {} · {}",
            b.element_id,
            b.origin.entity_handle,
            super::short(&b.name, 20)
        ));
        lines.push(format!("{} {}", t("Source"), &b.origin.source_sha256[..16]));
        lines.push(format!(
            "{}: {}",
            t("Declared source unit"),
            b.origin.unit.symbol()
        ));
        if b.origin.centerline_width_native.is_some() {
            lines.push(t("Centerline only; source width omitted"));
        }
    }
    if let Some(region) = &source.context.control_region {
        lines.push(format!(
            "{}: #{} · {}",
            t("Declared control region"),
            region.element_id,
            super::short(&region.name, 20)
        ));
        lines.push(format!(
            "{} {} · {}",
            t("Source"),
            &region.origin.source_sha256[..16],
            region.origin.entity_handle
        ));
        lines.push(t(
            "A declared control region is not an approved building line.",
        ));
    }
    if source.user_modified() {
        lines.push(t(
            "Boundary edited after import; origin declaration retained",
        ));
    }
    lines.push(format!(
        "{}: {}",
        t("Projected reference"),
        source
            .context
            .horizontal
            .as_ref()
            .map(|h| super::short(&h.crs, 24))
            .unwrap_or_else(|| t("Unknown"))
    ));
    lines.push(format!(
        "{}: {}",
        t("North"),
        t(if source.context.true_north.is_some() {
            "Declared"
        } else {
            "Unknown"
        })
    ));
    if let Presentation::SetbackStudy {
        distance_mm,
        assumption,
    } = &d.definition.presentation
    {
        lines.push(format!(
            "{}: {:.3} m",
            t("Study inset"),
            f64::from(*distance_mm) / 1000.
        ));
        lines.push(super::short(assumption, 40));
        if let Ok(page) = Page::new(d) {
            lines.push(format!(
                "{}: {:.3} m²",
                t("Inset area (study)"),
                page.inset_area_m2.unwrap_or(0.)
            ));
            lines.push(format!("{}: {}", t("Interior parts"), page.inset_parts));
            if page.inset_parts == 0 {
                lines.push(t("No interior remains at this study distance"));
            }
        }
    }
    for (i, line) in lines.iter().enumerate() {
        v.push((835., 112. + i as f64 * 17., line.clone(), 11.));
    }
    let start = 112. + (lines.len() + 1) as f64 * 17.;
    for (i, e) in d.snapshot.elements.iter().take(16).enumerate() {
        v.push((
            835.,
            start + i as f64 * 17.,
            format!("#{} {}", e.id, super::short(&e.name, 22)),
            11.,
        ));
    }
    if d.snapshot.elements.len() > 16 {
        v.push((
            835.,
            start + 16. * 17.,
            format!(
                "+{} · {}",
                d.snapshot.elements.len() - 16,
                t("See source records")
            ),
            11.,
        ));
    }
    v.push((
        40.,
        735.,
        t("Walls, roads, neighbors and terrain are not drawn"),
        12.,
    ));
    v.push((
        40.,
        760.,
        t("Study inset is not an approved building line or compliance result"),
        12.,
    ));
    v.push((
        40.,
        783.,
        format!(
            "{} {} · {}",
            t("Source"),
            &d.source_sha256[..16],
            t("Assumptions remain assumptions")
        ),
        13.,
    ));
    v
}
pub(super) fn append_svg(d: &Diagram, s: &mut String) -> Result<(), String> {
    use std::fmt::Write as _;
    let page = Page::new(d)?;
    write!(s,"<metadata id=\"site-plan\">{}</metadata>",super::xml(&serde_json::json!({"site":summary(&d.snapshot),"min":page.layout.min,"max":page.layout.max,"pixels_per_m":page.layout.pixels_per_m,"boundary_area_m2":page.boundary_area_m2,"inset_area_m2":page.inset_area_m2,"inset_parts":page.inset_parts,"algorithm":if page.constraint_overlay{"shared authored footprints/check rows; no solver or approval"}else{"geo0.33.1 inward buffer; round L/R0.04; local origin; illustration only"}}).to_string())).unwrap();
    for p in &page.paths {
        let mut path = String::new();
        for ring in &p.rings {
            for (i, pt) in ring.iter().enumerate() {
                let q = page.layout.point(*pt);
                write!(
                    path,
                    "{} {:.8} {:.8} ",
                    if i == 0 { "M" } else { "L" },
                    q[0],
                    q[1]
                )
                .unwrap();
            }
            path.push_str("Z ");
        }
        let c = p.color();
        if page.constraint_overlay {
            write!(
                s,
                "<metadata>{}</metadata>",
                super::xml(
                    &serde_json::json!({"element_id":p.element_id,"check_state":p.check_state})
                        .to_string()
                )
            )
            .unwrap();
        }
        write!(s,r#"<path data-site-kind="{:?}" data-element-id="{}" d="{}" fill="rgb({},{},{})" fill-opacity="0.12" fill-rule="evenodd" stroke="rgb({},{},{})" stroke-width="2"/>"#,p.kind,p.element_id.map(|v|v.to_string()).unwrap_or_default(),path,c[0],c[1],c[2],c[0],c[1],c[2]).unwrap();
    }
    for (id, p, centered) in page.labels() {
        write!(s,r#"<text data-site-object-label="{}" x="{:.8}" y="{:.8}" text-anchor="{}" dominant-baseline="{}" font-family="sans-serif" font-size="12">#{}</text>"#,id,p[0],p[1],if centered{"middle"}else{"start"},if centered{"middle"}else{"auto"},id).unwrap();
    }
    for (i, (_, c, label)) in legend(&page).iter().enumerate() {
        write!(s,r#"<rect data-site-legend="true" x="835" y="{}" width="10" height="10" fill="rgb({},{},{})"/>"#,legend_y(&page,i),c[0],c[1],c[2]).unwrap();
        super::svg_text(s, 853., legend_y(&page, i) + 10., label, 11.);
    }
    for (x, y, text, size) in d.text_lines() {
        super::svg_text(s, x, y, &text, size);
    }
    let len = page.layout.scale_length();
    write!(
        s,
        r#"<path d="M 50 700 v 8 h {} v -8" stroke="black" fill="none"/>"#,
        len * page.layout.pixels_per_m
    )
    .unwrap();
    super::svg_text(s, 50., 693., &format!("{len} m (model)"), 13.);
    if let Some(n) = &d.snapshot.true_north {
        let a = n.clockwise_from_model_y_deg.to_radians();
        write!(
            s,
            r#"<path d="M 770 680 l {} {}" stroke="black"/>"#,
            a.sin() * 35.,
            -a.cos() * 35.
        )
        .unwrap();
        super::svg_text(s, 720., 715., "N (declared)", 12.);
    }
    Ok(())
}

pub fn figure_summary(d: &Diagram) -> serde_json::Value {
    if !d.definition.presentation.is_site() {
        return serde_json::Value::Null;
    }
    let mut v = summary(&d.snapshot);
    let mut zone = (0.0, 0.0);
    let mut mass = (0.0, 0.0);
    for e in &d.snapshot.elements {
        let (a, b) = super::quantities(e);
        match e.shape {
            Shape::Zone { .. } => {
                zone.0 += a;
                zone.1 += b;
            }
            Shape::ConceptMass { .. } => {
                mass.0 += a;
                mass.1 += b;
            }
            _ => {}
        }
    }
    v["object_quantities_by_domain"] = serde_json::json!({"zones":{"area_m2":zone.0,"volume_m3":zone.1},"concept_masses":{"footprint_m2":mass.0,"volume_m3":mass.1},"interpretation":"Separate authored domains, overlaps counted separately; not site/buildable/legal area"});
    match Page::new(d) {
        Ok(p) => {
            if p.constraint_overlay {
                v["common_context_ids"] = serde_json::json!(
                    p.paths
                        .iter()
                        .filter(|p| p.kind == Kind::CommonContext)
                        .filter_map(|p| p.element_id)
                        .collect::<Vec<_>>()
                );
            }
            v["boundary_area_m2"] = serde_json::json!(p.boundary_area_m2);
            v["inset_area_m2"] = serde_json::json!(p.inset_area_m2);
            v["inset_parts"] = serde_json::json!(p.inset_parts);
            v["geometry_interpretation"] = serde_json::json!(if matches!(
                d.definition.presentation,
                Presentation::ConstraintOverlay
            ) {
                "Shared check rows over authored footprints; not statutory geometry, original verification or compliance"
            } else {
                "Uniform sampled inward buffer study, not buildable area or a legal building line"
            });
        }
        Err(e) => {
            v["derived_error"] = e.into();
        }
    }
    v
}

pub fn legend(page: &Page) -> Vec<(Kind, [u8; 3], &'static str)> {
    if page.constraint_overlay {
        let mut legend = vec![];
        for kind in [
            Kind::Boundary,
            Kind::ControlRegion,
            Kind::CommonContext,
            Kind::Model,
        ] {
            if let Some(p) = page
                .paths
                .iter()
                .find(|p| p.kind == kind && p.check_state.is_none())
            {
                legend.push((kind, p.color(), p.label()));
            }
        }
        for state in [
            crate::constraints::State::Pass,
            crate::constraints::State::Fail,
            crate::constraints::State::Unknown,
            crate::constraints::State::NotApplicable,
        ] {
            if page.paths.iter().any(|p| p.check_state == Some(state)) {
                legend.push((Kind::Model, super::checks::color(state), state.label()));
            }
        }
        return legend;
    }
    [
        Kind::Boundary,
        Kind::ControlRegion,
        Kind::Inset,
        Kind::Model,
    ]
    .into_iter()
    .filter_map(|kind| {
        page.paths
            .iter()
            .find(|p| p.kind == kind)
            .map(|p| (kind, p.color(), p.label()))
    })
    .collect()
}

/// Keep the added control-reference legend clear of the footer; old figures retain their layout.
pub fn legend_y(page: &Page, index: usize) -> f64 {
    let extra = if page
        .paths
        .iter()
        .any(|p| matches!(p.kind, Kind::ControlRegion | Kind::CommonContext))
    {
        legend(page).len().saturating_sub(4)
    } else {
        0
    };
    650. - extra as f64 * 19. + index as f64 * 19.
}
