//! One project-owned object task. Transport/authority stay in ApplicationHost.
use super::{
    EvidenceRegister,
    topics::{Exchange, Topic},
};
use crate::model::Project;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Mode {
    Research,
    TwoWing {
        task_id: String,
        option: u32,
    },
    CreateTwoWing {
        task_id: String,
        story: u32,
        layer: u32,
        site_sha256: String,
    },
    CreateLinkedSpace {
        mass: u64,
        source_sha256: String,
        story: u32,
        option: u32,
        zone: u64,
        operation_id: String,
    },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum Event {
    Claim { agent_label: String },
    Progress { message: String },
    Complete { operation_ids: Vec<String> },
    Cancel,
    Resume,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Queued,
    Running,
    Completed,
    Cancelled,
}
impl Phase {
    pub fn label(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Cancelled => "cancelled",
        }
    }
    pub fn active(self) -> bool {
        matches!(self, Self::Queued | Self::Running)
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Assignment {
    pub id: String,
    pub mode: Mode,
    pub events: Vec<Event>,
}
fn text(s: &str, n: usize) -> bool {
    !s.trim().is_empty()
        && s.len() <= n
        && !s.chars().any(|c| c.is_control() && c != '\n' && c != '\t')
}
impl Mode {
    /// Explicit consent for one existing candidate member, never inferred from later selection.
    pub fn linked_space(p: &Project, exchange: &Exchange) -> Result<Self, String> {
        let ids = exchange
            .objects
            .iter()
            .filter_map(|o| match o.link.target {
                super::Target::Element { id }
                    if p.elements.iter().any(|e| {
                        e.id == id && matches!(e.shape, crate::model::Shape::ConceptMass { .. })
                    }) =>
                {
                    Some(id)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        let [mass] = ids.as_slice() else {
            return Err("Choose one source mass for a concept space".into());
        };
        let e = p.elements.iter().find(|e| e.id == *mass).unwrap();
        let mode = Self::CreateLinkedSpace {
            mass: *mass,
            source_sha256: crate::mass_spaces::element_stamp(p, e)?,
            story: e.story,
            option: e.design_option.ok_or("Activate the source candidate")?,
            zone: p.next_id,
            operation_id: crate::storage::new_document_identity(),
        };
        mode.check_current(p)?;
        Ok(mode)
    }
    pub fn validate(&self) -> Result<(), String> {
        if let Self::CreateLinkedSpace {
            mass,
            source_sha256,
            option,
            zone,
            operation_id,
            ..
        } = self
        {
            if *mass == 0
                || *zone == 0
                || mass == zone
                || *option == 0
                || !crate::storage::valid_document_identity(source_sha256)
                || !crate::storage::valid_document_identity(operation_id)
            {
                return Err("Invalid linked space task scope".into());
            }
        }
        if let Self::TwoWing { task_id, option } = self {
            if !crate::storage::valid_document_identity(task_id) || *option == 0 {
                return Err("Invalid object task model scope".into());
            }
        }
        if let Self::CreateTwoWing {
            task_id,
            site_sha256,
            ..
        } = self
        {
            if !crate::storage::valid_document_identity(task_id)
                || !crate::storage::valid_document_identity(site_sha256)
            {
                return Err("Invalid object task creation scope".into());
            }
        }
        Ok(())
    }
    /// The original discussion stays frozen when this task appends its one study.
    /// This is context matching, never proof of operation ownership or authority.
    pub fn matches_context(&self, p: &Project, exchange: &Exchange) -> bool {
        if let Self::CreateTwoWing { task_id, .. } = self
            && let Some(task) = p.design_process.tasks.iter().find(|t| t.id == *task_id)
        {
            return p.design_process.current_task.as_ref() == Some(task_id)
                && p.plan_options.active_design_option == Some(task.candidates[0].option)
                && task.intent.as_ref().is_some_and(|i| {
                    i.parent_task == exchange.scope.task
                        && i.parent_active_option == exchange.scope.option
                });
        }
        exchange.scope.task == p.design_process.current_task
            && exchange.scope.option == p.plan_options.active_design_option
    }
    pub fn check_current(&self, p: &Project) -> Result<Vec<u64>, String> {
        self.validate()?;
        match self {
            Self::Research => Ok(vec![]),
            Self::CreateLinkedSpace {
                mass,
                source_sha256,
                story,
                option,
                zone,
                ..
            } => {
                let task = p
                    .design_process
                    .task()
                    .ok_or("Object task model is missing")?;
                let source = p
                    .elements
                    .iter()
                    .find(|e| e.id == *mass)
                    .ok_or("Linked source is missing")?;
                if p.plan_options.active_worksheet.is_some()
                    || p.plan_options.active_design_option != Some(*option)
                    || task.story != *story
                    || !task
                        .candidates
                        .iter()
                        .any(|c| c.option == *option && c.initial_mass_ids.contains(mass))
                    || source.story != *story
                    || source.design_option != Some(*option)
                    || source.worksheet.is_some()
                    || !matches!(source.shape, crate::model::Shape::ConceptMass { .. })
                    || crate::editor::Editor::element_is_locked(p, *mass)
                    || crate::mass_spaces::element_stamp(p, source)? != *source_sha256
                {
                    return Err("Linked space task source changed or is protected".into());
                }
                let links = p
                    .design_process
                    .mass_spaces
                    .iter()
                    .filter(|l| l.mass == *mass)
                    .collect::<Vec<_>>();
                if let Some(result) = p.elements.iter().find(|e| e.id == *zone) {
                    if links.len() != 1
                        || links[0].zone != *zone
                        || !matches!(result.shape, crate::model::Shape::Zone { .. })
                        || crate::editor::Editor::element_is_locked(p, *zone)
                        || crate::mass_spaces::status(p, links[0]) != "Mass-space link is current"
                    {
                        return Err("Linked space task result changed or is protected".into());
                    }
                    Ok(vec![*mass, *zone])
                } else if p.next_id == *zone && links.is_empty() {
                    Ok(vec![*mass])
                } else {
                    Err("Linked space task result identity is no longer available".into())
                }
            }
            Self::CreateTwoWing {
                task_id,
                story,
                layer,
                site_sha256,
            } => {
                if p.plan_options.active_worksheet.is_some()
                    || !p.stories.iter().any(|s| s.id == *story)
                    || p.layers.layer(*layer).is_none()
                    || p.layers.is_locked(*layer)
                    || !p.layers.is_visible(*layer)
                    || crate::automation_journal::canonical_hash(&p.site)? != *site_sha256
                {
                    return Err("Object task creation inputs changed".into());
                }
                let Some(task) = p.design_process.tasks.iter().find(|t| t.id == *task_id) else {
                    return Ok(vec![]);
                };
                if task.story != *story || task.entry != crate::design_process::Entry::AgentProposal
                {
                    return Err("Object task creation result differs".into());
                }
                let rule = task
                    .intent
                    .as_ref()
                    .and_then(|i| i.parametric.as_ref())
                    .ok_or("Object task creation rule is missing")?;
                rule.check_inputs(p)?;
                if !task.compare(p)?.basis_current {
                    return Err("Object task creation inputs changed".into());
                }
                for c in &task.candidates {
                    if p.design_process.parametric_states.get(&c.option)
                        != Some(&crate::design_process::parametric::State::Initial)
                    {
                        return Err("Object task creation result was edited".into());
                    }
                    crate::design_process::parametric::current_parameters(p, task_id, c.option)?;
                }
                let ids = task
                    .source_mass_ids
                    .iter()
                    .chain(task.candidates.iter().flat_map(|c| &c.initial_mass_ids))
                    .copied()
                    .collect::<Vec<_>>();
                if ids.len() != 6
                    || ids.iter().any(|id| {
                        crate::editor::Editor::element_is_locked(p, *id)
                            || p.elements
                                .iter()
                                .find(|e| e.id == *id)
                                .is_none_or(|e| e.layer.0 != *layer)
                    })
                {
                    return Err("Object task creation members are protected".into());
                }
                Ok(ids)
            }
            Self::TwoWing { task_id, option } => {
                if p.design_process.current_task.as_ref() != Some(task_id)
                    || p.plan_options.active_design_option != Some(*option)
                    || p.plan_options.active_worksheet.is_some()
                {
                    return Err("Object task scope is no longer active".into());
                }
                let t = p
                    .design_process
                    .task()
                    .ok_or("Object task model is missing")?;
                let _rule = t
                    .intent
                    .as_ref()
                    .and_then(|v| v.parametric.as_ref())
                    .ok_or("Object task requires an existing two-wing rule")?;
                // A newly confirmed native task may review changed declared inputs.
                // Missing source identities and manual geometry still reject it;
                // existing task authority is separately and stickily fenced by Host.
                crate::design_process::parametric::capture_inputs(p, task_id, *option)?;
                crate::design_process::parametric::current_parameters(p, task_id, *option)?;
                let ids = t
                    .candidates
                    .iter()
                    .find(|c| c.option == *option)
                    .ok_or("Object task candidate is missing")?
                    .initial_mass_ids
                    .clone();
                if ids.len() != 2
                    || ids
                        .iter()
                        .any(|id| crate::editor::Editor::element_is_locked(p, *id))
                {
                    return Err("Object task members are protected".into());
                }
                Ok(ids)
            }
        }
    }
}
impl Assignment {
    pub fn state(&self) -> Result<(Phase, Option<&str>), String> {
        if !crate::storage::valid_document_identity(&self.id) || self.events.len() > 64 {
            return Err("Invalid object task identity or event limit".into());
        }
        self.mode.validate()?;
        let (mut phase, mut agent) = (Phase::Queued, None);
        for event in &self.events {
            match event {
                Event::Claim { agent_label }
                    if phase == Phase::Queued && text(agent_label, 256) =>
                {
                    phase = Phase::Running;
                    agent = Some(agent_label.as_str());
                }
                Event::Progress { message } if phase == Phase::Running && text(message, 1024) => {}
                Event::Complete { operation_ids }
                    if phase == Phase::Running
                        && !operation_ids.is_empty()
                        && operation_ids.len() <= 16
                        && operation_ids
                            .iter()
                            .all(|i| crate::storage::valid_document_identity(i))
                        && operation_ids
                            .iter()
                            .collect::<std::collections::BTreeSet<_>>()
                            .len()
                            == operation_ids.len() =>
                {
                    phase = Phase::Completed
                }
                Event::Cancel if phase.active() => phase = Phase::Cancelled,
                Event::Resume if phase.active() => {
                    // Resume keeps the original contribution/assignee; no new task identity.
                }
                _ => return Err("Invalid object task transition".into()),
            }
        }
        Ok((phase, agent))
    }
}
pub fn find<'a>(
    p: &'a Project,
    id: &str,
) -> Option<(&'a Topic, usize, &'a Exchange, &'a Assignment)> {
    p.evidence.topics.iter().find_map(|t| {
        t.exchanges.iter().enumerate().find_map(|(i, e)| {
            e.assignment
                .as_ref()
                .filter(|a| a.id == id)
                .map(|a| (t, i, e, a))
        })
    })
}
pub fn submit(
    p: &Project,
    register: &mut EvidenceRegister,
    id: &str,
    exchange: &Exchange,
    previous: &Option<Topic>,
    request_id: &str,
    mode: &Mode,
) -> Result<(), String> {
    if register
        .topics
        .iter()
        .flat_map(|t| &t.exchanges)
        .filter_map(|e| e.assignment.as_ref())
        .any(|a| a.state().is_ok_and(|(s, _)| s.active()))
    {
        return Err("Finish or cancel the existing object task first".into());
    }
    mode.check_current(p)?;
    if let Mode::CreateLinkedSpace {
        mass,
        zone,
        operation_id,
        ..
    } = mode
    {
        if operation_id == request_id
            || p.elements.iter().any(|e| e.id == *zone)
            || !exchange.objects.iter().any(|o| {
                o.link.target == super::Target::Element { id: *mass }
                    && o.link.status(p) == super::LinkStatus::Unchanged
            })
        {
            return Err(
                "Linked space task requires an unchanged captured source and unused result".into(),
            );
        }
    }
    if let Mode::CreateTwoWing { task_id, .. } = mode
        && p.design_process.tasks.iter().any(|t| t.id == *task_id)
    {
        return Err("Object task creation identity is already used".into());
    }
    super::topics::record(p, register, id, exchange, previous)?;
    let topic = register.topics.iter_mut().find(|t| t.id == id).unwrap();
    topic.exchanges.last_mut().unwrap().assignment = Some(Assignment {
        id: request_id.into(),
        mode: mode.clone(),
        events: vec![],
    });
    topic.validate()
}
pub fn event(
    p: &Project,
    register: &mut EvidenceRegister,
    id: &str,
    index: usize,
    expected: &str,
    request_id: &str,
    event: &Event,
) -> Result<(), String> {
    let topic = register
        .topics
        .iter_mut()
        .find(|t| t.id == id)
        .ok_or("Discussion is missing")?;
    if crate::automation_journal::canonical_hash(topic)? != expected {
        return Err("Discussion changed; reload its history".into());
    }
    let e = topic
        .exchanges
        .get_mut(index)
        .ok_or("Discussion contribution is missing")?;
    let a = e
        .assignment
        .as_mut()
        .filter(|a| a.id == request_id)
        .ok_or("Object task identity differs")?;
    if matches!(event, Event::Resume) {
        a.mode.check_current(p)?;
    }
    if let Event::Complete { operation_ids } = event {
        if e.reply
            .as_ref()
            .is_none_or(|r| !operation_ids.contains(&r.id))
        {
            return Err("Object task completion requires its saved answer operation".into());
        }
    }
    a.events.push(event.clone());
    topic.validate()
}
