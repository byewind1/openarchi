//! Read-only, versioned concept figures derived from the one shared project model.
use crate::model::{Element, Point, Project, Shape, Story};
use serde::{Deserialize, Serialize};
use std::fmt::Write as _;
pub mod axon;
pub mod checks;
pub mod functions;
pub mod report;
pub mod site;
pub use functions::{FunctionGroup, Presentation};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Domain {
    Zones,
    ConceptMasses,
    Site,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Definition {
    pub story: u32,
    #[serde(deserialize_with = "required_option")]
    #[schemars(required)]
    pub option: Option<u32>,
    pub domain: Domain,
    #[serde(default, skip_serializing_if = "Presentation::is_default")]
    pub presentation: Presentation,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    pub elements: Vec<Element>,
    pub story: Story,
    pub option_name: String,
    /// Task declarations, relevant update records and image catalogue versions, never file paths.
    pub context: serde_json::Value,
    pub true_north: Option<crate::site::TrueNorth>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub site: Option<site::Source>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Diagram {
    pub id: u64,
    pub name: String,
    pub generation: u32,
    pub definition: Definition,
    pub source_sha256: String,
    pub snapshot: Snapshot,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum Edit {
    Report {
        edit: report::Edit,
    },
    Create {
        name: String,
        definition: Definition,
    },
    Configure {
        id: u64,
        expected_sha256: String,
        presentation: Presentation,
    },
    Refresh {
        id: u64,
        expected_sha256: String,
    },
    Rename {
        id: u64,
        expected_sha256: String,
        name: String,
    },
    Delete {
        id: u64,
        expected_sha256: String,
    },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Current,
    Stale,
    Unavailable,
}
impl Status {
    pub fn label(self) -> &'static str {
        match self {
            Self::Current => "Figure matches model",
            Self::Stale => "Figure is out of date",
            Self::Unavailable => "Figure source unavailable",
        }
    }
}
fn name_ok(s: &str) -> bool {
    !s.trim().is_empty() && s.len() <= 256 && !s.chars().any(char::is_control)
}
fn hash<T: Serialize>(v: &T) -> Result<String, String> {
    crate::automation_journal::canonical_hash(v)
}
fn accepts(d: &Definition, e: &Element) -> bool {
    e.worksheet.is_none()
        && e.story == d.story
        && e.design_option == d.option
        && matches!(
            (d.domain, &e.shape),
            (Domain::Zones, Shape::Zone { .. })
                | (Domain::ConceptMasses, Shape::ConceptMass { .. })
                | (Domain::Site, Shape::Zone { .. } | Shape::ConceptMass { .. })
        )
}
fn capture(p: &Project, d: &Definition) -> Result<Snapshot, String> {
    let story = p
        .stories
        .iter()
        .find(|s| s.id == d.story)
        .ok_or("Figure story is missing")?
        .clone();
    let option_name = match d.option {
        Some(id) => p
            .design_options
            .name(id)
            .ok_or("Figure option is missing")?,
        None => "Main model",
    }
    .to_owned();
    let mut elements: Vec<_> = p
        .elements
        .iter()
        .filter(|e| accepts(d, e))
        .cloned()
        .collect();
    elements.sort_by_key(|e| e.id);
    let task = p.design_process.tasks.iter().find(|t| {
        t.story == d.story
            && (Some(t.baseline_option) == d.option
                || t.candidates.iter().any(|c| Some(c.option) == d.option))
    });
    let updates: Vec<_> = p
        .design_process
        .updates
        .iter()
        .filter(|r| Some(r.option) == d.option && task.is_some_and(|t| t.id == r.task_id))
        .collect();
    let mut sources = std::collections::BTreeMap::new();
    if let Some(intent) = task.and_then(|t| t.intent.as_ref()) {
        for image in &intent.images {
            sources.insert(image.page, image.sha256.clone());
        }
    }
    for record in &updates {
        for source in &record.sources {
            if let crate::design_process::update::Source::Drawing { page, sha256 } = source {
                sources.insert(*page, sha256.clone());
            }
        }
    }
    let catalogue: Vec<_> = sources.into_iter().map(|(id, expected)| serde_json::json!({"page":id,"expected_sha256":expected,"actual":p.drawings.pages.iter().find(|v| v.id==id).map(|v|serde_json::json!({"sha256":v.sha256,"pixels":v.pixels}))})).collect();
    let mut context = serde_json::json!({"task":task,"updates":updates,"image_catalogue":catalogue,"source_reading":"catalogue declarations; original bytes and approvals not verified by this figure"});
    if d.domain == Domain::Zones {
        let mut links: Vec<_> = p
            .design_process
            .mass_spaces
            .iter()
            .filter(|l| elements.iter().any(|e| e.id == l.zone))
            .cloned()
            .collect();
        links.sort_by_key(|l| l.zone);
        if !links.is_empty() {
            context["mass_space_links"] = serde_json::to_value(links).map_err(|e| e.to_string())?;
        }
    }
    if matches!(d.presentation, Presentation::ConstraintOverlay) {
        context["constraint_overlay"] =
            serde_json::to_value(checks::Source::capture(p, d, &elements)?)
                .map_err(|e| e.to_string())?;
    }
    let snapshot = Snapshot {
        elements,
        story,
        option_name,
        context,
        true_north: p.site.true_north.clone(),
        site: if d.presentation.is_site() {
            Some(site::Source::capture(p)?)
        } else {
            None
        },
    };
    validate_snapshot(d, &snapshot)?;
    Ok(snapshot)
}
fn validate_snapshot(d: &Definition, s: &Snapshot) -> Result<(), String> {
    d.presentation.validate(d.domain)?;
    match (&d.presentation, s.context.get("constraint_overlay")) {
        (Presentation::ConstraintOverlay, Some(value)) => {
            let source: checks::Source = serde_json::from_value(value.clone())
                .map_err(|_| "Invalid constraint figure snapshot")?;
            source.validate(d, &s.elements)?;
        }
        (Presentation::ConstraintOverlay, None) => {
            return Err("Constraint figure snapshot is missing".into());
        }
        (_, Some(_)) => return Err("Unexpected constraint figure snapshot".into()),
        _ => {}
    }

    if let Some(value) = s.context.get("mass_space_links") {
        let links: Vec<crate::mass_spaces::Link> = serde_json::from_value(value.clone())
            .map_err(|_| "Invalid figure mass-space sources")?;
        crate::mass_spaces::validate(&links)?;
        if d.domain != Domain::Zones
            || links.is_empty()
            || links.windows(2).any(|w| w[0].zone >= w[1].zone)
            || links.iter().any(|l| {
                l.story != d.story
                    || l.option != d.option
                    || !s.elements.iter().any(|e| e.id == l.zone)
            })
        {
            return Err("Invalid figure mass-space sources".into());
        }
    }
    if d.presentation.is_functional()
        && s.elements
            .iter()
            .any(|e| matches!(&e.shape,Shape::Zone{spec}if !name_ok(&spec.purpose)))
    {
        return Err("Functional diagram purposes must contain printable text".into());
    }
    if !(if d.presentation.is_site() {
        0..=32
    } else {
        1..=32
    })
    .contains(&s.elements.len())
        || s.story.id != d.story
        || !s.story.elevation.is_finite()
        || !name_ok(&s.story.name)
        || !name_ok(&s.option_name)
        || s.elements.windows(2).any(|w| w[0].id >= w[1].id)
        || serde_json::to_vec(s).map_err(|e| e.to_string())?.len() > 512 * 1024
    {
        return Err(
            "Figure requires 1–32 authored objects and at most 512 KiB of source data".into(),
        );
    }
    if d.presentation.is_site() {
        let source = s
            .site
            .as_ref()
            .ok_or("Site figure source snapshot is missing")?;
        source.validate()?;
        if source.context.true_north != s.true_north {
            return Err("Site figure north declaration differs from its source".into());
        }
        if source.boundary_element.is_none()
            && (s.elements.is_empty()
                || matches!(d.presentation, Presentation::SetbackStudy { .. }))
        {
            return Err("A setback study needs a source boundary; a location figure needs a boundary or authored spaces/masses".into());
        }
    } else if s.site.is_some() {
        return Err("Unexpected site figure source snapshot".into());
    }
    for e in &s.elements {
        if e.id == 0 || !name_ok(&e.name) || !accepts(d, e) {
            return Err("Invalid figure object scope".into());
        }
        match &e.shape {
            Shape::Zone { spec } => spec.validate()?,
            Shape::ConceptMass { spec } => spec.validate()?,
            _ => return Err("Unsupported figure geometry".into()),
        }
        if d.presentation.is_axon() {
            match e.shape {
                Shape::Zone { .. } => crate::zones::derive(e, s.story.elevation),
                Shape::ConceptMass { .. } => crate::concept_mass::derive(e, s.story.elevation),
                _ => unreachable!(),
            }
            .map_err(|e| e.message)?;
        }
    }
    if d.presentation.is_site() {
        site::Page::for_snapshot(&d.presentation, s)?;
    }
    if let Presentation::ExplodedAxon { offsets_mm } = &d.presentation
        && offsets_mm
            .iter()
            .any(|o| !s.elements.iter().any(|e| e.id == o.element_id))
    {
        return Err("Axon offset refers to an object outside its source snapshot".into());
    }
    if let Some(n) = &s.true_north {
        // Reuse the declared site validation, without inventing a surveyed orientation.
        let mut site = crate::site::SiteContext::default();
        site.true_north = Some(n.clone());
        site.validate()?;
    }
    Ok(())
}
impl Diagram {
    pub fn validate(&self) -> Result<(), String> {
        if self.id == 0 || self.generation == 0 || !name_ok(&self.name) {
            return Err("Invalid model figure".into());
        }
        validate_snapshot(&self.definition, &self.snapshot)?;
        if self.source_sha256 != hash(&(&self.definition, &self.snapshot))? {
            return Err("Figure source fingerprint mismatch".into());
        }
        Ok(())
    }
    pub fn status(&self, p: &Project) -> Status {
        if self.definition.domain == Domain::Zones
            && p.design_process.mass_spaces.iter().any(|l| {
                p.elements
                    .iter()
                    .any(|e| accepts(&self.definition, e) && e.id == l.zone)
                    && crate::mass_spaces::status(p, l) != "Mass-space link is current"
            })
        {
            return Status::Stale;
        }
        match capture(p, &self.definition).and_then(|s| hash(&(&self.definition, s))) {
            Ok(h) if h == self.source_sha256 => Status::Current,
            Ok(_) => Status::Stale,
            Err(_) => Status::Unavailable,
        }
    }
    pub fn total(&self) -> (f64, f64) {
        self.snapshot
            .elements
            .iter()
            .map(quantities)
            .fold((0., 0.), |a, b| (a.0 + b.0, a.1 + b.1))
    }
    pub fn svg(&self, project: &Project) -> Result<String, String> {
        self.validate()?;
        if self.status(project) != Status::Current {
            return Err("Update the figure before exporting".into());
        }
        let layout = Layout::new(self);
        let mut s = String::from(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="1200" height="800" viewBox="0 0 1200 800"><rect width="1200" height="800" fill="white"/>"#,
        );
        write!(s,"<metadata>{}</metadata>",xml(&serde_json::json!({"name":self.name,"source_names":self.snapshot.elements.iter().map(|e|serde_json::json!({"id":e.id,"name":e.name})).collect::<Vec<_>>(),"figure":self.id,"generation":self.generation,"source_sha256":self.source_sha256,"definition":self.definition,"context":self.snapshot.context,"function_groups":self.function_groups(),"units":"m,m2,m3","interpretation":"Authored concept quantities; overlaps counted separately, not net floor area or compliance"}).to_string())).unwrap();
        if self.definition.presentation.is_axon() {
            axon::append_svg(self, &mut s)?;
            s.push_str("</svg>");
            return Ok(s);
        }
        if self.definition.presentation.is_site() {
            site::append_svg(self, &mut s)?;
            s.push_str("</svg>");
            return Ok(s);
        }
        for e in &self.snapshot.elements {
            let (outer, holes) = e.authored_prism_rings().unwrap();
            let mut path = String::new();
            for ring in std::iter::once(&outer).chain(holes.iter()) {
                for (i, p) in ring.iter().enumerate() {
                    let q = layout.point(*p);
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
            let c = self.element_color(e);
            write!(s,r##"<path data-element-id="{}" d="{}" fill="rgb({},{},{})" fill-opacity="0.6" fill-rule="evenodd" stroke="#303b46" stroke-width="1.5"/>"##,e.id,path,c[0],c[1],c[2]).unwrap();
            if !self.definition.presentation.is_default()
                && let Shape::Zone { spec } = &e.shape
            {
                let q = layout.point(spec.label_point());
                write!(s,r#"<text x="{}" y="{}" text-anchor="middle" dominant-baseline="middle" font-family="sans-serif" font-size="12">{}</text>"#,q[0],q[1],xml(&format!("#{} {}",e.id,short(&e.name,12)))).unwrap();
            } else {
                let q = layout.point(outer[0]);
                svg_text(&mut s, q[0] + 4., q[1] - 4., &format!("#{}", e.id), 12.);
            }
        }
        for (i, g) in self.function_groups().iter().enumerate() {
            let c = g.color;
            write!(s,r#"<rect data-function-key="{}" x="835" y="{}" width="10" height="10" fill="rgb({},{},{})"/>"#,g.key,130.+i as f64*17.,c[0],c[1],c[2]).unwrap();
        }
        for (x, y, text, size) in self.text_lines() {
            svg_text(&mut s, x, y, &text, size);
        }
        let scale = layout.scale_length();
        let y = 738.;
        write!(
            s,
            r#"<path d="M 50 {} v 10 h {} v -10" stroke="black" fill="none"/>"#,
            y,
            scale * layout.pixels_per_m
        )
        .unwrap();
        svg_text(&mut s, 50., y - 6., &format!("{} m", scale), 14.);
        if let Some(n) = &self.snapshot.true_north {
            let a = n.clockwise_from_model_y_deg.to_radians();
            write!(
                s,
                r#"<path d="M 770 680 l {} {}" stroke="black" fill="none"/>"#,
                a.sin() * 35.,
                -a.cos() * 35.
            )
            .unwrap();
            svg_text(&mut s, 720., 738., "N (declared)", 13.);
        }
        s.push_str("</svg>");
        Ok(s)
    }
    pub fn text_lines(&self) -> Vec<(f64, f64, String, f32)> {
        self.localized_text_lines(|s| s.to_owned())
    }
    pub fn localized_text_lines(&self, t: impl Fn(&str) -> String) -> Vec<(f64, f64, String, f32)> {
        if self.definition.presentation.is_site() {
            return site::text_lines(self, &t);
        }
        let (area, volume) = self.total();
        let mut v = vec![
            (
                40.,
                38.,
                format!(
                    "#{} · v{} · {}",
                    self.id,
                    self.generation,
                    short(&self.name, 80)
                ),
                22.,
            ),
            (
                40.,
                66.,
                format!(
                    "{} · {} · {}",
                    short(&self.snapshot.option_name, 55),
                    short(&self.snapshot.story.name, 30),
                    match self.definition.domain {
                        Domain::Zones => t("Zone"),
                        Domain::ConceptMasses => t("Concept mass"),
                        Domain::Site => t("Site context"),
                    }
                ),
                15.,
            ),
            (
                40.,
                91.,
                t("Authored model · m / m² / m³ · overlaps counted separately"),
                14.,
            ),
            (
                835.,
                112.,
                format!("{}: {:.3} m² / {:.3} m³", t("Total"), area, volume),
                15.,
            ),
        ];
        if !self.definition.presentation.is_functional() {
            for (i, e) in self.snapshot.elements.iter().enumerate() {
                let (a, vv) = quantities(e);
                v.push((
                    835.,
                    140. + i as f64 * 17.,
                    format!("#{} {} · {:.3} / {:.3}", e.id, short(&e.name, 22), a, vv),
                    11.,
                ));
            }
        } else {
            v[2].2 =
                t("Declared Zone purposes · no approval verdict · overlaps counted separately");
            for (i, g) in self.function_groups().iter().enumerate() {
                v.push((
                    853.,
                    140. + i as f64 * 17.,
                    format!(
                        "{} · {} · {} · {:.2} m²",
                        short(&g.purpose, 20),
                        t(if g.unknown { "Unknown" } else { "Declared" }),
                        g.element_ids.len(),
                        g.area_m2
                    ),
                    11.,
                ));
            }
        }
        if let Presentation::ExplodedAxon { offsets_mm } = &self.definition.presentation {
            v[2].2 = t("Orthographic axon · display offsets only · original model quantities");
            // Keep the same bounded right-hand object list, with explicit display Z values.
            for (i, e) in self.snapshot.elements.iter().enumerate() {
                let xyz = offsets_mm
                    .iter()
                    .find(|o| o.element_id == e.id)
                    .map(|o| o.xyz)
                    .unwrap_or([0; 3]);
                v[4 + i].2 = format!(
                    "#{} {} · Δ({:.2}, {:.2}, {:.2}) m",
                    e.id,
                    short(&e.name, 14),
                    f64::from(xyz[0]) / 1000.,
                    f64::from(xyz[1]) / 1000.,
                    f64::from(xyz[2]) / 1000.
                );
            }
            v.push((
                835.,
                710.,
                t("XYZ directions only · not a paper scale"),
                12.,
            ));
            v.push((
                40.,
                760.,
                t("Display separation is not a storey elevation or measured distance"),
                12.,
            ));
        }
        v.push((
            40.,
            783.,
            format!(
                "{} {} · {} · {} {}",
                t("Source"),
                &self.source_sha256[..16],
                t("Assumptions remain assumptions"),
                t("North"),
                t(if self.snapshot.true_north.is_some() {
                    "Declared"
                } else {
                    "Unknown"
                })
            ),
            13.,
        ));
        v
    }
}
pub fn quantities(e: &Element) -> (f64, f64) {
    match &e.shape {
        Shape::Zone { spec } => (spec.area(), spec.area() * spec.height),
        Shape::ConceptMass { spec } => (spec.area(), spec.volume()),
        _ => (0., 0.),
    }
}
pub(crate) fn needs_mass_space_format(p: &Project) -> bool {
    !p.design_process.mass_spaces.is_empty() || needs_mass_space_figure_format(p)
}
pub(crate) fn needs_mass_space_figure_format(p: &Project) -> bool {
    p.drawings
        .diagrams
        .iter()
        .any(|f| f.snapshot.context.get("mass_space_links").is_some())
}
fn ensure_space_sources_current(p: &Project, s: &Snapshot) -> Result<(), String> {
    if let Some(value) = s.context.get("mass_space_links") {
        let links: Vec<crate::mass_spaces::Link> = serde_json::from_value(value.clone())
            .map_err(|_| "Invalid figure mass-space sources")?;
        if links
            .iter()
            .any(|l| crate::mass_spaces::status(p, l) != "Mass-space link is current")
        {
            return Err(
                "Update or detach the changed mass-space link before updating this space figure"
                    .into(),
            );
        }
    }
    Ok(())
}
pub fn apply(p: &Project, edit: &Edit) -> Result<crate::drawings::DrawingDocument, String> {
    if let Edit::Report { edit } = edit {
        return report::apply(p, edit);
    }
    let mut d = p.drawings.clone();
    match edit {
        Edit::Report { .. } => unreachable!(),
        Edit::Create { name, definition } => {
            if d.diagrams.len() >= 64 || !name_ok(name) {
                return Err("Invalid figure name or figure limit reached".into());
            }
            let snapshot = capture(p, definition)?;
            ensure_space_sources_current(p, &snapshot)?;
            let source_sha256 = hash(&(definition, &snapshot))?;
            d.diagrams.push(Diagram {
                id: d.next_id()?,
                name: name.clone(),
                generation: 1,
                definition: definition.clone(),
                source_sha256,
                snapshot,
            });
        }
        Edit::Configure {
            id,
            expected_sha256,
            ..
        }
        | Edit::Refresh {
            id,
            expected_sha256,
        }
        | Edit::Rename {
            id,
            expected_sha256,
            ..
        }
        | Edit::Delete {
            id,
            expected_sha256,
        } => {
            let index = d
                .diagrams
                .iter()
                .position(|v| v.id == *id)
                .ok_or("Figure is missing")?;
            // Exact record guard includes name and generation, not just source geometry.
            if hash(&d.diagrams[index])? != *expected_sha256 {
                return Err("Figure changed; preview again".into());
            }
            match edit {
                Edit::Configure { presentation, .. } => {
                    let item = &mut d.diagrams[index];
                    if presentation == &item.definition.presentation {
                        return Err("Figure settings are unchanged".into());
                    }
                    item.definition.presentation = presentation.clone();
                    validate_snapshot(&item.definition, &item.snapshot)?;
                    item.source_sha256 = hash(&(&item.definition, &item.snapshot))?;
                    item.generation = item
                        .generation
                        .checked_add(1)
                        .ok_or("Figure generations exhausted")?;
                }
                Edit::Refresh { .. } => {
                    let item = &mut d.diagrams[index];
                    let snapshot = capture(p, &item.definition)?;
                    ensure_space_sources_current(p, &snapshot)?;
                    let sha = hash(&(&item.definition, &snapshot))?;
                    if sha == item.source_sha256 {
                        return Err("Figure already matches model".into());
                    }
                    item.snapshot = snapshot;
                    item.source_sha256 = sha;
                    item.generation = item
                        .generation
                        .checked_add(1)
                        .ok_or("Figure generations exhausted")?;
                }
                Edit::Rename { name, .. } => {
                    if !name_ok(name) || *name == d.diagrams[index].name {
                        return Err("Invalid or unchanged figure name".into());
                    }
                    d.diagrams[index].name = name.clone();
                }
                Edit::Delete { .. } => {
                    d.diagrams.remove(index);
                }
                _ => unreachable!(),
            }
        }
    }
    Ok(d)
}
pub fn expected(diagram: &Diagram) -> String {
    hash(diagram).expect("valid figure serializes")
}
/// Shared vector page geometry. Local origin subtraction precedes any f32 UI conversion.
pub struct Layout {
    pub min: Point,
    pub max: Point,
    pub pixels_per_m: f64,
}
impl Layout {
    pub fn new(d: &Diagram) -> Self {
        let mut min = Point::new(f64::INFINITY, f64::INFINITY);
        let mut max = Point::new(f64::NEG_INFINITY, f64::NEG_INFINITY);
        for p in d
            .snapshot
            .elements
            .iter()
            .flat_map(|e| e.authored_prism_rings().unwrap().0)
        {
            min.x = min.x.min(p.x);
            min.y = min.y.min(p.y);
            max.x = max.x.max(p.x);
            max.y = max.y.max(p.y);
        }
        let pixels_per_m = (750. / (max.x - min.x).max(0.01)).min(580. / (max.y - min.y).max(0.01));
        Self {
            min,
            max,
            pixels_per_m,
        }
    }
    pub fn point(&self, p: Point) -> [f64; 2] {
        [
            40. + (750. - (self.max.x - self.min.x) * self.pixels_per_m) / 2.
                + (p.x - self.min.x) * self.pixels_per_m,
            115. + (580. - (self.max.y - self.min.y) * self.pixels_per_m) / 2.
                + (self.max.y - p.y) * self.pixels_per_m,
        ]
    }
    pub fn scale_length(&self) -> f64 {
        let raw = 150. / self.pixels_per_m;
        let base = 10f64.powf(raw.log10().floor());
        let v = raw / base;
        [5., 2., 1.].into_iter().find(|n| *n <= v).unwrap_or(1.) * base
    }
}
fn xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('\"', "&quot;")
        .replace('\'', "&apos;")
}
fn svg_text(s: &mut String, x: f64, y: f64, t: &str, size: f32) {
    write!(
        s,
        r#"<text x="{x}" y="{y}" font-family="sans-serif" font-size="{size}">{}</text>"#,
        xml(t)
    )
    .unwrap();
}

/// Portable summary sufficient to inspect scope, quantities and refresh/rename preconditions.
/// Cached source geometry/context are preserved in the native file, never vendor chat state.
pub fn query(p: &Project, offset: usize, limit: usize) -> Result<serde_json::Value, String> {
    if !(1..=4).contains(&limit) || offset > p.drawings.diagrams.len() {
        return Err("Invalid figure range".into());
    }
    let end = (offset + limit).min(p.drawings.diagrams.len());
    Ok(
        serde_json::json!({"report_templates":report::query(p),"total":p.drawings.diagrams.len(),"next_offset":(end<p.drawings.diagrams.len()).then_some(end),"figures":p.drawings.diagrams[offset..end].iter().map(|f|{let(a,v)=f.total();serde_json::json!({"id":f.id,"name":f.name,"generation":f.generation,"definition":f.definition,"expected_sha256":expected(f),"source_sha256":f.source_sha256,"status":f.status(p).label(),"source_element_ids":f.snapshot.elements.iter().map(|e|e.id).collect::<Vec<_>>(),"task_id":f.snapshot.context["task"]["id"],"mass_space_links":f.snapshot.context.get("mass_space_links"),"area_m2":(!matches!(f.definition.presentation,Presentation::ConstraintOverlay)).then_some(a),"volume_m3":(!matches!(f.definition.presentation,Presentation::ConstraintOverlay)).then_some(v),"quantity_interpretation":if matches!(f.definition.presentation,Presentation::ConstraintOverlay){"Separate authored domains in site_plan.object_quantities_by_domain; no mixed mass/Zone total or legal area"}else{"Authored quantities; overlaps counted separately"},"true_north":f.snapshot.true_north,"function_groups":f.function_groups(),"site_plan":site::figure_summary(f),"constraint_overlay":f.snapshot.context.get("constraint_overlay"),"cached_snapshot":"native project drawings.diagrams; read-only derived representation"})}).collect::<Vec<_>>(),"interpretation":"Authored quantities; overlaps counted separately, not net floor area or compliance. Source catalogue is not proof of reading/approval; current status only compares model and recorded context."}),
    )
}

fn short(s: &str, limit: usize) -> String {
    let mut v: String = s.chars().take(limit).collect();
    if s.chars().count() > limit {
        v.push('…');
    }
    v
}

fn required_option<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<u32>, D::Error> {
    Option::<u32>::deserialize(deserializer)
}
