//! Portable check overlays, derived from the existing check engine and model.
//! No live source read or authority certificate is persisted in a figure.
use super::{Definition, Diagram, Snapshot};
use crate::{
    constraints::{self, Basis, Check, Scope, State},
    model::{Element, Project},
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub check: Check,
    pub input_sha256: String,
    pub state: State,
    pub active_ids: Vec<u64>,
    pub citation: Option<crate::rules::Citation>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Lineage {
    pub link: crate::mass_spaces::Link,
    pub status: String,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub version: u32,
    pub entries: Vec<Entry>,
    pub other_scope_ids: Vec<String>,
    /// Missing objects cannot be assigned to a candidate by inference.
    pub unlocated_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lineage: Vec<Lineage>,
    /// Only Main footprints explicitly paired with this candidate by a supported
    /// spacing check. These are context, never candidate quantities or authority.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub common_context: Vec<Element>,
}
fn active_ids(c: &Check, d: &Definition, elements: &[Element]) -> Vec<u64> {
    elements
        .iter()
        .filter(|e| {
            c.definition.domain.accepts(e)
                && match &c.definition.scope {
                    Scope::Objects { ids } => ids.contains(&e.id),
                    Scope::Candidate { story, option } => *story == d.story && *option == d.option,
                }
        })
        .map(|e| e.id)
        .collect()
}
impl Source {
    pub fn capture(p: &Project, d: &Definition, elements: &[Element]) -> Result<Self, String> {
        Self::capture_if_present(p, d, elements)?.ok_or_else(|| {
            "Constraint figure needs checks for this candidate (at most 128 project checks)".into()
        })
    }
    /// Read the same candidate scope for comparison without requiring the user
    /// to create checks. None means no bound check, never a numerical pass.
    /// Figure capture/validation still requires at least one real entry.
    pub fn capture_if_present(
        p: &Project,
        d: &Definition,
        elements: &[Element],
    ) -> Result<Option<Self>, String> {
        let mut source = Self {
            version: 1,
            entries: vec![],
            other_scope_ids: vec![],
            unlocated_ids: vec![],
            common_context: vec![],
            lineage: p
                .design_process
                .mass_spaces
                .iter()
                .filter(|l| elements.iter().any(|e| e.id == l.zone))
                .map(|l| Lineage {
                    link: l.clone(),
                    status: crate::mass_spaces::status(p, l).into(),
                })
                .collect(),
        };
        source.lineage.sort_by_key(|l| l.link.zone);
        for c in &p.rules.checks {
            let ids = active_ids(c, d, elements);
            let candidate = matches!(c.definition.scope,Scope::Candidate{story,option} if story==d.story && option==d.option);
            if !ids.is_empty() || candidate {
                let citation = match &c.definition.basis {
                    Basis::CitedRequirement { id } => p
                        .rules
                        .requirements
                        .iter()
                        .find(|r| r.id == *id)
                        .map(|r| r.citation.clone()),
                    _ => None,
                };
                source.entries.push(Entry {
                    check: c.clone(),
                    input_sha256: constraints::fingerprint(p, &c.definition)?,
                    state: c.state(p),
                    active_ids: ids,
                    citation,
                });
            } else {
                let unlocated = match &c.definition.scope {
                    Scope::Objects { ids } => {
                        ids.iter().all(|id| !p.elements.iter().any(|e| e.id == *id))
                    }
                    Scope::Candidate { story, option } => {
                        !p.stories.iter().any(|s| s.id == *story)
                            || option.is_some_and(|id| p.design_options.name(id).is_none())
                    }
                };
                if unlocated {
                    source.unlocated_ids.push(c.definition.id.clone());
                } else {
                    source.other_scope_ids.push(c.definition.id.clone());
                }
            }
        }
        source
            .entries
            .sort_by(|a, b| a.check.definition.id.cmp(&b.check.definition.id));
        source.other_scope_ids.sort();
        source.unlocated_ids.sort();
        if source.entries.is_empty() {
            return Ok(None);
        }
        let mut common = std::collections::BTreeMap::new();
        for entry in &source.entries {
            if let Scope::Objects { ids } = &entry.check.definition.scope {
                let items: Vec<_> = ids
                    .iter()
                    .filter_map(|id| p.elements.iter().find(|e| e.id == *id))
                    .collect();
                if d.option.is_some()
                    && items.iter().any(|e| e.design_option == d.option)
                    && constraints::common_mass_spacing(p, &entry.check.definition, &items)
                {
                    for e in items.into_iter().filter(|e| e.design_option.is_none()) {
                        common.insert(e.id, e.clone());
                    }
                }
            }
        }
        source.common_context = common.into_values().collect();
        if !source.common_context.is_empty() {
            source.version = 2;
        }
        source.validate(d, elements)?;
        Ok(Some(source))
    }
    pub fn read(s: &Snapshot) -> Result<Self, String> {
        serde_json::from_value(
            s.context
                .get("constraint_overlay")
                .ok_or("Constraint figure snapshot is missing")?
                .clone(),
        )
        .map_err(|_| "Invalid constraint figure snapshot".into())
    }
    pub fn validate(&self, d: &Definition, elements: &[Element]) -> Result<(), String> {
        let count = self.entries.len() + self.other_scope_ids.len() + self.unlocated_ids.len();
        if self.version != if self.common_context.is_empty() { 1 } else { 2 }
            || count > 128
            || self.entries.is_empty()
            || self.common_context.len() > 32
        {
            return Err(
                "Constraint figure needs checks for this candidate (at most 128 project checks)"
                    .into(),
            );
        }
        crate::mass_spaces::validate(
            &self
                .lineage
                .iter()
                .map(|v| v.link.clone())
                .collect::<Vec<_>>(),
        )?;
        if self
            .lineage
            .windows(2)
            .any(|w| w[0].link.zone >= w[1].link.zone)
            || self.lineage.iter().any(|l| {
                !elements.iter().any(|e| {
                    e.id == l.link.zone && matches!(e.shape, crate::model::Shape::Zone { .. })
                }) || !matches!(
                    l.status.as_str(),
                    "Mass-space link is current"
                        | "Mass-space source is unavailable"
                        | "Linked concept space is missing"
                        | "Mass-space scenario changed"
                        | "Linked space has user edits"
                        | "Mass-space source changed"
                )
            })
        {
            return Err("Invalid constraint figure space lineage".into());
        }
        let mut ids = BTreeSet::new();
        for id in self
            .entries
            .iter()
            .map(|e| &e.check.definition.id)
            .chain(&self.other_scope_ids)
            .chain(&self.unlocated_ids)
        {
            if id.trim().is_empty() || id.len() > 128 || id.contains('\0') || !ids.insert(id) {
                return Err("Invalid constraint figure identities".into());
            }
        }
        if self
            .entries
            .windows(2)
            .any(|w| w[0].check.definition.id >= w[1].check.definition.id)
            || self.other_scope_ids.windows(2).any(|w| w[0] >= w[1])
            || self.unlocated_ids.windows(2).any(|w| w[0] >= w[1])
        {
            return Err("Invalid constraint figure ordering".into());
        }
        let mut book = crate::rules::RuleBook::default();
        book.checks = self.entries.iter().map(|e| e.check.clone()).collect();
        constraints::validate(&book)?;
        if self.common_context.windows(2).any(|w| w[0].id >= w[1].id) {
            return Err("Invalid common check context ordering".into());
        }
        for common in &self.common_context {
            let crate::model::Shape::ConceptMass { spec } = &common.shape else {
                return Err("Common check context must be a concept mass".into());
            };
            spec.validate()?;
            let paired = self.entries.iter().any(|entry| {
                let def = &entry.check.definition;
                let Scope::Objects { ids } = &def.scope else {
                    return false;
                };
                def.domain == constraints::Domain::ConceptMasses
                    && def.metric == constraints::Metric::HorizontalSpacing
                    && ids.len() == 2
                    && ids.contains(&common.id)
                    && elements.iter().any(|e| {
                        ids.contains(&e.id)
                            && e.id != common.id
                            && e.worksheet.is_none()
                            && e.story == d.story
                            && e.design_option == d.option
                            && e.design_option.is_some()
                            && matches!(e.shape, crate::model::Shape::ConceptMass { .. })
                    })
            });
            if !paired
                || common.id == 0
                || common.story != d.story
                || common.design_option.is_some()
                || common.worksheet.is_some()
                || common.name.trim().is_empty()
                || common.name.len() > 256
                || common.name.chars().any(char::is_control)
                || elements.iter().any(|e| e.id == common.id)
            {
                return Err("Invalid common check context scope".into());
            }
        }
        for e in &self.entries {
            if !crate::storage::valid_document_identity(&e.input_sha256)
                || e.active_ids != active_ids(&e.check, d, elements)
            {
                return Err("Invalid constraint figure input binding".into());
            }
            let cited = matches!(e.check.definition.basis, Basis::CitedRequirement { .. });
            let expected = if cited {
                State::Unknown
            } else {
                e.check
                    .report
                    .as_ref()
                    .filter(|r| r.source_sha256 == e.input_sha256)
                    .map_or(State::Unknown, |r| constraints::rows_state(&r.rows))
            };
            if expected != e.state || (!cited && e.citation.is_some()) {
                return Err("Constraint figure state contradicts its stored inputs".into());
            }
        }
        Ok(())
    }
    /// Exact per-row status; an aggregate failure does not colour unrelated
    /// passing rows of the same multi-object height check as failures.
    pub fn element_state(&self, id: u64) -> Option<State> {
        let mut rows = vec![];
        for e in &self.entries {
            if !e.active_ids.contains(&id) {
                continue;
            }
            let state = if e.state == State::Unknown {
                State::Unknown
            } else {
                let matching = e
                    .check
                    .report
                    .as_ref()
                    .map(|r| {
                        r.rows
                            .iter()
                            .filter(|r| r.ids.contains(&id))
                            .cloned()
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                constraints::rows_state(&matching)
            };
            rows.push(constraints::Row {
                ids: vec![id],
                state,
                value: None,
                reason: String::new(),
            });
        }
        if self
            .lineage
            .iter()
            .any(|l| l.link.zone == id && l.status != "Mass-space link is current")
        {
            rows.push(constraints::Row {
                ids: vec![id],
                state: State::Unknown,
                value: None,
                reason: "Linked source needs review".into(),
            });
        }
        (!rows.is_empty()).then(|| constraints::rows_state(&rows))
    }
}
pub fn color(state: State) -> [u8; 3] {
    match state {
        State::Fail => [193, 74, 60],
        State::Unknown => [183, 132, 36],
        State::Pass => [52, 123, 92],
        State::NotApplicable => [130, 137, 145],
    }
}
fn symbol(s: State) -> &'static str {
    match s {
        State::Pass => "✓",
        State::Fail => "×",
        State::Unknown => "?",
        State::NotApplicable => "—",
    }
}
/// Endpoints from this portable figure's boundary snapshot, never today's model.
pub fn reference_edge_lines(
    d: &Diagram,
    metric: constraints::Metric,
    t: &impl Fn(&str) -> String,
) -> Vec<String> {
    if metric == constraints::Metric::ReferenceControlRegionClearance {
        return match d
            .snapshot
            .site
            .as_ref()
            .and_then(|s| s.context.control_region.as_ref())
        {
            Some(region) => vec![
                format!(
                    "{} #{} · {}",
                    t("Declared control region"),
                    region.element_id,
                    super::short(&region.name, 20)
                ),
                format!("{} {}", t("Source"), &region.origin.source_sha256[..12]),
            ],
            None => vec![t(
                "Declared control region is missing; no approved control line is inferred",
            )],
        };
    }
    let constraints::Metric::ReferenceBoundaryEdgeClearance { edge } = metric else {
        return vec![];
    };
    let outline = d
        .snapshot
        .site
        .as_ref()
        .and_then(|s| s.outline().ok())
        .unwrap_or_default();
    let points: Vec<_> = outline.iter().map(|p| [p.x, p.y]).collect();
    match constraints::reference_edge_endpoints(&points, edge) {
        Some([a, b]) => vec![
            format!(
                "{} {}: ({:.3}, {:.3})",
                t("Reference edge"),
                u64::from(edge) + 1,
                a[0],
                a[1]
            ),
            format!("→ ({:.3}, {:.3}) m", b[0], b[1]),
        ],
        None => vec![t("Reference edge is missing")],
    }
}
pub fn text_lines(d: &Diagram, t: &impl Fn(&str) -> String) -> Vec<(f64, f64, String, f32)> {
    let mut lines = vec![
        (
            40.,
            38.,
            format!(
                "#{} · v{} · {}",
                d.id,
                d.generation,
                super::short(&d.name, 65)
            ),
            22.,
        ),
        (
            40.,
            66.,
            format!(
                "{} · {}",
                super::short(&d.snapshot.option_name, 45),
                super::short(&d.snapshot.story.name, 30)
            ),
            15.,
        ),
        (
            40.,
            91.,
            t("Authored constraint checks · not project approval"),
            14.,
        ),
    ];
    let Ok(source) = Source::read(&d.snapshot) else {
        return lines;
    };
    lines.push((
        835.,
        112.,
        format!(
            "{}: {}",
            t("Checks in this candidate"),
            source.entries.len()
        ),
        12.,
    ));
    let mut y = 138.;
    let mut shown = 0;
    for (i, e) in source.entries.iter().take(9).enumerate() {
        let def = &e.check.definition;
        let edge_lines = reference_edge_lines(d, def.metric, t);
        if y + 28. + edge_lines.len() as f64 * 12. > 560. {
            break;
        }
        lines.push((
            835.,
            y,
            format!(
                "C{} {} {}",
                i + 1,
                symbol(e.state),
                super::short(&def.title, 20)
            ),
            11.,
        ));
        lines.push((835., y + 14., t(e.state.label()), 10.));
        let values = e
            .check
            .report
            .as_ref()
            .map(|r| {
                let numbers = r.rows.iter().filter_map(|r| r.value).collect::<Vec<_>>();
                let range = if numbers.is_empty() {
                    t("Unknown")
                } else {
                    let lo = numbers.iter().copied().fold(f64::INFINITY, f64::min);
                    let hi = numbers.iter().copied().fold(f64::NEG_INFINITY, f64::max);
                    if lo == hi {
                        format!("{lo:.3}")
                    } else {
                        format!("{lo:.3}–{hi:.3}")
                    }
                };
                format!(
                    "v{}: {} {}",
                    r.generation,
                    range,
                    r.input_definition.metric.unit()
                )
            })
            .unwrap_or_else(|| t("Check has not been run"));
        lines.push((
            835.,
            y + 28.,
            format!(
                "{} {} {} · {}",
                t(def.relation.label()),
                def.limit
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| t("Unknown")),
                def.metric.unit(),
                values
            ),
            10.,
        ));
        for (j, line) in edge_lines.iter().enumerate() {
            lines.push((835., y + 40. + j as f64 * 12., line.clone(), 9.));
        }
        y += 47. + edge_lines.len() as f64 * 12.;
        shown += 1;
    }
    let summary = format!(
        "{}: {} · {}: {} · {}: {}",
        t("Other scope"),
        source.other_scope_ids.len(),
        t("Unlocated checks"),
        source.unlocated_ids.len(),
        t("Full check list"),
        source.entries.len()
    );
    lines.push((
        40.,
        735.,
        format!(
            "{} · {}: {}",
            summary,
            t("Derived spaces needing review"),
            source
                .lineage
                .iter()
                .filter(|l| l.status != "Mass-space link is current")
                .count()
        ),
        11.,
    ));
    if source.entries.len() > shown {
        lines.push((
            835.,
            580.,
            format!(
                "+{} · {}",
                source.entries.len() - shown,
                t("Full check list")
            ),
            11.,
        ));
    }
    lines.push((
        40.,
        760.,
        t("Cited originals remain unverified in this portable figure; read originals in checks."),
        11.,
    ));
    lines.push((
        40.,
        783.,
        format!(
            "{} {} · {} · {}: {}",
            t("Source"),
            &d.source_sha256[..16],
            t("Assumptions remain assumptions"),
            t("North"),
            t(if d.snapshot.true_north.is_some() {
                "Declared"
            } else {
                "Unknown"
            })
        ),
        12.,
    ));
    lines
}
