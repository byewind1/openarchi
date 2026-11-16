//! Read-only automation boundary. The caller borrows the application's Editor.
use crate::editor::Editor;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
pub const CAPABILITY_VERSION: u32 = 46;
pub mod intent;
pub mod sources;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Target {
    pub instance_id: String,
    pub project_id: String,
    pub document_session_id: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TerrainRequest {
    pub target: Target,
    pub expected_revision: u64,
    pub xy_m: [f64; 2],
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceRequest {
    #[serde(default)]
    pub topics: bool,
    #[serde(default)]
    pub object_targets: Vec<crate::evidence::Target>,
    pub target: Target,
    pub expected_revision: u64,
    pub offset: usize,
    pub limit: usize,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RulesRequest {
    pub target: Target,
    pub expected_revision: u64,
    pub section: RulesSection,
    pub offset: usize,
    pub limit: usize,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RulesSection {
    Checks,
    Lifecycle,
    Conditions,
    Sources,
    Requirements,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpaceRequest {
    pub target: Target,
    pub expected_revision: u64,
    pub options: crate::zones::analysis::AnalysisOptions,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Query {
    #[serde(default)]
    pub design_scope: crate::view_settings::DesignOptionScope,
    pub target: Target,
    pub expected_revision: u64,
    #[serde(default)]
    pub ids: Vec<u64>,
    pub kind: Option<String>,
    pub story: Option<u32>,
    pub region: Option<Region>,
    pub fields: Vec<String>,
    pub limit: usize,
    pub cursor: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Region {
    pub min: [f64; 3],
    pub max: [f64; 3],
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Validate {
    pub target: Target,
    pub expected_revision: u64,
    pub limit: usize,
    pub cursor: Option<String>,
}

#[derive(Debug, PartialEq, Serialize)]
pub struct ReadError {
    pub code: &'static str,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<crate::derived_geometry::GeometryDiagnostic>,
}
fn fail(code: &'static str) -> ReadError {
    ReadError {
        code,
        diagnostics: vec![],
    }
}

/// One runtime identity; never owns a Project or Editor.
pub struct ReadService {
    instance_id: String,
}
impl ReadService {
    pub fn new(instance_id: String) -> Self {
        Self { instance_id }
    }
    pub fn target(&self, editor: &Editor) -> Target {
        Target {
            instance_id: self.instance_id.clone(),
            project_id: editor.project_identity().into(),
            document_session_id: editor.document_session(),
        }
    }
    fn check(&self, editor: &Editor, target: &Target) -> Result<(), ReadError> {
        if *target != self.target(editor) {
            return Err(fail("TARGET_MISMATCH"));
        }
        Ok(())
    }
    /// Snapshot equality token, not a monotonic counter. Language is presentation only.
    pub fn context_revision(&self, editor: &Editor) -> String {
        crate::automation_journal::canonical_hash(&(
            self.target(editor),
            editor.revision,
            &editor.selected_ids,
            editor.story,
            editor.project.plan_options.active_design_option,
            &editor.project.design_process.current_task,
        ))
        .expect("context is serializable")
    }
    pub fn context(&self, editor: &Editor, target: &Target) -> Result<Value, ReadError> {
        self.check(editor, target)?;
        Ok(
            json!({"target":target,"revision":editor.revision,"units":{"length":"m","area":"m2","volume":"m3"},"coordinate_space":"project","site":editor.project.site,"program":editor.project.program,"program_summary":editor.project.program.summary(),"evidence_summary":editor.project.evidence.summary(),"rules_summary":{"checks":editor.project.rules.checks.len(),"sources":editor.project.rules.sources.len(),"requirements":editor.project.rules.requirements.len(),"lifecycle":editor.project.rules.lifecycle.len(),"conditions":editor.project.rules.conditions.len(),"compliance_verdict":"not_evaluated","read_tool":"query_project_rules"},"terrain_summary":crate::site::terrain::summary(&editor.project.site),"site_control_check":editor.project.site.control_check().map(|check|json!(check)).unwrap_or_else(|reason|json!({"status":"invalid","reason":reason})),"context_revision":self.context_revision(editor),"view_relative_coordinates_supported":false,"layers":editor.project.layers,"stories":editor.project.stories,"selection":editor.selected_ids,"active_story":editor.story,"active_design_option":editor.project.plan_options.active_design_option,"model_3d_style":editor.project.plan_options.model_3d_style,"design_options":editor.project.design_options,"mass_spaces":{"count":editor.project.design_process.mass_spaces.len(),"read_tool":"query_mass_spaces"},"model_figures":{"report_template_count":editor.project.drawings.reports.len(),"count":editor.project.drawings.diagrams.len(),"read_tool":"query_model_figures"},"design_process":editor.project.design_process.context(),"parametric_candidates":crate::design_process::parametric::readback(&editor.project),"dirty":editor.dirty(),"undo_available":!editor.undo.is_empty(),"redo_available":!editor.redo.is_empty(),"permissions":["read"],"capability_version":CAPABILITY_VERSION}),
        )
    }
    pub fn project_rules(
        &self,
        editor: &Editor,
        request: &RulesRequest,
    ) -> Result<Value, ReadError> {
        self.check(editor, &request.target)?;
        if request.expected_revision != editor.revision {
            return Err(fail("REVISION_CONFLICT"));
        }
        let book = &editor.project.rules;
        let total = match request.section {
            RulesSection::Checks => book.checks.len(),
            RulesSection::Lifecycle => book.lifecycle.len(),
            RulesSection::Conditions => book.conditions.len(),
            RulesSection::Sources => book.sources.len(),
            RulesSection::Requirements => book.requirements.len(),
        };
        if !(1..=10).contains(&request.limit) || request.offset > total {
            return Err(fail("INVALID_LIMIT"));
        }
        let mut entries = vec![];
        let mut bytes = 0;
        for index in request.offset..request.offset.saturating_add(request.limit).min(total) {
            let entry = match request.section {
                RulesSection::Checks => {
                    crate::constraints::entry(&editor.project, &book.checks[index])
                }
                RulesSection::Lifecycle => book.lifecycle_entry(&book.lifecycle[index]),
                RulesSection::Conditions => {
                    book.condition_entry(&editor.project, &book.conditions[index])
                }
                RulesSection::Sources => json!({"source":book.sources[index]}),
                RulesSection::Requirements => {
                    let r = &book.requirements[index];
                    json!({"requirement":r,"decision_state":book.requirement_state(&editor.project,r),"links":r.links.iter().map(|l|json!({"target":l.target,"status":l.status(&editor.project)})).collect::<Vec<_>>()})
                }
            };
            let size = serde_json::to_vec(&entry)
                .map_err(|_| fail("INVALID_QUERY_RESULT"))?
                .len();
            if size > 900 * 1024 {
                return Err(fail("RESULT_TOO_LARGE"));
            }
            if bytes + size > 900 * 1024 {
                break;
            }
            bytes += size;
            entries.push(entry);
        }
        let next = request.offset + entries.len();
        Ok(
            json!({"target":request.target,"revision":editor.revision,"section":request.section,"total":total,"offset":request.offset,"entries":entries,"next_offset":(next<total).then_some(next),"context":book.context,"declarations_only":!matches!(request.section,RulesSection::Checks),"compliance_verdict":"not_evaluated","original_bytes":"verify through the bound repository","coverage_is_exhaustive":false,"coverage":book.coverage(&editor.project)}),
        )
    }
    pub fn source_evidence(
        &self,
        editor: &Editor,
        request: &EvidenceRequest,
    ) -> Result<Value, ReadError> {
        self.check(editor, &request.target)?;
        if request.expected_revision != editor.revision {
            return Err(fail("REVISION_CONFLICT"));
        }
        if request.object_targets.len() > 16
            || (!request.topics && !request.object_targets.is_empty())
            || request.object_targets.iter().any(|t| t.validate().is_err())
            || request
                .object_targets
                .iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                != request.object_targets.len()
        {
            return Err(fail("INVALID_OBJECT_TARGETS"));
        }
        if request.topics {
            let topics = editor
                .project
                .evidence
                .topics
                .iter()
                .filter(|t| {
                    request.object_targets.is_empty()
                        || t.related_to(&editor.project, &request.object_targets)
                })
                .collect::<Vec<_>>();
            if !(1..=10).contains(&request.limit) || request.offset > topics.len() {
                return Err(fail("INVALID_LIMIT"));
            }
            let mut entries = vec![];
            for topic in topics.iter().skip(request.offset).take(request.limit) {
                let entry = topic.view(&editor.project);
                let mut candidate = entries.clone();
                candidate.push(entry.clone());
                if serde_json::to_vec(&candidate)
                    .map_err(|_| fail("INVALID_QUERY_RESULT"))?
                    .len()
                    > 900 * 1024
                {
                    if entries.is_empty() {
                        return Err(fail("RESULT_TOO_LARGE"));
                    }
                    break;
                }
                entries.push(entry);
            }
            let next = request.offset + entries.len();
            return Ok(
                json!({"target":request.target,"revision":editor.revision,"topics":entries,"total":topics.len(),"offset":request.offset,"next_offset":(next<topics.len()).then_some(next),"object_targets":request.object_targets,"relation_basis":"recorded object IDs, explicit mass-space associations and task result mappings; not write authority","external_agent_dispatch":"not_connected","object_task_handoff":"explicit connected-client pull; inspect assignment task_status and native scope","read_only":true}),
            );
        }
        let notes = &editor.project.evidence.notes;
        if !(1..=10).contains(&request.limit) || request.offset > notes.len() {
            return Err(fail("INVALID_LIMIT"));
        }
        let end = (request.offset + request.limit).min(notes.len());
        let page = crate::evidence::EvidenceRegister {
            notes: notes[request.offset..end].to_vec(),
            topics: vec![],
        };
        let status = page.status(&editor.project);
        let mut entries = vec![];
        let mut bytes = 0usize;
        for (index, note) in page.notes.iter().enumerate() {
            let entry = json!({"note":note,"links":status["notes"][index]["links"]});
            let size = serde_json::to_vec(&entry)
                .map_err(|_| fail("INVALID_QUERY_RESULT"))?
                .len();
            if size > 900 * 1024 {
                return Err(fail("RESULT_TOO_LARGE"));
            }
            if bytes + size > 900 * 1024 {
                break;
            }
            bytes += size;
            entries.push(entry);
        }
        let next = request.offset + entries.len();
        Ok(
            json!({"target":request.target,"revision":editor.revision,"total":notes.len(),"offset":request.offset,"notes":entries,"next_offset":(next<notes.len()).then_some(next),"source_verification":"unverified declarations; repository text match is not authority or approval"}),
        )
    }
    pub fn validate(&self, editor: &Editor, query: &Validate) -> Result<Value, ReadError> {
        self.check(editor, &query.target)?;
        if query.expected_revision != editor.revision {
            return Err(fail("REVISION_CONFLICT"));
        }
        if !(1..=100).contains(&query.limit) {
            return Err(fail("INVALID_LIMIT"));
        }
        let report = editor.geometry_report();
        let fingerprint = crate::automation_journal::canonical_hash(&(
            &query.target,
            query.expected_revision,
            query.limit,
            &report,
        ))
        .map_err(|_| fail("INVALID_QUERY_RESULT"))?;
        let offset = if let Some(cursor) = &query.cursor {
            let (hash, offset) = cursor
                .split_once(':')
                .ok_or_else(|| fail("INVALID_CURSOR"))?;
            if hash != fingerprint {
                return Err(fail("INVALID_CURSOR"));
            }
            offset
                .parse::<usize>()
                .map_err(|_| fail("INVALID_CURSOR"))?
        } else {
            0
        };
        if offset > report.diagnostics.len() {
            return Err(fail("INVALID_CURSOR"));
        }
        let errors = report
            .diagnostics
            .iter()
            .filter(|d| d.severity == crate::derived_geometry::DiagnosticSeverity::Error)
            .count();
        let warnings = report.diagnostics.len() - errors;
        let mut diagnostics = Vec::new();
        let mut bytes = 0;
        for d in report.diagnostics.iter().skip(offset).take(query.limit) {
            let size = serde_json::to_vec(d)
                .map_err(|_| fail("INVALID_QUERY_RESULT"))?
                .len();
            if size > 48 * 1024 {
                return Err(fail("RESOURCE_REQUIRED"));
            }
            if size + bytes > 48 * 1024 {
                break;
            }
            bytes += size;
            diagnostics.push(d);
        }
        let end = offset + diagnostics.len();
        Ok(
            json!({"target":query.target,"revision":editor.revision,"scope":"whole_project","input_checked":report.input_checked,"valid":errors==0 && report.input_checked,"inspected_elements":report.inspected_elements,"diagnostic_count":report.diagnostics.len(),"error_count":errors,"warning_count":warnings,"explicit_empty_material_count":report.explicit_empty_material.len(),"diagnostics":diagnostics,"next_cursor":(end<report.diagnostics.len()).then(||format!("{fingerprint}:{end}"))}),
        )
    }
    pub fn terrain_height(
        &self,
        editor: &Editor,
        request: &TerrainRequest,
    ) -> Result<Value, ReadError> {
        self.check(editor, &request.target)?;
        if editor.revision != request.expected_revision {
            return Err(fail("REVISION_CONFLICT"));
        }
        if request
            .xy_m
            .iter()
            .any(|v| !v.is_finite() || v.abs() > 100000.)
        {
            return Err(fail("INVALID_TERRAIN_QUERY"));
        }
        let sample = match &editor.project.site.terrain {
            None => {
                json!({"status":"unknown","reason":"No terrain surface; flat ground is not inferred"})
            }
            Some(t) => {
                t.validate().map_err(|_| fail("INVALID_TERRAIN"))?;
                match t
                    .geometry
                    .height_at(request.xy_m)
                    .map_err(|_| fail("INVALID_TERRAIN_QUERY"))?
                {
                    None => {
                        json!({"status":"unknown","reason":"Point is outside terrain triangle coverage or inside a hole"})
                    }
                    Some((height, triangle)) => {
                        json!({"status":"declared_unverified","elevation_model_m":height,"triangle_index":triangle,"source_sha256":t.origin.source_sha256,"reference_current":t.reference_current(&editor.project.site),"geometry_user_modified":t.user_modified()})
                    }
                }
            }
        };
        Ok(
            json!({"target":request.target,"revision":editor.revision,"xy_m":request.xy_m,"sample":sample}),
        )
    }
    pub fn analyze_spaces(
        &self,
        editor: &Editor,
        request: &SpaceRequest,
    ) -> Result<Value, ReadError> {
        self.check(editor, &request.target)?;
        if editor.revision != request.expected_revision {
            return Err(fail("REVISION_CONFLICT"));
        }
        let report = crate::zones::analysis::analyze(&editor.project, &request.options).map_err(
            |reason| {
                fail(match reason.as_str() {
                    "Space analysis design option does not exist" => "INVALID_DESIGN_OPTION_SCOPE",
                    "Option-scoped wall joins are not supported by this analysis" => {
                        "UNSUPPORTED_OPTION_WALL_CONTEXT"
                    }
                    _ => "INVALID_SPACE_ANALYSIS",
                })
            },
        )?;
        Ok(
            json!({"target":request.target,"revision":editor.revision,"units":{"length":"m","area":"m2","volume":"m3"},"analysis":report}),
        )
    }
    pub fn query(&self, editor: &Editor, query: &Query) -> Result<Value, ReadError> {
        self.check(editor, &query.target)?;
        use crate::view_settings::DesignOptionScope;
        if let DesignOptionScope::Option { id } = query.design_scope {
            if editor.project.design_options.name(id).is_none() {
                return Err(fail("INVALID_DESIGN_OPTION_SCOPE"));
            }
        }
        if !matches!(query.design_scope, DesignOptionScope::All {})
            && query
                .fields
                .iter()
                .any(|f| ["bounds", "quantities", "validation"].contains(&f.as_str()))
            && editor.project.elements.iter().any(|e| {
                e.design_option.is_some()
                    && matches!(e.shape, crate::model::Shape::Wall { .. })
                    && query.story.is_none_or(|s| e.story == s)
            })
        {
            return Err(fail("UNSUPPORTED_OPTION_WALL_CONTEXT"));
        }

        if query.expected_revision != editor.revision {
            return Err(fail("REVISION_CONFLICT"));
        }
        if !(1..=100).contains(&query.limit) {
            return Err(fail("INVALID_LIMIT"));
        }
        if query.ids.contains(&0) {
            return Err(fail("INVALID_ELEMENT_ID"));
        }
        if query.region.as_ref().is_some_and(|r| {
            (0..3).any(|i| !r.min[i].is_finite() || !r.max[i].is_finite() || r.min[i] > r.max[i])
        }) {
            return Err(fail("INVALID_REGION"));
        }
        if query.fields.is_empty()
            || query.fields.iter().any(|f| {
                ![
                    "id",
                    "kind",
                    "story",
                    "name",
                    "shape",
                    "slab_contour",
                    "wall_contour",
                    "opening_joinery",
                    "design_option",
                    "layer",
                    "material",
                    "bounds",
                    "quantities",
                    "validation",
                ]
                .contains(&f.as_str())
            })
        {
            return Err(fail("INVALID_PROJECTION"));
        }
        if query.kind.as_ref().is_some_and(|k| {
            ![
                "Concept mass",
                "Wall",
                "Slab",
                "Column",
                "Beam",
                "Roof",
                "Stair",
                "Polyline",
                "Zone",
                "Door",
                "Window",
            ]
            .contains(&k.as_str())
        }) {
            return Err(fail("INVALID_KIND"));
        }
        let binding = serde_json::to_vec(&(
            &query.target,
            query.expected_revision,
            &query.ids,
            &query.design_scope,
            &query.kind,
            query.story,
            &query.region,
            &query.fields,
            query.limit,
        ))
        .map_err(|_| fail("INVALID_QUERY"))?;
        let fingerprint = format!("{:x}", Sha256::digest(binding));
        let after = if let Some(cursor) = &query.cursor {
            let (hash, id) = cursor
                .split_once(':')
                .ok_or_else(|| fail("INVALID_CURSOR"))?;
            if hash != fingerprint {
                return Err(fail("INVALID_CURSOR"));
            }
            Some(id.parse::<u64>().map_err(|_| fail("INVALID_CURSOR"))?)
        } else {
            None
        };
        let mut elements: Vec<_> = editor
            .project
            .elements
            .iter()
            .filter(|e| {
                query.design_scope.contains(e.design_option)
                    && (query.ids.is_empty() || query.ids.contains(&e.id))
                    && query.kind.as_ref().is_none_or(|k| k == e.kind())
                    && query.story.is_none_or(|s| s == e.story)
            })
            .collect();
        elements.sort_by_key(|e| e.id);
        if let Some(region) = &query.region {
            let ids: Vec<_> = elements.iter().map(|e| e.id).collect();
            let batch = editor.geometry_batch(&ids, None).map_err(|d| ReadError {
                code: "GEOMETRY_QUERY_FAILED",
                diagnostics: vec![d],
            })?;
            let mut matched = Vec::new();
            for e in elements {
                let handle = batch.source(e.id).map_err(|d| ReadError {
                    code: "GEOMETRY_QUERY_FAILED",
                    diagnostics: vec![d],
                })?;
                if handle.source.display_bounds().is_some_and(|[lo, hi]| {
                    (0..3).all(|i| lo[i] <= region.max[i] && hi[i] >= region.min[i])
                }) {
                    matched.push(e);
                }
            }
            elements = matched;
        }
        let matched_count = elements.len();
        let mut matched_by_kind = std::collections::BTreeMap::new();
        for e in &elements {
            *matched_by_kind.entry(e.kind()).or_insert(0usize) += 1;
        }
        elements.retain(|e| after.is_none_or(|id| e.id > id));
        let geometry = query
            .fields
            .iter()
            .any(|f| ["bounds", "quantities", "validation"].contains(&f.as_str()));
        let batch = if geometry {
            Some(
                editor
                    .geometry_batch(
                        &elements
                            .iter()
                            .take(query.limit)
                            .map(|e| e.id)
                            .collect::<Vec<_>>(),
                        None,
                    )
                    .map_err(|_| fail("GEOMETRY_READ_BUSY"))?,
            )
        } else {
            None
        };
        let mut rows = Vec::new();
        let mut bytes = 0;
        let mut last = None;
        let quantities_requested = query.fields.iter().any(|f| f == "quantities");
        let mut material_volume = 0.;
        let mut void_volume = 0.;
        let mut quantity_errors = Vec::new();
        for e in elements.iter().take(query.limit) {
            let mut row = serde_json::Map::new();
            for field in &query.fields {
                row.insert(
                    field.clone(),
                    match field.as_str() {
                        "id" => json!(e.id),
                        "kind" => json!(e.kind()),
                        "story" => json!(e.story),
                        "name" => json!(e.name),
                        "shape" => json!(e.shape),
                        "slab_contour" => json!(e.slab_contour),
                        "wall_contour" => json!(e.wall_contour),
                        "opening_joinery" => if matches!(e.shape,crate::model::Shape::Opening{..}) {json!(e.opening_joinery.unwrap_or_default())} else {Value::Null},
                        "design_option" => json!(e.design_option),
                        "layer" => json!(e.layer),
                        "material" => json!(e.material),
                        "bounds" | "quantities" | "validation" => {
                            let source = batch.as_ref().unwrap().source(e.id);
                            match source {
                                Ok(handle) => {
                                    let source = handle.source.as_ref();
                                    match field.as_str() {
                                        "bounds" => source.display_bounds().map_or(Value::Null, |[lo,hi]| json!({"min":lo.to_array(),"max":hi.to_array(),"unit":"m","space":"project"})),
                                        "quantities" => json!({"nominal":source.nominal_measure(),"material_volume_m3":source.material_volume_m3(),"void_volume_m3":source.void_volume_m3(),"concept_volume_m3":source.concept_volume_m3(),"space_volume_m3":if let crate::derivation_cache::GeometrySource::Zone(g)=source {Some(g.volume_m3)}else{None}}),
                                        _ => json!({"scope":"derived_source","status":"valid","diagnostics":[]}),
                                    }
                                }
                                Err(diagnostic) => json!({"scope":"derived_source","status":"invalid","diagnostics":[diagnostic]}),
                            }
                        }
                        _ => unreachable!(),
                    },
                );
            }
            let row = Value::Object(row);
            let size = serde_json::to_vec(&row)
                .map_err(|_| fail("INVALID_QUERY_RESULT"))?
                .len();
            if size > 48 * 1024 {
                return Err(fail("RESOURCE_REQUIRED"));
            }
            if bytes + size > 48 * 1024 {
                break;
            }
            bytes += size;
            if quantities_requested {
                if row["quantities"]["status"] == "invalid" {
                    quantity_errors.push(e.id);
                } else {
                    material_volume += row["quantities"]["material_volume_m3"]
                        .as_f64()
                        .unwrap_or(0.);
                    void_volume += row["quantities"]["void_volume_m3"].as_f64().unwrap_or(0.);
                }
            }
            rows.push(row);
            last = Some(e.id);
        }
        let cursor = if elements.len() > rows.len() {
            last.map(|id| format!("{fingerprint}:{id}"))
        } else {
            None
        };
        Ok(
            json!({"target":query.target,"revision":editor.revision,"design_scope":query.design_scope,"scope_warning":matches!(query.design_scope,DesignOptionScope::All {}).then_some("All alternatives; not a candidate comparison"),"elements":rows,"next_cursor":cursor,"summary":{"scope":"all_matching_elements","count":matched_count,"by_kind":matched_by_kind},"page_quantities":quantities_requested.then(|| json!({"scope":"returned_page","material_volume_m3":material_volume,"void_volume_m3":void_volume,"failed_ids":quantity_errors,"complete":quantity_errors.is_empty()}))}),
        )
    }
}
