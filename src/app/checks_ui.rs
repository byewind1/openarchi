use super::*;
use crate::command::ModelCommand;
use crate::{
    application_host::CommandContext,
    constraints::{self, Basis, Check, Definition, Domain, Edit, Metric, Relation, Scope, State},
};

pub(super) struct ChecksJob {
    pub base: CommandContext,
    pub request_id: String,
    pub commands: Vec<ModelCommand>,
    pub commit: bool,
    pub admission: Result<crate::project_repository::constraints::ConstraintCommitGuard, String>,
}
#[derive(Clone, PartialEq)]
pub(super) struct ChecksReadKey {
    base: CommandContext,
    sources: Option<(PathBuf, String)>,
}
pub(super) struct ChecksReadJob {
    pub key: ChecksReadKey,
    pub entries: Vec<serde_json::Value>,
    pub plan: constraints::originals::ReadPlan,
}
pub(super) struct ChecksReadResult {
    pub key: ChecksReadKey,
    pub entries: Vec<serde_json::Value>,
    pub checked_at: std::time::Instant,
}
pub(super) struct CheckDraft {
    base: CommandContext,
    definition: Definition,
    expected: Option<String>,
    limit: String,
}
#[derive(Default)]
pub(super) struct ChecksUi {
    pub open: bool,
    pub draft: Option<CheckDraft>,
    pub(super) error: String,
    session: Option<u64>,
    pub(super) pending: Option<(String, Option<String>)>,
    last_open: bool,
    last_read: Option<ChecksReadKey>,
    pub(super) read_pending: Option<ChecksReadKey>,
    pub(super) read_error: String,
    read_result: Option<ChecksReadResult>,
    pub(super) focus: Option<String>,
}
impl ChecksUi {
    pub(super) fn open_focused(&mut self, session: u64, focus: Option<String>) {
        if self.session != Some(session) {
            *self = Self {
                session: Some(session),
                ..Default::default()
            };
        }
        self.focus = focus;
        self.open = true;
    }
}
fn field(ui: &mut Ui, l: UiLanguage, key: &str, value: &mut String) {
    ui.horizontal(|ui| {
        let label = ui.label(l.translate(key));
        let response = ui.text_edit_singleline(value).labelled_by(label.id);
        #[cfg(test)]
        ui.ctx()
            .data_mut(|data| data.insert_temp(Id::new(("check_field", key)), response.rect));
        let _ = response;
    });
}
fn edge_label(outline: &[[f64; 2]], edge: u32, l: UiLanguage) -> String {
    let Some([a, b]) = constraints::reference_edge_endpoints(outline, edge) else {
        return l.translate("Reference edge is missing").into();
    };
    format!(
        "{} {}: ({:.6}, {:.6}) → ({:.6}, {:.6}) m",
        l.translate("Reference edge"),
        u64::from(edge) + 1,
        a[0],
        a[1],
        b[0],
        b[1]
    )
}
// Present only this verified source-blob diagnostic as a localized message.
// Keep the exact error in state/receipts and on hover; other resource failures
// (including stored model files) must not be relabelled as changed originals.
pub(super) fn original_read_error_label(language: UiLanguage, message: &str) -> &str {
    language.translate(
        if message.starts_with("Repository resource changed: sources/") {
            "Stored original bytes changed"
        } else {
            message
        },
    )
}
fn details(
    ui: &mut Ui,
    p: &Project,
    c: &Check,
    l: UiLanguage,
    view: Option<(&serde_json::Value, u64)>,
) {
    ui.label(format!(
        "{} · {}",
        l.translate(c.definition.domain.label()),
        l.translate(c.definition.metric.label())
    ));
    if c.definition.metric == Metric::ReferenceControlRegionClearance {
        if let Some(region) = &p.site.control_region {
            ui.label(format!(
                "{}: {} (#{} )",
                l.translate("Declared control region"),
                region.name,
                region.element_id
            ));
            ui.label(format!(
                "{}: {} · {} · {}",
                l.translate("Source"),
                &region.origin.source_sha256[..12],
                region.origin.entity_handle,
                region.origin.unit.symbol()
            ));
            ui.label(l.translate("A declared control region is not an approved building line."));
        } else {
            ui.label(l.translate(
                "Declared control region is missing; no approved control line is inferred",
            ));
        }
    }
    if let Metric::ReferenceBoundaryEdgeClearance { edge } = c.definition.metric {
        let outline = crate::site::boundary::outline(p)
            .ok()
            .flatten()
            .unwrap_or_default();
        ui.label(edge_label(&outline, edge, l));
        ui.label(l.translate("Current edge endpoints; changed boundaries make old values stale."));
        ui.label(l.translate("Convex reference only; signed distance to the supporting line, negative outside. No compass direction or approved setback is inferred."));
    }
    ui.label(format!(
        "{} {} {}",
        l.translate(c.definition.relation.label()),
        c.definition
            .limit
            .map_or_else(|| l.translate("Unknown").into(), |v| format!("{v}")),
        c.definition.metric.unit()
    ));
    let fresh = c.freshness(p);
    ui.strong(l.translate(fresh));
    let verified = view.is_some_and(|(e, _)| e["original_verification"]["status"] == "verified");
    let state = if matches!(c.definition.basis, Basis::CitedRequirement { .. })
        && verified
        && fresh == "Check inputs unchanged"
    {
        view.and_then(|(e, _)| serde_json::from_value::<State>(e["state"].clone()).ok())
            .unwrap_or(State::Unknown)
    } else {
        c.state(p)
    };
    // A source read is a time-bound observation, never a permanent certification.
    ui.label(format!(
        "{}: {}",
        l.translate(
            if matches!(c.definition.basis, Basis::CitedRequirement { .. }) {
                "Read result"
            } else {
                "Result"
            }
        ),
        l.translate(state.label())
    ));
    if let Some((entry, seconds)) = view {
        ui.label(format!(
            "{} · {seconds} s",
            l.translate("Original verification snapshot")
        ));
        if verified {
            ui.label(l.translate("Exact originals verified at this read"));
        } else {
            ui.label(l.translate("Original verification unavailable"));
            if let Some(reason) = entry["original_verification"]["reason"].as_str() {
                ui.label(original_read_error_label(l, reason))
                    .on_hover_text(reason);
            }
        }
    }

    match &c.definition.basis {
        Basis::DesignAssumption { note } => {
            ui.label(l.translate("Design assumption; not a surveyed or statutory limit"));
            if !note.is_empty() {
                ui.label(note);
            }
        }
        Basis::CitedRequirement { id } => {
            if !verified {
                ui.label(l.translate(
                    "Stored numerical values do not verify current original availability.",
                ));
            }
            if let Some(r) = p.rules.requirements.iter().find(|r| r.id == *id) {
                ui.label(&r.title);
                ui.label(&r.citation.text);
                ui.label(l.translate(p.rules.requirement_state(p, r)));
            } else {
                ui.label(l.translate("Cited requirement is missing"));
            }
            ui.label(l.translate("Original authority is not certified by this numerical check."));
        }
    }
    if let Some(report) = &c.report {
        ui.label(format!(
            "v{} · {}",
            report.generation,
            &report.source_sha256[..12]
        ));
        if fresh != "Check inputs unchanged" {
            ui.label(format!(
                "{}: {} {} {}",
                l.translate("Previous limit"),
                l.translate(report.input_definition.relation.label()),
                report
                    .input_definition
                    .limit
                    .map_or_else(|| l.translate("Unknown").into(), |v| v.to_string()),
                report.input_definition.metric.unit()
            ));
        }
        for row in &report.rows {
            let identity = row
                .ids
                .iter()
                .map(|id| format!("#{id}"))
                .collect::<Vec<_>>()
                .join(" / ");
            let value = row.value.map_or_else(
                || l.translate("Unknown").into(),
                |v| format!("{v:.6} {}", report.input_definition.metric.unit()),
            );
            ui.label(format!(
                "{} · {} · {}",
                identity,
                value,
                l.translate(
                    if matches!(c.definition.basis, Basis::CitedRequirement { .. })
                        && (!verified || fresh != "Check inputs unchanged")
                    {
                        "Stored value; current cited result unknown"
                    } else if fresh == "Check inputs unchanged" {
                        row.state.label()
                    } else {
                        "Previous value; current result unknown"
                    }
                )
            ));
            ui.label(l.translate(&row.reason));
        }
    }
}
impl AtelierApp {
    fn checks_read_key(&self) -> ChecksReadKey {
        ChecksReadKey {
            base: self.host.context(),
            sources: self
                .source_ui
                .repository
                .as_ref()
                .map(|r| (r.root().to_owned(), r.source_context_identity().to_owned())),
        }
    }
    fn refresh_check_originals(&mut self) {
        if self.source_ui.pending_for_ui() || self.checks_ui.pending.is_some() {
            return;
        }
        let key = self.checks_read_key();
        self.checks_ui.last_read = Some(key.clone());
        self.checks_ui.read_result = None;
        self.checks_ui.read_error.clear();
        let p = &self.host.document.editor.project;
        let cited = p
            .rules
            .checks
            .iter()
            .filter(|c| matches!(c.definition.basis, Basis::CitedRequirement { .. }))
            .collect::<Vec<_>>();
        if cited.is_empty() {
            return;
        }
        if self.source_ui.repository.is_none() {
            self.checks_ui.read_error =
                "Connect this project's source folder to verify cited originals".into();
            return;
        }
        let entries = cited.iter().map(|c| constraints::entry(p, c)).collect();
        let plan = constraints::originals::ReadPlan::new(
            self.host.document.editor.project_identity(),
            self.source_ui.repository.as_ref(),
            &p.rules,
            cited.into_iter(),
        );
        self.checks_ui.read_pending = Some(key.clone());
        self.start_source_action(super::project_sources_ui::SourceAction::ReadChecks(
            ChecksReadJob { key, entries, plan },
        ));
        if !self.source_ui.pending_for_ui() {
            self.checks_ui.read_pending = None;
            self.checks_ui.read_error = self.source_ui.error.clone();
        }
    }
    pub(super) fn accept_checks_read(&mut self, result: ChecksReadResult) {
        let expected = self.checks_ui.read_pending.take();
        if !self.checks_ui.open
            || expected.as_ref() != Some(&result.key)
            || result.key != self.checks_read_key()
        {
            return;
        }
        self.checks_ui.read_error.clear();
        self.checks_ui.read_result = Some(result);
    }
    pub(super) fn open_current_design_checks(&mut self) {
        let e = &self.host.document.editor;
        let option = e.project.plan_options.active_design_option;
        // Navigation only: use the existing check scopes, without evaluating
        // or adopting a report. Historical checks remain in the same list.
        let focus = e
            .project
            .rules
            .checks
            .iter()
            .filter(|check| match &check.definition.scope {
                Scope::Candidate {
                    story,
                    option: candidate,
                } => *story == e.story && *candidate == option,
                Scope::Objects { ids } => e.project.elements.iter().any(|object| {
                    ids.contains(&object.id)
                        && object.story == e.story
                        && object.design_option == option
                        && object.worksheet.is_none()
                        && check.definition.domain.accepts(object)
                }),
            })
            .map(|check| check.definition.id.clone())
            .min();
        self.checks_ui.open_focused(e.document_session(), focus);
    }
    fn new_check_draft(&self) -> CheckDraft {
        let e = &self.host.document.editor;
        let selected = e.selection_ids();
        let domain = selected
            .first()
            .and_then(|id| e.project.elements.iter().find(|e| e.id == *id))
            .map_or(Domain::ConceptMasses, |e| {
                if matches!(e.shape, Shape::Zone { .. }) {
                    Domain::Zones
                } else {
                    Domain::ConceptMasses
                }
            });
        CheckDraft {
            base: self.host.context(),
            expected: None,
            limit: String::new(),
            definition: Definition {
                id: storage::new_document_identity(),
                title: self
                    .language
                    .translate("Quantified design condition")
                    .into(),
                domain,
                scope: if selected.is_empty() {
                    Scope::Candidate {
                        story: e.story,
                        option: e.project.plan_options.active_design_option,
                    }
                } else {
                    Scope::Objects { ids: selected }
                },
                metric: Metric::Height,
                relation: Relation::AtMost,
                limit: None,
                basis: Basis::DesignAssumption {
                    note: String::new(),
                },
            },
        }
    }
    fn check_draft_stamp(&self) -> Option<String> {
        self.checks_ui.draft.as_ref().and_then(|d| {
            crate::automation_journal::canonical_hash(&(
                d.base.session,
                d.base.revision,
                &d.definition,
                &d.expected,
                &d.limit,
            ))
            .ok()
        })
    }
    pub(super) fn start_checks_admission(
        &mut self,
        base: CommandContext,
        request_id: String,
        commands: Vec<ModelCommand>,
        commit: bool,
    ) -> Result<(), String> {
        if self.source_ui.pending_for_ui() {
            return Err("A source read is already running".into());
        }
        if self.source_ui.repository.is_none() {
            return Err("Cited originals unavailable; save the definition without running or reconnect project sources".into());
        }
        self.checks_ui.pending = Some((request_id.clone(), self.check_draft_stamp()));
        self.start_source_action(super::project_sources_ui::SourceAction::CheckOriginals {
            commands,
            base,
            request_id,
            commit,
            book: self.host.document.editor.project.rules.clone(),
        });
        if !self.source_ui.pending_for_ui() {
            self.checks_ui.pending = None;
            return Err(self.source_ui.error.clone());
        }
        Ok(())
    }
    pub(super) fn invalidate_checks_original_read(&mut self) {
        // A known failed admission invalidates the earlier read even if the
        // repository catalogue identity has not changed. Keep last_read so this
        // negative observation cannot start an automatic retry loop.
        self.checks_ui.read_result = None;
        self.checks_ui.read_pending = None;
    }
    pub(super) fn accept_checks_job(&mut self, job: ChecksJob) {
        let pending = self.checks_ui.pending.take();
        if job.base != self.host.context()
            || pending.as_ref().is_none_or(|(id, stamp)| {
                id != &job.request_id || *stamp != self.check_draft_stamp()
            })
        {
            self.checks_ui.error =
                "Check context changed; cancel the draft and use the current design".into();
            return;
        }
        let admission = match job.admission {
            Ok(g) => g,
            Err(e) => {
                self.invalidate_checks_original_read();
                if job.commit {
                    self.process_ui.error = e.clone();
                }
                self.checks_ui.error = e;
                return;
            }
        };
        let prepared = match self.host.document.editor.prepare_constraint_change(
            job.base.revision,
            job.commands,
            &admission,
        ) {
            Ok(p) => p,
            Err(e) => {
                self.checks_ui.error = e.message;
                return;
            }
        };
        if job.commit {
            let Some(reviewed) = &mut self.process_ui.preview else {
                return;
            };
            if reviewed.id != job.request_id
                || reviewed.context != job.base
                || reviewed.change.commands() != prepared.commands()
                || !reviewed
                    .change
                    .project()
                    .same_model_state(prepared.project())
                || reviewed.sent_target.is_some()
            {
                self.checks_ui.error =
                    "Check context changed; cancel the draft and use the current design".into();
                return;
            }
            reviewed.change = std::rc::Rc::new(prepared);
            self.process_ui.error.clear();
            if let Err(e) = self.process_commit_proposal() {
                self.process_ui.error = e;
            }
        } else {
            if self.process_ui.preview.is_some() {
                self.checks_ui.error = "Finish or cancel the current design preview".into();
                return;
            }
            self.process_set_preview(job.request_id, "Edit quantified checks", Ok(prepared));
        }
        self.checks_ui.error.clear();
        // admission and its repository lock drop here. A displayed preview keeps
        // only a weak lease and cannot be committed without fresh source IO.
    }
    fn preview_check_commands(&mut self, base: CommandContext, commands: Vec<ModelCommand>) {
        if base != self.host.context() || self.workbench_pointer_active() || self.menu_gated() {
            self.checks_ui.error =
                "Check context changed; cancel the draft and use the current design".into();
            return;
        }
        match crate::project_repository::constraints::cited_run_citations(
            &self.host.document.editor.project.rules,
            &commands,
        ) {
            Ok(citations) if !citations.is_empty() => {
                if let Err(error) = self.start_checks_admission(
                    base,
                    storage::new_document_identity(),
                    commands,
                    false,
                ) {
                    self.checks_ui.error = error;
                }
                return;
            }
            Err(error) => {
                self.checks_ui.error = error;
                return;
            }
            _ => {}
        }
        let result = self
            .host
            .document
            .editor
            .prepare_change(base.revision, commands)
            .map_err(|e| e.message);
        if let Err(error) = &result {
            self.checks_ui.error = error.clone();
            return;
        }
        self.checks_ui.error.clear();
        self.process_set_preview(
            storage::new_document_identity(),
            "Edit quantified checks",
            result,
        );
    }
    pub(super) fn check_preview_contents(
        ui: &mut Ui,
        before: &Project,
        after: &Project,
        l: UiLanguage,
    ) {
        ui.heading(l.translate("Review quantified design checks"));
        ui.label(l.translate(
            "Numerical checks do not approve the project or waive missing mandatory conditions.",
        ));
        ScrollArea::vertical()
            .id_salt("check_result_review")
            .max_height(300.)
            .show(ui, |ui| {
                for check in &after.rules.checks {
                    if !before.rules.checks.contains(check) {
                        ui.strong(&check.definition.title);
                        details(ui, after, check, l, None);
                        ui.separator();
                    }
                }
                for check in &before.rules.checks {
                    if !after
                        .rules
                        .checks
                        .iter()
                        .any(|c| c.definition.id == check.definition.id)
                    {
                        ui.label(format!(
                            "{}: {}",
                            l.translate("Remove check"),
                            check.definition.title
                        ));
                    }
                }
            });
    }
    pub(super) fn checks_window(&mut self, ctx: &Context) {
        let session = self.host.document.editor.document_session();
        if self.checks_ui.session != Some(session) {
            // Never carry a previous document's draft into a new file.
            self.checks_ui = ChecksUi {
                open: self.checks_ui.open,
                session: Some(session),
                ..Default::default()
            };
        }
        if !self.checks_ui.open {
            self.checks_ui.last_open = false;
            return;
        }
        if !self.checks_ui.last_open {
            self.checks_ui.last_open = true;
            self.checks_ui.last_read = None;
            self.checks_ui.read_result = None;
            if self.checks_ui.focus.is_none() {
                self.checks_ui.focus = self
                    .host
                    .document
                    .editor
                    .project
                    .rules
                    .checks
                    .iter()
                    .find(|c| matches!(c.definition.basis, Basis::CitedRequirement { .. }))
                    .map(|c| c.definition.id.clone());
            }
        }
        if self.checks_ui.read_result.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_secs(1));
        }
        let key = self.checks_read_key();
        if self
            .checks_ui
            .read_result
            .as_ref()
            .is_some_and(|r| r.key != key)
        {
            self.checks_ui.read_result = None;
        }
        if self.checks_ui.last_read.as_ref() != Some(&key)
            && self.checks_ui.draft.is_none()
            && self.process_ui.preview.is_none()
        {
            self.refresh_check_originals();
        }
        let reading = self.checks_ui.read_pending.is_some();
        let checking = self.checks_ui.pending.is_some();
        let l = self.language;
        let has_constraint_figure = self.current_constraint_figure_id().is_some();
        let figure_label = if has_constraint_figure {
            "Show constraint figure"
        } else {
            "Preview constraint figure"
        };
        let figure_enabled = self.checks_ui.draft.is_none()
            && !self.menu_gated()
            && !self.workbench_pointer_active()
            && !self.pending_design_parameters();
        let mut show_figure = false;
        let mut compare = false;
        let p = &self.host.document.editor.project;
        let base = self.host.context();
        let checks = p.rules.checks.clone();
        let mut open = true;
        let mut new = false;
        let mut cancel = false;
        let mut preview = false;
        let mut definition_only = false;
        let mut edit = None;
        let mut locate = None;
        let mut citation = None;
        let mut editing = None;
        let mut refresh = false;
        egui::Window::new(l.translate("Design condition checks")).id(Id::new("quantified_checks")).open(&mut open).default_width(740.).default_height(650.).resizable(true).show(ctx,|ui|{
            ui.label(l.translate("Numerical checks do not approve the project or waive missing mandatory conditions."));
            ui.label(l.translate("Parameter heights and authored footprints are not statutory building height or floor area. Overlaps are counted separately."));
            if ui.add_enabled(figure_enabled && (!checks.is_empty() || has_constraint_figure),Button::new(l.translate(figure_label))).clicked(){show_figure=true;}
            if p.design_process.task().is_some() {
                let response=ui.add_enabled(figure_enabled,Button::new(l.translate("Compare options…")));
                #[cfg(test)]ctx.data_mut(|data|data.insert_temp(Id::new("checks_compare_button"),response.rect));
                if response.clicked(){compare=true;}
            }

            if checks.iter().any(|c|matches!(c.definition.basis,Basis::CitedRequirement{..})) {
                if ui.add_enabled(!reading&&!self.source_ui.pending_for_ui(),Button::new(l.translate("Refresh cited originals"))).clicked(){refresh=true;}
                ui.label(l.translate("Source snapshots apply to this read; refresh or confirm to check originals again."));
                if reading{ui.label(l.translate("Reading cited originals…"));}
                if !self.checks_ui.read_error.is_empty(){ui.colored_label(Color32::LIGHT_RED,l.translate(&self.checks_ui.read_error));}
            }
            if let Some(d)=&mut self.checks_ui.draft {
                ui.strong(l.translate("Check the current selection or candidate"));
                field(ui,l,"Check name",&mut d.definition.title);
                let previous_domain = d.definition.domain;
                ui.horizontal_wrapped(|ui|{for domain in [Domain::ConceptMasses,Domain::Zones]{ui.selectable_value(&mut d.definition.domain,domain,l.translate(domain.label()));}});
                match &d.definition.scope {
                    Scope::Objects{ids}=>{ui.label(format!("{}: {}",l.translate("Selected check objects"),ids.iter().map(|id|format!("#{id}")).collect::<Vec<_>>().join(" / ")));},
                    Scope::Candidate{story,option}=>{let story=p.stories.iter().find(|s|s.id==*story).map_or(l.translate("Unknown"),|s|s.name.as_str());let option=option.and_then(|id|p.design_options.name(id)).unwrap_or(l.translate("Main model"));ui.label(format!("{}: {} · {}",l.translate("Exact candidate and story"),option,story));}
                }
                let previous_metric = d.definition.metric;
                let metric_response = egui::ComboBox::from_id_salt("check_metric").selected_text(l.translate(d.definition.metric.label())).show_ui(ui,|ui|{for mut metric in Metric::ALL{if matches!(metric,Metric::ReferenceBoundaryEdgeClearance{..}) && matches!(d.definition.metric,Metric::ReferenceBoundaryEdgeClearance{..}){metric=d.definition.metric;}ui.selectable_value(&mut d.definition.metric,metric,l.translate(metric.label()));}});
                #[cfg(test)] ui.ctx().data_mut(|data|data.insert_temp(Id::new("check_metric_control"),metric_response.response.rect));
                let _ = metric_response;
                if let Metric::ReferenceBoundaryEdgeClearance{edge}=&mut d.definition.metric {
                    let outline=crate::site::boundary::outline(p).ok().flatten().unwrap_or_default();
                    let response=egui::ComboBox::from_id_salt("check_reference_edge").selected_text(edge_label(&outline,*edge,l)).show_ui(ui,|ui|{for i in 0..outline.len(){ui.selectable_value(edge,i as u32,edge_label(&outline,i as u32,l));}});
                    #[cfg(test)] ui.ctx().data_mut(|data|data.insert_temp(Id::new("check_edge_control"),response.response.rect));
                    let _=response;
                    ui.label(l.translate("Convex reference only; signed distance to the supporting line, negative outside. No compass direction or approved setback is inferred."));
                }
                if previous_metric != d.definition.metric || previous_domain != d.definition.domain { d.limit.clear(); d.definition.limit = None; }
                if previous_metric != d.definition.metric && d.definition.metric.uses_boundary() { d.definition.relation = Relation::AtLeast; }
                ui.horizontal(|ui|{for relation in [Relation::AtMost,Relation::AtLeast]{ui.selectable_value(&mut d.definition.relation,relation,l.translate(relation.label()));}});
                field(ui,l,"Limit (blank means unknown)",&mut d.limit);ui.label(d.definition.metric.unit());
                let selected=match &d.definition.basis{Basis::DesignAssumption{..}=>l.translate("Design assumption"),Basis::CitedRequirement{id}=>p.rules.requirements.iter().find(|r|r.id==*id).map_or(id.as_str(),|r|r.title.as_str())};
                egui::ComboBox::from_id_salt("check_basis").selected_text(selected).show_ui(ui,|ui|{
                    if ui.selectable_label(matches!(d.definition.basis,Basis::DesignAssumption{..}),l.translate("Design assumption")).clicked(){d.definition.basis=Basis::DesignAssumption{note:String::new()};}
                    for requirement in &p.rules.requirements {if ui.selectable_label(matches!(&d.definition.basis,Basis::CitedRequirement{id} if *id==requirement.id),&requirement.title).clicked(){d.definition.basis=Basis::CitedRequirement{id:requirement.id.clone()};}}
                });
                if let Basis::DesignAssumption{note}=&mut d.definition.basis{field(ui,l,"Optional assumption note",note);}
                if d.base!=base{ui.colored_label(Color32::LIGHT_RED,l.translate("Check context changed; cancel the draft and use the current design"));}
                ui.horizontal(|ui|{if ui.add_enabled(d.base==base&&!checking&&(!matches!(d.definition.basis,Basis::CitedRequirement{..})||!self.source_ui.pending_for_ui()),Button::new(l.translate("Preview quantified check"))).clicked(){preview=true;}if matches!(d.definition.basis,Basis::CitedRequirement{..}) && ui.add_enabled(d.base==base&&!checking,Button::new(l.translate("Preview definition without running"))).clicked(){definition_only=true;}if ui.button(l.translate("Cancel check draft")).clicked(){cancel=true;}});
            }else if ui.button(l.translate("Add check for current selection or candidate")).clicked(){new=true;}
            if checking{ui.label(l.translate("Verifying cited originals…"));}
            if !self.checks_ui.error.is_empty(){ui.colored_label(Color32::LIGHT_RED,original_read_error_label(l,&self.checks_ui.error)).on_hover_text(&self.checks_ui.error);}
            ScrollArea::vertical().id_salt("design_checks_list").max_height(380.).show(ui,|ui|{
                for check in &checks {
                    ui.group(|ui|{
                        let title=ui.strong(&check.definition.title);
                        if self.checks_ui.focus.as_deref()==Some(check.definition.id.as_str()){title.scroll_to_me(Some(egui::Align::Min));self.checks_ui.focus=None;}
                        ui.add_enabled_ui(self.checks_ui.draft.is_none(),|ui|{
                            ui.horizontal_wrapped(|ui|{
                                let run=ui.add_enabled(!matches!(check.definition.basis,Basis::CitedRequirement{..})||!self.source_ui.pending_for_ui(),Button::new(l.translate("Preview check update"))).on_disabled_hover_text(l.translate("Source task in progress"));
                                #[cfg(test)] ctx.data_mut(|d|{d.insert_temp(Id::new(("check_run",&check.definition.id)),run.rect);d.insert_temp(Id::new(("check_run_id",&check.definition.id)),run.id);});
                                if run.clicked(){edit=Some(Edit::Run{id:check.definition.id.clone(),expected_sha256:constraints::expected(check)});}
                                if ui.button(l.translate("Edit check")).clicked(){editing=Some(check.clone());}
                                if ui.button(l.translate("Remove check")).clicked(){edit=Some(Edit::Remove{id:check.definition.id.clone(),expected_sha256:constraints::expected(check)});}
                                if let Basis::CitedRequirement{id}=&check.definition.basis {if ui.button(l.translate("Read cited original")).clicked(){citation=Some(id.clone());}}
                            });
                        });
                        let view=self.checks_ui.read_result.as_ref().filter(|r|r.key==key).and_then(|r|r.entries.iter().find(|e|e["definition"]["id"]==check.definition.id && e["original_verification_binding"].as_str()==constraints::originals::binding(&p.rules,check).as_deref()).map(|e|(e,r.checked_at.elapsed().as_secs())));
                        details(ui,p,check,l,view);
                        ui.add_enabled_ui(self.checks_ui.draft.is_none(),|ui|{
                            if let Some(report)=&check.report{for id in report.rows.iter().flat_map(|r|&r.ids).copied().collect::<std::collections::BTreeSet<_>>(){if let Some(e)=p.elements.iter().find(|e|e.id==id){if ui.button(format!("#{} · {}",id,e.name)).clicked(){locate=Some(id);}}}}
                        });
                    });
                }
                if checks.is_empty(){ui.label(l.translate("No design checks yet; modelling remains available."));}
                ui.collapsing(l.translate("Required project conditions remain separate"),|ui|{ui.label(l.translate("A numerical pass does not certify coverage, source authority or legal applicability."));for (topic,coverage) in crate::rules::Topic::ALL.into_iter().zip(p.rules.coverage(p)){ ui.label(format!("{} · {}",l.translate(topic.label()),l.translate(if coverage["status"]=="missing"{"Missing project condition"}else{"Recorded; not verified"}))); }ui.label(l.translate("Project compliance: not evaluated"));});
            });
        });
        self.checks_ui.open = open;
        if !open {
            self.checks_ui.last_open = false;
            self.checks_ui.read_result = None;
        }
        if refresh {
            self.checks_ui.focus = self
                .host
                .document
                .editor
                .project
                .rules
                .checks
                .iter()
                .find(|c| matches!(c.definition.basis, Basis::CitedRequirement { .. }))
                .map(|c| c.definition.id.clone());
            self.refresh_check_originals();
        }
        if compare {
            self.checks_ui.open = false;
            self.open_design_process();
        }
        if show_figure {
            self.show_constraint_figure();
        }
        if cancel {
            self.checks_ui.draft = None;
            self.checks_ui.error.clear();
        }
        if new {
            self.checks_ui.draft = Some(self.new_check_draft());
        }
        if let Some(c) = editing {
            self.checks_ui.draft = Some(CheckDraft {
                base,
                limit: c.definition.limit.map_or(String::new(), |v| v.to_string()),
                expected: Some(constraints::expected(&c)),
                definition: c.definition,
            });
        }
        if preview || definition_only {
            let draft = self.checks_ui.draft.as_mut().unwrap();
            let value = if draft.limit.trim().is_empty() {
                Ok(None)
            } else {
                draft
                    .limit
                    .trim()
                    .parse::<f64>()
                    .map(Some)
                    .map_err(|_| "Enter a finite SI limit or leave it unknown")
            };
            match value {
                Err(e) => self.checks_ui.error = e.into(),
                Ok(value) => {
                    draft.definition.limit = value;
                    let definition = draft.definition.clone();
                    let base = draft.base;
                    let next = Check {
                        definition: definition.clone(),
                        report: self
                            .host
                            .document
                            .editor
                            .project
                            .rules
                            .checks
                            .iter()
                            .find(|c| c.definition.id == definition.id)
                            .and_then(|c| c.report.clone()),
                    };
                    let mut commands = vec![
                        ModelCommand::Constraint {
                            edit: Edit::Put {
                                expected_sha256: draft.expected.clone(),
                                definition: definition.clone(),
                            },
                        },
                        ModelCommand::Constraint {
                            edit: Edit::Run {
                                id: definition.id,
                                expected_sha256: constraints::expected(&next),
                            },
                        },
                    ];
                    if definition_only {
                        commands.pop();
                    }
                    self.preview_check_commands(base, commands);
                }
            }
        }
        if let Some(edit) = edit {
            self.preview_check_commands(base, vec![ModelCommand::Constraint { edit }]);
        }
        if let Some(id) = locate {
            self.focus_mass_space_element(id);
        }
        if let Some(id) = citation {
            let book = self.host.document.editor.project.rules.clone();
            if let Some(requirement) = book.requirements.iter().find(|r| r.id == id) {
                if let Some(action) = self.rules_ui.request_original(&requirement.citation, &book) {
                    self.open_project_conditions(ConditionsGroup::Requirements);
                    self.start_source_action(action);
                }
            }
        }
    }
}

