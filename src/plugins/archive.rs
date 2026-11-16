//! Immutable, validated history carried by the native project; never execution authority.
use super::{MAX_BYTES, jobs::Bundle, provider::Capture};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, sync::Arc};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Record {
    pub bundle: Bundle,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub process: Option<Capture>,
}
impl Record {
    /// Merge observations of the same immutable request, never a new identity.
    pub fn merge(&self, other: &Self) -> Result<Self, String> {
        if self.bundle.intent != other.bundle.intent
            || matches!((&self.bundle.result,&other.bundle.result),(Some(a),Some(b)) if a!=b)
            || matches!((&self.process,&other.process),(Some(a),Some(b)) if a!=b)
        {
            return Err("Local and saved job evidence conflict; originals preserved".into());
        }
        let mut merged = self.clone();
        if merged.bundle.result.is_none() {
            merged.bundle.result = other.bundle.result.clone();
        }
        merged.bundle.cancel_requested |= other.bundle.cancel_requested;
        if merged.process.is_none() {
            merged.process = other.process.clone();
        }
        Ok(merged)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Wire {
    schema_version: u32,
    records: Vec<Record>,
}
#[derive(Debug)]
struct Validated {
    wire: Wire,
    sha256: String,
}

/// Fields are private: construction/deserialization validates each artifact once.
/// Editor clones share history. Ordinary Project::validate does not reparse or
/// recalculate saved reports on every edit, preview or frame.
#[derive(Clone, Debug)]
pub struct Archive(Arc<Validated>);
impl Default for Archive {
    fn default() -> Self {
        Self(Arc::new(Validated {
            wire: Wire {
                schema_version: 1,
                records: vec![],
            },
            sha256: String::new(),
        }))
    }
}
impl PartialEq for Archive {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
            || (self.0.sha256 == other.0.sha256 && self.0.wire == other.0.wire)
    }
}
impl Archive {
    pub fn merge(&self, other: &Self) -> Result<Self, String> {
        if self == other || other.is_empty() {
            return Ok(self.clone());
        }
        if self.is_empty() {
            return Ok(other.clone());
        }
        if self.project_id() != other.project_id() {
            return Err("Project job archive belongs to another document".into());
        }
        let mut records = self.records().to_vec();
        for record in other.records() {
            if let Some(old) = records.iter_mut().find(|r| {
                r.bundle.intent.identity.request_id == record.bundle.intent.identity.request_id
            }) {
                *old = old.merge(record)?;
            } else {
                records.push(record.clone());
            }
        }
        Self::new(records)
    }
    pub fn new(mut records: Vec<Record>) -> Result<Self, String> {
        if records.is_empty() {
            return Ok(Self::default());
        }
        if records.len() > 64 {
            return Err("Project job archive exceeds 64 original receipts".into());
        }
        records.sort_by(|a, b| {
            a.bundle
                .intent
                .identity
                .request_id
                .cmp(&b.bundle.intent.identity.request_id)
        });
        let project = records[0].bundle.intent.identity.project_id.clone();
        let mut ids = BTreeSet::new();
        for record in &records {
            record.bundle.validate()?;
            let identity = &record.bundle.intent.identity;
            if identity.project_id != project || !ids.insert(&identity.request_id) {
                return Err("Project job archive identities conflict".into());
            }
            if serde_json::to_vec_pretty(&record.bundle)
                .map_err(|e| e.to_string())?
                .len() as u64
                > MAX_BYTES
            {
                return Err("Plugin artifact exceeds 4 MiB".into());
            }
            if let Some(process) = &record.process {
                process.validate(identity)?;
                if record.bundle.intent.execution.is_none() {
                    return Err("Process receipt has no original executable identity".into());
                }
                if serde_json::to_vec_pretty(process)
                    .map_err(|e| e.to_string())?
                    .len() as u64
                    > MAX_BYTES
                {
                    return Err("Plugin artifact exceeds 4 MiB".into());
                }
            }
        }
        let wire = Wire {
            schema_version: 1,
            records,
        };
        let bytes = serde_json::to_vec_pretty(&wire).map_err(|e| e.to_string())?;
        if bytes.len() as u64 > crate::storage::MAX_PROJECT_BYTES {
            return Err("Project exceeds 32 MB demo limit".into());
        }
        Ok(Self(Arc::new(Validated {
            wire,
            sha256: crate::drawings::digest(&bytes),
        })))
    }
    pub fn is_empty(&self) -> bool {
        self.0.wire.records.is_empty()
    }
    pub fn records(&self) -> &[Record] {
        &self.0.wire.records
    }
    pub fn project_id(&self) -> Option<&str> {
        self.records()
            .first()
            .map(|r| r.bundle.intent.identity.project_id.as_str())
    }
    pub fn get(&self, project: &str, request: &str) -> Result<Option<&Record>, String> {
        self.check_project(project)?;
        Ok(self
            .records()
            .iter()
            .find(|r| r.bundle.intent.identity.request_id == request))
    }
    pub fn check_project(&self, project: &str) -> Result<(), String> {
        if self.project_id().is_some_and(|id| id != project) {
            return Err("Project job archive belongs to another document".into());
        }
        Ok(())
    }
}
impl Serialize for Archive {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.0.wire.serialize(s)
    }
}
impl<'de> Deserialize<'de> for Archive {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let wire = Wire::deserialize(d)?;
        if wire.schema_version != 1 {
            return Err(serde::de::Error::custom(
                "Unsupported project job archive version",
            ));
        }
        Self::new(wire.records).map_err(serde::de::Error::custom)
    }
}
