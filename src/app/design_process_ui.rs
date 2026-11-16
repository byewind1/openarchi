use super::*;
use crate::{
    application_host::CommandContext,
    design_process::{Comparison, Entry, Statement, StatementKind, service},
    model_service::PreparedChange,
};
pub(super) struct Proposal {
    pub id: String,
    pub context: CommandContext,
    pub change: std::rc::Rc<PreparedChange>,
    pub sent_target: Option<crate::automation::Target>,
    pub label: &'static str,
    pub delta: crate::command::ProjectDelta,
    pub declared_check_previews: Vec<crate::constraints::Preview>,
}
pub(super) struct ProcessUi {
    canvas_session: Option<u64>,
    task_canvases: std::collections::BTreeMap<Option<String>, TaskCanvas>,
    pub open: bool,
    pub comparison_3d: super::comparison_3d_ui::Comparison3d,
    pub preview: Option<Proposal>,
    preview_scroll_tail: bool,
    pub name: String,
    pub question: String,
    pub assumptions: String,
    pub unknowns: String,
    pub preferences: String,
    pub requirements: String,
    pub names: [String; 2],
    pub use_selection: bool,
    pub selected_space: Option<u64>,
    pub dx: f64,
    pub dy: f64,
    pub height: f64,
    space_context: Option<(u64, u64, f64)>,
    pub reason: String,
    pub chosen: Option<u32>,
    pub history: Option<String>,
    pub error: String,
    pub status: String,
    reviewed_comparison: Option<String>,
    check_summaries: Vec<(u32, Result<Option<crate::diagrams::checks::Source>, String>)>,
    cache: Option<((u64, u64, String), Result<Comparison, String>)>,
}
impl Default for ProcessUi {
    fn default() -> Self {
        Self {
            canvas_session: None,
            task_canvases: Default::default(),
            open: false,
            comparison_3d: Default::default(),
            preview: None,
            preview_scroll_tail: false,
            name: String::new(),
            question: String::new(),
            assumptions: String::new(),
            unknowns: String::new(),
            preferences: String::new(),
            requirements: String::new(),
            names: ["A".into(), "B".into()],
            use_selection: false,
            selected_space: None,
            dx: 0.,
            dy: 0.,
            height: 3.,
            space_context: None,
            reason: String::new(),
            chosen: None,
            history: None,
            error: String::new(),
            status: String::new(),
            reviewed_comparison: None,
            check_summaries: vec![],
            cache: None,
        }
    }
}
enum Action {
    Create,
    Activate(u32),
    Edit,
    Choose(String),
    Lock(u32, bool),
    Select(u64),
    Confirm,
    InspectPending,
    Cancel,
    Back,
    Save,
    Canvas,
    Checks,
}
#[derive(Clone)]
pub(super) struct TaskCanvas {
    selection: Vec<u64>,
    story: u32,
    center: Point,
    scale: f64,
    camera: Camera,
    view: View,
}
impl AtelierApp {
    pub(super) fn canvas_scroll_delta(&mut self, ui: &Ui) -> f32 {
        let (scrolling, fresh_wheel, delta) = ui.input(|input| {
            (
                input.is_scrolling(),
                input
                    .raw
                    .events
                    .iter()
                    .any(|e| matches!(e, egui::Event::MouseWheel { .. })),
                input.smooth_scroll_delta.y,
            )
        });
        if self.process_ui.preview.is_some() {
            self.process_ui.preview_scroll_tail |= scrolling;
            return 0.;
        }
        if self.process_ui.preview_scroll_tail {
            // egui smooths a wheel event over subsequent frames. Closing its
            // modal owner must not turn that remaining scroll into camera zoom.
            // An explicit new wheel event starts an ordinary canvas interaction.
            if scrolling && !fresh_wheel {
                return 0.;
            }
            self.process_ui.preview_scroll_tail = false;
        }
        delta
    }
    pub(super) fn pending_design_parameters(&self) -> bool {
        self.pending_parametric_parameters()
            || self.checks_ui.draft.is_some()
            || self.mass_spaces_ui.dirty
            || self.pending_other_design_parameters()
    }
    pub(super) fn pending_other_design_parameters(&self) -> bool {
        if self
            .draft
            .as_ref()
            .zip(self.draft_source.as_ref())
            .is_some_and(|(draft, source)| draft != source)
        {
            return true;
        }
        self.process_ui.open
            && self.process_ui.selected_space.is_some()
            && (self.process_ui.dx != 0.
                || self.process_ui.dy != 0.
                || self
                    .process_ui
                    .space_context
                    .is_some_and(|(_, _, height)| height != self.process_ui.height))
    }
    pub(super) fn capture_task_canvas(&self) -> (u64, Option<String>, TaskCanvas) {
        let e = &self.host.document.editor;
        (
            e.document_session(),
            e.project.design_process.current_task.clone(),
            TaskCanvas {
                selection: e.selection_ids(),
                story: e.story,
                center: self.plan_center,
                scale: self.plan_scale,
                camera: self.camera.clone(),
                view: self.view,
            },
        )
    }
    pub(super) fn finish_task_context_change(&mut self, before: (u64, Option<String>, TaskCanvas)) {
        let session = self.host.document.editor.document_session();
        if self.process_ui.canvas_session != Some(session) {
            self.process_ui.task_canvases.clear();
            self.process_ui.canvas_session = Some(session);
        }
        let current = self
            .host
            .document
            .editor
            .project
            .design_process
            .current_task
            .clone();
        if before.0 != session || before.1 == current {
            return;
        }
        self.process_ui.task_canvases.insert(before.1, before.2);
        if let Some(saved) = self.process_ui.task_canvases.get(&current).cloned() {
            let editor = &mut self.host.document.editor;
            editor.switch_story(saved.story);
            let selection = saved
                .selection
                .into_iter()
                .filter_map(|id| {
                    if editor.project.elements.iter().any(|e| {
                        e.id == id
                            && e.story == saved.story
                            && editor
                                .project
                                .plan_options
                                .element_visible(e, &editor.project.layers)
                    }) {
                        return Some(id);
                    }
                    editor.project.design_process.task().and_then(|task| {
                        editor
                            .project
                            .plan_options
                            .active_design_option
                            .and_then(|option| {
                                task.corresponding_element(&editor.project, id, option)
                            })
                    })
                })
                .collect();
            editor.set_selection(selection);
            self.plan_center = saved.center;
            self.plan_scale = saved.scale;
            self.camera = saved.camera;
            self.view = saved.view;
        } else if let Some(task) = self.host.document.editor.project.design_process.task() {
            let p = &self.host.document.editor.project;
            let active = p.plan_options.active_design_option;
            let bounds = p
                .elements
                .iter()
                .filter(|e| {
                    e.design_option == active
                        && e.story == task.story
                        && task.accepts_shape(&e.shape)
                })
                .flat_map(|e| geometry::corners(p, e))
                .collect::<Vec<_>>();
            if !bounds.is_empty() {
                let lo = bounds
                    .iter()
                    .copied()
                    .fold(glam::DVec3::splat(f64::INFINITY), glam::DVec3::min);
                let hi = bounds
                    .iter()
                    .copied()
                    .fold(glam::DVec3::splat(f64::NEG_INFINITY), glam::DVec3::max);
                self.plan_center = Point::new((lo.x + hi.x) * 0.5, (lo.y + hi.y) * 0.5);
                self.plan_scale = (420. / (hi.x - lo.x).max(hi.y - lo.y).max(5.)).clamp(4., 140.);
                self.camera.target = (lo + hi) * 0.5;
                self.camera.span = ((hi - lo).length() * 1.3).max(8.);
            }
        }
        self.process_ui.space_context = None;
        self.process_ui.cache = None;
        self.sync_process_space();
        self.tab = Tab::Properties;
    }
    pub(super) fn preview_design_task_activation(&mut self, id: &str) {
        if self.menu_gated() || self.workbench_pointer_active() || self.pending_design_parameters()
        {
            return;
        }
        let result =
            crate::design_process::intent::prepare_activate(&self.host.document.editor, id);
        self.process_set_preview(
            crate::storage::new_document_identity(),
            "Resume saved design task",
            result,
        );
    }
    pub(super) fn open_design_process(&mut self) {
        // Every normal entry returns from checks without leaving an overlapping
        // window behind. An unfinished check draft is retained, never discarded.
        if self.checks_ui.draft.is_none() {
            self.checks_ui.open = false;
        }
        self.process_ui.open = true;
        if self
            .host
            .document
            .editor
            .project
            .design_process
            .task()
            .is_none()
        {
            let ids = self.host.document.editor.selection_ids();
            if !ids.is_empty()
                && ids.iter().all(|id| {
                    self.host.document.editor.project.elements.iter().any(|e| {
                        e.id == *id
                            && matches!(e.shape, Shape::Zone { .. } | Shape::ConceptMass { .. })
                    })
                })
            {
                self.process_ui.use_selection = true;
            }
        }
        if self.process_ui.name.is_empty() {
            self.process_ui.name = self.language.translate("Space exploration").into();
        }
    }
    pub(super) fn process_set_preview(
        &mut self,
        id: String,
        label: &'static str,
        result: Result<PreparedChange, String>,
    ) {
        match result {
            Ok(change) => {
                let delta = change.difference();
                let declared_check_previews = if label == "Parametric two-wing proposal" {
                    change.declared_check_previews()
                } else {
                    vec![]
                };
                self.process_ui.preview = Some(Proposal {
                    id,
                    context: self.host.context(),
                    change: std::rc::Rc::new(change),
                    sent_target: None,
                    label,
                    delta,
                    declared_check_previews,
                });
                self.process_ui.error.clear();
            }
            Err(e) => self.process_ui.error = e,
        }
    }
    pub(super) fn sync_process_space(&mut self) {
        let editor = &self.host.document.editor;
        let space = service::selected_space(editor);
        let Some((id, height)) = space else {
            self.process_ui.selected_space = None;
            self.process_ui.space_context = None;
            return;
        };
        let session = editor.document_session();
        if self
            .process_ui
            .space_context
            .is_none_or(|(old_session, old_id, _)| old_session != session || old_id != id)
        {
            self.process_ui.dx = 0.;
            self.process_ui.dy = 0.;
            self.process_ui.height = height;
        } else if let Some((_, _, old_height)) = self.process_ui.space_context {
            if self.process_ui.height == old_height {
                self.process_ui.height = height;
            }
        }
        self.process_ui.selected_space = Some(id);
        self.process_ui.space_context = Some((session, id, height));
    }
    fn process_select_space(&mut self, id: u64) {
        self.host.document.editor.set_selection(vec![id]);
        self.process_ui.space_context = None;
        self.sync_process_space();
        self.tab = Tab::Properties;
    }
    fn retain_process_selection(&mut self, previous: Vec<u64>, initial: bool) {
        let editor = &mut self.host.document.editor;
        let Some(task) = editor.project.design_process.task() else {
            return;
        };
        let option = editor.project.plan_options.active_design_option;
        let mut selection = vec![];
        let mut missing = false;
        for id in previous {
            let element = editor.project.elements.iter().find(|e| e.id == id);
            if let Some(e) = element {
                if e.design_option.is_none() || e.design_option == option {
                    selection.push(id);
                    continue;
                }
            }
            if let Some(mapped) =
                option.and_then(|option| task.corresponding_element(&editor.project, id, option))
            {
                selection.push(mapped);
            } else {
                missing = true;
            }
        }
        if missing {
            selection.clear();
            self.process_ui.status =
                "A corresponding space is missing; select a space in the current candidate.".into();
        } else if initial && selection.is_empty() {
            if let Some(id) = editor
                .project
                .elements
                .iter()
                .find(|e| {
                    e.design_option == option
                        && e.story == task.story
                        && task.accepts_shape(&e.shape)
                })
                .map(|e| e.id)
            {
                selection.push(id);
            }
        }
        editor.set_selection(selection);
        self.process_ui.space_context = None;
        self.sync_process_space();
    }
    pub(super) fn activate_process_option(&mut self, option: Option<u32>) -> Result<(), String> {
        if self.workbench_pointer_active() || self.menu_gated() || self.pending_design_parameters()
        {
            return Err(
                "Finish or cancel the current model interaction before applying settings".into(),
            );
        }
        let previous = self.host.document.editor.selection_ids();
        if self
            .host
            .document
            .editor
            .project
            .plan_options
            .active_design_option
            != option
        {
            let prepared = service::prepare_activate(&self.host.document.editor, option)?;
            if prepared.project().design_process.current_task
                != self
                    .host
                    .document
                    .editor
                    .project
                    .design_process
                    .current_task
            {
                self.process_set_preview(
                    crate::storage::new_document_identity(),
                    "Resume saved design task",
                    Ok(prepared),
                );
                return Ok(());
            }
            self.host.commit_prepared(
                self.host.context(),
                &crate::storage::new_document_identity(),
                &prepared,
            )?;
            self.retain_process_selection(previous, false);
        } else {
            self.sync_process_space();
        }
        self.process_ui.cache = None;
        Ok(())
    }
    pub(super) fn process_commit_proposal(&mut self) -> Result<(), String> {
        let canvas_before = self.capture_task_canvas();
        let mut proposal = self.process_ui.preview.take().ok_or("No design proposal")?;
        if proposal.sent_target.is_some() {
            self.process_ui.preview = Some(proposal);
            return Err(
                "Design operation outcome is unknown. Inspect the original operation.".into(),
            );
        }
        if self.workbench_pointer_active() || self.menu_gated() {
            self.process_ui.preview = Some(proposal);
            return Err(
                "Finish or cancel the current model interaction before applying settings".into(),
            );
        }
        if proposal.change.needs_cited_recheck()
            && proposal
                .change
                .validate_cited_admission(&self.host.document.editor)
                .is_err()
        {
            let context = proposal.context;
            let id = proposal.id.clone();
            let commands = proposal.change.commands().to_vec();
            self.process_ui.preview = Some(proposal);
            self.start_checks_admission(context, id, commands, true)?;
            return Ok(());
        }
        let previous_selection = self.host.document.editor.selection_ids();
        let initial_task = self
            .host
            .document
            .editor
            .project
            .design_process
            .task()
            .is_none();
        let proposed_mass_space = proposal
            .change
            .project()
            .design_process
            .mass_spaces
            .iter()
            .find(|link| {
                self.host
                    .document
                    .editor
                    .project
                    .design_process
                    .mass_spaces
                    .iter()
                    .find(|old| old.zone == link.zone)
                    != Some(*link)
            })
            .map(|link| link.zone);
        let proposed_report = self.proposed_report(proposal.change.project());
        let proposed_figure = self.proposed_figure_id(proposal.change.project());
        let changed_check = (proposal.label == "Edit quantified checks")
            .then(|| {
                proposal
                    .change
                    .project()
                    .rules
                    .checks
                    .iter()
                    .find(|c| !self.host.document.editor.project.rules.checks.contains(c))
                    .map(|c| c.definition.id.clone())
            })
            .flatten();
        let original_target = self.host.operation_target();
        let result = self.host.confirm_task_prepared(
            proposal.context,
            &proposal.id,
            proposal.label,
            proposal.change.clone(),
        );
        if result["result"]["status"] == "committed" {
            self.finish_native_object_task_control(&proposal.change);
            if proposal.label == "Parametric two-wing proposal" {
                if self
                    .research_ui
                    .parameters
                    .as_ref()
                    .is_some_and(|d| d.discussion.is_some())
                {
                    self.research_ui.draft = None;
                }
                self.research_ui.parameters = None;
            }
            if proposal.label == "Save object discussion" {
                self.research_ui.draft = None;
            }
            self.process_ui.cache = None;
            self.process_ui.status = "Design change recorded; save the project to keep it".into();
            self.host.document.editor.status = self.process_ui.status.clone();
            if initial_task
                || canvas_before.1
                    == self
                        .host
                        .document
                        .editor
                        .project
                        .design_process
                        .current_task
            {
                self.retain_process_selection(previous_selection, initial_task);
            }
            self.finish_task_context_change(canvas_before);
            if matches!(
                proposal.label,
                "Parametric two-wing proposal"
                    | "Submit object task"
                    | "Control original object task"
            ) {
                self.show_right = true;
                self.tab = Tab::Assistant;
            }
            if proposal.label == "Edit quantified checks" {
                self.checks_ui.draft = None;
                self.checks_ui.focus = changed_check;
            }
            if proposal.label == "Edit mass-space links" {
                if matches!(self.view, View::Figure(_))
                    && proposal.delta.elements.iter().any(|edit| {
                        edit.before.is_none()
                            && edit
                                .after
                                .as_ref()
                                .is_some_and(|e| matches!(e.shape, Shape::Zone { .. }))
                    })
                {
                    self.view = View::Plan;
                }
                self.mass_spaces_ui = Default::default();
                if let Some(id) = proposed_mass_space.or_else(|| {
                    proposal
                        .delta
                        .elements
                        .iter()
                        .filter_map(|e| e.after.as_ref())
                        .find(|e| matches!(e.shape, Shape::Zone { .. }))
                        .map(|e| e.id)
                }) {
                    self.host.document.editor.set_selection(vec![id]);
                    self.tab = Tab::Properties;
                }
            }
            if proposal.label == "Edit report template" {
                if let Some(report) = proposed_report {
                    if self
                        .drawings_ui
                        .report_draft
                        .as_ref()
                        .is_some_and(|d| d.name == report.name && d.sections == report.sections)
                    {
                        self.drawings_ui.report_draft = None;
                        self.clear_report_draft_error();
                    }
                    self.drawings_ui.report_selected = Some(report.id);
                    self.drawings_ui.report_export = std::path::Path::new(&self.path)
                        .parent()
                        .unwrap_or(std::path::Path::new("."))
                        .join(format!("report-{}-v{}.svg", report.id, report.generation))
                        .to_string_lossy()
                        .into();
                    self.drawings_ui.report_open = true;
                    self.drawings_ui.manager = true;
                    self.process_ui.open = false;
                }
            }
            if proposal.label == "Create linked model figure"
                || proposal.label == "Update linked model figure"
                || proposal.label == "Rename linked model figure"
                || proposal.label == "Configure linked model figure"
            {
                if let Some(id) = proposed_figure {
                    let report_page =
                        self.drawings_ui
                            .report_navigation
                            .filter(|(report, source)| {
                                *source == id
                                    && self
                                        .host
                                        .document
                                        .editor
                                        .project
                                        .drawings
                                        .reports
                                        .iter()
                                        .any(|r| {
                                            r.id == *report
                                                && r.sections.iter().any(|s| s.figure == id)
                                        })
                            });
                    self.open_figure(id);
                    if self.view == View::Figure(id) {
                        self.drawings_ui.report_navigation = report_page;
                    }
                }
            }
            Ok(())
        } else {
            self.finish_task_context_change(canvas_before);
            let code = result["error"]["code"]
                .as_str()
                .unwrap_or("OPERATION_OUTCOME_UNKNOWN");
            if code == "RECOVERY_REQUIRED" {
                proposal.sent_target =
                    serde_json::from_value(result["error"]["original_target"].clone()).ok();
            } else if result["operation_submitted"] == true
                && matches!(code, "OPERATION_OUTCOME_UNKNOWN" | "JOURNAL_WRITE_FAILED")
            {
                proposal.sent_target = Some(original_target);
            }
            self.process_ui.preview = Some(proposal);
            Err(code.into())
        }
    }
    pub(super) fn design_process_window(&mut self, ctx: &Context) {
        // Release only on a later frame: the closing frame may still paint these textures.
        if !self.process_ui.open {
            self.process_ui.comparison_3d.release();
        }
        if !self.process_ui.open && self.process_ui.preview.is_none() {
            return;
        }
        self.sync_process_space();
        let language = self.language;
        if self.process_ui.preview.is_some() {
            self.process_ui.preview_scroll_tail |= ctx.input(|input| input.is_scrolling());
            let proposal = self.process_ui.preview.take().unwrap();
            let mut action = None;
            let stale = !proposal.change.matches_baseline(&self.host.document.editor);
            egui::Modal::new(Id::new("design_process_proposal")).show(ctx, |ui| {
                ui.set_min_width(540.);
                ui.heading(language.translate(proposal.label));
                if proposal.label == "Assign space surfaces" {
                    ui.label(language.translate("Geometry and function areas remain unchanged"));
                    for e in &proposal.change.project().elements {
                        if !proposal.delta.elements.iter().any(|v| v.id == e.id) {
                            continue;
                        }
                        if let Shape::Zone { spec } = &e.shape {
                            if let crate::zones::surfaces::ZoneSurface::Solid { name, color } =
                                &spec.surface
                            {
                                ui.horizontal(|ui| {
                                    ui.colored_label(
                                        Color32::from_rgb(color[0], color[1], color[2]),
                                        "■",
                                    );
                                    ui.label(format!("#{} · {} · {}", e.id, e.name, name));
                                });
                            }
                        }
                    }
                }
                if proposal.label == "Save object discussion" {
                    self.object_research_preview(ui, proposal.change.project());
                }
                if proposal.change.project().design_process.current_task
                    != self
                        .host
                        .document
                        .editor
                        .project
                        .design_process
                        .current_task
                {
                    if let Some(task) = proposal.change.project().design_process.task() {
                        ui.strong(&task.name);
                        if let Some(name) = proposal
                            .change
                            .project()
                            .plan_options
                            .active_design_option
                            .and_then(|id| proposal.change.project().design_options.name(id))
                        {
                            ui.label(name);
                        }
                    }
                }
                let pending = proposal.sent_target.is_some();
                ui.label(language.translate(if pending {
                    "Design operation outcome is unknown. Inspect the original operation."
                } else {
                    "The current model is unchanged until confirmation."
                }));
                let delta = &proposal.delta;
                let mass_creation = proposal.label == "Create editable mass candidates";
                let added = delta.elements.iter().filter(|e| e.before.is_none()).count();
                let changed = delta.elements.iter().filter(|e| e.before.is_some()).count();
                ui.label(format!(
                    "{}: {added} · {}: {changed}",
                    language.translate(
                        if mass_creation || proposal.label == "Parametric two-wing proposal" {
                            "New concept masses"
                        } else {
                            "New spaces"
                        }
                    ),
                    language.translate(if proposal.label == "Parametric two-wing proposal" {
                        "Changed geometry records"
                    } else {
                        "Changed records"
                    })
                ));
                if let Some(task) = proposal.change.project().design_process.task() {
                    ui.strong(&task.name);
                    ui.label(&task.question);
                }
                if mass_creation {
                    let proposed = delta
                        .elements
                        .iter()
                        .filter(|e| e.before.is_none())
                        .filter_map(|e| e.after.clone())
                        .collect::<Vec<_>>();
                    Self::design_snapshot_plan(ui, &proposed);
                }
                if proposal.label.contains("linked model figure") {
                    if let Some(id) = self.proposed_figure_id(proposal.change.project()) {
                        if let Some(figure) = proposal
                            .change
                            .project()
                            .drawings
                            .diagrams
                            .iter()
                            .find(|v| v.id == id)
                        {
                            Self::paint_figure(ui, figure, language, 540.);
                        }
                    }
                }
                if proposal.label == "Edit mass-space links" {
                    ui.label(language.translate("Concept space: not measured or approved use"));
                    for link in &proposal.change.project().design_process.mass_spaces {
                        if !self
                            .host
                            .document
                            .editor
                            .project
                            .design_process
                            .mass_spaces
                            .contains(link)
                        {
                            ui.label(format!("#{} → Zone #{}", link.mass, link.zone));
                            ui.label(&link.assumption);
                        }
                    }
                    let proposed = delta
                        .elements
                        .iter()
                        .filter_map(|e| e.after.clone())
                        .collect::<Vec<_>>();
                    if !proposed.is_empty() {
                        Self::design_snapshot_plan(ui, &proposed);
                    }
                }
                Self::object_task_preview_contents(
                    ui,
                    language,
                    &self.host.document.editor.project,
                    &proposal.change,
                );
                if proposal.label == "Parametric two-wing proposal" {
                    if let Some((_, exchange, _)) = self
                        .research_ui
                        .parameters
                        .as_ref()
                        .and_then(|d| d.discussion.as_ref())
                    {
                        ui.strong(language.translate("Current object question"));
                        ui.label(&exchange.question);
                    }
                    Self::parametric_preview_contents(
                        ui,
                        &self.host.document.editor.project,
                        proposal.change.project(),
                        language,
                    );
                    Self::declared_check_preview_contents(
                        ui,
                        language,
                        &proposal.declared_check_previews,
                        stale,
                    );
                }
                if proposal.label == "Edit quantified checks" {
                    Self::check_preview_contents(
                        ui,
                        &self.host.document.editor.project,
                        proposal.change.project(),
                        language,
                    );
                }
                if proposal.label == "Edit report template" {
                    if let Some(report) = self.proposed_report(proposal.change.project()) {
                        ScrollArea::vertical()
                            .id_salt("report_frozen_preview")
                            .max_height(240.)
                            .show(ui, |ui| {
                                Self::report_pages(
                                    ui,
                                    proposal.change.project(),
                                    &report,
                                    language,
                                );
                            });
                    }
                }
                if proposal.label == "Record choice and candidate versions" {
                    if let Some(decision) =
                        proposal.change.project().design_process.decisions.last()
                    {
                        let name = decision
                            .comparison
                            .candidates
                            .iter()
                            .find(|c| c.option == decision.option)
                            .map(|c| c.name.as_str())
                            .unwrap_or("");
                        ui.label(format!("{}: {name}", language.translate("Current choice")));
                        ui.label(&decision.reason);
                        ui.label(language.translate(&decision.operator_label));
                        for version in &decision.comparison.candidates {
                            let (area, volume) = version
                                .mass_metrics
                                .as_ref()
                                .map(|m| (m.footprint_sum_m2, m.envelope_volume_sum_m3))
                                .unwrap_or((version.metrics.area_m2, version.metrics.volume_m3));
                            ui.label(format!(
                                "{} · {:.3} m² · {:.3} m³",
                                version.name, area, volume
                            ));
                        }
                    }
                }
                if matches!(
                    proposal.label,
                    "Edit candidate space" | "Edit candidate mass"
                ) {
                    let old = delta
                        .elements
                        .iter()
                        .filter_map(|e| e.before.clone())
                        .collect::<Vec<_>>();
                    let proposed = delta
                        .elements
                        .iter()
                        .filter_map(|e| e.after.clone())
                        .collect::<Vec<_>>();
                    ui.label(
                        language.translate(if proposal.label == "Edit candidate mass" {
                            "Existing mass footprint"
                        } else {
                            "Existing space footprint"
                        }),
                    );
                    Self::design_snapshot_plan(ui, &old);
                    ui.label(
                        language.translate(if proposal.label == "Edit candidate mass" {
                            "Proposed mass footprint"
                        } else {
                            "Proposed space footprint"
                        }),
                    );
                    Self::design_snapshot_plan(ui, &proposed);
                }
                ui.collapsing(
                    language.translate(if mass_creation {
                        "Inspect proposed masses"
                    } else {
                        "Inspect proposed spaces"
                    }),
                    |ui| {
                        ScrollArea::vertical().max_height(220.).show(ui, |ui| {
                            for e in &delta.elements {
                                if let Some(after) = &e.after {
                                    ui.label(format!(
                                        "{}{} · {}",
                                        if matches!(after.shape, Shape::ConceptMass { .. }) {
                                            "#"
                                        } else {
                                            "Z"
                                        },
                                        after.id,
                                        after.name
                                    ));
                                    if let Shape::Zone { spec } = &after.shape {
                                        ui.label(format!(
                                            "{:.3} m² · {:.3} m",
                                            spec.area(),
                                            spec.height
                                        ));
                                    }
                                    if let Shape::ConceptMass { spec } = &after.shape {
                                        ui.label(format!(
                                            "{:.3} m² · {:.3} m · {:.3} m³",
                                            spec.area(),
                                            spec.height,
                                            spec.volume()
                                        ));
                                    }
                                }
                            }
                        });
                    },
                );
                if stale && !pending {
                    ui.colored_label(
                        Color32::from_rgb(224, 125, 105),
                        language.translate("Model changed after preview; return to editing."),
                    );
                }
                if self
                    .checks_ui
                    .pending
                    .as_ref()
                    .is_some_and(|(id, _)| *id == proposal.id)
                {
                    ui.label(language.translate("Verifying cited originals…"));
                }
                if !self.process_ui.error.is_empty() {
                    ui.colored_label(
                        Color32::from_rgb(224, 125, 105),
                        super::checks_ui::original_read_error_label(
                            language,
                            &self.process_ui.error,
                        ),
                    )
                    .on_hover_text(&self.process_ui.error);
                }
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(
                            !stale
                                && !pending
                                && self
                                    .checks_ui
                                    .pending
                                    .as_ref()
                                    .is_none_or(|(id, _)| *id != proposal.id),
                            Button::new(language.translate("Confirm design change")),
                        )
                        .clicked()
                    {
                        action = Some(Action::Confirm);
                    }
                    if pending
                        && ui
                            .button(language.translate("Inspect original design operation"))
                            .clicked()
                    {
                        action = Some(Action::InspectPending);
                    }
                    if ui
                        .add_enabled(
                            !pending,
                            Button::new(language.translate("Return to design editing")),
                        )
                        .clicked()
                    {
                        action = Some(Action::Back);
                    }
                    if ui
                        .button(language.translate(if pending {
                            "Close and retain operation"
                        } else {
                            "Cancel design proposal"
                        }))
                        .clicked()
                    {
                        action = Some(Action::Cancel);
                    }
                });
            });
            self.process_ui.preview = Some(proposal);
            match action {
                Some(Action::Confirm) => {
                    if let Err(e) = self.process_commit_proposal() {
                        self.process_ui.error = e;
                    }
                }
                Some(Action::InspectPending) => {
                    let p = self.process_ui.preview.as_ref().unwrap();
                    if let Some(target) = &p.sent_target {
                        let result = self.host.inspect_operation(target.clone(), &p.id);
                        self.process_ui.error = result["error"]["code"]
                            .as_str()
                            .or_else(|| result["result"]["status"].as_str())
                            .unwrap_or("unknown")
                            .into();
                        if matches!(
                            result["result"]["status"].as_str(),
                            Some("committed" | "undone" | "cancelled")
                        ) {
                            self.process_ui.preview = None;
                            self.process_ui.status = "Original design operation found; its outcome has not been replayed.".into();
                            self.process_ui.cache = None;
                        }
                    }
                }
                Some(Action::Back) | Some(Action::Cancel) => {
                    if self
                        .process_ui
                        .preview
                        .as_ref()
                        .is_some_and(|p| p.sent_target.is_some())
                    {
                        self.process_ui.status =
                            "Original operation retained; inspect it in Operations.".into();
                    }
                    self.process_ui.preview = None;
                    self.process_ui.error.clear();
                }
                _ => {}
            }
            return;
        }
        let task = self
            .host
            .document
            .editor
            .project
            .design_process
            .task()
            .cloned();
        let busy = self.workbench_pointer_active() || self.menu_gated();
        let mut cache = self.process_ui.cache.take();
        if let Some(task) = &task {
            let key = (
                self.host.document.editor.document_session(),
                self.host.document.editor.revision,
                task.id.clone(),
            );
            if !busy && cache.as_ref().is_none_or(|c| c.0 != key) {
                self.process_ui.reviewed_comparison = None;
                let p = &self.host.document.editor.project;
                let comparison = task.compare(p);
                self.process_ui.check_summaries = comparison
                    .as_ref()
                    .map(|comparison| {
                        comparison
                            .candidates
                            .iter()
                            .map(|candidate| {
                                let definition = crate::diagrams::Definition {
                                    presentation: crate::diagrams::Presentation::ConstraintOverlay,
                                    story: task.story,
                                    option: Some(candidate.option),
                                    domain: if task.is_mass() {
                                        crate::diagrams::Domain::ConceptMasses
                                    } else {
                                        crate::diagrams::Domain::Zones
                                    },
                                };
                                (
                                    candidate.option,
                                    crate::diagrams::checks::Source::capture_if_present(
                                        p,
                                        &definition,
                                        &candidate.elements,
                                    ),
                                )
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                cache = Some((key, comparison));
            }
        }
        let mut open = true;
        let mut action = None;
        egui::Window::new(language.translate(if task.as_ref().is_some_and(|t|t.is_mass()){"Concept mass exploration"}else{"Explore space options…"})).id(Id::new("design_process_window")).open(&mut open).default_pos(pos2(40.,90.)).default_width(660.).default_height(720.).vscroll(true).show(ctx,|ui|{
   if let Some(task)=&task{
    ui.heading(&task.name);ui.label(&task.question);
    ui.label(language.translate("Authored model quantities; requirements and compliance are not automatically verified."));
    if busy{ui.label(language.translate("Finish the model interaction before comparing or recording a choice."));}
    ui.horizontal_wrapped(|ui|{
     if ui.add_enabled(!busy,Button::new(language.translate("View baseline"))).clicked(){action=Some(Action::Activate(task.baseline_option));}
     if ui.add_enabled(!busy, Button::new(language.translate("Save project"))).clicked(){action=Some(Action::Save);}
     if ui.button(language.translate("Continue in canvas")).clicked(){action=Some(Action::Canvas);}
     let response=ui.add_enabled(!busy&&!self.pending_design_parameters(),Button::new(language.translate("Inspect candidate checks")));
     #[cfg(test)] ctx.data_mut(|data|data.insert_temp(Id::new("comparison_checks_button"),response.rect));
     if response.clicked(){action=Some(Action::Checks);}
    });
    let active=self.host.document.editor.project.plan_options.active_design_option;
    for candidate in &task.candidates{
     let name=self.host.document.editor.project.design_options.name(candidate.option).unwrap_or(&candidate.name);
     let owned=self.host.document.editor.project.elements.iter().filter(|e|e.design_option==Some(candidate.option)).collect::<Vec<_>>();
     let locked=!owned.is_empty() && owned.iter().all(|e|self.host.document.editor.project.edit_state.locked.contains(&e.id));
     ui.push_id(candidate.option,|ui|ui.horizontal_wrapped(|ui|{
      ui.label(format!("{}: {name}",language.translate("Candidate")));
      if ui.add_enabled(!busy,Button::new(language.translate("Edit candidate")).selected(active==Some(candidate.option))).clicked(){action=Some(Action::Activate(candidate.option));}
      let label=match(task.is_mass(),locked){(true,true)=>"Unlock candidate masses",(true,false)=>"Lock candidate masses",(false,true)=>"Unlock candidate spaces",(false,false)=>"Lock candidate spaces"};if ui.add_enabled(!busy,Button::new(language.translate(label))).clicked(){action=Some(Action::Lock(candidate.option,!locked));}
     }));
    }
    if let Some((_,result))=&cache{match result{Err(e)=>{ui.colored_label(Color32::from_rgb(224,125,105),language.translate(e));},Ok(comparison)=>{
     if let Some(decision)=self.host.document.editor.project.design_process.current_choice.as_ref().and_then(|id|self.host.document.editor.project.design_process.decisions.iter().find(|d|&d.id==id)){
      let state=if decision.comparison.fingerprint==comparison.fingerprint{"Saved choice inputs unchanged"}else{"Saved choice inputs are stale"};
      ui.strong(format!("{} · {}",language.translate("Current choice"),language.translate(state)));
     }
     if !comparison.basis_current{ui.colored_label(Color32::from_rgb(224,125,105),language.translate("Exploration basis changed; current quantities are for review only."));}
     if !comparison.different{ui.label(language.translate("The candidates are initially identical. Edit a spatial parameter before choosing."));}
     egui::Grid::new("design_comparison").striped(true).show(ui,|ui|{
      if task.is_mass(){
       for label in ["Candidate","Concept masses","Mass footprint sum","Concept envelope volume sum"]{ui.strong(language.translate(label));}ui.end_row();
       for c in &comparison.candidates {if let Some(m)=&c.mass_metrics {ui.label(&c.name);ui.label(m.masses.to_string());ui.label(format!("{:.3} m²",m.footprint_sum_m2));ui.label(format!("{:.3} m³",m.envelope_volume_sum_m3));ui.end_row();}}
      }else{
       for label in ["Candidate","Spaces","Authored space area","Space volume","Space overlap sum","Wall overlap sum"]{ui.strong(language.translate(label));}ui.end_row();
       for c in &comparison.candidates{ui.label(&c.name);ui.label(c.metrics.spaces.to_string());ui.label(format!("{:.3} m²",c.metrics.area_m2));ui.label(format!("{:.3} m³",c.metrics.volume_m3));ui.label(format!("{:.4} m²",c.metrics.pair_overlap_sum_m2));ui.label(format!("{:.4} m²",c.metrics.wall_overlap_sum_m2));ui.end_row();}
      }
     });
     ui.label(language.translate(if task.is_mass(){"Mass sums count overlapping pieces separately; not floor, space or material quantities."}else{"Overlap values are sums for these spaces; they are not a circulation or code verdict."}));
     if task.intent.as_ref().is_some_and(|v|v.parametric.is_some()){for v in &comparison.candidates{Self::parameter_version_summary(ui,language,v,true);}}
     ui.collapsing(language.translate("Inspect candidate footprints"),|ui|{Self::design_comparison_plans(ui,language,comparison);});
     ui.collapsing(language.translate("Compare candidate volumes in 3D"),|ui|{self.design_comparison_3d(ui,comparison);});
     ui.collapsing(language.translate("Checks bound to these candidates"),|ui|{
      ui.label(language.translate("Numerical checks do not approve the project or waive missing mandatory conditions."));
      for candidate in &comparison.candidates {
       ui.strong(&candidate.name);
       match self.process_ui.check_summaries.iter().find(|(option,_)|*option==candidate.option).map(|(_,result)|result) {
        Some(Ok(Some(source)))=>for entry in &source.entries {
         let check=&entry.check;
         ui.push_id((candidate.option,&check.definition.id),|ui|{ui.collapsing(format!("{} · {}",check.definition.title,language.translate(entry.state.label())),|ui|{
         let freshness=match &check.report {None=>"Check has not been run",Some(report) if report.source_sha256==entry.input_sha256=>"Check inputs unchanged",_=>"Check result is out of date"};
         ui.small(language.translate(freshness));
         ui.label(format!("{} · {} {} {}",language.translate(check.definition.metric.label()),language.translate(check.definition.relation.label()),check.definition.limit.map_or_else(||language.translate("Unknown").into(),|v|v.to_string()),check.definition.metric.unit()));
         match &check.definition.basis {
          crate::constraints::Basis::DesignAssumption{note}=>{ui.small(language.translate("Design assumption; not a surveyed or statutory limit"));ui.label(note);},
          crate::constraints::Basis::CitedRequirement{..}=>{ui.small(language.translate("Current cited result requires the bound original; a stored numerical snapshot is not original verification"));if let Some(citation)=&entry.citation{ui.label(&citation.text);}},
         }
         });});
        },
        Some(Err(message))=>{ui.colored_label(Color32::LIGHT_RED,message);},
        _=>{let response=ui.label(language.translate("No check is bound to this candidate"));#[cfg(test)]ctx.data_mut(|data|data.insert_temp(Id::new("unbound_candidate_check_label"),response.id));let _=response;},
       }
      }
     });
     ui.separator();
     let current_owned=self.host.document.editor.project.elements.iter().filter(|e|e.design_option==active && task.accepts_shape(&e.shape) && task.candidates.iter().any(|c|Some(c.option)==active)).map(|e|(e.id,e.name.clone())).collect::<Vec<_>>();
     if !current_owned.is_empty(){
      ui.label(language.translate(if task.is_mass(){"Edit one mass in the active candidate"}else{"Edit one space in the active candidate"}));
      let mut selected=self.process_ui.selected_space.filter(|id|current_owned.iter().any(|e|e.0==*id));
      egui::ComboBox::from_id_salt("design_space").selected_text(current_owned.iter().find(|e|Some(e.0)==selected).map(|e|e.1.as_str()).unwrap_or(language.translate(if task.is_mass(){"Select a mass on the canvas"}else{"Select a space on the canvas"}))).show_ui(ui,|ui|{for (id,name) in &current_owned{if ui.selectable_value(&mut selected,Some(*id),name).clicked(){action=Some(Action::Select(*id));}}});
      ui.add_enabled_ui(selected.is_some(),|ui|ui.horizontal_wrapped(|ui|{
       let label=ui.label(language.translate("ΔX"));ui.add(DragValue::new(&mut self.process_ui.dx).speed(0.01).suffix(" m")).labelled_by(label.id);let label=ui.label(language.translate("ΔY"));let response=ui.add(DragValue::new(&mut self.process_ui.dy).speed(0.01).suffix(" m")).labelled_by(label.id);#[cfg(test)]ctx.data_mut(|d|d.insert_temp(Id::new("candidate_dy_field"),response.id));let _=response;let label=ui.label(language.translate(if task.is_mass(){"Height"}else{"Clear height"}));let response=ui.add(DragValue::new(&mut self.process_ui.height).speed(0.01).range(0.01..=100.).suffix(" m")).labelled_by(label.id);#[cfg(test)]ctx.data_mut(|d|d.insert_temp(Id::new("candidate_height_field"),response.id));let _=response;
       if ui.add_enabled(!busy,Button::new(language.translate("Preview candidate edit"))).clicked(){action=Some(Action::Edit);}
      }));
     }
     ui.separator();
     ui.horizontal_wrapped(|ui|{ui.label(language.translate("Choose a direction"));for c in &comparison.candidates{let response=ui.selectable_value(&mut self.process_ui.chosen,Some(c.option),&c.name);#[cfg(test)]ctx.data_mut(|data|data.insert_temp(Id::new(("comparison_choice",c.option)),response.id));let _=response;}});
     let label=ui.label(language.translate("Choice reason"));let response=ui.add(TextEdit::multiline(&mut self.process_ui.reason).hint_text(language.translate("Optional; you can record the reason later")).desired_rows(2)).labelled_by(label.id);#[cfg(test)]ctx.data_mut(|data|data.insert_temp(Id::new("comparison_reason"),response.id));let _=response;
     if !comparison.basis_current {
      let mut reviewed=self.process_ui.reviewed_comparison.as_ref()==Some(&comparison.fingerprint);
      let response=ui.checkbox(&mut reviewed,language.translate("I reviewed the changed basis for this choice"));
      #[cfg(test)]ctx.data_mut(|data|{data.insert_temp(Id::new("comparison_review"),response.rect);data.insert_temp(Id::new("comparison_review_state"),(reviewed,busy,comparison.fingerprint.clone()));});
      if response.changed(){self.process_ui.reviewed_comparison=reviewed.then(||comparison.fingerprint.clone());}
      ui.small(language.translate("This choice keeps the changed-basis record and all unknowns; it does not approve project conditions."));
     }
     let response=ui.add_enabled(!busy&&(comparison.basis_current||self.process_ui.reviewed_comparison.as_ref()==Some(&comparison.fingerprint))&&comparison.different,Button::new(language.translate("Preview choice and saved versions")));
     #[cfg(test)]ctx.data_mut(|data|data.insert_temp(Id::new("comparison_preview_action"),(response.id,response.enabled())));
     if response.clicked(){action=Some(Action::Choose(comparison.fingerprint.clone()));}
    }}}
    ui.collapsing(language.translate("Basis, declarations and unknowns"),|ui|{ui.label(language.translate("Basis: saved authored model geometry and recorded project context."));for s in &task.statements{ui.label(format!("{}: {}",language.translate(s.kind.label()),s.text));}ui.label(language.translate("These declarations do not approve project use, survey data or regulations."));if let Some(intent)=&task.intent{ui.label(language.translate("All numeric geometry is a design assumption, not a measurement."));ui.label(&intent.geometry_assumptions);for claim in &intent.claims{let kind=match claim.kind{crate::design_process::intent::ClaimKind::Observation=>"Observation",crate::design_process::intent::ClaimKind::Inference=>"Inference",crate::design_process::intent::ClaimKind::DesignCompletion=>"Design completion"};ui.label(format!("{}: {}",language.translate(kind),claim.text));}for source in &intent.images{let page=self.host.document.editor.project.drawings.pages.iter().find(|p|p.id==source.page);ui.label(format!("{} · {}",page.map(|p|p.name.as_str()).unwrap_or(language.translate("Missing image source")),&source.sha256[..8]));}ui.label(language.translate("Image references and claims are declarations; intent response and source reading require separate verification."));}});
   }else{
    ui.label(language.translate("Start from 1–16 main-model Zones or concept masses of one type on one story. The baseline is retained."));
    let label=ui.label(language.translate("Exploration name"));ui.add(TextEdit::singleline(&mut self.process_ui.name)).labelled_by(label.id);
    let label=ui.label(language.translate("What question are you exploring?"));let response=ui.add(TextEdit::multiline(&mut self.process_ui.question).desired_rows(2)).labelled_by(label.id);#[cfg(test)]ctx.data_mut(|d|d.insert_temp(Id::new("exploration_question_field"),response.id));let _=response;
    for (label,value) in ["Candidate A name","Candidate B name"].into_iter().zip(self.process_ui.names.iter_mut()){let label=ui.label(language.translate(label));ui.add(TextEdit::singleline(value)).labelled_by(label.id);}
    ui.checkbox(&mut self.process_ui.use_selection,language.translate("Use selected concept objects instead of this story"));
    ui.collapsing(language.translate("Declarations, preferences, assumptions and unknowns"),|ui|{
     for (kind,value) in [(StatementKind::Requirement,&mut self.process_ui.requirements),(StatementKind::Preference,&mut self.process_ui.preferences),(StatementKind::Assumption,&mut self.process_ui.assumptions),(StatementKind::Unknown,&mut self.process_ui.unknowns)]{let label=ui.label(language.translate(kind.label()));ui.add(TextEdit::multiline(value).desired_rows(2)).labelled_by(label.id);}
    });
    if ui.add_enabled(!busy,Button::new(language.translate("Preview two editable candidates"))).clicked(){action=Some(Action::Create);}
   }
   if let Some(task)=self.host.document.editor.project.design_process.task(){
    if self.host.document.editor.project.design_process.updates.iter().any(|r|r.task_id==task.id){
     ui.collapsing(language.translate("Task updates and sources"),|ui|{
      for record in self.host.document.editor.project.design_process.updates.iter().filter(|r|r.task_id==task.id){
       ui.strong(&record.instruction);ui.label(&record.expected_result);
       ui.label(language.translate(record.effect(&self.host.document.editor.project)));
       ui.label(language.translate(record.source_status(&self.host.document.editor.project,self.source_ui.repository.as_ref())));
       ui.label(format!("{}: {:?}",language.translate("Affected elements"),record.affected_ids));
       for source in &record.sources{ui.label(source.label());}
       for claim in &record.claims{ui.label(format!("{}: {}",language.translate(match claim.kind{crate::design_process::intent::ClaimKind::Observation=>"Observation",crate::design_process::intent::ClaimKind::Inference=>"Inference",crate::design_process::intent::ClaimKind::DesignCompletion=>"Design completion"}),claim.text));}
       for statement in &record.statements{ui.label(format!("{}: {}",language.translate(statement.kind.label()),statement.text));}
       ui.separator();
      }
     });
    }
   }
   if !self.host.document.editor.project.design_process.decisions.is_empty(){
    ui.collapsing(language.translate("Choice history and saved versions"),|ui|{
     let current=&self.host.document.editor.project.design_process.current_choice;
     for decision in &self.host.document.editor.project.design_process.decisions{let label=format!("{} · {}",if current.as_ref()==Some(&decision.id){language.translate("Current choice")}else{language.translate("Past choice")},decision.reason);if ui.selectable_label(self.process_ui.history.as_ref()==Some(&decision.id),label).clicked(){self.process_ui.history=Some(decision.id.clone());}}
     if let Some(decision)=self.process_ui.history.as_ref().and_then(|id|self.host.document.editor.project.design_process.decisions.iter().find(|d|&d.id==id)){
      ui.label(&decision.operator_label);ui.label(if decision.reason.trim().is_empty(){language.translate("Reason not recorded yet")}else{&decision.reason});
      let state=cache.as_ref().and_then(|c|c.1.as_ref().ok()).map(|c|if c.fingerprint==decision.comparison.fingerprint{"Saved choice inputs unchanged"}else{"Saved choice inputs are stale"}).unwrap_or("Saved choice references need review");ui.label(language.translate(state));
      ui.label(language.translate("Saved candidate snapshots are read-only; current candidates remain editable."));
      for version in &decision.comparison.candidates{
       let parametric=self.host.document.editor.project.design_process.tasks.iter().find(|t|t.id==decision.task_id).is_some_and(|t|t.intent.as_ref().is_some_and(|v|v.parametric.is_some()));Self::parameter_version_summary(ui,language,version,parametric);
       if let Some(m)=&version.mass_metrics {ui.label(format!("{} · {}: {:.3} m² · {}: {:.3} m³",version.name,language.translate("Mass footprint sum"),m.footprint_sum_m2,language.translate("Concept envelope volume sum"),m.envelope_volume_sum_m3));}
       else{ui.label(format!("{} · {:.3} m² · {:.3} m³",version.name,version.metrics.area_m2,version.metrics.volume_m3));}
      }
      Self::design_comparison_plans(ui,language,&decision.comparison);
     }
    });
   }
   if !self.process_ui.error.is_empty(){ui.colored_label(Color32::from_rgb(224,125,105),super::checks_ui::original_read_error_label(language,&self.process_ui.error)).on_hover_text(&self.process_ui.error);}
   if !self.process_ui.status.is_empty(){ui.label(language.translate(&self.process_ui.status));}
  });
        self.process_ui.open = open;
        self.process_ui.cache = cache;
        let result = match action {
            Some(Action::Create) => {
                let editor = &self.host.document.editor;
                let source_ids = if self.process_ui.use_selection {
                    editor.selection_ids()
                } else {
                    editor
                        .project
                        .elements
                        .iter()
                        .filter(|e| {
                            e.story == editor.story
                                && e.design_option.is_none()
                                && e.worksheet.is_none()
                                && matches!(e.shape, Shape::Zone { .. } | Shape::ConceptMass { .. })
                        })
                        .map(|e| e.id)
                        .collect()
                };
                let mut statements = vec![];
                for (kind, value) in [
                    (StatementKind::Requirement, &self.process_ui.requirements),
                    (StatementKind::Preference, &self.process_ui.preferences),
                    (StatementKind::Assumption, &self.process_ui.assumptions),
                    (StatementKind::Unknown, &self.process_ui.unknowns),
                ] {
                    if !value.trim().is_empty() {
                        statements.push(Statement {
                            kind,
                            text: value.clone(),
                        });
                    }
                }
                let id = crate::storage::new_document_identity();
                let mass =
                    !source_ids.is_empty()
                        && source_ids.iter().all(|id| {
                            editor.project.elements.iter().any(|e| {
                                e.id == *id && matches!(e.shape, Shape::ConceptMass { .. })
                            })
                        });
                let proposal = service::prepare_create(
                    editor,
                    &id,
                    service::Draft {
                        name: self.process_ui.name.clone(),
                        question: self.process_ui.question.clone(),
                        statements,
                        candidate_names: self.process_ui.names.clone(),
                        source_ids,
                        entry: Entry::ManualInterface,
                    },
                );
                self.process_set_preview(
                    id,
                    if mass {
                        "Create editable mass candidates"
                    } else {
                        "Create editable space candidates"
                    },
                    proposal,
                );
                Ok(())
            }
            Some(Action::Activate(option)) => self.activate_process_option(Some(option)),
            Some(Action::Select(id)) => {
                self.process_select_space(id);
                Ok(())
            }
            Some(Action::Edit) => {
                let id = crate::storage::new_document_identity();
                let proposal = self
                    .process_ui
                    .selected_space
                    .ok_or("Select an active candidate space".into())
                    .and_then(|space| {
                        service::prepare_edit(
                            &self.host.document.editor,
                            space,
                            Point::new(self.process_ui.dx, self.process_ui.dy),
                            Some(self.process_ui.height),
                        )
                    });
                self.process_set_preview(
                    id,
                    if task.as_ref().is_some_and(|t| t.is_mass()) {
                        "Edit candidate mass"
                    } else {
                        "Edit candidate space"
                    },
                    proposal,
                );
                Ok(())
            }
            Some(Action::Choose(fingerprint)) => {
                let id = crate::storage::new_document_identity();
                let proposal = task
                    .as_ref()
                    .ok_or("Current exploration changed".into())
                    .and_then(|task| {
                        if self.process_ui.reviewed_comparison.as_ref() == Some(&fingerprint) {
                            let comparison = task.compare(&self.host.document.editor.project)?;
                            service::prepare_reviewed_choice(
                                &self.host.document.editor,
                                &id,
                                &task.id,
                                self.process_ui.chosen.ok_or("Choose a candidate")?,
                                self.process_ui.reason.clone(),
                                fingerprint,
                                comparison.current_basis_sha256,
                            )
                        } else {
                            service::prepare_choose(
                                &self.host.document.editor,
                                &id,
                                &task.id,
                                self.process_ui.chosen.ok_or("Choose a candidate")?,
                                self.process_ui.reason.clone(),
                                fingerprint,
                                Entry::ManualInterface,
                                "Local manual interface; not client approval".into(),
                            )
                        }
                    });
                self.process_set_preview(id, "Record choice and candidate versions", proposal);
                Ok(())
            }
            Some(Action::Lock(option, locked)) => {
                let id = crate::storage::new_document_identity();
                let proposal = task
                    .as_ref()
                    .ok_or("Current exploration changed".into())
                    .and_then(|task| {
                        service::prepare_lock(&self.host.document.editor, &task.id, option, locked)
                    });
                self.process_set_preview(
                    id,
                    if task.as_ref().is_some_and(|t| t.is_mass()) {
                        "Change candidate mass locks"
                    } else {
                        "Change candidate space locks"
                    },
                    proposal,
                );
                Ok(())
            }
            Some(Action::Checks) => {
                // Reuse the comparison's exact candidate/object scope rather
                // than making the user search through earlier studies.
                let active = self
                    .host
                    .document
                    .editor
                    .project
                    .plan_options
                    .active_design_option;
                let focus = self
                    .process_ui
                    .check_summaries
                    .iter()
                    .find(|(option, _)| Some(*option) == active)
                    .and_then(|(_, source)| source.as_ref().ok().and_then(Option::as_ref))
                    .and_then(|source| source.entries.first())
                    .map(|entry| entry.check.definition.id.clone());
                self.checks_ui
                    .open_focused(self.host.document.editor.document_session(), focus);
                self.process_ui.open = false;
                Ok(())
            }
            Some(Action::Canvas) => {
                self.process_ui.open = false;
                Ok(())
            }
            Some(Action::Save) => {
                self.dispatch_workbench(crate::workbench::WorkbenchCommand::Save);
                Ok(())
            }
            _ => Ok(()),
        };
        if let Err(e) = result {
            self.process_ui.error = e;
        }
    }
    fn design_snapshot_bounds<'a>(
        elements: impl Iterator<Item = &'a crate::model::Element>,
    ) -> Option<(Point, Point)> {
        let mut min = Point::new(f64::INFINITY, f64::INFINITY);
        let mut max = Point::new(f64::NEG_INFINITY, f64::NEG_INFINITY);
        for e in elements {
            if !matches!(e.shape, Shape::ConceptMass { .. } | Shape::Zone { .. }) {
                continue;
            }
            if let Some((outline, _)) = e.authored_prism_rings() {
                for p in outline {
                    min.x = min.x.min(p.x);
                    min.y = min.y.min(p.y);
                    max.x = max.x.max(p.x);
                    max.y = max.y.max(p.y);
                }
            }
        }
        (min.x.is_finite() && min.y.is_finite() && max.x.is_finite() && max.y.is_finite())
            .then_some((min, max))
    }
    pub(super) fn design_comparison_plans(
        ui: &mut Ui,
        language: UiLanguage,
        comparison: &Comparison,
    ) {
        ui.label(language.translate("Same scale and model coordinates · read-only plan outlines"));
        ui.small(
            language
                .translate("M: concept mass · Z: Zone; model geometry, not surveyed or approved"),
        );
        let Some(bounds) = Self::design_snapshot_bounds(
            comparison.candidates.iter().flat_map(|c| c.elements.iter()),
        ) else {
            ui.label(language.translate("No supported footprint in these candidates"));
            return;
        };
        ui.small(format!(
            "{}: X {:.3} m · Y {:.3} m",
            language.translate("Shared lower-left model coordinate"),
            bounds.0.x,
            bounds.0.y
        ));
        // Fix both viewports before laying out candidate names. Never fit or
        // recenter individual candidates: translations must remain visible.
        let width = ui.available_width().max(1.);
        for c in &comparison.candidates {
            ui.label(egui::RichText::new(&c.name).strong());
            Self::design_snapshot_plan_in_bounds(ui, &c.elements, bounds, Some(width));
            if Self::design_snapshot_bounds(c.elements.iter()).is_none() {
                ui.small(language.translate("No supported footprint in this candidate"));
            }
        }
    }
    pub(super) fn design_snapshot_plan(ui: &mut Ui, elements: &[crate::model::Element]) {
        let Some(bounds) = Self::design_snapshot_bounds(elements.iter()) else {
            return;
        };
        Self::design_snapshot_plan_in_bounds(ui, elements, bounds, None);
    }
    fn design_snapshot_plan_in_bounds(
        ui: &mut Ui,
        elements: &[crate::model::Element],
        (min, max): (Point, Point),
        comparison_width: Option<f32>,
    ) {
        let (response, painter) = ui.allocate_painter(
            vec2(
                comparison_width.unwrap_or_else(|| ui.available_width().max(120.)) as f64,
                if comparison_width.is_some() {
                    160.
                } else {
                    135.
                },
            ),
            Sense::hover(),
        );
        let mut rect = response.rect.shrink(8.);
        if comparison_width.is_some() {
            rect.max.y -= 25.;
        }
        if !rect.is_positive() {
            return;
        }
        let scale = (rect.width() as f64 / (max.x - min.x).max(0.01))
            .min(rect.height() as f64 / (max.y - min.y).max(0.01));
        let screen = |p: Point| {
            pos2(
                rect.min.x as f64 + (p.x - min.x) * scale,
                rect.max.y as f64 - (p.y - min.y) * scale,
            )
        };
        for e in elements {
            if let Some((outline, holes)) = e.authored_prism_rings() {
                for path in std::iter::once(&outline).chain(holes.iter()) {
                    let mut p = path.iter().copied().map(screen).collect::<Vec<_>>();
                    if let Some(first) = p.first().copied() {
                        p.push(first);
                    }
                    painter.add(egui::Shape::line(p, Stroke::new(1.4, ACCENT)));
                }
                painter.text(
                    screen(match &e.shape {
                        Shape::Zone { spec } => spec.label_point(),
                        _ => Point::new(
                            outline.iter().map(|p| p.x).sum::<f64>() / outline.len() as f64,
                            outline.iter().map(|p| p.y).sum::<f64>() / outline.len() as f64,
                        ),
                    }),
                    Align2::CENTER_CENTER,
                    format!(
                        "{}{}",
                        if matches!(e.shape, Shape::ConceptMass { .. }) {
                            "M"
                        } else {
                            "Z"
                        },
                        e.id
                    ),
                    FontId::proportional(10.),
                    INK,
                );
            }
        }
        if comparison_width.is_some() && rect.width() > 0. && rect.height() > 0. {
            painter.rect_stroke(
                rect,
                0.,
                Stroke::new(0.5, INK.gamma_multiply(0.35)),
                egui::StrokeKind::Inside,
            );
            let max_m = (rect.width() as f64 * 0.35).min(80.) / scale;
            if max_m.is_finite() && max_m > 0. {
                let power = 10_f64.powf(max_m.log10().floor());
                let length = [5., 2., 1.]
                    .into_iter()
                    .map(|v| v * power)
                    .find(|v| *v <= max_m)
                    .unwrap_or(power);
                let a = pos2(rect.min.x as f64, rect.max.y as f64 + 11.);
                let b = a + vec2(length * scale, 0.);
                painter.line_segment([a, b], Stroke::new(1., INK));
                for p in [a, b] {
                    painter
                        .line_segment([p - vec2(0., 3.), p + vec2(0., 3.)], Stroke::new(1., INK));
                }
                painter.text(
                    b + vec2(5., 0.),
                    Align2::LEFT_CENTER,
                    format!("{length} m"),
                    FontId::proportional(10.),
                    INK,
                );
            }
        }
    }
}
