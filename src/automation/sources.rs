//! Task context reads over existing project assets. No new model, write grant or interpretation.
use super::*;
use crate::{drawings::SourcePage, project_repository::Repository};
pub const MAX_CONTENT: usize = 2 * 1024 * 1024;
pub const MAX_CHUNK: usize = 32 * 1024;
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Search {
    pub target: Target,
    pub expected_revision: u64,
    pub expected_sources_revision: Option<String>,
    #[serde(default)]
    pub query: String,
    pub offset: usize,
    pub limit: usize,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Representation {
    Original,
    Text,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DxfOutline {
    pub handle: String,
    #[serde(default)]
    pub allow_constant_width_centerline: bool,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Read {
    pub target: Target,
    pub expected_revision: u64,
    pub sources_revision: String,
    pub source_id: String,
    pub representation: Representation,
    /// One strict source record, not an import or a larger original-byte allowance.
    #[serde(default)]
    pub dxf_outline: Option<DxfOutline>,
    pub offset: usize,
    pub limit: usize,
}
fn attached<'a>(editor: &Editor, repo: Option<&'a Repository>) -> Option<&'a Repository> {
    repo.filter(|r| r.manifest.document_id == editor.project_identity())
}
pub fn context(editor: &Editor, repo: Option<&Repository>) -> Value {
    let repo = attached(editor, repo);
    let revision = crate::automation_journal::canonical_hash(&(
        editor.project_identity(),
        repo.map(|r| r.source_context_identity()),
        &editor.project.drawings.pages,
    ))
    .expect("source metadata serializable");
    json!({"sources_revision":revision,"repository_attached":repo.is_some(),"repository_revision":repo.map(|r|r.manifest.revision),"repository_model_revision":repo.map(|r|r.manifest.model_revision),"repository_sources":repo.map_or(0,|r|r.manifest.sources.len()),"drawing_pages":editor.project.drawings.pages.len(),"search_tool":"query_project_sources","read_tool":"read_project_source","search_is_content_read":false,"max_read_bytes":MAX_CONTENT,"targeted_dxf_outline":"optional dxf_outline {handle, allow_constant_width_centerline:false} with text representation; strict native-coordinate record, no scale or retain-role inference","context_is_write_authorization":false})
}
fn check(
    service: &ReadService,
    editor: &Editor,
    target: &Target,
    revision: u64,
) -> Result<(), ReadError> {
    service.check(editor, target)?;
    if editor.revision != revision {
        return Err(fail("REVISION_CONFLICT"));
    }
    Ok(())
}
pub fn search(
    service: &ReadService,
    editor: &Editor,
    repo: Option<&Repository>,
    p: &Search,
) -> Result<Value, ReadError> {
    check(service, editor, &p.target, p.expected_revision)?;
    if !(1..=20).contains(&p.limit) || p.query.len() > 512 {
        return Err(fail("INVALID_SOURCE_QUERY"));
    }
    let ctx = context(editor, repo);
    if (p.offset > 0 && p.expected_sources_revision.is_none())
        || p.expected_sources_revision
            .as_ref()
            .is_some_and(|s| ctx["sources_revision"] != *s)
    {
        return Err(fail("SOURCE_CONTEXT_CONFLICT"));
    }
    let repo = attached(editor, repo);
    let mut entries = vec![];
    if let Some(repo) = repo {
        for s in &repo.manifest.sources {
            entries.push(json!({"source_id":format!("repository:{}",s.id),"names":s.names,"format":s.format,"sha256":s.id,"bytes":s.blob.bytes,"quality":s.quality,"confirmed_unit":s.confirmed_unit,"origin":"repository","content_read":false}));
        }
    }
    for page in &editor.project.drawings.pages {
        entries.push(json!({"source_id":format!("drawing:{}",page.id),"names":[page.name],"sha256":page.sha256,"pixels":page.pixels,"origin":"drawing","scale":"use explicit calibration regions if present; pixel dimensions are not measured dimensions","content_read":false}));
    }
    let query = p.query.to_lowercase();
    entries.retain(|v| {
        ["source_id", "sha256", "format"].iter().any(|key| {
            v[*key]
                .as_str()
                .is_some_and(|s| s.to_lowercase().contains(&query))
        }) || v["names"].as_array().is_some_and(|names| {
            names.iter().any(|n| {
                n.as_str()
                    .is_some_and(|s| s.to_lowercase().contains(&query))
            })
        })
    });
    entries.sort_by(|a, b| a["source_id"].as_str().cmp(&b["source_id"].as_str()));
    let total = entries.len();
    if p.offset > total {
        return Err(fail("INVALID_SOURCE_QUERY"));
    }
    let end = (p.offset + p.limit).min(total);
    Ok(
        json!({"target":p.target,"revision":editor.revision,"context_revision":service.context_revision(editor),"source_context":ctx,"entries":entries[p.offset..end],"total":total,"next_offset":(end<total).then_some(end),"retrieval":"metadata substring only; read originals before interpreting; no semantic relevance or latest/approved version inferred"}),
    )
}
enum Origin {
    Repository(Repository, String, String),
    Drawing(SourcePage),
}
pub(crate) struct ReadPlan {
    request: Read,
    context: Value,
    origin: Origin,
}
pub(crate) fn prepare_read(
    service: &ReadService,
    editor: &Editor,
    repo: Option<&Repository>,
    p: Read,
) -> Result<ReadPlan, ReadError> {
    check(service, editor, &p.target, p.expected_revision)?;
    if !(1..=MAX_CHUNK).contains(&p.limit) || p.offset > MAX_CONTENT {
        return Err(fail("INVALID_SOURCE_RANGE"));
    }
    let ctx = context(editor, repo);
    if ctx["sources_revision"] != p.sources_revision {
        return Err(fail("SOURCE_CONTEXT_CONFLICT"));
    }
    if p.dxf_outline.as_ref().is_some_and(|s| {
        !matches!(p.representation, Representation::Text)
            || s.handle.is_empty()
            || s.handle.len() > 16
            || u64::from_str_radix(&s.handle, 16)
                .ok()
                .is_none_or(|id| id == 0)
            || !p.source_id.starts_with("repository:")
    }) {
        return Err(fail("INVALID_SOURCE_SELECTOR"));
    }
    let origin = if let Some(id) = p.source_id.strip_prefix("repository:") {
        let repo = attached(editor, repo).ok_or_else(|| fail("REPOSITORY_NOT_ATTACHED"))?;
        let source = repo
            .manifest
            .sources
            .iter()
            .find(|s| s.id == id)
            .ok_or_else(|| fail("SOURCE_NOT_FOUND"))?;
        if p.dxf_outline.is_some() && source.format != "dxf" {
            return Err(fail("INVALID_SOURCE_SELECTOR"));
        }
        Origin::Repository(repo.clone(), id.to_owned(), source.format.clone())
    } else if let Some(id) = p.source_id.strip_prefix("drawing:") {
        let page = editor
            .project
            .drawings
            .pages
            .iter()
            .find(|s| s.id.to_string() == id)
            .ok_or_else(|| fail("SOURCE_NOT_FOUND"))?;
        Origin::Drawing(page.clone())
    } else {
        return Err(fail("SOURCE_NOT_FOUND"));
    };
    Ok(ReadPlan {
        request: p,
        context: json!({"sources":ctx,"context_revision":service.context_revision(editor),"task_id":editor.project.design_process.current_task,"active_option":editor.project.plan_options.active_design_option,"selection":editor.selection_ids()}),
        origin,
    })
}
impl ReadPlan {
    /// Runs on a bounded background reader, never borrows Editor or writes a source.
    pub(crate) fn execute(self) -> Value {
        let result = (|| -> Result<Value, String> {
            let (bytes, original_sha, format) = match &self.origin {
                Origin::Repository(r, id, format) => {
                    if let Some(selector) = &self.request.dxf_outline {
                        let value = r.read_dxf_outline(
                            id,
                            &selector.handle,
                            selector.allow_constant_width_centerline,
                        )?;
                        (
                            serde_json::to_vec(&value).map_err(|e| e.to_string())?,
                            id.clone(),
                            "dxf_outline_json",
                        )
                    } else {
                        (
                            r.read_original(id, MAX_CONTENT)?,
                            id.clone(),
                            format.as_str(),
                        )
                    }
                }
                Origin::Drawing(page) => {
                    if std::fs::symlink_metadata(&page.path)
                        .map_err(|e| e.to_string())?
                        .file_type()
                        .is_symlink()
                    {
                        return Err("Drawing read symlink rejected".into());
                    }
                    let bytes = crate::drawings::verified_bytes(page)?;
                    if bytes.len() > MAX_CONTENT {
                        return Err("Source read exceeds 2 MiB; import limit is unchanged".into());
                    }
                    let image = crate::drawings::decode(&bytes)?;
                    if [image.width(), image.height()] != page.pixels {
                        return Err("Drawing pixel dimensions changed".into());
                    }
                    (bytes, page.sha256.clone(), "image")
                }
            };
            let image_format = image::guess_format(&bytes)
                .ok()
                .filter(|f| matches!(f, image::ImageFormat::Png | image::ImageFormat::Jpeg));
            let (content,mime,extractor)=match self.request.representation {
                Representation::Original=>{
                    if image_format.is_some(){crate::drawings::decode(&bytes)?;}
                    let mime=match image_format {Some(image::ImageFormat::Png)=>"image/png",Some(image::ImageFormat::Jpeg)=>"image/jpeg",_=>"application/octet-stream"};
                    (bytes,mime,"original_bytes")
                }
                Representation::Text=>match format {
                    "dxf_outline_json"=>(bytes,"application/json","single_closed_dxf_outline; source native coordinates; no block expansion or model import"),
                    "txt"|"md"|"html"|"htm"|"json"=>{std::str::from_utf8(&bytes).map_err(|_|"Source text is not UTF-8")?;(bytes,"text/plain","exact_utf8; HTML is source text, never executed")},
                    "docx"=>(serde_json::to_vec(&crate::source_document::inspect_docx(&bytes)?).map_err(|e|e.to_string())?,"application/json","bounded_docx_xml_body; locators not rendered pages; unsupported hidden/revision/layout content rejected by existing extractor"),
                    _=>return Err("Text extraction unsupported for this source; no OCR or invisible content inferred".into()),
                }
            };
            if content.len() > MAX_CONTENT {
                return Err("Source representation exceeds 2 MiB".into());
            }
            if self.request.offset > content.len() {
                return Err("Source range exceeds representation".into());
            }
            let end = (self.request.offset + self.request.limit).min(content.len());
            let hex = content[self.request.offset..end]
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>();
            let mut result = json!({"target":self.request.target,"revision":self.request.expected_revision,"source_id":self.request.source_id,"sources_revision":self.request.sources_revision,"read_context":self.context,"representation":self.request.representation,"original_sha256":original_sha,"content_sha256":crate::drawings::digest(&content),"total_bytes":content.len(),"offset":self.request.offset,"returned_bytes":end-self.request.offset,"next_offset":(end<content.len()).then_some(end),"encoding":"hex","data":hex,"mime_type":mime,"extractor":extractor,"interpretation":"not_performed; received bytes are not proof of Agent understanding, surveyed dimensions, approval or task authority"});
            if let Some(selector) = &self.request.dxf_outline {
                result["dxf_outline"] = json!(selector);
            }
            Ok(result)
        })();
        match result {
            Ok(v) => json!({"result":v}),
            Err(message) => json!({"error":{"code":"SOURCE_READ_FAILED","message":message}}),
        }
    }
}
