//! Quantified design checks over the shared model. No statutory approval verdict.
use crate::model::{Element, Point, Project, Shape};
use geo::{Area, BooleanOps, Distance, Euclidean, LineString, Polygon};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
pub mod originals;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Domain {
    ConceptMasses,
    Zones,
}
impl Domain {
    pub fn accepts(self, e: &Element) -> bool {
        matches!(
            (self, &e.shape),
            (Self::ConceptMasses, Shape::ConceptMass { .. }) | (Self::Zones, Shape::Zone { .. })
        )
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::ConceptMasses => "Concept masses",
            Self::Zones => "Authored Zones",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Metric {
    Height,
    FootprintAreaSum,
    HorizontalSpacing,
    ReferenceBoundaryClearance,
    ReferenceControlRegionClearance,
    /// Zero-based edge in the current declared boundary, not a compass direction.
    ReferenceBoundaryEdgeClearance {
        edge: u32,
    },
}
impl Metric {
    pub const ALL: [Self; 6] = [
        Self::Height,
        Self::FootprintAreaSum,
        Self::HorizontalSpacing,
        Self::ReferenceBoundaryClearance,
        Self::ReferenceControlRegionClearance,
        Self::ReferenceBoundaryEdgeClearance { edge: 0 },
    ];
    pub fn unit(self) -> &'static str {
        if self == Self::FootprintAreaSum {
            "m²"
        } else {
            "m"
        }
    }
    pub fn unit_code(self) -> &'static str {
        if self.unit() == "m²" { "m2" } else { "m" }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Height => "Parameter height / concept clear height",
            Self::FootprintAreaSum => "Authored footprint area sum (overlaps counted)",
            Self::HorizontalSpacing => "Horizontal footprint clearance",
            Self::ReferenceBoundaryClearance => "Clearance to declared reference boundary",
            Self::ReferenceControlRegionClearance => "Clearance within declared control region",
            Self::ReferenceBoundaryEdgeClearance { .. } => "Signed clearance to one reference edge",
        }
    }
    pub fn uses_boundary(self) -> bool {
        matches!(
            self,
            Self::ReferenceBoundaryClearance
                | Self::ReferenceBoundaryEdgeClearance { .. }
                | Self::ReferenceControlRegionClearance
        )
    }
    pub fn boundary_role(self) -> crate::site::boundary::Role {
        if self == Self::ReferenceControlRegionClearance {
            crate::site::boundary::Role::ControlRegion
        } else {
            crate::site::boundary::Role::Site
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Scope {
    Objects { ids: Vec<u64> },
    Candidate { story: u32, option: Option<u32> },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Basis {
    DesignAssumption { note: String },
    CitedRequirement { id: String },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Relation {
    AtMost,
    AtLeast,
}
impl Relation {
    pub fn label(self) -> &'static str {
        match self {
            Self::AtMost => "At most",
            Self::AtLeast => "At least",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Definition {
    pub id: String,
    pub title: String,
    pub domain: Domain,
    pub scope: Scope,
    pub metric: Metric,
    pub relation: Relation,
    /// SI metres or square metres according to metric; None stays unknown.
    pub limit: Option<f64>,
    pub basis: Basis,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum State {
    Pass,
    Fail,
    Unknown,
    NotApplicable,
}
impl State {
    pub fn label(self) -> &'static str {
        match self {
            Self::Pass => "Numerical check passed",
            Self::Fail => "Numerical check failed",
            Self::Unknown => "Check unknown",
            Self::NotApplicable => "Declared not applicable",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Row {
    pub ids: Vec<u64>,
    pub value: Option<f64>,
    pub state: State,
    pub reason: String,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Report {
    pub input_definition: Definition,
    pub generation: u32,
    pub source_sha256: String,
    pub rows: Vec<Row>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Check {
    pub definition: Definition,
    pub report: Option<Report>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum Edit {
    Put {
        expected_sha256: Option<String>,
        definition: Definition,
    },
    Run {
        id: String,
        expected_sha256: String,
    },
    Remove {
        id: String,
        expected_sha256: String,
    },
}
fn text(s: &str, max: usize, required: bool) -> bool {
    s.len() <= max && !s.contains('\0') && (!required || !s.trim().is_empty())
}
impl Definition {
    pub fn validate(&self) -> Result<(), String> {
        if !text(&self.id, 128, true)
            || !text(&self.title, 256, true)
            || self
                .limit
                .is_some_and(|v| !v.is_finite() || !(0. ..=1e12).contains(&v))
        {
            return Err("Invalid quantified check definition or limit".into());
        }
        if let Scope::Objects { ids } = &self.scope {
            if ids.is_empty()
                || ids.len() > 256
                || ids.contains(&0)
                || ids.iter().collect::<std::collections::BTreeSet<_>>().len() != ids.len()
            {
                return Err("Choose between 1 and 256 distinct check objects".into());
            }
        }
        if self.metric.uses_boundary() && self.relation != Relation::AtLeast {
            return Err("Reference boundary clearance requires a minimum limit".into());
        }
        if self.metric == Metric::HorizontalSpacing
            && !matches!(&self.scope,Scope::Objects{ids} if ids.len()==2)
        {
            return Err("Horizontal clearance requires exactly two explicit objects".into());
        }
        match &self.basis {
            Basis::DesignAssumption { note } if !text(note, 4096, false) => {
                return Err("Design assumption exceeds check limit".into());
            }
            Basis::CitedRequirement { id } if !text(id, 256, true) => {
                return Err("Choose a cited requirement".into());
            }
            _ => {}
        }
        Ok(())
    }
}
pub fn expected(c: &Check) -> String {
    crate::automation_journal::canonical_hash(c).expect("validated finite check")
}
// Main members are shared context in an option view, not another alternative.
// This exception is only an explicit two-mass distance, never an area/height
// aggregate or an automatic selection of retained buildings.
pub(crate) fn common_mass_spacing(p: &Project, d: &Definition, items: &[&Element]) -> bool {
    d.metric == Metric::HorizontalSpacing
        && d.domain == Domain::ConceptMasses
        && matches!(&d.scope, Scope::Objects { ids } if ids.len() == 2)
        && items.len() == 2
        && items[0].story == items[1].story
        && p.stories.iter().any(|s| s.id == items[0].story)
        && items
            .iter()
            .all(|e| e.worksheet.is_none() && d.domain.accepts(e))
        && items.iter().filter(|e| e.design_option.is_none()).count() == 1
        && items
            .iter()
            .filter_map(|e| e.design_option)
            .all(|id| p.design_options.name(id).is_some())
}
fn objects<'a>(p: &'a Project, d: &Definition) -> Result<Vec<&'a Element>, String> {
    let mut items = match &d.scope {
        Scope::Objects { ids } => ids
            .iter()
            .map(|id| {
                p.elements
                    .iter()
                    .find(|e| e.id == *id)
                    .ok_or_else(|| format!("Check object #{id} is missing"))
            })
            .collect::<Result<Vec<_>, _>>()?,
        Scope::Candidate { story, option } => {
            if !p.stories.iter().any(|s| s.id == *story)
                || option.is_some_and(|id| p.design_options.name(id).is_none())
            {
                return Err("Check candidate or story is missing".into());
            }
            p.elements
                .iter()
                .filter(|e| {
                    e.story == *story
                        && e.design_option == *option
                        && e.worksheet.is_none()
                        && d.domain.accepts(e)
                })
                .collect()
        }
    };
    if items.is_empty() || items.len() > 256 {
        return Err("Check scope is empty or exceeds 256 objects".into());
    }
    if items
        .iter()
        .any(|e| e.worksheet.is_some() || !d.domain.accepts(e))
    {
        return Err("Check supports only its declared concept mass or Zone domain".into());
    }
    if items.iter().any(|e| e.story != items[0].story)
        || (items
            .iter()
            .any(|e| e.design_option != items[0].design_option)
            && !common_mass_spacing(p, d, &items))
    {
        return Err("Check objects must share one story and candidate".into());
    }
    items.sort_by_key(|e| e.id);
    Ok(items)
}
fn rule_basis(p: &Project, d: &Definition) -> serde_json::Value {
    match &d.basis {
        Basis::DesignAssumption { note } => serde_json::json!({"assumption":note}),
        Basis::CitedRequirement { id } => {
            let r = p.rules.requirements.iter().find(|r| r.id == *id);
            serde_json::json!({"requirement":r,"basis":r.map(|r|p.rules.basis(&r.citation.source_id)),"state":r.map(|r|p.rules.requirement_state(p,r)),"targets":r.map(|r|r.links.iter().map(|l|l.target.current_hash(p)).collect::<Vec<_>>())})
        }
    }
}
pub fn fingerprint(p: &Project, d: &Definition) -> Result<String, String> {
    let inputs = match &d.scope {
        Scope::Objects { ids } => ids
            .iter()
            .map(|id| serde_json::json!({"id":id,"element":p.elements.iter().find(|e|e.id==*id)}))
            .collect::<Vec<_>>(),
        Scope::Candidate { story, option } => p
            .elements
            .iter()
            .filter(|e| {
                e.story == *story
                    && e.design_option == *option
                    && e.worksheet.is_none()
                    && d.domain.accepts(e)
            })
            .map(|e| serde_json::json!(e))
            .collect(),
    };
    let mut input = serde_json::json!({"engine":1,"geometry_policy":crate::precision::GEOMETRY_POLICY_VERSION,"boundary_area_tolerance_m2":crate::precision::TolerancePolicy::default().area_m2.absolute,"definition":d,"inputs":inputs,"story":match &d.scope {Scope::Candidate{story,..}=>p.stories.iter().find(|s|s.id==*story),_=>None},"candidate_exists":match &d.scope {Scope::Candidate{option:Some(id),..}=>p.design_options.name(*id).is_some(),_=>true},"basis":rule_basis(p,d),"boundary":if d.metric.uses_boundary() {Some(serde_json::json!({"reference":d.metric.boundary_role().get(&p.site),"outline":crate::site::boundary::outline_for(p,d.metric.boundary_role())}))}else{None}});
    if let Scope::Objects { ids } = &d.scope {
        let items: Vec<_> = p.elements.iter().filter(|e| ids.contains(&e.id)).collect();
        if common_mass_spacing(p, d, &items) {
            // Old mixed-scope snapshots must be reviewed; unrelated historical
            // reports retain their exact engine-1 fingerprint and freshness.
            input["scope_policy"] = serde_json::json!("explicit_common_mass_spacing_v1");
        }
    }
    crate::automation_journal::canonical_hash(&input)
}
fn basis_state(p: &Project, d: &Definition) -> (Option<State>, String) {
    let Basis::CitedRequirement { id } = &d.basis else {
        return (
            None,
            "Explicit design assumption; not a measured or statutory limit".into(),
        );
    };
    if !p.rules.context.complete() {
        return (
            Some(State::Unknown),
            "Project applicability context is incomplete".into(),
        );
    }
    let Some(r) = p.rules.requirements.iter().find(|r| r.id == *id) else {
        return (Some(State::Unknown), "Cited requirement is missing".into());
    };
    let state = p.rules.requirement_state(p, r);
    if state != r.decision.applicability.label() {
        return (Some(State::Unknown), state.into());
    }
    use crate::rules::{Adoption as B, Applicability as A};
    match (r.decision.applicability,r.decision.adoption) {
        (A::NotApplicable,B::NotAdopted)=>(Some(State::NotApplicable),"Current authored non-applicability declaration; not a legal ruling".into()),
        (A::Applicable,B::Adopted)=>(None,"Numerical interpretation of a declared adopted citation; original authority not certified".into()),
        _=>(Some(State::Unknown),"Requirement applicability or adoption needs review".into())
    }
}
fn line(r: &[Point]) -> LineString<f64> {
    let mut v = r.iter().map(|p| (p.x, p.y)).collect::<Vec<_>>();
    if v.first() != v.last() {
        if let Some(first) = v.first().copied() {
            v.push(first);
        }
    }
    LineString::from(v)
}
fn polygon(e: &Element) -> Result<Polygon<f64>, String> {
    let (outline, holes) = match &e.shape {
        Shape::ConceptMass { spec } => {
            spec.validate()?;
            {
                let mut paths = spec.paths().into_iter();
                (paths.next().unwrap_or_default(), paths.collect::<Vec<_>>())
            }
        }
        Shape::Zone { spec } => {
            spec.validate()?;
            (spec.outline.clone(), spec.holes.clone())
        }
        _ => return Err("Unsupported check geometry".into()),
    };
    Ok(Polygon::new(
        line(&outline),
        holes.iter().map(|h| line(h)).collect(),
    ))
}
/// Shared zero-based edge mapping for calculation and presentation.
pub fn reference_edge_endpoints(outline: &[[f64; 2]], edge: u32) -> Option<[[f64; 2]; 2]> {
    let index = usize::try_from(edge).ok()?;
    let a = *outline.get(index)?;
    Some([a, outline[(index + 1) % outline.len()]])
}
/// Restrict half-plane distances to convex reference boundaries.
fn reference_edge(p: &Project, edge: u32) -> Result<(Point, [f64; 2]), String> {
    let outline = crate::site::boundary::outline(p)?
        .ok_or("Declared reference boundary is missing; no legal boundary is inferred")?;
    let [a, b] = reference_edge_endpoints(&outline, edge).ok_or("Reference edge is missing")?;
    let origin = outline[0];
    let signed_area: f64 = outline
        .iter()
        .zip(outline.iter().cycle().skip(1))
        .take(outline.len())
        .map(|(a, b)| {
            (a[0] - origin[0]) * (b[1] - origin[1]) - (a[1] - origin[1]) * (b[0] - origin[0])
        })
        .sum();
    let orientation = signed_area.signum();
    for i in 0..outline.len() {
        let a = outline[i];
        let b = outline[(i + 1) % outline.len()];
        let c = outline[(i + 2) % outline.len()];
        let turn = (b[0] - a[0]) * (c[1] - b[1]) - (b[1] - a[1]) * (c[0] - b[0]);
        if turn * orientation < 0. {
            return Err("Per-edge clearance requires a convex reference boundary".into());
        }
    }
    let dx = b[0] - a[0];
    let dy = b[1] - a[1];
    let length = dx.hypot(dy);
    if length <= 0. || !length.is_finite() || orientation == 0. {
        return Err("Invalid reference edge geometry".into());
    }
    Ok((
        Point::new(a[0], a[1]),
        [-dy / length * orientation, dx / length * orientation],
    ))
}
fn evaluate(p: &Project, d: &Definition) -> Vec<Row> {
    let (basis, reason) = basis_state(p, d);
    if basis == Some(State::NotApplicable) {
        return vec![Row {
            ids: vec![],
            value: None,
            state: State::NotApplicable,
            reason,
        }];
    }
    let unknown = |ids: Vec<u64>, why: String| Row {
        ids,
        value: None,
        state: State::Unknown,
        reason: why,
    };
    let items = match objects(p, d) {
        Ok(v) => v,
        Err(e) => return vec![unknown(vec![], e)],
    };
    let row = |ids: Vec<u64>, value: f64| {
        if !value.is_finite() {
            return unknown(ids, "Non-finite geometry result".into());
        }
        let state = if let Some(s) = basis {
            s
        } else if let Some(limit) = d.limit {
            if match d.relation {
                Relation::AtMost => value <= limit,
                Relation::AtLeast => value >= limit,
            } {
                State::Pass
            } else {
                State::Fail
            }
        } else {
            State::Unknown
        };
        Row {
            ids,
            value: Some(value),
            state,
            reason: if d.limit.is_none() {
                "Limit is unknown; no default threshold is inferred".into()
            } else {
                reason.clone()
            },
        }
    };
    let ids = items.iter().map(|e| e.id).collect::<Vec<_>>();
    match d.metric {
        Metric::Height => items
            .iter()
            .map(|e| match &e.shape {
                Shape::ConceptMass { spec } => row(vec![e.id], spec.height),
                Shape::Zone { spec } => row(vec![e.id], spec.height),
                _ => unreachable!(),
            })
            .collect(),
        Metric::FootprintAreaSum => {
            let result = items
                .iter()
                .map(|e| polygon(e).map(|p| p.unsigned_area()))
                .sum::<Result<f64, _>>();
            vec![match result {
                Ok(v) => row(ids, v),
                Err(e) => unknown(ids, e),
            }]
        }
        Metric::HorizontalSpacing => {
            let result = polygon(items[0])
                .and_then(|a| polygon(items[1]).map(|b| Euclidean.distance(&a, &b)));
            vec![match result {
                Ok(v) => row(ids, v),
                Err(e) => unknown(ids, e),
            }]
        }
        Metric::ReferenceBoundaryEdgeClearance { edge } => {
            let (origin, normal) = match reference_edge(p, edge) {
                Ok(v) => v,
                Err(e) => return vec![unknown(ids, e)],
            };
            items
                .iter()
                .map(|e| match polygon(e) {
                    Ok(poly) => row(
                        vec![e.id],
                        poly.exterior()
                            .0
                            .iter()
                            .map(|q| (q.x - origin.x) * normal[0] + (q.y - origin.y) * normal[1])
                            .fold(f64::INFINITY, f64::min),
                    ),
                    Err(err) => unknown(vec![e.id], err),
                })
                .collect()
        }
        Metric::ReferenceBoundaryClearance | Metric::ReferenceControlRegionClearance => {
            let control = d.metric == Metric::ReferenceControlRegionClearance;
            let border = match crate::site::boundary::outline_for(p, d.metric.boundary_role()) {
                Ok(Some(r)) => Polygon::new(
                    LineString::from(r.into_iter().map(|v| (v[0], v[1])).collect::<Vec<_>>()),
                    vec![],
                ),
                Ok(None) => {
                    return vec![unknown(
                        ids,
                        (if control {"Declared control region is missing; no approved control line is inferred"} else {"Declared reference boundary is missing; no legal boundary is inferred"}).into(),
                    )];
                }
                Err(e) => return vec![unknown(ids, e)],
            };
            items
                .iter()
                .map(|e| match polygon(e) {
                    Ok(poly) => {
                        // A footprint outside a concave boundary has no positive inside clearance.
                        if poly.difference(&border).unsigned_area()
                            > crate::precision::TolerancePolicy::default()
                                .area_m2
                                .absolute
                        {
                            return Row {
                                ids: vec![e.id],
                                value: None,
                                state: if basis.is_none() && d.limit.is_some() {
                                    State::Fail
                                } else {
                                    State::Unknown
                                },
                                reason: (if control {
                                    "Footprint lies outside the declared control region"
                                } else {
                                    "Footprint lies outside the declared reference boundary"
                                })
                                .into(),
                            };
                        }
                        row(
                            vec![e.id],
                            Euclidean.distance(poly.exterior(), border.exterior()),
                        )
                    }
                    Err(err) => unknown(vec![e.id], err),
                })
                .collect()
        }
    }
}
impl Check {
    pub fn freshness(&self, p: &Project) -> &'static str {
        match &self.report {
            None => "Check has not been run",
            Some(r)
                if fingerprint(p, &self.definition).ok().as_deref() == Some(&r.source_sha256) =>
            {
                "Check inputs unchanged"
            }
            _ => "Check result is out of date",
        }
    }
    pub fn state(&self, p: &Project) -> State {
        // A portable project contains a numerical snapshot, not current access
        // to an external original. Only the shared live-original read can lift
        // this unknown for the returned revision-bound observation.
        if matches!(self.definition.basis, Basis::CitedRequirement { .. }) {
            return State::Unknown;
        }
        self.numerical_state(p)
    }
    pub fn numerical_state(&self, p: &Project) -> State {
        if self.freshness(p) != "Check inputs unchanged" {
            return State::Unknown;
        }
        rows_state(&self.report.as_ref().unwrap().rows)
    }
}
/// A numerical observation over an immutable proposed project, never a saved
/// Report generation or evidence that an external original was read.
#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct Preview {
    pub definition: Definition,
    pub source_sha256: Option<String>,
    pub state: State,
    pub numerical_state: State,
    pub rows: Vec<Row>,
}
pub fn preview(p: &Project, d: &Definition) -> Preview {
    let input = d.validate().and_then(|_| fingerprint(p, d));
    let (source_sha256, rows) = match input {
        Ok(hash) => (Some(hash), evaluate(p, d)),
        Err(reason) => (
            None,
            vec![Row {
                ids: vec![],
                value: None,
                state: State::Unknown,
                reason,
            }],
        ),
    };
    let numerical_state = rows_state(&rows);
    let state = if matches!(d.basis, Basis::CitedRequirement { .. }) {
        State::Unknown
    } else {
        numerical_state
    };
    Preview {
        definition: d.clone(),
        source_sha256,
        state,
        numerical_state,
        rows,
    }
}
/// Only stored definitions actually touched by these changed model members.
/// No invented threshold, check creation, report mutation or Project clone.
pub fn preview_affected(before: &Project, after: &Project, ids: &[u64]) -> Vec<Preview> {
    after
        .rules
        .checks
        .iter()
        .filter(|c| match &c.definition.scope {
            Scope::Objects { ids: scope } => scope.iter().any(|id| ids.contains(id)),
            Scope::Candidate { story, option } => {
                before.elements.iter().chain(&after.elements).any(|e| {
                    ids.contains(&e.id)
                        && e.story == *story
                        && e.design_option == *option
                        && c.definition.domain.accepts(e)
                })
            }
        })
        .map(|c| preview(after, &c.definition))
        .collect()
}

/// Shared reduction for check results and their read-only figure projections.
pub(crate) fn rows_state(rows: &[Row]) -> State {
    if rows.is_empty() {
        State::Unknown
    } else if rows.iter().any(|r| r.state == State::Fail) {
        State::Fail
    } else if rows.iter().any(|r| r.state == State::Unknown) {
        State::Unknown
    } else if rows.iter().all(|r| r.state == State::NotApplicable) {
        State::NotApplicable
    } else {
        State::Pass
    }
}
pub fn validate(book: &crate::rules::RuleBook) -> Result<(), String> {
    if book.checks.len() > 128 {
        return Err("Quantified check limit is 128".into());
    }
    let mut ids = std::collections::BTreeSet::new();
    for c in &book.checks {
        c.definition.validate()?;
        if !ids.insert(&c.definition.id) {
            return Err("Duplicate quantified check identity".into());
        }
        if let Some(r) = &c.report {
            r.input_definition.validate()?;
            if r.input_definition.id != c.definition.id
                || r.generation == 0
                || !crate::storage::valid_document_identity(&r.source_sha256)
                || r.rows.is_empty()
                || r.rows.len() > 256
                || r.rows.iter().map(|row| row.ids.len()).sum::<usize>() > 256
                || r.rows.iter().any(|row| {
                    row.ids.len() > 256
                        || row.value.is_some_and(|v| {
                            !v.is_finite()
                                || (v < 0.
                                    && !matches!(
                                        r.input_definition.metric,
                                        Metric::ReferenceBoundaryEdgeClearance { .. }
                                    ))
                        })
                        || !text(&row.reason, 512, false)
                })
            {
                return Err("Invalid stored quantified check report".into());
            }
        }
    }
    Ok(())
}
pub fn validate_project(p: &Project) -> Result<(), String> {
    for c in &p.rules.checks {
        if let Some(r) = &c.report {
            if c.freshness(p) == "Check inputs unchanged"
                && (r.input_definition != c.definition || r.rows != evaluate(p, &c.definition))
            {
                return Err("Stored quantified check does not match its current inputs".into());
            }
        }
    }
    Ok(())
}
pub fn apply(p: &Project, edit: &Edit) -> Result<Vec<Check>, String> {
    let mut next = p.rules.checks.clone();
    match edit {
        Edit::Put {
            expected_sha256,
            definition,
        } => {
            definition.validate()?;
            let index = next.iter().position(|c| c.definition.id == definition.id);
            if index.map(|i| expected(&next[i])) != *expected_sha256 {
                return Err("Quantified check changed after preview".into());
            }
            // Reject unrelated/unsupported selected objects rather than dropping them.
            if matches!(definition.scope, Scope::Objects { .. }) {
                objects(p, definition)?;
            }
            let value = Check {
                definition: definition.clone(),
                report: index.and_then(|i| next[i].report.clone()),
            };
            if let Some(i) = index {
                next[i] = value
            } else {
                next.push(value)
            }
        }
        Edit::Run {
            id,
            expected_sha256,
        } => {
            let c = next
                .iter_mut()
                .find(|c| c.definition.id == *id)
                .ok_or("Quantified check is missing")?;
            if expected(c) != *expected_sha256 {
                return Err("Quantified check changed after preview".into());
            }
            c.report = Some(Report {
                input_definition: c.definition.clone(),
                generation: c.report.as_ref().map_or(Ok(1), |r| {
                    r.generation
                        .checked_add(1)
                        .ok_or("Check generation exhausted")
                })?,
                source_sha256: fingerprint(p, &c.definition)?,
                rows: evaluate(p, &c.definition),
            });
        }
        Edit::Remove {
            id,
            expected_sha256,
        } => {
            let index = next
                .iter()
                .position(|c| c.definition.id == *id)
                .ok_or("Quantified check is missing")?;
            if expected(&next[index]) != *expected_sha256 {
                return Err("Quantified check changed after preview".into());
            }
            next.remove(index);
        }
    }
    let mut book = p.rules.clone();
    book.checks = next;
    validate(&book)?;
    Ok(book.checks)
}
/// Paged caller supplies its own bounds; no project-wide compliance pass is emitted.
pub fn report(p: &Project) -> serde_json::Value {
    serde_json::json!({"compliance_verdict":"not_evaluated","mandatory_coverage":p.rules.coverage(p),"semantics":"Numerical design checks only; authored parameter heights/footprints, not statutory building height or floor area; declared reference boundary is not approved legal boundary","checks":p.rules.checks.iter().map(|c|entry(p,c)).collect::<Vec<_>>()})
}
pub fn entry(p: &Project, c: &Check) -> serde_json::Value {
    serde_json::json!({"definition":c.definition,"expected_sha256":expected(c),"freshness":c.freshness(p),"freshness_scope":"model_and_declarations_only","state":c.state(p),"numerical_state":c.numerical_state(p),"original_verification_binding":originals::binding(&p.rules,c),"original_verification":if matches!(c.definition.basis,Basis::CitedRequirement{..}) {serde_json::json!({"status":"not_checked","meaning":"Current cited result requires the bound original; a stored numerical snapshot is not original verification"})}else{serde_json::json!({"status":"not_required","meaning":"Explicit design assumption"})},"unit":c.definition.metric.unit_code(),"stored_report_unit":c.report.as_ref().map(|r|r.input_definition.metric.unit_code()),"report":c.report,"semantics":"Numerical design declarations; not legal approval. Missing mandatory project conditions are not waived."})
}
