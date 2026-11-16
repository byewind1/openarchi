//! Read-only public projections of the existing registry and original job store.
//! Plans carry bounded immutable context, never an Editor or a dispatch handle.
use super::{
    Registry,
    jobs::{Input, Jobs},
};
use crate::{automation::Target, editor::Editor};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::PathBuf;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogRequest {
    pub target: Target,
    pub offset: usize,
    pub limit: usize,
    pub catalog_sha256: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ListRequest {
    pub target: Target,
    pub expected_revision: u64,
    pub offset: usize,
    pub limit: usize,
    pub index_sha256: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReadRequest {
    pub target: Target,
    pub expected_revision: u64,
    pub expected_context_revision: String,
    pub request_id: String,
    pub offset: usize,
    pub limit: usize,
    pub content_sha256: Option<String>,
    #[serde(default)]
    pub artifact: Artifact,
}
#[derive(Default, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Artifact {
    #[default]
    Record,
    Process,
}
fn error(code: &str) -> Value {
    json!({"error":{"code":code}})
}
fn bound(offset: usize, limit: usize, total: usize, max: usize) -> Result<usize, Value> {
    if !(1..=max).contains(&limit) || offset > total {
        return Err(error("INVALID_PAGE"));
    }
    Ok(offset.saturating_add(limit).min(total))
}
fn token(expected: &Option<String>, actual: &str, offset: usize) -> Result<(), Value> {
    if expected.as_deref().is_some_and(|v| v != actual) || (offset != 0 && expected.is_none()) {
        return Err(error("ARTIFACT_CHANGED"));
    }
    Ok(())
}
pub fn catalog(registry: &Registry, p: CatalogRequest) -> Value {
    let run = || {
        let entries = registry.entries();
        let hash = crate::automation_journal::canonical_hash(&entries)
            .map_err(|_| error("PLUGIN_CATALOG_UNKNOWN"))?;
        let end = bound(p.offset, p.limit, entries.len(), 4)?;
        token(&p.catalog_sha256, &hash, p.offset)?;
        let result = json!({"target":p.target,"catalog_sha256":hash,"total":entries.len(),
            "offset":p.offset,"next_offset":(end<entries.len()).then_some(end),
            "entries":&entries[p.offset..end],"discovery_only":true,"remote_dispatch":false});
        if serde_json::to_vec(&result)
            .map_err(|_| error("PLUGIN_CATALOG_UNKNOWN"))?
            .len()
            > 96 * 1024
        {
            return Err(error("OUTPUT_LIMIT"));
        }
        Ok(json!({"result":result}))
    };
    run().unwrap_or_else(|e| e)
}
pub enum Plan {
    List {
        root: PathBuf,
        request: ListRequest,
        archive: super::archive::Archive,
    },
    Read {
        root: PathBuf,
        request: ReadRequest,
        input: Result<Input, String>,
        processed_at_request: Option<usize>,
        program: crate::program::Program,
        solar_context: super::solar::Context,
        archive: super::archive::Archive,
    },
}
impl Plan {
    pub fn list(jobs: &Jobs, request: ListRequest) -> Self {
        Self::list_with_archive(jobs, &super::archive::Archive::default(), request)
    }
    pub fn list_with_archive(
        jobs: &Jobs,
        archive: &super::archive::Archive,
        request: ListRequest,
    ) -> Self {
        Self::List {
            root: jobs.read_root(),
            request,
            archive: archive.clone(),
        }
    }
    pub fn read(jobs: &Jobs, editor: &Editor, request: ReadRequest) -> Self {
        let processed_at_request = jobs.progress(&request.target.project_id, &request.request_id);
        Self::Read {
            root: jobs.read_root(),
            request,
            input: Input::capture(editor),
            processed_at_request,
            program: editor.project.program.clone(),
            solar_context: super::solar::Context::capture(editor),
            archive: editor.project.job_archive.clone(),
        }
    }
    pub fn execute(self) -> Value {
        self.execute_inner().unwrap_or_else(|e| e)
    }
    fn execute_inner(self) -> Result<Value, Value> {
        match self {
            Self::List {
                root,
                request: p,
                archive,
            } => {
                if !(1..=16).contains(&p.limit) {
                    return Err(error("INVALID_PAGE"));
                }
                let ids = Jobs::new(root)
                    .ids_with_archive(&p.target.project_id, &archive)
                    .map_err(|_| error("JOB_INDEX_UNKNOWN"))?;
                let hash = crate::automation_journal::canonical_hash(&ids)
                    .map_err(|_| error("JOB_INDEX_UNKNOWN"))?;
                let end = bound(p.offset, p.limit, ids.len(), 16)?;
                token(&p.index_sha256, &hash, p.offset)?;
                Ok(
                    json!({"result":{"target":p.target,"revision":p.expected_revision,
                    "request_ids":&ids[p.offset..end],"total":ids.len(),"offset":p.offset,
                    "index_sha256":hash,"next_offset":(end<ids.len()).then_some(end),
                    "order":"request_id; not chronological","dispatch":false}}),
                )
            }
            Self::Read {
                root,
                request: p,
                input,
                processed_at_request,
                program,
                solar_context,
                archive,
            } => {
                if !(1..=32768).contains(&p.limit) {
                    return Err(error("INVALID_PAGE"));
                }
                let jobs = Jobs::new(root);
                let b=jobs.bundle_with_archive(&p.target.project_id,&p.request_id,&archive)
                    .map_err(|message| json!({"error":{"code":"JOB_ARTIFACT_UNKNOWN","message":message,
                        "request_id":p.request_id,"recovery":"Inspect the original operation; never replay or replace its identity."}}))?;
                let current = input.and_then(|mut i| {
                    if b.intent.input.program.is_some() {
                        i.program = Some(program);
                        i.validate()?;
                    }
                    if let Some(s) = &b.intent.input.solar {
                        i = solar_context.apply(i, &s.parameters())?;
                    }
                    i.fingerprint()
                });
                let status = match &current {
                    Ok(hash) if *hash == b.intent.identity.input_sha256 => "current",
                    Ok(_) => "stale",
                    Err(_) => "unknown",
                };
                let bytes = match p.artifact {
                    Artifact::Record => serde_json::to_vec(&b),
                    Artifact::Process => {
                        let receipt = jobs
                            .process_with_archive(&b.intent.identity, &archive)
                            .map_err(|_| error("JOB_PROCESS_UNKNOWN"))?
                            .ok_or_else(|| error("JOB_PROCESS_UNKNOWN"))?;
                        serde_json::to_vec(&receipt)
                    }
                }
                .map_err(|_| error("JOB_ARTIFACT_UNKNOWN"))?;
                if bytes.len() as u64 > super::MAX_BYTES {
                    return Err(error("OUTPUT_LIMIT"));
                }
                let hash = crate::drawings::digest(&bytes);
                let end = bound(p.offset, p.limit, bytes.len(), 32768)?;
                token(&p.content_sha256, &hash, p.offset)?;
                let data: String = bytes[p.offset..end]
                    .iter()
                    .map(|v| format!("{v:02x}"))
                    .collect();
                Ok(
                    json!({"result":{"target":p.target,"revision":p.expected_revision,
                    "context_revision":p.expected_context_revision,"request_id":p.request_id,
                    "artifact":p.artifact,
                    "identity":b.intent.identity,"phase":b.phase(),"input_status":status,
                    "processed_at_request":processed_at_request,
                    "execution_observation":if b.result.is_some(){"terminal_artifact"}else if processed_at_request.is_some(){"running_observed_at_request"}else{"outcome_unknown"},
                    "current_input_sha256":current.as_ref().ok(),"current_input_error":current.as_ref().err(),
                    "source_session":b.intent.source_session,"source_revision":b.intent.source_revision,
                    "content_sha256":hash,"total_bytes":bytes.len(),"offset":p.offset,
                    "returned_bytes":end-p.offset,"next_offset":(end<bytes.len()).then_some(end),
                    "encoding":"hex","data":data,"dispatch":false}}),
                )
            }
        }
    }
}
