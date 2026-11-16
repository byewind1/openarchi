//! Bounded discussion snapshots over existing objects, never a second live model.
use super::{EvidenceRegister, Link, LinkStatus, Quote, Target};
use crate::model::{Element, Project, Shape};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Scope {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parameter_input_sha256: Option<String>,
    pub task: Option<String>,
    pub option: Option<u32>,
}
impl Scope {
    pub fn capture(p: &Project) -> Self {
        Self {
            parameter_input_sha256: p
                .design_process
                .task()
                .filter(|t| t.intent.as_ref().is_some_and(|v| v.parametric.is_some()))
                .and_then(|t| t.compare(p).ok())
                .map(|c| c.fingerprint),
            task: p.design_process.current_task.clone(),
            option: p.plan_options.active_design_option,
        }
    }
    pub fn input_status(&self, p: &Project) -> Option<&'static str> {
        let task = self
            .task
            .as_ref()
            .and_then(|id| p.design_process.tasks.iter().find(|t| &t.id == id));
        match &self.parameter_input_sha256 {
            Some(expected) => Some(
                if task
                    .and_then(|t| t.compare(p).ok())
                    .is_some_and(|c| c.fingerprint == *expected)
                {
                    "Discussion design inputs unchanged"
                } else {
                    "Discussion design inputs changed; old discussion retained"
                },
            ),
            None if task
                .is_some_and(|t| t.intent.as_ref().is_some_and(|v| v.parametric.is_some())) =>
            {
                Some("Discussion parameter version was not recorded")
            }
            None => None,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Object {
    pub link: Link,
    /// Immutable historical source data; cannot be applied as live geometry.
    pub data: Value,
}
impl Object {
    pub fn summary(&self) -> Result<Value, String> {
        match &self.link.target {
            Target::Element { id } => {
                let e: Element =
                    serde_json::from_value(self.data.clone()).map_err(|e| e.to_string())?;
                let (height, volume) =
                    match &e.shape {
                        Shape::ConceptMass { spec } => {
                            spec.validate()?;
                            (spec.height, spec.volume())
                        }
                        Shape::Zone { spec } => {
                            spec.validate()?;
                            (spec.height, spec.area() * spec.height)
                        }
                        _ => return Err(
                            "Object discussion supports concept masses, Zones and requirement rows"
                                .into(),
                        ),
                    };
                if e.id != *id {
                    return Err("Discussion object identity mismatch".into());
                }
                Ok(
                    json!({"target":self.link.target,"name":e.name,"kind":e.kind(),"option":e.design_option,"story":e.story,"footprint_m2":e.area(),"height_m":height,"envelope_m3":volume,"quantity_basis":"authored geometry; not statutory area or capacity"}),
                )
            }
            Target::ProgramRow { .. } => {
                let values: crate::program::Values =
                    serde_json::from_value(self.data["row"]["values"].clone())
                        .map_err(|e| e.to_string())?;
                values.validate()?;
                Ok(
                    json!({"target":self.link.target,"name":values.label,"kind":"Requirement row","values":values,"source_sha256":self.data["source"],"quantity_basis":"declared requirement; authority and approval unverified"}),
                )
            }
        }
    }
}
/// A declared external contribution, not authenticated model identity or write authority.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AgentReply {
    pub id: String,
    pub agent_label: String,
    pub text: String,
    pub claims: Vec<crate::design_process::intent::Claim>,
}
impl AgentReply {
    pub fn validate(&self) -> Result<(), String> {
        let bounded = |s: &str, n| {
            !s.trim().is_empty()
                && s.len() <= n
                && !s.chars().any(|c| c.is_control() && c != '\n' && c != '\t')
        };
        if !crate::storage::valid_document_identity(&self.id)
            || !bounded(&self.agent_label, 256)
            || !bounded(&self.text, 16 * 1024)
            || !(1..=32).contains(&self.claims.len())
            || self.claims.iter().any(|c| !bounded(&c.text, 4096))
        {
            return Err("Invalid bounded Agent contribution".into());
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Exchange {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assignment: Option<super::topic_tasks::Assignment>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reply: Option<AgentReply>,
    pub question: String,
    pub scope: Scope,
    pub objects: Vec<Object>,
    pub sources: Vec<Quote>,
}
impl Exchange {
    /// A saved answer is historical. Report the existing association's current
    /// status without restoring a task, replacing inputs or interpreting prose.
    pub fn result_status(&self, p: &Project) -> Option<&'static str> {
        let Some(super::topic_tasks::Assignment {
            mode: super::topic_tasks::Mode::CreateLinkedSpace { mass, zone, .. },
            ..
        }) = &self.assignment
        else {
            return None;
        };
        if !p.elements.iter().any(|e| e.id == *zone) {
            return Some("Linked concept space is missing");
        }
        Some(
            p.design_process
                .mass_spaces
                .iter()
                .find(|l| l.mass == *mass && l.zone == *zone)
                .map_or("Mass-space association is missing", |l| {
                    crate::mass_spaces::status(p, l)
                }),
        )
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Topic {
    pub id: String,
    pub exchanges: Vec<Exchange>,
}
impl Topic {
    /// Local navigation/retrieval only. Recorded IDs and explicit associations are
    /// evidence of relevance, never current inputs, approval or write authority.
    pub fn related_to(&self, p: &Project, targets: &[Target]) -> bool {
        let mut bound = targets
            .iter()
            .cloned()
            .collect::<std::collections::BTreeSet<_>>();
        for target in targets {
            if let Target::Element { id } = target {
                for link in &p.design_process.mass_spaces {
                    if link.mass == *id || link.zone == *id {
                        bound.insert(Target::Element { id: link.mass });
                        bound.insert(Target::Element { id: link.zone });
                    }
                }
            }
        }
        self.exchanges.iter().any(|e| {
            if e.objects.iter().any(|o| bound.contains(&o.link.target)) {
                return true;
            }
            use super::topic_tasks::Mode;
            match e.assignment.as_ref().map(|a| &a.mode) {
                Some(Mode::CreateLinkedSpace { mass, zone, .. }) => {
                    bound.contains(&Target::Element { id: *mass })
                        || bound.contains(&Target::Element { id: *zone })
                }
                Some(Mode::CreateTwoWing { task_id, .. } | Mode::TwoWing { task_id, .. }) => p
                    .design_process
                    .tasks
                    .iter()
                    .find(|t| &t.id == task_id)
                    .is_some_and(|task| {
                        task.candidates
                            .iter()
                            .filter(|c| match e.assignment.as_ref().map(|a| &a.mode) {
                                Some(Mode::TwoWing { option, .. }) => c.option == *option,
                                _ => true,
                            })
                            .any(|c| {
                                c.initial_mass_ids
                                    .iter()
                                    .any(|id| bound.contains(&Target::Element { id: *id }))
                            })
                    }),
                _ => false,
            }
        })
    }
    pub fn validate(&self) -> Result<(), String> {
        if !crate::storage::valid_document_identity(&self.id)
            || !(1..=32).contains(&self.exchanges.len())
        {
            return Err("Invalid discussion identity or history limit".into());
        }
        if serde_json::to_vec(self).map_err(|e| e.to_string())?.len() > 700 * 1024 {
            return Err("Discussion exceeds supported readback limit".into());
        }
        let mut replies = std::collections::BTreeSet::new();
        for e in &self.exchanges {
            if let Some(a) = &e.assignment {
                let (phase, agent) = a.state()?;
                if let Some(reply) = &e.reply {
                    if agent != Some(reply.agent_label.as_str()) {
                        return Err(
                            "Object task answer belongs to another declared assignee".into()
                        );
                    }
                }
                if phase == super::topic_tasks::Phase::Completed && (e.reply.is_none() || !a.events.iter().any(|event|matches!(event,super::topic_tasks::Event::Complete{operation_ids} if e.reply.as_ref().is_some_and(|r|operation_ids.contains(&r.id))))) {
                    return Err("Completed object task has no bound answer operation".into());
                }
            }
            if let Some(reply) = &e.reply {
                reply.validate()?;
                if !replies.insert(&reply.id) {
                    return Err("Repeated Agent contribution identity".into());
                }
            }
            if e.question.trim().is_empty()
                || e.question.len() > 4096
                || !(1..=16).contains(&e.objects.len())
                || e.sources.len() > 32
                || e.scope
                    .task
                    .as_ref()
                    .is_some_and(|id| !crate::storage::valid_document_identity(id))
                || e.scope.parameter_input_sha256.as_ref().is_some_and(|s| {
                    !crate::storage::valid_document_identity(s) || e.scope.task.is_none()
                })
                || e.scope.option == Some(0)
            {
                return Err("Invalid discussion question or object context".into());
            }
            let mut seen = std::collections::BTreeSet::new();
            for o in &e.objects {
                o.link.target.validate()?;
                if !seen.insert(&o.link.target)
                    || crate::automation_journal::canonical_hash(&o.data)? != o.link.baseline_sha256
                {
                    return Err("Invalid discussion object snapshot".into());
                }
                o.summary()?;
            }
            for s in &e.sources {
                s.validate()?;
            }
        }
        Ok(())
    }
    pub fn view(&self, p: &Project) -> Value {
        json!({"topic":self,"topic_sha256":crate::automation_journal::canonical_hash(self).ok(),"exchanges":self.exchanges.iter().map(|e|json!({"question":e.question,"design_input_status":e.scope.input_status(p),"result_status":e.result_status(p),"task_status":e.assignment.as_ref().and_then(|a|a.state().ok().map(|(phase,agent)|json!({"request_id":a.id,"phase":phase.label(),"declared_agent":agent,"mode":a.mode}))),"objects":e.objects.iter().map(|o|json!({"summary":o.summary().ok(),"status":o.link.status(p)})).collect::<Vec<_>>()})).collect::<Vec<_>>(),"execution":if self.exchanges.iter().any(|e|e.reply.is_some()) {"contains declared Agent contributions; execution identity and correctness not verified by this read"}else{"local object readback; not AI advice or approval"},"allowed_modifications":"discussion record only; no geometry authorization","external_agent_dispatch":"no_automatic_outbound_call","object_task_handoff":"explicit connected-client pull","source_verification":"recorded quotes; exact original bytes not read by this query"})
    }
}
fn value(p: &Project, target: &Target) -> Result<Value, String> {
    match target {
        Target::Element { id } => p
            .elements
            .iter()
            .find(|e| e.id == *id)
            .ok_or("Discussion object is missing")
            .and_then(|e| serde_json::to_value(e).map_err(|_| "Invalid discussion object")),
        Target::ProgramRow { sheet_id, key } => {
            let s = p
                .program
                .sheets
                .iter()
                .find(|s| s.id == *sheet_id)
                .ok_or("Discussion requirement sheet is missing")?;
            let row = s
                .rows
                .iter()
                .find(|r| r.key == *key)
                .ok_or("Discussion requirement row is missing")?;
            Ok(json!({"source":s.source_sha256,"mapping":s.mapping,"row":row}))
        }
    }
    .map_err(str::to_string)
}
pub fn capture(p: &Project, targets: &[Target], question: String) -> Result<Exchange, String> {
    let objects = targets
        .iter()
        .map(|target| {
            let data = value(p, target)?;
            let object = Object {
                link: Link {
                    target: target.clone(),
                    baseline_sha256: crate::automation_journal::canonical_hash(&data)?,
                },
                data,
            };
            object.summary()?;
            Ok(object)
        })
        .collect::<Result<Vec<_>, String>>()?;
    let mut sources = vec![];
    for note in &p.evidence.notes {
        if note.links.iter().any(|l| targets.contains(&l.target)) && !sources.contains(&note.quote)
        {
            sources.push(note.quote.clone());
        }
    }
    let exchange = Exchange {
        assignment: None,
        reply: None,
        question,
        scope: Scope::capture(p),
        objects,
        sources,
    };
    Topic {
        id: "a".repeat(64),
        exchanges: vec![exchange.clone()],
    }
    .validate()?;
    Ok(exchange)
}
pub fn record(
    p: &Project,
    register: &mut EvidenceRegister,
    id: &str,
    exchange: &Exchange,
    previous: &Option<Topic>,
) -> Result<(), String> {
    let old = register.topics.iter().find(|t| t.id == id);
    if old != previous.as_ref() {
        return Err("Discussion changed; reload its history".into());
    }
    if Scope::capture(p) != exchange.scope
        || exchange
            .objects
            .iter()
            .any(|o| o.link.status(p) != LinkStatus::Unchanged)
    {
        return Err("Discussion inputs changed; explicitly capture a new contribution".into());
    }
    // Recompute all facts and quote associations. Caller text cannot forge a readback.
    let targets = exchange
        .objects
        .iter()
        .map(|o| o.link.target.clone())
        .collect::<Vec<_>>();
    if &capture(p, &targets, exchange.question.clone())? != exchange {
        return Err("Discussion snapshot does not match current objects".into());
    }
    let mut next = old.cloned().unwrap_or(Topic {
        id: id.into(),
        exchanges: vec![],
    });
    next.exchanges.push(exchange.clone());
    next.validate()?;
    if let Some(i) = register.topics.iter().position(|t| t.id == id) {
        register.topics[i] = next;
    } else {
        register.topics.push(next);
    }
    Ok(())
}

/// Append one answer to exactly one existing frozen contribution. No geometry effect.
pub fn record_reply(
    p: &Project,
    register: &mut EvidenceRegister,
    id: &str,
    exchange_index: usize,
    expected_topic_sha256: &str,
    reply: &AgentReply,
) -> Result<(), String> {
    reply.validate()?;
    let topic = register
        .topics
        .iter_mut()
        .find(|t| t.id == id)
        .ok_or("Discussion is missing")?;
    if crate::automation_journal::canonical_hash(topic)? != expected_topic_sha256 {
        return Err("Discussion changed; reload its history".into());
    }
    let exchange = topic
        .exchanges
        .get_mut(exchange_index)
        .ok_or("Discussion contribution is missing")?;
    if exchange.reply.is_some() {
        return Err("Agent contribution already recorded; inspect the original result".into());
    }
    let targets = exchange
        .objects
        .iter()
        .map(|o| o.link.target.clone())
        .collect::<Vec<_>>();
    let mut frozen = exchange.clone();
    frozen.assignment = None;
    if let Some(a) = &exchange.assignment {
        let (phase, agent) = a.state()?;
        if phase != super::topic_tasks::Phase::Running || agent != Some(reply.agent_label.as_str())
        {
            return Err("Object task is not running for this declared assignee".into());
        }
        // A scoped task may itself have changed geometry. The answer retains its
        // original input snapshot; only the shared host can authorize its writes.
    } else if capture(p, &targets, exchange.question.clone())? != frozen {
        return Err("Discussion inputs changed; explicitly capture a new contribution".into());
    }
    exchange.reply = Some(reply.clone());
    topic.validate()
}