impl AtelierApp {
    pub(super) fn declared_check_preview_contents(
        ui: &mut Ui,
        l: UiLanguage,
        checks: &[constraints::Preview],
        stale: bool,
    ) {
        ui.strong(l.translate("Declared checks on proposed parameters"));
        if stale {
            ui.colored_label(
                Color32::LIGHT_RED,
                l.translate("Model changed; proposed check values are no longer current"),
            );
            return;
        }
        ui.small(l.translate("Read-only numerical preview. Saved reports are unchanged; this is not a compliance verdict."));
        if checks.is_empty() {
            ui.label(l.translate(
                "No stored check covers these changed objects; conflicts remain unchecked",
            ));
            return;
        }
        ScrollArea::vertical()
            .id_salt("proposed_parameter_checks")
            .max_height(180.)
            .show(ui, |ui| {
                for check in checks {
                    ui.strong(&check.definition.title);
                    ui.label(format!(
                        "{} · {}",
                        l.translate(check.definition.metric.label()),
                        l.translate(check.state.label())
                    ));
                    match &check.definition.basis {
                        Basis::DesignAssumption { note } => {
                            ui.label(l.translate(
                                "Explicit design assumption; not a measured or statutory limit",
                            ));
                            ui.label(note);
                        }
                        Basis::CitedRequirement { id } => {
                            ui.label(format!("{}: {id}", l.translate("Cited requirement")));
                            ui.label(l.translate(
                                "Original not verified in this preview; assessment remains unknown",
                            ));
                        }
                    }
                    ui.label(format!(
                        "{} {} {}",
                        l.translate(check.definition.relation.label()),
                        check
                            .definition
                            .limit
                            .map(|v| format!("{v:.3}"))
                            .unwrap_or_else(|| l.translate("Unknown").into()),
                        check.definition.metric.unit()
                    ));
                    for row in &check.rows {
                        ui.label(format!(
                            "{:?}: {} {} · {}",
                            row.ids,
                            row.value
                                .map(|v| format!("{v:.3}"))
                                .unwrap_or_else(|| l.translate("Unknown").into()),
                            check.definition.metric.unit(),
                            l.translate(row.state.label())
                        ));
                        ui.small(l.translate(&row.reason));
                    }
                    ui.collapsing(l.translate("Input fingerprint"), |ui| {
                        ui.monospace(
                            check
                                .source_sha256
                                .as_deref()
                                .unwrap_or(l.translate("Unknown")),
                        );
                    });
                    ui.separator();
                }
            });
    }
}
