//! Authored conditions and cited originals. No executable compliance verdict.
use crate::{evidence::Link, model::Project};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
mod registers;
pub use registers::*;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProjectContext {
    pub jurisdiction: String,
    pub uses: String,
    /// Empty means unknown; no today's-date or occupancy inference.
    pub assessment_date: String,
    pub stage: String,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Topic {
    Planning,
    Boundary,
    Use,
    AreaMassing,
    Setbacks,
    Sunlight,
    Fire,
    Accessibility,
    Parking,
    TerrainNeighbors,
    Energy,
    Environment,
    ClientBrief,
}
impl Topic {
    pub const ALL: [Self; 13] = [
        Self::Planning,
        Self::Boundary,
        Self::Use,
        Self::AreaMassing,
        Self::Setbacks,
        Self::Sunlight,
        Self::Fire,
        Self::Accessibility,
        Self::Parking,
        Self::TerrainNeighbors,
        Self::Energy,
        Self::Environment,
        Self::ClientBrief,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Planning => "Planning conditions",
            Self::Boundary => "Legal boundary",
            Self::Use => "Use and occupancy",
            Self::AreaMassing => "Area and massing",
            Self::Setbacks => "Setbacks",
            Self::Sunlight => "Sunlight",
            Self::Fire => "Fire safety",
            Self::Accessibility => "Accessibility",
            Self::Parking => "Parking",
            Self::TerrainNeighbors => "Terrain and neighbors",
            Self::Energy => "Energy",
            Self::Environment => "Environment",
            Self::ClientBrief => "Client brief",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Origin {
    Unverified,
    OfficialPublication,
    ClientDocument,
    Survey,
}
impl Origin {
    pub fn label(self) -> &'static str {
        match self {
            Self::Unverified => "Unverified origin",
            Self::OfficialPublication => "Declared official publication",
            Self::ClientDocument => "Declared client document",
            Self::Survey => "Declared survey",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RuleSource {
    pub id: String,
    pub title: String,
    pub code: String,
    pub edition: String,
    pub publisher: String,
    pub jurisdiction: String,
    pub published_on: String,
    pub effective_on: String,
    pub original_sha256: String,
    pub origin_url: String,
    pub origin: Origin,
    pub verification_note: String,
    /// A recorded relation, never automatic clause migration or legal priority.
    pub replaces: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub same_edition_as: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Citation {
    pub source_id: String,
    pub locator: String,
    pub text: String,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Applicability {
    Unknown,
    Applicable,
    NotApplicable,
    Conflict,
}
impl Applicability {
    pub fn label(self) -> &'static str {
        match self {
            Self::Unknown => "Applicability unknown",
            Self::Applicable => "Declared applicable",
            Self::NotApplicable => "Declared not applicable",
            Self::Conflict => "Applicability conflict",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Adoption {
    Pending,
    Adopted,
    NotAdopted,
}
impl Adoption {
    pub fn label(self) -> &'static str {
        match self {
            Self::Pending => "Adoption pending",
            Self::Adopted => "Declared adopted",
            Self::NotAdopted => "Declared not adopted",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Decision {
    pub applicability: Applicability,
    pub adoption: Adoption,
    pub rationale: String,
    pub missing: String,
    /// Hash of declared project context, exact source revision and known successors.
    pub basis_sha256: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Requirement {
    pub id: String,
    pub title: String,
    pub topic: Topic,
    pub citation: Citation,
    pub interpretation: String,
    pub decision: Decision,
    pub links: Vec<Link>,
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RuleBook {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub checks: Vec<crate::constraints::Check>,
    pub context: ProjectContext,
    pub sources: Vec<RuleSource>,
    pub requirements: Vec<Requirement>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lifecycle: Vec<LifecycleRecord>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conditions: Vec<ConditionNeed>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum RuleEdit {
    PutLifecycle {
        expected: Option<LifecycleRecord>,
        record: LifecycleRecord,
    },
    RemoveLifecycle {
        expected: LifecycleRecord,
    },
    PutCondition {
        expected: Option<ConditionNeed>,
        condition: ConditionNeed,
    },
    RemoveCondition {
        expected: ConditionNeed,
    },
    SetContext {
        expected: ProjectContext,
        context: ProjectContext,
    },
    AddSource {
        source: RuleSource,
    },
    RemoveSource {
        expected: RuleSource,
    },
    PutRequirement {
        expected: Option<Requirement>,
        requirement: Requirement,
    },
    RemoveRequirement {
        expected: Requirement,
    },
}
fn bounded(s: &str, max: usize, required: bool) -> bool {
    s.len() <= max && (!required || !s.trim().is_empty()) && !s.contains('\0')
}
fn identity(s: &str) -> bool {
    bounded(s, 256, true)
}
fn date(s: &str) -> bool {
    if s.is_empty() {
        return true;
    }
    let b = s.as_bytes();
    if b.len() != 10
        || b[4] != b'-'
        || b[7] != b'-'
        || b.iter()
            .enumerate()
            .any(|(i, c)| i != 4 && i != 7 && !c.is_ascii_digit())
    {
        return false;
    }
    let y = s[..4].parse::<u32>().unwrap_or(0);
    let m = s[5..7].parse::<usize>().unwrap_or(0);
    let d = s[8..].parse::<u32>().unwrap_or(0);
    let days = [
        31,
        if y % 4 == 0 && (y % 100 != 0 || y % 400 == 0) {
            29
        } else {
            28
        },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    y > 0 && (1..=12).contains(&m) && d > 0 && d <= days[m - 1]
}
impl ProjectContext {
    fn validate(&self) -> Result<(), String> {
        if !bounded(&self.jurisdiction, 512, false)
            || !bounded(&self.uses, 4096, false)
            || !bounded(&self.stage, 256, false)
            || !date(&self.assessment_date)
        {
            return Err("Invalid rule project context or ISO date".into());
        }
        Ok(())
    }
    pub fn complete(&self) -> bool {
        !self.jurisdiction.trim().is_empty()
            && !self.uses.trim().is_empty()
            && !self.assessment_date.is_empty()
            && !self.stage.trim().is_empty()
    }
}
impl RuleBook {
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
    pub fn source(&self, id: &str) -> Option<&RuleSource> {
        self.sources.iter().find(|s| s.id == id)
    }
    fn edition_root<'a>(&'a self, id: &'a str) -> &'a str {
        self.source(id)
            .and_then(|s| s.same_edition_as.as_deref())
            .unwrap_or(id)
    }
    fn successors(&self, id: &str) -> Vec<&RuleSource> {
        let root = self.edition_root(id);
        self.sources
            .iter()
            .filter(|s| s.replaces.iter().any(|old| self.edition_root(old) == root))
            .collect()
    }
    pub fn basis(&self, source_id: &str) -> Result<String, String> {
        let source = self
            .source(source_id)
            .ok_or("Rule original revision is missing")?;
        let mut successors = self.successors(source_id);
        successors.sort_by(|a, b| a.id.cmp(&b.id));
        let mut value =
            serde_json::json!({"context":self.context,"source":source,"successors":successors});
        let lifecycle = self.lifecycle_for(source_id);
        if !lifecycle.is_empty() {
            value["lifecycle"] = serde_json::json!(lifecycle);
        }
        crate::automation_journal::canonical_hash(&value)
    }
    pub fn validate(&self) -> Result<(), String> {
        self.context.validate()?;
        crate::constraints::validate(self)?;
        if self.sources.len() > 1000 || self.requirements.len() > 5000 {
            return Err("Rule register exceeds source or requirement limit".into());
        }
        let mut ids = BTreeSet::new();
        let mut editions = BTreeSet::new();
        for s in &self.sources {
            if (s.same_edition_as.is_none()
                && !editions.insert((&s.code, &s.jurisdiction, &s.edition)))
                || !identity(&s.id)
                || !ids.insert(&s.id)
                || !bounded(&s.title, 512, true)
                || !bounded(&s.code, 256, true)
                || !bounded(&s.edition, 256, true)
                || !bounded(&s.publisher, 512, false)
                || !bounded(&s.jurisdiction, 512, false)
                || !date(&s.published_on)
                || !date(&s.effective_on)
                || !crate::storage::valid_document_identity(&s.original_sha256)
                || !bounded(&s.origin_url, 2048, false)
                || (!s.origin_url.is_empty()
                    && !s.origin_url.starts_with("https://")
                    && !s.origin_url.starts_with("http://"))
                || !bounded(&s.verification_note, 4096, false)
                || s.replaces.len() > 100
            {
                return Err("Invalid rule original metadata or duplicate revision".into());
            }
            if let Some(id) = &s.same_edition_as {
                let canonical = self.source(id).ok_or("Same-edition original is missing")?;
                if id == &s.id
                    || canonical.same_edition_as.is_some()
                    || !s.replaces.is_empty()
                    || canonical.code != s.code
                    || canonical.edition != s.edition
                    || canonical.jurisdiction != s.jurisdiction
                    || canonical.publisher != s.publisher
                    || canonical.published_on != s.published_on
                    || canonical.effective_on != s.effective_on
                    || canonical.original_sha256 == s.original_sha256
                    || s.verification_note.trim().is_empty()
                {
                    return Err("Invalid same-edition original relation or metadata".into());
                }
            }
            let mut previous = BTreeSet::new();
            for id in &s.replaces {
                let old = self
                    .source(id)
                    .ok_or("Replaced rule original revision is missing")?;
                if id == &s.id
                    || !previous.insert(id)
                    || old.code != s.code
                    || old.jurisdiction != s.jurisdiction
                {
                    return Err("Invalid rule revision relation".into());
                }
            }
        }
        // Revision cycles are checked across canonical editions as well as originals.
        for source in &self.sources {
            let root = self.edition_root(&source.id);
            let mut stack = vec![root];
            let mut seen = BTreeSet::new();
            while let Some(id) = stack.pop() {
                if !seen.insert(id) {
                    continue;
                }
                let current = self
                    .source(id)
                    .ok_or("Replaced rule original revision is missing")?;
                for old in &current.replaces {
                    let previous = self.edition_root(old);
                    if previous == root {
                        return Err("Cyclic rule revision relation".into());
                    }
                    stack.push(previous);
                }
            }
        }
        ids.clear();
        for r in &self.requirements {
            if !identity(&r.id)
                || !ids.insert(&r.id)
                || !bounded(&r.title, 512, true)
                || self.source(&r.citation.source_id).is_none()
                || !bounded(&r.citation.locator, 512, true)
                || !bounded(&r.citation.text, 16_384, true)
                || !bounded(&r.interpretation, 4096, false)
                || !bounded(&r.decision.rationale, 4096, false)
                || !bounded(&r.decision.missing, 4096, false)
                || !crate::storage::valid_document_identity(&r.decision.basis_sha256)
                || r.links.len() > 100
            {
                return Err("Invalid cited requirement or decision".into());
            }
            let mut targets = BTreeSet::new();
            for link in &r.links {
                link.target.validate()?;
                if !targets.insert(&link.target)
                    || !crate::storage::valid_document_identity(&link.baseline_sha256)
                {
                    return Err("Invalid or duplicate rule target".into());
                }
            }
            if (r.decision.applicability != Applicability::Unknown
                || r.decision.adoption != Adoption::Pending)
                && r.decision.rationale.trim().is_empty()
            {
                return Err("Applicability and adoption declarations need a rationale".into());
            }
            if r.decision.applicability == Applicability::Unknown
                && r.decision.missing.trim().is_empty()
            {
                return Err("Unknown applicability needs explicit missing information".into());
            }
            if matches!(
                r.decision.applicability,
                Applicability::Unknown | Applicability::Conflict
            ) && r.decision.adoption == Adoption::Adopted
            {
                return Err("Unknown or conflicting requirements cannot be adopted".into());
            }
        }
        self.validate_registers()?;
        Ok(())
    }
    pub fn decision_state(&self, r: &Requirement) -> &'static str {
        if self.basis(&r.citation.source_id).ok().as_ref() != Some(&r.decision.basis_sha256) {
            return "Rule decision is stale";
        }
        if let Some(state) = self.lifecycle_restriction(&r.citation.source_id) {
            return state;
        }
        if !self.context.complete() {
            return "Project applicability context is incomplete";
        }
        if !self.successors(&r.citation.source_id).is_empty() {
            return "Newer rule revision recorded; review applicability";
        }
        if let Some(source) = self.source(&r.citation.source_id) {
            if source.effective_on.is_empty() {
                return "Rule effective date is unknown";
            }
            if source.effective_on > self.context.assessment_date {
                return "Rule is not effective on the assessment date";
            }
        }
        r.decision.applicability.label()
    }
    pub fn requirement_state(&self, project: &Project, r: &Requirement) -> &'static str {
        if r.links
            .iter()
            .any(|l| l.status(project) != crate::evidence::LinkStatus::Unchanged)
        {
            "Linked rule target changed or missing"
        } else {
            self.decision_state(r)
        }
    }
    pub fn report(&self, project: &Project) -> serde_json::Value {
        serde_json::json!({"context":self.context,"declarations_only":true,"compliance_verdict":"not_evaluated","original_bytes":"verify through the bound repository","coverage_is_exhaustive":false,
            "coverage":self.coverage(project),"lifecycle":self.lifecycle.iter().map(|r|self.lifecycle_entry(r)).collect::<Vec<_>>(),"conditions":self.conditions.iter().map(|r|self.condition_entry(project,r)).collect::<Vec<_>>(),
            "requirements":self.requirements.iter().map(|r|serde_json::json!({"id":r.id,"title":r.title,"decision_state":self.requirement_state(project,r),"decision":r.decision,"citation":r.citation,"links":r.links.iter().map(|l|serde_json::json!({"target":l.target,"status":l.status(project)})).collect::<Vec<_>>()})).collect::<Vec<_>>()})
    }
}
pub fn apply(book: &RuleBook, edit: &RuleEdit) -> Result<RuleBook, String> {
    let mut next = book.clone();
    match edit {
        RuleEdit::PutLifecycle { expected, record } => {
            let index = book.lifecycle.iter().position(|r| r.id == record.id);
            if index.map(|i| &book.lifecycle[i]) != expected.as_ref() {
                return Err("Lifecycle record changed after preview".into());
            }
            if let Some(i) = index {
                next.lifecycle[i] = record.clone()
            } else {
                next.lifecycle.push(record.clone())
            }
        }
        RuleEdit::RemoveLifecycle { expected } => {
            let i = book
                .lifecycle
                .iter()
                .position(|r| r == expected)
                .ok_or("Lifecycle record changed after preview")?;
            next.lifecycle.remove(i);
        }
        RuleEdit::PutCondition {
            expected,
            condition,
        } => {
            let index = book.conditions.iter().position(|r| r.id == condition.id);
            if index.map(|i| &book.conditions[i]) != expected.as_ref() {
                return Err("Condition need changed after preview".into());
            }
            if matches!(
                condition.status,
                ConditionStatus::Addressed | ConditionStatus::NotApplicable
            ) && !condition.missing.trim().is_empty()
            {
                return Err(
                    "Resolve missing information before declaring the condition addressed".into(),
                );
            }
            if matches!(
                condition.status,
                ConditionStatus::Addressed | ConditionStatus::NotApplicable
            ) && (!book.context.complete()
                || condition.originals.iter().any(DocumentRef::is_missing))
            {
                return Err(
                    "Condition declaration needs complete current original evidence".into(),
                );
            }
            if matches!(
                condition.status,
                ConditionStatus::Addressed | ConditionStatus::NotApplicable
            ) && condition.requirement_ids.iter().any(|id| {
                book.requirements
                    .iter()
                    .find(|r| &r.id == id)
                    .is_none_or(|r| {
                        book.decision_state(r) != r.decision.applicability.label()
                            || !matches!(
                                (r.decision.applicability, r.decision.adoption),
                                (Applicability::Applicable, Adoption::Adopted)
                                    | (Applicability::NotApplicable, Adoption::NotAdopted)
                            )
                    })
            }) {
                return Err(
                    "Condition declaration has pending or stale linked requirements".into(),
                );
            }
            if book.condition_basis(condition)? != condition.basis_sha256 {
                return Err("Condition review basis changed; preview again".into());
            }
            if let Some(i) = index {
                next.conditions[i] = condition.clone()
            } else {
                next.conditions.push(condition.clone())
            }
        }
        RuleEdit::RemoveCondition { expected } => {
            let i = book
                .conditions
                .iter()
                .position(|r| r == expected)
                .ok_or("Condition need changed after preview")?;
            next.conditions.remove(i);
        }
        RuleEdit::SetContext { expected, context } => {
            if &book.context != expected {
                return Err("Rule project context changed after preview".into());
            }
            context.validate()?;
            next.context = context.clone();
        }
        RuleEdit::AddSource { source } => {
            if book.source(&source.id).is_some() {
                return Err(
                    "Rule original revision already exists; add a distinct revision".into(),
                );
            }
            next.sources.push(source.clone());
        }
        RuleEdit::RemoveSource { expected } => {
            if book.source(&expected.id) != Some(expected) {
                return Err("Rule original changed after preview".into());
            }
            if book.referenced_by_registers(&expected.id)
                || book
                    .requirements
                    .iter()
                    .any(|r| r.citation.source_id == expected.id)
                || book.sources.iter().any(|s| {
                    s.replaces.contains(&expected.id)
                        || s.same_edition_as.as_ref() == Some(&expected.id)
                })
            {
                return Err("Referenced rule original cannot be removed".into());
            }
            next.sources.retain(|s| s.id != expected.id);
        }
        RuleEdit::PutRequirement {
            expected,
            requirement,
        } => {
            let index = book
                .requirements
                .iter()
                .position(|r| r.id == requirement.id);
            if index.map(|i| &book.requirements[i]) != expected.as_ref() {
                return Err("Requirement changed after preview".into());
            }
            if book.basis(&requirement.citation.source_id)? != requirement.decision.basis_sha256 {
                return Err("Rule decision basis changed; preview again".into());
            }
            if !book.context.complete()
                && (requirement.decision.applicability != Applicability::Unknown
                    || requirement.decision.adoption != Adoption::Pending)
            {
                return Err("Complete project context before declaring applicability".into());
            }
            if requirement.decision.applicability == Applicability::Applicable {
                if book
                    .lifecycle_restriction(&requirement.citation.source_id)
                    .is_some()
                {
                    return Err(
                        "Review lifecycle evidence before declaring current applicability".into(),
                    );
                }
                let source = book
                    .source(&requirement.citation.source_id)
                    .ok_or("Rule original revision is missing")?;
                if source.effective_on.is_empty()
                    || source.effective_on > book.context.assessment_date
                {
                    return Err(
                        "Review the effective date before declaring current applicability".into(),
                    );
                }
            }
            if let Some(i) = index {
                next.requirements[i] = requirement.clone()
            } else {
                next.requirements.push(requirement.clone())
            }
        }
        RuleEdit::RemoveRequirement { expected } => {
            if book
                .conditions
                .iter()
                .any(|r| r.requirement_ids.contains(&expected.id))
            {
                return Err("Referenced requirement cannot be removed".into());
            }
            let index = book
                .requirements
                .iter()
                .position(|r| r.id == expected.id)
                .ok_or("Requirement no longer exists")?;
            if &book.requirements[index] != expected {
                return Err("Requirement changed after preview".into());
            }
            next.requirements.remove(index);
        }
    }
    next.validate()?;
    Ok(next)
}
