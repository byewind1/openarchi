//! Bounded project design records. Live geometry stays exclusively in Project.elements.
use crate::{
    model::{Element, Project, Shape},
    view_settings::DesignOptionScope,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
pub mod intent;
pub mod parametric;
pub mod service;
pub mod update;
fn hash<T: Serialize>(value: &T) -> Result<String, String> {
    crate::automation_journal::canonical_hash(value)
}
fn text(value: &str, limit: usize, required: bool) -> bool {
    value.len() <= limit
        && (!required || !value.trim().is_empty())
        && !value
            .chars()
            .any(|c| c.is_control() && c != '\n' && c != '\t')
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum StatementKind {
    Requirement,
    Preference,
    Assumption,
    Unknown,
}
impl StatementKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Requirement => "Declared requirement (not evaluated)",
            Self::Preference => "Preference",
            Self::Assumption => "Exploration assumption",
            Self::Unknown => "Unknown condition",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Statement {
    pub kind: StatementKind,
    pub text: String,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Entry {
    ManualInterface,
    AgentProposal,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Candidate {
    pub option: u32,
    pub name: String,
    pub initial_zone_ids: Vec<u64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub initial_mass_ids: Vec<u64>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Task {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intent: Option<intent::Provenance>,
    pub id: String,
    pub name: String,
    pub question: String,
    pub statements: Vec<Statement>,
    pub entry: Entry,
    pub source_document: String,
    pub created_unix: u64,
    pub story: u32,
    pub source_zone_ids: Vec<u64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_mass_ids: Vec<u64>,
    pub baseline_option: u32,
    pub basis_sha256: String,
    pub candidates: [Candidate; 2],
}
impl Candidate {
    pub fn element_ids(&self) -> &[u64] {
        if self.initial_mass_ids.is_empty() {
            &self.initial_zone_ids
        } else {
            &self.initial_mass_ids
        }
    }
}
impl Task {
    pub fn is_mass(&self) -> bool {
        !self.source_mass_ids.is_empty()
    }
    pub fn source_element_ids(&self) -> &[u64] {
        if self.is_mass() {
            &self.source_mass_ids
        } else {
            &self.source_zone_ids
        }
    }
    pub fn accepts_shape(&self, shape: &Shape) -> bool {
        if self.is_mass() {
            matches!(shape, Shape::ConceptMass { .. })
        } else {
            matches!(shape, Shape::Zone { .. })
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        let mut options = BTreeSet::from([self.baseline_option]);
        if !crate::storage::valid_document_identity(&self.id)
            || !crate::storage::valid_document_identity(&self.source_document)
            || !crate::storage::valid_document_identity(&self.basis_sha256)
            || !text(&self.name, 256, true)
            || !text(&self.question, 4096, true)
            || self.statements.len() > 32
            || !(1..=16).contains(&self.source_element_ids().len())
            || (self.is_mass()
                && (!self.source_zone_ids.is_empty()
                    || (self.intent.is_none() && self.entry != Entry::ManualInterface)))
            || self.baseline_option == 0
        {
            return Err("Invalid exploration task".into());
        }
        let mut ids = BTreeSet::new();
        for id in self.source_element_ids() {
            if *id == 0 || !ids.insert(*id) {
                return Err("Invalid exploration space identity".into());
            }
        }
        for c in &self.candidates {
            if c.option == 0
                || !options.insert(c.option)
                || !text(&c.name, 256, true)
                || c.element_ids().len() != self.source_element_ids().len()
                || (self.is_mass()
                    && (!c.initial_zone_ids.is_empty() || c.initial_mass_ids.is_empty()))
                || (!self.is_mass() && !c.initial_mass_ids.is_empty())
            {
                return Err("Invalid exploration candidate".into());
            }
            for id in c.element_ids() {
                if *id == 0 || !ids.insert(*id) {
                    return Err("Duplicate exploration space identity".into());
                }
            }
        }
        for s in &self.statements {
            if !text(&s.text, 2048, true) {
                return Err("Invalid exploration statement".into());
            }
        }
        if let Some(intent) = &self.intent {
            intent.validate()?;
        }
        Ok(())
    }
    /// The declared initial copy correspondence, never a name/geometry guess.
    /// Added or removed spaces have no implied counterpart.
    pub fn corresponding_space(&self, p: &Project, source: u64, target_option: u32) -> Option<u64> {
        if self.is_mass() {
            None
        } else {
            self.corresponding_element(p, source, target_option)
        }
    }
    pub fn corresponding_element(
        &self,
        p: &Project,
        source: u64,
        target_option: u32,
    ) -> Option<u64> {
        let index = self
            .source_element_ids()
            .iter()
            .position(|id| *id == source)
            .or_else(|| {
                self.candidates
                    .iter()
                    .find_map(|c| c.element_ids().iter().position(|id| *id == source))
            })?;
        let target = if target_option == self.baseline_option {
            *self.source_element_ids().get(index)?
        } else {
            *self
                .candidates
                .iter()
                .find(|c| c.option == target_option)?
                .element_ids()
                .get(index)?
        };
        p.elements
            .iter()
            .find(|e| {
                e.id == target
                    && e.design_option == Some(target_option)
                    && e.story == self.story
                    && e.worksheet.is_none()
                    && self.accepts_shape(&e.shape)
            })
            .map(|e| e.id)
    }
    pub fn common_hash(&self, p: &Project) -> Result<String, String> {
        self.common_hash_with_appended(p, &[])
    }
    pub(crate) fn common_hash_with_appended(
        &self,
        p: &Project,
        appended: &[Element],
    ) -> Result<String, String> {
        let own_options = BTreeSet::from([
            self.baseline_option,
            self.candidates[0].option,
            self.candidates[1].option,
        ]);
        let other_task_options = p
            .design_process
            .tasks
            .iter()
            .filter(|t| t.id != self.id)
            .flat_map(|t| {
                [
                    t.baseline_option,
                    t.candidates[0].option,
                    t.candidates[1].option,
                ]
            })
            .filter(|id| !own_options.contains(id))
            .collect::<BTreeSet<_>>();
        let mut elements = p
            .elements
            .iter()
            .chain(appended)
            .filter(|e| {
                !e.design_option
                    .is_some_and(|id| other_task_options.contains(&id))
            })
            .filter(|e| {
                !self
                    .candidates
                    .iter()
                    .any(|c| Some(c.option) == e.design_option)
            })
            .cloned()
            .collect::<Vec<_>>();
        for e in &mut elements {
            if self.source_element_ids().contains(&e.id)
                && e.design_option == Some(self.baseline_option)
            {
                e.design_option = None;
            }
        }
        elements.sort_by_key(|e| e.id);
        let mut input = serde_json::json!({"elements":elements,"stories":p.stories,"site":p.site,"program":p.program,"evidence":{"notes":p.evidence.notes},"rules":p.rules});
        // Preserve the exact legacy comparison basis for existing saved explorations.
        if let Some(intent) = &self.intent {
            input["intent_source_versions"] = intent.source_versions(p);
        }
        hash(&input)
    }
    pub fn compare(&self, p: &Project) -> Result<Comparison, String> {
        if p.design_options.name(self.baseline_option).is_none()
            || self.source_element_ids().iter().any(|id| {
                !p.elements
                    .iter()
                    .any(|e| e.id == *id && e.design_option == Some(self.baseline_option))
            })
        {
            return Err("Exploration baseline is missing".into());
        }
        let mut versions = Vec::new();
        for candidate in &self.candidates {
            if p.design_options.name(candidate.option).is_none() {
                return Err("Exploration candidate is missing".into());
            }
            let report = if self.is_mass() {
                None
            } else {
                Some(crate::zones::analysis::analyze(
                    p,
                    &crate::zones::analysis::AnalysisOptions {
                        story: Some(self.story),
                        design_scope: DesignOptionScope::Option {
                            id: candidate.option,
                        },
                        ..Default::default()
                    },
                )?)
            };
            let mut elements = p
                .elements
                .iter()
                .filter(|e| e.design_option == Some(candidate.option))
                .cloned()
                .collect::<Vec<_>>();
            elements.sort_by_key(|e| e.id);
            if elements.is_empty()
                || elements.len() > 32
                || elements.iter().any(|e| {
                    e.story != self.story
                        || e.worksheet.is_some()
                        || (!self.accepts_shape(&e.shape)
                            && !(self.is_mass() && matches!(e.shape, Shape::Zone { .. })))
                })
            {
                return Err("Exploration requires 1–32 Zone spaces in each candidate".into());
            }
            // A linked (or subsequently detached) editable Zone is supplementary
            // to this mass task. Never add its area/volume to the envelope metric
            // or include it in a mass candidate's readonly comparison snapshot.
            elements.retain(|e| self.accepts_shape(&e.shape));
            if elements.is_empty() {
                return Err("Exploration requires 1–32 Zone spaces in each candidate".into());
            }
            let mass_metrics = if self.is_mass() {
                Some(MassMetrics::from_elements(&elements)?)
            } else {
                None
            };
            let metrics = if let Some(report) = report {
                let ids = elements.iter().map(|e| e.id).collect::<BTreeSet<_>>();
                let rows = report
                    .spaces
                    .iter()
                    .filter(|s| ids.contains(&s.id))
                    .collect::<Vec<_>>();
                Metrics {
                    spaces: elements.len(),
                    area_m2: rows.iter().map(|s| s.area_m2).sum(),
                    volume_m3: rows.iter().map(|s| s.space_volume_m3).sum(),
                    wall_overlap_sum_m2: rows.iter().map(|s| s.wall_footprint_overlap_m2).sum(),
                    pair_overlap_sum_m2: report
                        .relations
                        .iter()
                        .filter(|r| ids.contains(&r.a) || ids.contains(&r.b))
                        .map(|r| r.overlap_m2)
                        .sum(),
                }
            } else {
                Metrics::default()
            };
            let mut geometry = elements
                .iter()
                .map(|e| {
                    if let Shape::Zone { spec } = &e.shape {
                        serde_json::to_string(&(
                            e.story,
                            &spec.outline,
                            &spec.holes,
                            spec.height,
                            spec.bottom_offset,
                        ))
                        .map_err(|e| e.to_string())
                    } else if let Shape::ConceptMass { spec } = &e.shape {
                        if spec
                            .arcs
                            .iter()
                            .chain(spec.hole_arcs.iter().flatten())
                            .any(Option::is_some)
                        {
                            serde_json::to_string(&(e.story, spec)).map_err(|e| e.to_string())
                        } else {
                            serde_json::to_string(&(
                                e.story,
                                &spec.outline,
                                &spec.holes,
                                spec.height,
                                spec.bottom_offset,
                            ))
                            .map_err(|e| e.to_string())
                        }
                    } else {
                        unreachable!()
                    }
                })
                .collect::<Result<Vec<_>, _>>()?;
            geometry.sort();
            versions.push(Version {
                option: candidate.option,
                name: p
                    .design_options
                    .name(candidate.option)
                    .ok_or("Exploration candidate is missing")?
                    .to_string(),
                geometry_sha256: hash(&geometry)?,
                elements,
                metrics,
                mass_metrics,
                parametric_parameters: self
                    .intent
                    .as_ref()
                    .filter(|v| v.parametric.is_some())
                    .and_then(|_| {
                        parametric::current_parameters(p, &self.id, candidate.option).ok()
                    }),
                parametric_inputs: self
                    .intent
                    .as_ref()
                    .and_then(|i| i.parametric.as_ref())
                    .and_then(|rule| {
                        parametric::current_inputs(p, &self.id, candidate.option)
                            .ok()
                            .filter(|inputs| *inputs != rule.inputs())
                    }),
            });
        }
        let candidates: [Version; 2] = versions.try_into().map_err(|_| "Invalid candidate pair")?;
        let current_basis_sha256 = self.common_hash(p)?;
        let mut result = Comparison {
            task_id: self.id.clone(),
            basis_current: current_basis_sha256 == self.basis_sha256,
            current_basis_sha256,
            different: candidates[0].geometry_sha256 != candidates[1].geometry_sha256
                || candidates[0].parametric_parameters != candidates[1].parametric_parameters
                || candidates[0].parametric_inputs != candidates[1].parametric_inputs,
            candidates,
            fingerprint: String::new(),
        };
        result.fingerprint = result.calculate_fingerprint()?;
        Ok(result)
    }
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Metrics {
    pub spaces: usize,
    pub area_m2: f64,
    pub volume_m3: f64,
    pub wall_overlap_sum_m2: f64,
    pub pair_overlap_sum_m2: f64,
}
/// Sum of authored prisms, overlaps counted separately; no floor/space/material claim.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MassMetrics {
    pub masses: usize,
    pub footprint_sum_m2: f64,
    pub envelope_volume_sum_m3: f64,
}
impl MassMetrics {
    pub fn from_elements(elements: &[Element]) -> Result<Self, String> {
        let mut result = Self {
            masses: 0,
            footprint_sum_m2: 0.,
            envelope_volume_sum_m3: 0.,
        };
        for e in elements {
            let Shape::ConceptMass { spec } = &e.shape else {
                return Err("Mass version contains another object type".into());
            };
            spec.validate()?;
            result.masses += 1;
            result.footprint_sum_m2 += spec.area();
            result.envelope_volume_sum_m3 += spec.volume();
        }
        Ok(result)
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Version {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parametric_inputs: Option<parametric::Inputs>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parametric_parameters: Option<parametric::Parameters>,
    pub option: u32,
    pub name: String,
    pub geometry_sha256: String,
    pub elements: Vec<Element>,
    pub metrics: Metrics,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mass_metrics: Option<MassMetrics>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Comparison {
    pub task_id: String,
    pub current_basis_sha256: String,
    pub basis_current: bool,
    pub different: bool,
    pub candidates: [Version; 2],
    pub fingerprint: String,
}
impl Comparison {
    fn calculate_fingerprint(&self) -> Result<String, String> {
        hash(&(
            &self.task_id,
            &self.current_basis_sha256,
            self.basis_current,
            self.different,
            &self.candidates,
        ))
    }
    fn validate(&self) -> Result<(), String> {
        if self.calculate_fingerprint()? != self.fingerprint {
            return Err("Exploration comparison fingerprint is invalid".into());
        }
        for c in &self.candidates {
            if let Some(parameters) = &c.parametric_parameters {
                parameters.validate()?;
                if c.mass_metrics.is_none() {
                    return Err("Saved parameters require a mass version".into());
                }
            }
            if c.elements.is_empty()
                || c.elements.len() > 32
                || (c.mass_metrics.is_none() && c.metrics.spaces != c.elements.len())
                || !crate::storage::valid_document_identity(&c.geometry_sha256)
            {
                return Err("Invalid saved exploration version".into());
            }
            let mut ids = BTreeSet::new();
            for e in &c.elements {
                if !ids.insert(e.id) || e.id == 0 || e.design_option != Some(c.option) {
                    return Err("Invalid saved exploration space".into());
                }
                match (&e.shape, &c.mass_metrics) {
                    (Shape::Zone { spec }, None) => spec.validate()?,
                    (Shape::ConceptMass { spec }, Some(_)) => spec.validate()?,
                    _ => return Err("Saved exploration version object domain differs".into()),
                }
            }
            if let Some(mass) = &c.mass_metrics {
                if mass != &MassMetrics::from_elements(&c.elements)?
                    || c.metrics != Metrics::default()
                {
                    return Err("Saved mass quantities differ from geometry".into());
                }
            }
            if [
                c.metrics.area_m2,
                c.metrics.volume_m3,
                c.metrics.wall_overlap_sum_m2,
                c.metrics.pair_overlap_sum_m2,
            ]
            .iter()
            .any(|v| !v.is_finite() || *v < 0.)
            {
                return Err("Invalid saved exploration metrics".into());
            }
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Decision {
    pub id: String,
    pub task_id: String,
    pub option: u32,
    pub reason: String,
    pub entry: Entry,
    pub operator_label: String,
    pub created_unix: u64,
    pub comparison: Comparison,
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Journal {
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub parametric_states: std::collections::BTreeMap<u32, parametric::State>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mass_spaces: Vec<crate::mass_spaces::Link>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub updates: Vec<update::Record>,
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub suspended_contexts: std::collections::BTreeMap<String, intent::TaskContext>,
    pub tasks: Vec<Task>,
    pub decisions: Vec<Decision>,
    pub current_task: Option<String>,
    pub current_choice: Option<String>,
}
impl Journal {
    pub fn needs_mass_format(&self) -> bool {
        !self.mass_spaces.is_empty()
            || self.tasks.iter().any(Task::is_mass)
            || self.decisions.iter().any(|d| {
                d.comparison
                    .candidates
                    .iter()
                    .any(|v| v.mass_metrics.is_some())
            })
    }

    pub fn needs_intent_format(&self) -> bool {
        !self.suspended_contexts.is_empty() || self.tasks.iter().any(|t| t.intent.is_some())
    }
    pub fn is_empty(&self) -> bool {
        self.parametric_states.is_empty()
            && self.mass_spaces.is_empty()
            && self.updates.is_empty()
            && self.tasks.is_empty()
            && self.suspended_contexts.is_empty()
            && self.decisions.is_empty()
            && self.current_task.is_none()
            && self.current_choice.is_none()
    }
    pub fn task(&self) -> Option<&Task> {
        self.current_task
            .as_ref()
            .and_then(|id| self.tasks.iter().find(|t| &t.id == id))
    }
    pub fn validate(&self) -> Result<(), String> {
        crate::mass_spaces::validate(&self.mass_spaces)?;
        parametric::validate_states(self)?;
        if self.updates.len() > 64
            || self.tasks.len() > 32
            || self.decisions.len() > 64
            || serde_json::to_vec(self).map_err(|e| e.to_string())?.len() > 4 * 1024 * 1024
        {
            return Err("Design journal exceeds supported limits".into());
        }
        let mut ids = BTreeSet::new();
        for t in &self.tasks {
            t.validate()?;
            if !ids.insert(&t.id) {
                return Err("Duplicate design journal identity".into());
            }
        }
        for d in &self.decisions {
            if !crate::storage::valid_document_identity(&d.id)
                || !ids.insert(&d.id)
                || !text(&d.reason, 4096, false)
                || !text(&d.operator_label, 256, true)
            {
                return Err("Invalid design choice".into());
            }
            let t = self
                .tasks
                .iter()
                .find(|t| t.id == d.task_id)
                .ok_or("Design choice task is missing")?;
            if !t.candidates.iter().any(|c| c.option == d.option)
                || d.comparison.task_id != d.task_id
            {
                return Err("Design choice candidate is missing".into());
            }
            d.comparison.validate()?;
            for v in &d.comparison.candidates {
                parametric::validate_saved_version(t, v)?;
            }
            if d.comparison
                .candidates
                .iter()
                .any(|v| v.mass_metrics.is_some() != t.is_mass())
            {
                return Err("Saved choice domain differs from task".into());
            }
        }
        for update in &self.updates {
            update.validate(self)?;
            if !ids.insert(&update.id) {
                return Err("Duplicate design journal identity".into());
            }
        }
        if self.current_task.as_ref().is_some_and(|id| {
            !self
                .tasks
                .iter()
                .any(|t| &t.id == id && (t.entry == Entry::ManualInterface || t.intent.is_some()))
        }) || self.current_choice.as_ref().is_some_and(|id| {
            !self.decisions.iter().any(|d| {
                &d.id == id
                    && Some(&d.task_id) == self.current_task.as_ref()
                    && d.entry == Entry::ManualInterface
            })
        }) {
            return Err("Invalid current design pointer".into());
        }
        for (task, context) in &self.suspended_contexts {
            let saved = self.tasks.iter().find(|t| &t.id == task);
            if self.current_task.as_ref() == Some(task)
                || saved.is_none()
                || context.choice.as_ref().is_some_and(|id| {
                    !self.decisions.iter().any(|d| {
                        &d.id == id && &d.task_id == task && d.entry == Entry::ManualInterface
                    })
                })
                || context.active_option.is_some_and(|id| {
                    saved.is_none_or(|t| {
                        id != t.baseline_option && !t.candidates.iter().any(|c| c.option == id)
                    })
                })
            {
                return Err("Invalid suspended design context".into());
            }
        }
        Ok(())
    }
    pub fn assert_append_only_from(&self, before: &Self) -> Result<(), String> {
        if before.updates.iter().any(|r| !self.updates.contains(r))
            || before.tasks.iter().any(|t| !self.tasks.contains(t))
            || before.decisions.iter().any(|d| !self.decisions.contains(d))
        {
            return Err("Design history is immutable; append a new record".into());
        }
        Ok(())
    }
    /// Used only by intentional model Undo/Redo. Validation rollback remains exact.
    pub fn restore_with_history(target: &Self, current: &Self) -> Self {
        let mut out = current.clone();
        for t in &target.tasks {
            if !out.tasks.iter().any(|r| r.id == t.id) {
                out.tasks.push(t.clone());
            }
        }
        for d in &target.decisions {
            if !out.decisions.iter().any(|r| r.id == d.id) {
                out.decisions.push(d.clone());
            }
        }
        for record in &target.updates {
            if !out.updates.iter().any(|r| r.id == record.id) {
                out.updates.push(record.clone());
            }
        }
        // Live model associations follow geometry Undo/Redo; immutable task and
        // decision evidence above remains append-only.
        out.parametric_states = target.parametric_states.clone();
        out.mass_spaces = target.mass_spaces.clone();
        out.current_task = target.current_task.clone();
        out.current_choice = target.current_choice.clone();
        out.suspended_contexts = target.suspended_contexts.clone();
        out
    }
    pub fn context(&self) -> serde_json::Value {
        let choice = self
            .current_choice
            .as_ref()
            .and_then(|id| self.decisions.iter().find(|d| &d.id == id));
        serde_json::json!({"task_update_count":self.updates.len(),"task_count":self.tasks.len(),"choice_count":self.decisions.len(),"task_history":self.tasks.iter().map(|t|serde_json::json!({"id":t.id,"name":t.name,"entry":t.entry,"current":self.current_task.as_ref()==Some(&t.id),"suspended_context":self.suspended_contexts.get(&t.id),"candidates":t.candidates.iter().map(|c|serde_json::json!({"option":c.option,"name":c.name})).collect::<Vec<_>>()})).collect::<Vec<_>>(),"current_task":self.task(),"current_choice":choice.map(|d|serde_json::json!({"id":d.id,"option":d.option,"reason":d.reason,"entry":d.entry,"operator_label":d.operator_label,"input_fingerprint":d.comparison.fingerprint})),"choice_freshness":"recompute comparison; stored choice is not a compliance verdict","history":"immutable task and readonly candidate-version records in the project"})
    }
}
#[derive(Clone, Debug, PartialEq)]
pub enum Edit {
    RecordUpdate {
        record: update::Record,
    },
    CreateIntent {
        task: Task,
    },
    ActivateTask {
        task_id: String,
    },
    Create {
        task: Task,
    },
    Choose {
        id: String,
        task_id: String,
        option: u32,
        reason: String,
        entry: Entry,
        operator_label: String,
        expected_comparison: String,
        /// Local manual review of this exact changed basis, never an AI grant.
        reviewed_basis: Option<String>,
        created_unix: u64,
    },
    Lock {
        task_id: String,
        option: u32,
        locked: bool,
    },
}
pub(crate) fn apply(
    p: &Project,
    edit: &Edit,
) -> Result<(Journal, Option<crate::editing::EditState>), String> {
    let mut out = p.design_process.clone();
    let mut locks = None;
    match edit {
        Edit::RecordUpdate { record } => update::apply(p, &mut out, record)?,
        Edit::CreateIntent { task } => {
            intent::apply_create(p, &mut out, task)?;
        }
        Edit::ActivateTask { task_id } => {
            intent::activate(p, &mut out, task_id)?;
        }
        Edit::Create { task } => {
            if task.entry != Entry::ManualInterface || task.intent.is_some() {
                return Err("Exploration creation requires explicit manual adoption".into());
            }
            task.validate()?;
            if out.current_task.is_some() {
                return Err("Continue the current exploration before starting another".into());
            }
            if out.tasks.iter().any(|t| t.id == task.id) {
                return Err(
                    "Exploration request already recorded; inspect its original state".into(),
                );
            }
            if task
                .candidates
                .iter()
                .any(|c| p.design_options.name(c.option) != Some(c.name.as_str()))
                || p.design_options.name(task.baseline_option).is_none()
            {
                return Err("Exploration options are missing".into());
            }
            for (i, source_id) in task.source_element_ids().iter().enumerate() {
                let source = p
                    .elements
                    .iter()
                    .find(|e| e.id == *source_id && e.design_option == Some(task.baseline_option))
                    .ok_or("Exploration source is missing")?;
                for c in &task.candidates {
                    let e = p
                        .elements
                        .iter()
                        .find(|e| e.id == c.element_ids()[i])
                        .ok_or("Exploration copy is missing")?;
                    let mut expected = source.clone();
                    expected.id = e.id;
                    expected.design_option = Some(c.option);
                    if expected != *e
                        || !task.accepts_shape(&e.shape)
                        || e.story != task.story
                        || e.worksheet.is_some()
                    {
                        return Err("Exploration copy differs from its declared source".into());
                    }
                }
            }
            if task.common_hash(p)? != task.basis_sha256 {
                return Err("Exploration basis changed".into());
            }
            out.tasks.push(task.clone());
            out.current_task = Some(task.id.clone());
            out.current_choice = None;
        }
        Edit::Choose {
            id,
            task_id,
            option,
            reason,
            entry,
            operator_label,
            expected_comparison,
            reviewed_basis,
            created_unix,
        } => {
            if let Some(old) = out.decisions.iter().find(|d| d.id == *id) {
                if old.task_id == *task_id
                    && old.option == *option
                    && old.reason == *reason
                    && old.entry == *entry
                    && old.operator_label == *operator_label
                    && old.comparison.fingerprint == *expected_comparison
                {
                    return Ok((out, None));
                }
                return Err("Design choice request identity conflicts".into());
            }
            let task = out
                .task()
                .filter(|t| t.id == *task_id)
                .ok_or("Current exploration changed")?;
            let comparison = task.compare(p)?;
            if reviewed_basis.as_ref().is_some_and(|basis| {
                *entry != Entry::ManualInterface || basis != &comparison.current_basis_sha256
            }) {
                return Err(
                    "Changed-basis review does not match the current local comparison".into(),
                );
            }
            if !comparison.basis_current && reviewed_basis.is_none() {
                return Err(
                    "Exploration basis is stale; review the changed common model or conditions"
                        .into(),
                );
            }
            if !comparison.different {
                return Err("Edit a spatially different candidate before choosing".into());
            }
            if comparison.fingerprint != *expected_comparison {
                return Err("Exploration comparison changed; refresh before choosing".into());
            }
            if !task.candidates.iter().any(|c| c.option == *option) {
                return Err("Choice is outside this exploration".into());
            }
            out.decisions.push(Decision {
                id: id.clone(),
                task_id: task_id.clone(),
                option: *option,
                reason: reason.clone(),
                entry: *entry,
                operator_label: operator_label.clone(),
                created_unix: *created_unix,
                comparison,
            });
            if *entry == Entry::ManualInterface {
                out.current_choice = Some(id.clone());
            }
        }
        Edit::Lock {
            task_id,
            option,
            locked,
        } => {
            let task = out
                .task()
                .filter(|t| t.id == *task_id)
                .ok_or("Current exploration changed")?;
            if !task.candidates.iter().any(|c| c.option == *option) {
                return Err("Lock is outside this exploration".into());
            }
            let mut state = p.edit_state.clone();
            for e in p
                .elements
                .iter()
                .filter(|e| e.design_option == Some(*option))
            {
                if *locked {
                    state.locked.insert(e.id);
                } else {
                    state.locked.remove(&e.id);
                }
            }
            locks = Some(state);
        }
    }
    out.validate()?;
    Ok((out, locks))
}
