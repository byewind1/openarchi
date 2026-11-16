use super::*;
use crate::workbench::creation_presets::CreationPresets;
use crate::workbench::environment::{Configuration, EnvironmentService, Panels, Profile};
use crate::workbench::workspace::NativeWorkspace;
use crate::{
    application_host::HostCommand,
    workbench::{CommandRegistry, EnvironmentState, WorkMode, WorkbenchCommand},
};

const ENVIRONMENT_DIRTY_DRAFT: &str =
    "Save or cancel the current environment edits before reviewing another configuration";
const ENVIRONMENT_LIVE_CHANGED: &str =
    "The live workspace changed; capture the current layout or cancel before applying";

#[derive(Default)]
pub(super) struct WorkbenchUi {
    pub environment: EnvironmentState,
    preference_path: Option<PathBuf>,
    preference_error: String,
    pub(super) search_open: bool,
    task_controls_open: bool,
    search: String,
    pub(super) active_profile: String,
    pending_panels: Option<Panels>,
    pub(super) workspace: NativeWorkspace,
    #[cfg(test)]
    pub(super) panel_rects: std::collections::BTreeMap<String, Rect>,
    pub(super) layout_generation: u64,
    pending_workspace: Option<NativeWorkspace>,
    pending_creation_presets: Option<(CreationPresets, (f64, f64, String), u64)>,
    pub(super) configuration_open: bool,
    pub(super) shortcut_recording: Option<(String, bool)>,
    pub(super) shortcuts_expand: bool,
    pub(super) shortcut_filter: String,
    pub(super) configuration_draft: Option<(Configuration, Profile)>,
    configuration_draft_baseline: Option<Configuration>,
    configuration_live_baseline: Option<(Panels, NativeWorkspace)>,
    configuration_creation_baseline: Option<(f64, f64, String)>,
    pub(super) configuration_error: String,
    configuration_status: String,
    pub(super) transfer_path: String,
    pub(super) reviewed_proposal: Option<String>,
    pub(super) proposal_record: Option<crate::workbench::environment::proposal::View>,
}
impl AtelierApp {
    pub(super) fn initialize_work_mode(&mut self, directory: &std::path::Path) {
        let path = directory.join("work-mode.json");
        self.workbench.preference_error.clear();
        match crate::workbench::preferences::load(&path) {
            Ok(environment) => self.workbench.environment = environment,
            Err(error) => {
                self.workbench.environment = EnvironmentState::default();
                self.workbench.preference_error = error;
            }
        }
        self.workbench.preference_path = Some(path);
        match EnvironmentService::open(
            &directory.join("work-environment.json"),
            self.workbench.environment.mode,
        ) {
            Ok(service) => {
                let profile = service
                    .configuration
                    .resolve(&service.configuration.active)
                    .expect("validated active profile");
                self.workbench.active_profile = profile.id.clone();
                self.workbench.environment.switch_mode(profile.base_mode);
                if service.existed {
                    self.apply_environment_panels(profile.panels);
                    self.apply_environment_workspace(profile.workspace.clone());
                    self.apply_environment_creation_presets(profile.creation_presets.clone());
                }
                self.workbench.configuration_error = service.recovery.clone().unwrap_or_default();
                self.host.environment = Some(service);
            }
            Err(error) => {
                self.host.environment = None;
                self.workbench.configuration_error = error;
            }
        }
    }
    pub(super) fn switch_work_mode(&mut self, mode: WorkMode) {
        let id = mode.profile().id;
        self.activate_environment_profile(&id);
    }
    pub(super) fn current_environment_profile(&self) -> Profile {
        self.host
            .environment
            .as_ref()
            .and_then(|s| s.configuration.resolve(&self.workbench.active_profile).ok())
            .unwrap_or_else(|| Profile::builtin(self.workbench.environment.mode))
    }
    fn apply_environment_panels(&mut self, panels: Panels) {
        if self.workbench_pointer_active() || self.other_menu_gated() {
            self.workbench.pending_panels = Some(panels);
            return;
        }
        self.standard_toolbar_visible = panels.standard_toolbar;
        self.edit_toolbar_visible = panels.edit_toolbar;
        self.show_left = panels.toolbox;
        self.show_right = panels.navigator;
        self.info_box_visible = panels.info_box;
        self.quick_options_visible = panels.quick_options;
        self.quick_options_palette_visible = panels.quick_palette;
        self.workbench.pending_panels = None;
    }
    pub(super) fn current_environment_panels(&self) -> Panels {
        Panels {
            toolbox: self.show_left,
            navigator: self.show_right,
            info_box: self.info_box_visible,
            quick_options: self.quick_options_visible,
            quick_palette: self.quick_options_palette_visible,
            standard_toolbar: self.standard_toolbar_visible,
            edit_toolbar: self.edit_toolbar_visible,
        }
    }
    pub(super) fn toggle_work_toolbar(&mut self, standard: bool) {
        if self.workbench_pointer_active() || self.menu_gated() {
            self.host.document.editor.status =
                "Finish or cancel the current model interaction before applying settings".into();
            return;
        }
        let visible = if standard {
            &mut self.standard_toolbar_visible
        } else {
            &mut self.edit_toolbar_visible
        };
        *visible = !*visible;
    }
    fn apply_environment_creation_presets(&mut self, presets: Option<CreationPresets>) {
        self.workbench.pending_creation_presets = presets.map(|p| {
            (
                p,
                self.host.creation_defaults(),
                self.host.document.editor.document_session(),
            )
        });
        self.flush_environment_creation_presets();
    }
    fn flush_environment_creation_presets(&mut self) {
        if self.workbench_pointer_active() || self.other_menu_gated() {
            return;
        }
        let Some((presets, baseline, session)) = self.workbench.pending_creation_presets.take()
        else {
            return;
        };
        if session != self.host.document.editor.document_session()
            || baseline != self.host.creation_defaults()
        {
            self.workbench.configuration_error =
                "Live creation defaults changed; capture them explicitly or cancel before applying"
                    .into();
            return;
        }
        if let Err(error) = self.host.apply_creation_presets(&presets) {
            self.workbench.configuration_error = error;
        }
    }
    pub(super) fn restore_environment_creation_presets(&mut self) {
        let presets = self.current_environment_profile().creation_presets;
        self.apply_environment_creation_presets(presets);
    }
    fn apply_environment_workspace(&mut self, workspace: NativeWorkspace) {
        if self.workbench_pointer_active() || self.other_menu_gated() {
            self.workbench.pending_workspace = Some(workspace);
            return;
        }
        self.workbench.workspace = workspace;
        self.workbench.layout_generation = self.workbench.layout_generation.wrapping_add(1);
        self.workbench.pending_workspace = None;
    }
    pub(super) fn save_current_workspace(&mut self) -> Result<(), String> {
        if self.workbench_pointer_active() || self.menu_gated() {
            return Err(
                "Finish or cancel the current model interaction before applying settings".into(),
            );
        }
        let mut profile = self.current_environment_profile();
        profile.workspace = self.workbench.workspace.clone();
        profile.panels = self.current_environment_panels();
        let service = self
            .host
            .environment
            .as_mut()
            .ok_or("Work environment storage is unavailable")?;
        let mut config = service.configuration.clone();
        config.put(profile.layer());
        config.active = profile.id;
        let outcome = service.save(config)?;
        self.workbench.configuration_error = outcome.warning.unwrap_or_default();
        self.workbench.configuration_status = "Workspace layout saved".into();
        Ok(())
    }
    pub(super) fn reset_workspace_layout(&mut self) -> Result<(), String> {
        if self.workbench_pointer_active() || self.menu_gated() {
            return Err(
                "Finish or cancel the current model interaction before applying settings".into(),
            );
        }
        self.apply_environment_panels(Panels::default());
        self.apply_environment_workspace(NativeWorkspace::default());
        Ok(())
    }
    pub(super) fn workspace_layout_dirty(&self) -> bool {
        if self.workbench.pending_workspace.is_some() {
            return false;
        }
        let p = self.current_environment_profile();
        p.workspace != self.workbench.workspace || p.panels != self.current_environment_panels()
    }
    fn activate_environment_profile(&mut self, id: &str) {
        let profile = self
            .host
            .environment
            .as_ref()
            .and_then(|s| s.configuration.resolve(id).ok())
            .or_else(|| {
                WorkMode::ALL
                    .iter()
                    .copied()
                    .map(Profile::builtin)
                    .find(|p| p.id == id)
            });
        let Some(profile) = profile else {
            return;
        };
        // Keep the legacy preference readable until a structured configuration is explicitly saved.
        if let Some(service) = self.host.environment.as_mut().filter(|s| s.existed) {
            let mut draft = service.configuration.clone();
            draft.active = id.into();
            match service.save(draft) {
                Ok(outcome) => {
                    self.workbench.configuration_error = outcome.warning.unwrap_or_default()
                }
                Err(error) => {
                    self.workbench.configuration_error = error;
                    return;
                }
            }
        }
        if let Some(service) = self.host.environment.as_mut().filter(|s| !s.existed) {
            service.configuration.active = id.to_owned();
        }
        self.workbench.active_profile = profile.id.clone();
        self.workbench.environment.switch_mode(profile.base_mode);
        if self.host.environment.as_ref().is_some_and(|s| s.existed) {
            self.apply_environment_panels(profile.panels);
            self.apply_environment_workspace(profile.workspace.clone());
            self.apply_environment_creation_presets(profile.creation_presets.clone());
        }
        if let Some(path) = &self.workbench.preference_path {
            self.workbench.preference_error =
                crate::workbench::preferences::save(path, profile.base_mode)
                    .err()
                    .unwrap_or_default();
        }
    }
    fn open_environment_configuration(&mut self) {
        if matches!(
            self.workbench.configuration_error.as_str(),
            ENVIRONMENT_DIRTY_DRAFT | ENVIRONMENT_LIVE_CHANGED
        ) {
            self.workbench.configuration_error.clear();
        }
        self.workbench.reviewed_proposal = None;
        if self
            .workbench
            .proposal_record
            .as_ref()
            .is_some_and(|p| p.request.project_id != self.host.document.editor.project_identity())
        {
            self.workbench.proposal_record = None;
        }
        self.workbench.configuration_open = true;
        let mut profile = self.current_environment_profile();
        let configuration = self
            .host
            .environment
            .as_ref()
            .map(|s| s.configuration.clone())
            .unwrap_or_else(|| Configuration::new(self.workbench.environment.mode));
        // The current canvas layout is user input even before it has been saved.
        // Keep it in the existing draft; cancelling settings does not reset it.
        if self.workspace_layout_dirty() {
            profile.panels = self.current_environment_panels();
            profile.workspace = self.workbench.workspace.clone();
        }
        // An opted-in mode must not revert later direct toolbar input merely
        // because settings were opened. Carry representable live defaults into
        // the same draft; imports/proposals then see a genuine unsaved change.
        if profile.creation_presets.is_some() {
            let (mass, zone, purpose) = self.host.creation_defaults();
            match CreationPresets::from_metres(mass, zone, &purpose) {
                Ok(presets) => profile.creation_presets = Some(presets),
                Err(error) => self.workbench.configuration_error = error,
            }
        }
        self.set_environment_draft(configuration.clone(), profile, configuration);
        self.workbench.configuration_status.clear();
    }
    fn set_environment_draft(
        &mut self,
        configuration: Configuration,
        profile: Profile,
        baseline: Configuration,
    ) {
        self.workbench.shortcut_recording = None;
        self.workbench.configuration_draft = Some((configuration, profile));
        self.workbench.configuration_draft_baseline = Some(baseline);
        self.workbench.configuration_creation_baseline = Some(self.host.creation_defaults());
        self.workbench.configuration_live_baseline = Some((
            self.current_environment_panels(),
            self.workbench.workspace.clone(),
        ));
    }
    fn require_clean_environment_draft(&self) -> Result<(), String> {
        if self.workbench.configuration_draft.is_none() {
            if let Some(presets) = self.current_environment_profile().creation_presets {
                let (mass, zone, purpose) = self.host.creation_defaults();
                if CreationPresets::from_metres(mass, zone, &purpose)? != presets {
                    return Err(ENVIRONMENT_DIRTY_DRAFT.into());
                }
            }
        }
        self.require_unchanged_environment_layout()?;
        if let Some((configuration, profile)) = &self.workbench.configuration_draft {
            let mut current = configuration.clone();
            current.apply_profile_draft(profile);
            let clean = self
                .workbench
                .configuration_draft_baseline
                .as_ref()
                .and_then(|baseline| current.differences_from(baseline).ok())
                .is_some_and(|d| d.profiles.is_empty() && d.active_before == d.active_after);
            if !clean {
                return Err(ENVIRONMENT_DIRTY_DRAFT.into());
            }
        }
        Ok(())
    }
    fn require_unchanged_environment_layout(&self) -> Result<(), String> {
        if self
            .workbench
            .configuration_draft
            .as_ref()
            .is_some_and(|(_, p)| p.creation_presets.is_some())
        {
            let (mass, zone, purpose) = self.host.creation_defaults();
            CreationPresets::from_metres(mass, zone, &purpose)?;
        }
        if self
            .workbench
            .configuration_draft
            .as_ref()
            .is_some_and(|(_, p)| p.creation_presets.is_some())
            && self
                .workbench
                .configuration_creation_baseline
                .as_ref()
                .is_some_and(|before| *before != self.host.creation_defaults())
        {
            return Err(
                "Live creation defaults changed; capture them explicitly or cancel before applying"
                    .into(),
            );
        }
        if self
            .workbench
            .configuration_live_baseline
            .as_ref()
            .is_some_and(|(panels, workspace)| {
                *panels != self.current_environment_panels()
                    || *workspace != self.workbench.workspace
            })
        {
            return Err(
                "The live workspace changed; capture the current layout or cancel before applying"
                    .into(),
            );
        }
        Ok(())
    }
    pub(super) fn save_environment_draft(&mut self) -> Result<(), String> {
        if self.workbench_pointer_active() || self.other_menu_gated() {
            return Err(
                "Finish or cancel the current model interaction before applying settings".into(),
            );
        }
        self.require_unchanged_environment_layout()?;
        let (mut config, profile) = self
            .workbench
            .configuration_draft
            .clone()
            .ok_or("No work environment draft")?;
        config.apply_profile_draft(&profile);
        let service = self
            .host
            .environment
            .as_mut()
            .ok_or("Work environment storage is unavailable")?;
        let outcome = if let Some(id) = self.workbench.reviewed_proposal.as_deref() {
            service.confirm_proposal(id, self.host.document.editor.project_identity(), config)?
        } else {
            service.save(config)?
        };
        if let Some(id) = self.workbench.reviewed_proposal.take() {
            self.workbench.proposal_record = service
                .read_proposal(&id, self.host.document.editor.project_identity())
                .ok();
        }
        self.workbench.configuration_error = outcome.warning.unwrap_or_default();
        self.workbench.active_profile = profile.id.clone();
        self.workbench.environment.switch_mode(profile.base_mode);
        self.apply_environment_panels(profile.panels);
        self.apply_environment_workspace(profile.workspace.clone());
        self.apply_environment_creation_presets(profile.creation_presets.clone());
        let saved = self
            .host
            .environment
            .as_ref()
            .unwrap()
            .configuration
            .clone();
        self.set_environment_draft(saved.clone(), profile, saved);
        self.workbench.configuration_status = "Work environment saved".into();
        Ok(())
    }

    pub(super) fn dispatch_history(&mut self, redo: bool) {
        if self.navigation.mode != super::viewport_navigation::Mode::Edit {
            return;
        }
        let canvas_before = self.capture_task_canvas();
        let revision_before = self.host.document.editor.revision;
        let command = if redo {
            HostCommand::Redo
        } else {
            HostCommand::Undo
        };
        if let Err(error) = self.host.dispatch(self.host.context(), command) {
            self.host.document.editor.status = error;
        } else if self.host.document.editor.revision != revision_before {
            // This notice described the previous model state. The durable
            // operation record and any unresolved-operation guards stay intact.
            self.process_ui.error.clear();
        }
        self.finish_task_context_change(canvas_before);
    }
    /// True while a modal dialog or a special pointer mode blocks menu
    /// commands, mirroring the egui shortcut gating.
    pub(super) fn menu_gated(&self) -> bool {
        self.workbench.configuration_open || self.other_menu_gated()
    }
    pub(super) fn other_menu_gated(&self) -> bool {
        self.other_menu_gated_with_trace(true)
    }
    /// Trace controls may replace their own pointer mode, but never a modal
    /// draft or another tool's gesture.
    pub(super) fn trace_menu_gated(&self) -> bool {
        self.workbench.configuration_open || self.other_menu_gated_with_trace(false)
    }
    fn other_menu_gated_with_trace(&self, include_trace: bool) -> bool {
        self.process_ui.preview.is_some()
            || self.site_dialog.is_some()
            || self.edit_dialog.is_some()
            || self.reshape_session.is_some()
            || self.file_action.is_some()
            || self.help
            || self.close_prompt
            || self.recovery_open
            || self.element_settings.is_some()
            || self.layer_settings.is_some()
            || self.stair_settings_open
            || self.grid_dialog.is_some()
            || self.wall_settings_dialog.is_some()
            || self.command_dialog.is_some()
            || self.plan_option_dialog.is_some()
            || self.transfer_dialog.is_some()
            || self.guide_ui.values.is_some()
            || (include_trace && self.trace.pointer_mode_active())
            || self.orientation_pick_active
            || self.rotated_grid_tool.is_some()
            || self.pick_armed
            || self.inject_armed
            || self.guide_ui.action.is_some()
            || self.guide_ui.drag.is_some()
    }

    pub(super) fn execute_workbench(&mut self, command: WorkbenchCommand) -> Result<(), String> {
        if self.menu_gated() {
            return Err("Workbench is busy".into());
        }
        if let Some(reason) = command.disabled_reason(&self.host.document.editor) {
            return Err(reason.into());
        }
        if let Some(label) = command.edit_label() {
            if !self.edit_command_enabled(label) {
                return Err("Unavailable for the current model types".into());
            }
            self.dispatch_edit(label);
            return Ok(());
        }
        match command {
            WorkbenchCommand::InputX
            | WorkbenchCommand::InputY
            | WorkbenchCommand::InputDistance
            | WorkbenchCommand::InputAngle => {
                return self.request_pointer_drag_field(match command {
                    WorkbenchCommand::InputDistance => 0,
                    WorkbenchCommand::InputAngle => 1,
                    WorkbenchCommand::InputX => 2,
                    WorkbenchCommand::InputY => 3,
                    _ => unreachable!(),
                });
            }
            // All editing adapters return through the original dispatcher above.
            WorkbenchCommand::Drag
            | WorkbenchCommand::Rotate
            | WorkbenchCommand::Mirror
            | WorkbenchCommand::Elevate
            | WorkbenchCommand::Resize
            | WorkbenchCommand::Multiply
            | WorkbenchCommand::ToolSettings => unreachable!(),
            WorkbenchCommand::Checks => self.open_current_design_checks(),
            WorkbenchCommand::Conditions => self.open_project_conditions(ConditionsGroup::Sources),
            WorkbenchCommand::Reports => self.open_report_workspace()?,
            WorkbenchCommand::Drawings => {
                self.drawings_ui.manager = !self.drawings_ui.manager;
            }
            WorkbenchCommand::Plugins => self.plugins_ui.open = true,
            WorkbenchCommand::Select => self.activate_named_tool_legacy("Arrow"),
            WorkbenchCommand::Wall => self.activate_named_tool_legacy("Wall"),
            WorkbenchCommand::ConceptMass => self.activate_named_tool_legacy("Concept mass"),
            WorkbenchCommand::Zone => self.activate_named_tool_legacy("Zone"),
            WorkbenchCommand::Spaces => self.zone_analysis_open = true,
            WorkbenchCommand::Explore => self.open_design_process(),
            WorkbenchCommand::Sources => self.open_project_conditions(ConditionsGroup::Sources),
            WorkbenchCommand::Program => {
                self.open_project_conditions(ConditionsGroup::Requirements)
            }
            WorkbenchCommand::Evidence => self.open_project_conditions(ConditionsGroup::Sources),
            WorkbenchCommand::Rules => self.open_project_conditions(ConditionsGroup::Requirements),
            WorkbenchCommand::Terrain => self.open_project_conditions(ConditionsGroup::Site),
            WorkbenchCommand::Site => self.open_project_conditions(ConditionsGroup::Site),
            WorkbenchCommand::Plan => {
                self.ensure_canvas_selection_mode();
                self.view = View::Plan;
            }
            WorkbenchCommand::Model => {
                // The worksheet stays in DrawingDocument; only leave its editing scope.
                if self
                    .host
                    .document
                    .editor
                    .project
                    .plan_options
                    .active_worksheet
                    .is_some()
                {
                    self.host.document.editor.switch_worksheet(None);
                }
                self.ensure_canvas_selection_mode();
                self.view = View::Model;
            }
            WorkbenchCommand::Save => self.request_project_save(),
            WorkbenchCommand::Undo => self.dispatch_history(false),
            WorkbenchCommand::Redo => self.dispatch_history(true),
            WorkbenchCommand::Environment => self.open_environment_configuration(),
            WorkbenchCommand::SaveWorkspace => self.save_current_workspace()?,
            WorkbenchCommand::ResetWorkspace => self.reset_workspace_layout()?,
        }
        Ok(())
    }
    pub(super) fn dispatch_workbench(&mut self, command: WorkbenchCommand) {
        if let Err(error) = self.execute_workbench(command) {
            self.host.document.editor.status = error;
        }
    }
    #[cfg(unix)]
    pub(super) fn execute_remote_workbench(
        &mut self,
        endpoint: &crate::automation_ipc::Endpoint,
        params: &crate::automation_ipc::WorkbenchInvoke,
    ) -> serde_json::Value {
        use serde_json::json;
        let command = match endpoint.validate_workbench(&self.host.document.editor, params) {
            Ok(command) => command,
            Err(error) => return error,
        };
        if self.workbench_pointer_active()
            || self.project_conditions_has_draft()
            || self.pending_design_parameters()
            || self.drawings_ui.report_draft.is_some()
        {
            return json!({"error":{"code":"USER_INTERACTION_ACTIVE"}});
        }
        match self.execute_workbench(command) {
            Ok(()) => {
                json!({"result":{"status":"completed","request_id":params.request_id,"target":params.target,"command_id":params.command_id,"revision_before":params.expected_revision,"revision_after":self.host.document.editor.revision,"model_changed":false,"receipt_scope":"process"}})
            }
            Err(message) => json!({"error":{"code":"COMMAND_BLOCKED","message":message}}),
        }
    }
    pub(super) fn workbench_pointer_active(&self) -> bool {
        let editor = &self.host.document.editor;
        self.pointer_drag.is_some()
            || self.wall_insert.is_some()
            || self.mass_parameter_draft_active()
            || self.canvas_selection_box.is_some()
            || self.research_ui.canvas_move.is_some()
            || editor.anchor.is_some()
            || !editor.polyline_points.is_empty()
            || !editor.wall_points.is_empty()
            || editor.slab_polygon_draft().is_some()
            || editor.roof_polygon_draft().is_some()
            || editor.roof_draft.is_some()
            || editor.slab_draft.is_some()
            || editor.beam_arc_draft.is_some()
            || !self.marquee_points.is_empty()
            || !self.marquee_3d_points.is_empty()
    }
    pub(super) fn workbench_ui(&mut self, ui: &mut Ui) {
        self.flush_environment_creation_presets();
        if let Some(panels) = self.workbench.pending_panels {
            self.apply_environment_panels(panels);
        }
        if let Some(workspace) = self.workbench.pending_workspace.clone() {
            self.apply_environment_workspace(workspace);
        }
        self.sync_process_space();
        let language = self.language;
        let mut action = None;
        let mut plugin_command = None;
        let mut option_action = None;
        let mut task_action = None;
        let mut mass_action = None;
        let concept_task = self
            .host
            .document
            .editor
            .project
            .design_process
            .task()
            .cloned();
        let concept_busy = self.workbench_pointer_active()
            || self.menu_gated()
            || self.pending_design_parameters();
        if !self.ai_workspace.enabled {
            egui::Panel::top("workbench_tasks")
                .frame(
                    Frame::new()
                        .fill(PANEL)
                        .inner_margin(Margin::symmetric(10, 6)),
                )
                .show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        let current = self.current_environment_profile();
                        let profiles = self
                            .host
                            .environment
                            .as_ref()
                            .map(|s| s.configuration.profiles())
                            .unwrap_or_else(crate::workbench::environment::defaults);
                        egui::ComboBox::from_id_salt("workbench_mode")
                            .selected_text(if current.builtin_name {
                                language.translate(&current.name)
                            } else {
                                &current.name
                            })
                            .show_ui(ui, |ui| {
                                for profile in profiles {
                                    let label = if profile.builtin_name {
                                        language.translate(&profile.name)
                                    } else {
                                        &profile.name
                                    };
                                    if ui
                                        .selectable_label(current.id == profile.id, label)
                                        .clicked()
                                    {
                                        if profile.id.starts_with("user.") {
                                            self.activate_environment_profile(&profile.id);
                                        } else {
                                            self.switch_work_mode(profile.base_mode);
                                        }
                                        ui.close();
                                    }
                                }
                            });
                        if !self.workbench.preference_error.is_empty() {
                            ui.colored_label(
                                Color32::from_rgb(224, 125, 105),
                                language.translate(
                                    "Work mode preference could not be saved or restored.",
                                ),
                            )
                            .on_hover_text(&self.workbench.preference_error);
                        }

                        ui.menu_button(language.translate("Tasks and settings"), |ui| {
                            if !self.host.document.editor.selection_ids().is_empty()
                                && ui
                                    .add_enabled(
                                        !concept_busy,
                                        Button::new(language.translate("Discuss selected objects")),
                                    )
                                    .clicked()
                            {
                                self.discuss_selection();
                            }
                            let related = self.related_selection_count();
                            if related > 0
                                && ui
                                    .add_enabled(
                                        self.can_review_object_discussions(),
                                        Button::new(format!(
                                            "{} ({related})",
                                            language.translate("Related discussions")
                                        )),
                                    )
                                    .clicked()
                            {
                                self.review_selected_discussions();
                            }
                            let mut conditions_shown = false;
                            for id in &current.commands {
                                if let Some(mut command) = CommandRegistry::resolve(id) {
                                    if matches!(
                                        command,
                                        WorkbenchCommand::Conditions
                                            | WorkbenchCommand::Sources
                                            | WorkbenchCommand::Program
                                            | WorkbenchCommand::Evidence
                                            | WorkbenchCommand::Rules
                                            | WorkbenchCommand::Terrain
                                            | WorkbenchCommand::Site
                                    ) {
                                        if conditions_shown {
                                            continue;
                                        }
                                        conditions_shown = true;
                                        command = WorkbenchCommand::Conditions;
                                    }
                                    let reason =
                                        command.disabled_reason(&self.host.document.editor);
                                    let response = ui
                                        .add_enabled(
                                            reason.is_none(),
                                            Button::new(language.translate(command.label())),
                                        )
                                        .on_hover_text(format!("{}{}", id, {
                                            let hint = crate::workbench::shortcuts::hint(
                                                &current.shortcuts,
                                                id,
                                            );
                                            if hint.is_empty() {
                                                String::new()
                                            } else {
                                                format!(" · {hint}")
                                            }
                                        }));
                                    if let Some(reason) = reason {
                                        response.clone().on_hover_text(language.translate(reason));
                                    }
                                    if response.clicked() {
                                        action = Some(command);
                                    }
                                } else {
                                    ui.add_enabled(false, Button::new(id)).on_hover_text(
                                        language.translate(
                                            "Command provider is unavailable; saved ID retained",
                                        ),
                                    );
                                }
                            }
                            if ui
                                .button(language.translate("Customize work environment…"))
                                .clicked()
                            {
                                self.open_environment_configuration();
                            }
                            if self.workspace_layout_dirty()
                                && ui
                                    .button(language.translate("Save workspace layout"))
                                    .clicked()
                            {
                                action = Some(WorkbenchCommand::SaveWorkspace);
                            }
                            if !self.workbench.configuration_error.is_empty() {
                                if ui
                                    .button(language.translate("Work environment needs attention"))
                                    .on_hover_text(
                                        language.translate(&self.workbench.configuration_error),
                                    )
                                    .clicked()
                                {
                                    self.open_environment_configuration();
                                }
                            }
                            if ui.button(language.translate("Plugins and jobs…")).clicked() {
                                action = Some(WorkbenchCommand::Plugins);
                            }
                        });
                        ui.menu_button(language.translate("Checks and diagrams"), |ui| {
                            ui.set_max_width(320.);
                            if ui
                                .button(language.translate("Check current design"))
                                .clicked()
                            {
                                action = Some(WorkbenchCommand::Checks);
                            }
                            if concept_task.is_some() {
                                self.figure_task_controls(ui);
                            } else {
                                self.site_figure_controls(ui);
                            }
                        });
                        if concept_task.is_some()
                            || self.mass_space_canvas_selection().is_some()
                            || self.mass_spaces_ui.dirty
                        {
                            if ui.button(language.translate("Current task")).clicked() {
                                self.workbench.task_controls_open =
                                    !self.workbench.task_controls_open;
                            }
                        }
                        if ui.button(language.translate("Find command…")).clicked() {
                            self.workbench.search_open = true;
                        }
                    });
                });
        }
        if !self.ai_workspace.enabled && self.workbench.task_controls_open {
            let mut open = true;
            let mut close_task = false;
            let selection_before = self.host.document.editor.selection_ids();
            egui::Window::new(language.translate("Current task"))
                .id(Id::new("manual_current_task"))
                .open(&mut open)
                .collapsible(false)
                .default_width(380.)
                .default_pos(pos2(
                    (ui.ctx().content_rect().right() - 420.).max(16.) as f64,
                    (ui.available_rect_before_wrap().top() + 8.) as f64,
                ))
                .max_height((ui.available_height() - 24.).max(200.))
                .vscroll(true)
                .show(ui.ctx(), |ui| {
                    ui.set_max_width(380.);
                    if let Some(task) = &concept_task {
                        ui.vertical(|ui| {
                            egui::ComboBox::from_id_salt("concept_task_context")
                                .width(220.)
                                .selected_text(&task.name)
                                .show_ui(ui, |ui| {
                                    for saved in
                                        &self.host.document.editor.project.design_process.tasks
                                    {
                                        let available = saved.id == task.id
                                            || self
                                                .host
                                                .document
                                                .editor
                                                .project
                                                .design_process
                                                .suspended_contexts
                                                .contains_key(&saved.id);
                                        if ui
                                            .add_enabled(
                                                available && !concept_busy,
                                                egui::Button::new(&saved.name)
                                                    .selected(saved.id == task.id),
                                            )
                                            .clicked()
                                        {
                                            ui.close();
                                            if saved.id != task.id {
                                                task_action = Some(saved.id.clone());
                                            }
                                        }
                                    }
                                })
                                .response
                                .on_hover_text(format!("{}\n{}", task.name, task.question));
                            if let Some(intent) = &task.intent {
                                ui.small(language.translate("Agent concept · assumed dimensions"))
                                    .on_hover_text(&intent.geometry_assumptions);
                            }
                            ui.label(language.translate("Current candidate"));
                            let active = self
                                .host
                                .document
                                .editor
                                .project
                                .plan_options
                                .active_design_option;
                            let name = active
                                .and_then(|id| {
                                    self.host.document.editor.project.design_options.name(id)
                                })
                                .map(str::to_owned)
                                .unwrap_or_else(|| language.translate("Main model").into());
                            ui.add_enabled_ui(!concept_busy, |ui| {
                                egui::ComboBox::from_id_salt("concept_candidate_context")
                                    .width(150.)
                                    .selected_text(name)
                                    .show_ui(ui, |ui| {
                                        if ui
                                            .selectable_label(
                                                active.is_none(),
                                                language.translate("Main Model Only"),
                                            )
                                            .clicked()
                                        {
                                            option_action = Some(None);
                                            ui.close();
                                        }
                                        if let Some(name) = self
                                            .host
                                            .document
                                            .editor
                                            .project
                                            .design_options
                                            .name(task.baseline_option)
                                        {
                                            if ui
                                                .selectable_label(
                                                    active == Some(task.baseline_option),
                                                    name,
                                                )
                                                .clicked()
                                            {
                                                option_action = Some(Some(task.baseline_option));
                                                ui.close();
                                            }
                                        }
                                        for candidate in &task.candidates {
                                            let name = self
                                                .host
                                                .document
                                                .editor
                                                .project
                                                .design_options
                                                .name(candidate.option)
                                                .unwrap_or(&candidate.name);
                                            if ui
                                                .selectable_label(
                                                    active == Some(candidate.option),
                                                    name,
                                                )
                                                .clicked()
                                            {
                                                option_action = Some(Some(candidate.option));
                                                ui.close();
                                            }
                                        }
                                    });
                            });
                            if task.is_mass()
                                && task.candidates.iter().any(|c| Some(c.option) == active)
                            {
                                let selected = self.mass_space_canvas_selection();
                                let label = selected
                                    .as_ref()
                                    .map(|e| format!("#{} · {}", e.id, e.name))
                                    .unwrap_or_else(|| {
                                        language.translate("Select a source mass").into()
                                    });
                                ui.add_enabled_ui(!concept_busy, |ui| {
                                    egui::ComboBox::from_id_salt("candidate_source_mass")
                                        .width(170.)
                                        .selected_text(label)
                                        .show_ui(ui, |ui| {
                                            for mass in self
                                                .host
                                                .document
                                                .editor
                                                .project
                                                .elements
                                                .iter()
                                                .filter(|e| {
                                                    e.story == task.story
                                                        && e.design_option == active
                                                        && e.worksheet.is_none()
                                                        && matches!(
                                                            e.shape,
                                                            Shape::ConceptMass { .. }
                                                        )
                                                })
                                            {
                                                if ui
                                                    .selectable_label(
                                                        selected
                                                            .as_ref()
                                                            .is_some_and(|e| e.id == mass.id),
                                                        format!("#{} · {}", mass.id, mass.name),
                                                    )
                                                    .clicked()
                                                {
                                                    mass_action = Some(mass.id);
                                                    ui.close();
                                                }
                                            }
                                        });
                                });
                            } else {
                                if let Some(id) = self
                                    .process_ui
                                    .selected_space
                                    .or_else(|| self.mass_space_canvas_selection().map(|e| e.id))
                                {
                                    if let Some(space) = self
                                        .host
                                        .document
                                        .editor
                                        .project
                                        .elements
                                        .iter()
                                        .find(|e| e.id == id)
                                    {
                                        ui.label(&space.name);
                                    }
                                } else {
                                    ui.label(language.translate(if task.is_mass() {
                                        "Select a mass on the canvas"
                                    } else {
                                        "Select a space on the canvas"
                                    }));
                                }
                            }
                            if ui
                                .add_enabled(
                                    !concept_busy,
                                    Button::new(language.translate("Compare options…")),
                                )
                                .clicked()
                            {
                                action = Some(WorkbenchCommand::Explore);
                            }
                            if task.intent.as_ref().is_some_and(|i| i.parametric.is_some())
                                && task.candidates.iter().any(|c| Some(c.option) == active)
                                && ui
                                    .add_enabled(
                                        !concept_busy
                                            && !self.project_conditions_has_draft()
                                            && self.research_ui.draft.is_none()
                                            && self.research_ui.parameters.is_none(),
                                        Button::new(language.translate("Adjust generated wings")),
                                    )
                                    .clicked()
                            {
                                self.begin_parametric_adjustment();
                                close_task = true;
                            }
                        });
                    }

                    self.mass_space_canvas_controls(ui);
                    if action.is_some()
                        || option_action.is_some()
                        || task_action.is_some()
                        || mass_action.is_some()
                        || self.process_ui.preview.is_some()
                    {
                        close_task = true;
                    }
                });

            self.workbench.task_controls_open = open
                && !close_task
                && self.host.document.editor.selection_ids() == selection_before;
        }
        if let Some(option) = option_action {
            self.set_active_design_option(option);
        }
        if let Some(task) = task_action {
            self.preview_design_task_activation(&task);
        }
        if let Some(id) = mass_action {
            let e = &self.host.document.editor;
            if e.project.elements.iter().any(|v| {
                v.id == id
                    && v.design_option == e.project.plan_options.active_design_option
                    && v.story == e.story
                    && matches!(v.shape, Shape::ConceptMass { .. })
            }) {
                self.focus_mass_space_element(id);
            }
        }
        if self.workbench.search_open {
            let mut open = true;
            egui::Window::new(language.translate("Find command…"))
                .open(&mut open)
                .default_width(360.)
                .show(ui.ctx(), |ui| {
                    ui.add(
                        TextEdit::singleline(&mut self.workbench.search)
                            .hint_text(language.translate("Command name or ID")),
                    );
                    let query = self.workbench.search.to_lowercase();
                    for &command in CommandRegistry::commands() {
                        let label = language.translate(command.label());
                        if !format!("{label} {} {}", command.id(), command.label())
                            .to_lowercase()
                            .contains(&query)
                        {
                            continue;
                        }
                        let descriptor =
                            CommandRegistry::describe(command, &self.host.document.editor);
                        let reason = descriptor.disabled_reason;
                        let response = ui
                            .add_enabled(reason.is_none(), Button::new(label))
                            .on_hover_text(command.id());
                        if let Some(reason) = reason {
                            response.clone().on_hover_text(language.translate(reason));
                        }
                        if response.clicked() {
                            action = Some(command);
                        }
                    }
                    for entry in self.host.plugins.entries().iter().filter(|p| p.enabled) {
                        for command in &entry.manifest.commands {
                            let label = language.translate(&command.label);
                            if format!("{label} {}", command.id)
                                .to_lowercase()
                                .contains(&query)
                                && ui.button(label).on_hover_text(&command.id).clicked()
                            {
                                plugin_command = Some(command.id.clone());
                            }
                        }
                    }
                });
            self.workbench.search_open = open && action.is_none() && plugin_command.is_none();
        }
        self.environment_configuration_ui(ui.ctx());
        self.plugins_window(ui.ctx());
        if let Some(action) = action {
            self.dispatch_workbench(action);
        }
        if let Some(command) = plugin_command {
            self.run_plugin_command(&command);
        }
    }
}

impl AtelierApp {
    pub(super) fn workbench_project_navigation(&mut self, ui: &mut Ui) {
        let language = self.language;
        egui::CollapsingHeader::new(language.translate("Project and spaces"))
            .default_open(false)
            .show(ui, |ui| {
                ui.strong(&self.host.document.editor.project.name);
                ui.menu_button(language.translate("Design Options"), |ui| {
                    self.design_options_menu_contents(ui)
                });
                let editor = &self.host.document.editor;
                let rows: Vec<_> = editor
                    .project
                    .elements
                    .iter()
                    .filter(|e| {
                        e.story == editor.story
                            && e.worksheet == editor.project.plan_options.active_worksheet
                            && editor
                                .project
                                .plan_options
                                .element_visible(e, &editor.project.layers)
                    })
                    .filter_map(|e| match &e.shape {
                        Shape::Zone { spec } => {
                            Some((e.id, e.name.clone(), spec.purpose.clone(), spec.area()))
                        }
                        _ => None,
                    })
                    .collect();
                if rows.is_empty() {
                    ui.label(language.translate("No visible spaces on this story"));
                }
                egui::ScrollArea::vertical()
                    .id_salt("workbench_space_navigation")
                    .max_height(180.)
                    .show(ui, |ui| {
                        for (id, name, purpose, area) in rows {
                            let selected = self.host.document.editor.selection_ids().contains(&id);
                            if ui
                                .selectable_label(selected, format!("Z{id:03} · {name}"))
                                .on_hover_text(format!("{purpose} · {area:.2} m²"))
                                .clicked()
                            {
                                self.host.document.editor.set_selection(vec![id]);
                                self.tab = Tab::Properties;
                            }
                            if selected {
                                ui.label(format!(
                                    "{}: {purpose}",
                                    language.translate("Space purpose")
                                ));
                                ui.label(format!(
                                    "{}: {area:.2} m²",
                                    language.translate("Space area")
                                ));
                            }
                        }
                    });
                ui.label(language.translate("Space areas use authored boundaries."));
            });
        ui.separator();
    }
}

impl AtelierApp {
    fn environment_difference_ui(
        ui: &mut Ui,
        language: UiLanguage,
        baseline: &Configuration,
        draft: &Configuration,
    ) {
        let difference = match draft.differences_from(baseline) {
            Ok(d) => d,
            Err(error) => {
                ui.label(language.translate(&error));
                return;
            }
        };
        if difference.profiles.is_empty() && difference.active_before == difference.active_after {
            return;
        }
        let mode_name = |config: &Configuration, id: &str| {
            config
                .resolve(id)
                .map(|p| {
                    if p.builtin_name {
                        language.translate(&p.name).to_owned()
                    } else {
                        p.name
                    }
                })
                .unwrap_or_else(|_| id.to_owned())
        };
        let row = |ui: &mut Ui, label: &str, before: String, after: String| {
            if before != after {
                ui.strong(language.translate(label));
                ui.label(format!("{}: {}", language.translate("Before"), before));
                ui.label(format!("{}: {}", language.translate("After"), after));
            }
        };
        let commands = |p: &Option<Profile>, project_menu: bool| {
            p.as_ref()
                .map(|p| {
                    (if project_menu {
                        &p.project_menu
                    } else {
                        &p.commands
                    })
                    .iter()
                    .enumerate()
                    .map(|(i, id)| {
                        let name = CommandRegistry::resolve(id)
                            .map(|c| language.translate(c.label()).to_owned())
                            .unwrap_or_else(|| {
                                format!("{} ({})", id, language.translate("Unavailable"))
                            });
                        format!("{}. {name}", i + 1)
                    })
                    .collect::<Vec<_>>()
                    .join(" → ")
                })
                .unwrap_or_else(|| language.translate("None").into())
        };
        let panels = |p: &Option<Profile>| {
            p.as_ref()
                .map(|p| {
                    [
                        ("Toolbox", p.panels.toolbox),
                        ("Navigator", p.panels.navigator),
                        ("Info Box", p.panels.info_box),
                        ("Quick Options", p.panels.quick_options),
                        ("Quick Options Palette", p.panels.quick_palette),
                        ("Standard toolbar", p.panels.standard_toolbar),
                        ("Edit toolbar", p.panels.edit_toolbar),
                    ]
                    .iter()
                    .map(|(name, visible)| {
                        format!(
                            "{}: {}",
                            language.translate(name),
                            language.translate(if *visible { "Visible" } else { "Hidden" })
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(" · ")
                })
                .unwrap_or_else(|| language.translate("None").into())
        };
        let workspace = |p: &Option<Profile>| {
            p.as_ref()
                .map(|p| {
                    p.workspace
                        .groups
                        .iter()
                        .map(|g| {
                            let names = g
                                .panels
                                .iter()
                                .map(|id| {
                                    crate::workbench::workspace::panel_label(id)
                                        .map(|name| language.translate(name).to_owned())
                                        .unwrap_or_else(|| {
                                            format!(
                                                "{} ({})",
                                                id,
                                                language.translate("Unavailable")
                                            )
                                        })
                                })
                                .collect::<Vec<_>>()
                                .join(" + ");
                            format!(
                                "{} · {} · {} · {} · {},{} {}×{}",
                                names,
                                language.translate(g.placement.label()),
                                crate::workbench::workspace::panel_label(&g.active)
                                    .map(|label| language.translate(label))
                                    .unwrap_or(&g.active),
                                g.extent,
                                g.floating.x,
                                g.floating.y,
                                g.floating.width,
                                g.floating.height
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
                })
                .unwrap_or_else(|| language.translate("None").into())
        };
        ui.label(
            language.translate(
                "Reading a configuration does not apply it. Save to confirm, or cancel.",
            ),
        );
        egui::CollapsingHeader::new(language.translate("Changes from saved environment"))
            .default_open(false).show(ui, |ui| {
                row(ui,"Active mode",mode_name(baseline,&difference.active_before),mode_name(draft,&difference.active_after));
                for change in difference.profiles {
                    ui.separator();
                    ui.strong(mode_name(if change.after.is_some(){draft}else{baseline},&change.id)).on_hover_text(&change.id);
                    ui.label(language.translate(if change.before.is_none(){"Mode added"}else if change.after.is_none(){"Mode removed"}else{"Mode changed"}));
                    row(ui,"Mode name",change.before.as_ref().map(|p|p.name.clone()).unwrap_or_default(),change.after.as_ref().map(|p|p.name.clone()).unwrap_or_default());
                    row(ui,"Work mode",change.before.as_ref().map(|p|language.translate(p.base_mode.label()).to_owned()).unwrap_or_default(),change.after.as_ref().map(|p|language.translate(p.base_mode.label()).to_owned()).unwrap_or_default());
                    row(ui,"Command order",commands(&change.before,false),commands(&change.after,false));
                    row(ui,"Project menu",commands(&change.before,true),commands(&change.after,true));
                    let shortcuts=|p: &Option<Profile>| p.as_ref().map(|p|p.shortcuts.iter().map(|b|format!("{}: {}",CommandRegistry::resolve(&b.command_id).map(|c|language.translate(c.label())).unwrap_or(&b.command_id),if b.keys.is_empty() {language.translate("Unassigned").into()} else {b.keys.iter().map(crate::workbench::shortcuts::Chord::hint).collect::<Vec<_>>().join(" / ")})).collect::<Vec<_>>().join(" · ")).unwrap_or_default();
                    row(ui,"Keyboard shortcuts",shortcuts(&change.before),shortcuts(&change.after));
                    let presets = |p: &Option<Profile>| p.as_ref().and_then(|p|p.creation_presets.as_ref()).map(|p|format!("{}: {} mm · {}: {} mm · {}: {}", language.translate("New concept mass height"),p.mass_height_mm,language.translate("New Zone clear height"),p.zone_height_mm,language.translate("New Zone purpose"),p.zone_purpose)).unwrap_or_else(||language.translate("Keep current creation defaults").into());
                    row(ui,"Concept creation presets",presets(&change.before),presets(&change.after));
                    row(ui,"Panels",panels(&change.before),panels(&change.after));
                    row(ui,"Panel placement",workspace(&change.before),workspace(&change.after));
                    // A same-value override still changes inheritance at the next default upgrade.
                    if change.before_layer != change.after_layer {
                        ui.label(language.translate("Personal overrides changed; inherited defaults and explicit values remain distinct."));
                        let overrides = |layer: &Option<crate::workbench::environment::ProfileLayer>| {
                            layer.as_ref().map(|p|[("Mode name",p.name.is_some()),("Command order",p.commands.is_some()),("Project menu",p.project_menu.is_some()),("Panels",p.panels.is_some()),("Panel placement",p.workspace.is_some()),("Keyboard shortcuts",p.shortcuts.is_some()),("Concept creation presets",p.creation_presets.is_some())].iter().filter(|(_,changed)|*changed).map(|(label,_)|language.translate(label)).collect::<Vec<_>>().join(" · ")).filter(|s|!s.is_empty()).unwrap_or_else(||language.translate("Bundled default").into())
                        };
                        row(ui,"Personal",overrides(&change.before_layer),overrides(&change.after_layer));
                    }
                }
            });
    }
    fn environment_command_order_ui(
        ui: &mut Ui,
        language: UiLanguage,
        commands: &mut Vec<String>,
        project_menu: bool,
    ) {
        egui::ScrollArea::vertical()
            .id_salt(if project_menu {
                "environment_project_menu_order"
            } else {
                "environment_command_order"
            })
            .max_height(180.)
            .show(ui, |ui| {
                let mut edit = None;
                for (index, id) in commands.iter().enumerate() {
                    ui.push_id(id, |ui| {
                        ui.horizontal(|ui| {
                            let label = CommandRegistry::resolve(id)
                                .map(|c| language.translate(c.label()))
                                .unwrap_or(id);
                            ui.label(label).on_hover_text(id);
                            if CommandRegistry::resolve(id).is_none() {
                                ui.label(language.translate(
                                    "Command provider is unavailable; saved ID retained",
                                ));
                            }
                            if ui
                                .add_enabled(index > 0, Button::new(language.translate("Move up")))
                                .clicked()
                            {
                                edit = Some((index, -1));
                            }
                            if ui
                                .add_enabled(
                                    index + 1 < commands.len(),
                                    Button::new(language.translate("Move down")),
                                )
                                .clicked()
                            {
                                edit = Some((index, 1));
                            }
                            if ui
                                .add_enabled(
                                    !project_menu
                                        || !WorkbenchCommand::PROJECT_MENU
                                            .iter()
                                            .any(|c| c.id() == id),
                                    Button::new(language.translate("Remove command")),
                                )
                                .clicked()
                            {
                                edit = Some((index, 0));
                            }
                        });
                    });
                }
                if let Some((index, direction)) = edit {
                    if direction == 0 {
                        commands.remove(index);
                    } else {
                        commands.swap(index, (index as isize + direction) as usize);
                    }
                }
            });
        ui.menu_button(
            language.translate(if project_menu {
                "Add project menu command"
            } else {
                "Add task command"
            }),
            |ui| {
                for command in CommandRegistry::commands() {
                    if !commands.iter().any(|id| id == command.id())
                        && ui.button(language.translate(command.label())).clicked()
                    {
                        commands.push(command.id().into());
                        ui.close();
                    }
                }
            },
        );
    }
    fn environment_configuration_ui(&mut self, ctx: &egui::Context) {
        if !self.workbench.configuration_open {
            return;
        }
        let language = self.language;
        let mut open = true;
        let mut operation = "";
        let Some((mut configuration, mut profile)) = self.workbench.configuration_draft.take()
        else {
            return;
        };
        egui::Window::new(language.translate("Personal work environment"))
            .open(&mut open)
            .default_width(640.)
            .default_height(800.)
            .vscroll(true)
            .show(ctx, |ui| {
                if let Some(service) = &self.host.environment {
                    ui.label(service.path().display().to_string());
                    ui.label(format!(
                        "{}: {}",
                        language.translate("Configuration revision"),
                        service.configuration.revision
                    ));
                    if service.recovery.is_some() {
                        ui.colored_label(
                            Color32::from_rgb(224, 125, 105),
                            language.translate(
                                "Original configuration preserved; explicit recovery is required",
                            ),
                        );
                    }
                    if let Some(reason) = &service.backup_warning {
                        ui.label(language.translate("Work environment backup needs attention"));
                        ui.label(language.translate(reason));
                    }
                }
                if self
                    .host
                    .environment
                    .as_ref()
                    .is_some_and(|s| s.recovery.is_some())
                {
                    ui.horizontal_wrapped(|ui| {
                        if ui
                            .button(language.translate("Recover last valid environment"))
                            .clicked()
                        {
                            operation = "recover";
                        }
                        if ui
                            .button(language.translate("Recover with bundled defaults"))
                            .clicked()
                        {
                            operation = "defaults";
                        }
                    });
                }
                if !self.workbench.configuration_error.is_empty() {
                    ui.colored_label(
                        Color32::from_rgb(224, 125, 105),
                        &self.localized_shortcut_error(&self.workbench.configuration_error),
                    );
                }
                if !self.workbench.configuration_status.is_empty() {
                    ui.label(language.translate(&self.workbench.configuration_status));
                }
                ui.label(
                    language
                        .translate("Personal settings are separate from the model and model Undo."),
                );
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .button(language.translate("Save and activate mode"))
                        .clicked()
                    {
                        operation = "save";
                    }
                    if ui
                        .button(language.translate("Cancel environment edits"))
                        .clicked()
                    {
                        operation = "cancel";
                    }
                    if ui
                        .button(language.translate("Reload saved environment"))
                        .clicked()
                    {
                        operation = "reload";
                    }
                    if ui
                        .button(language.translate("Restore previous environment"))
                        .clicked()
                    {
                        operation = "previous";
                    }
                });
                self.personal_shortcuts_ui(ui, &mut profile);
                ui.horizontal(|ui| {
                    ui.label(language.translate("Configuration file"));
                    ui.add(
                        TextEdit::singleline(&mut self.workbench.transfer_path).hint_text(".json"),
                    );
                    if ui
                        .button(language.translate("Browse configuration"))
                        .clicked()
                        && let Some(path) = rfd::FileDialog::new()
                            .add_filter("JSON", &["json"])
                            .pick_file()
                    {
                        self.workbench.transfer_path = path.display().to_string();
                    }
                });
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .button(language.translate("Export saved environment"))
                        .clicked()
                    {
                        operation = "export";
                    }
                    if ui
                        .button(language.translate("Review environment file"))
                        .clicked()
                    {
                        operation = "import";
                    }
                });
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .button(language.translate("Review project configuration proposal"))
                        .clicked()
                    {
                        operation = "proposal";
                    }
                    if self
                        .workbench
                        .proposal_record
                        .as_ref()
                        .is_some_and(|p| p.status == "pending")
                        && ui
                            .button(language.translate("Cancel this configuration proposal"))
                            .clicked()
                    {
                        operation = "proposal_cancel";
                    }
                    if self.workbench.proposal_record.as_ref().is_some_and(|p| {
                        matches!(p.status.as_str(), "applied") && p.current_matches_applied
                    }) && ui
                        .button(language.translate("Restore this configuration proposal"))
                        .clicked()
                    {
                        operation = "proposal_restore";
                    }
                });
                if let Some(record) = &self.workbench.proposal_record {
                    if record.status == "unknown" && record.expected_configuration_observed {
                        ui.label(language.translate(
                            "Matching settings do not prove the original write completed.",
                        ));
                    }
                    ui.label(format!(
                        "{} · {}",
                        language.translate("Original configuration proposal"),
                        record.request.request_id
                    ));
                    ui.label(language.translate(match record.status.as_str() {
                        "pending" => "Pending review",
                        "applied" => "Applied",
                        "reverted" => "Restored",
                        "cancelled" => "Cancelled",
                        "rejected" => "Rejected",
                        _ => "Unknown",
                    }));
                    if record.status == "pending" && record.configuration_changed_since_proposal {
                        ui.label(language.translate(
                            "The original configuration changed; this proposal cannot be applied.",
                        ));
                    }
                }
                if let Some(service) = &self.host.environment {
                    let mut draft = configuration.clone();
                    draft.apply_profile_draft(&profile);
                    Self::environment_difference_ui(ui, language, &service.configuration, &draft);
                }
                ui.horizontal_wrapped(|ui| {
                    egui::ComboBox::from_id_salt("environment_edit_profile")
                        .selected_text(if profile.builtin_name {
                            language.translate(&profile.name)
                        } else {
                            &profile.name
                        })
                        .show_ui(ui, |ui| {
                            for other in configuration.profiles() {
                                let label = if other.builtin_name {
                                    language.translate(&other.name)
                                } else {
                                    &other.name
                                };
                                if ui.selectable_label(profile.id == other.id, label).clicked() {
                                    profile = other;
                                    ui.close();
                                }
                            }
                        });
                    if ui
                        .button(language.translate("Clone as personal mode"))
                        .clicked()
                    {
                        profile.id = format!("user.{}", crate::storage::new_document_identity());
                        profile.name = format!(
                            "{} · {}",
                            if profile.builtin_name {
                                language.translate(&profile.name)
                            } else {
                                &profile.name
                            },
                            language.translate("Personal")
                        );
                        profile.builtin_name = false;
                    }
                });
                ui.label(&profile.id);
                if profile.id.starts_with("user.") {
                    ui.horizontal(|ui| {
                        ui.label(language.translate("Mode name"));
                        ui.add(TextEdit::singleline(&mut profile.name).id_salt("environment_name"));
                    });
                }
                egui::CollapsingHeader::new(language.translate("Concept creation presets"))
                    .id_salt("environment_creation_presets").show(ui, |ui| {
                        ui.label(language.translate("Only future manual concept masses and Zones; existing objects and generated design rules stay unchanged."));
                        ui.label(language.translate("A mode without presets keeps the current creation defaults."));
                        if ui.button(language.translate("Capture current creation defaults")).clicked() {
                            let (mass, zone, purpose) = self.host.creation_defaults();
                            match CreationPresets::from_metres(mass, zone, &purpose) {
                                Ok(presets) => {
                                    profile.creation_presets = Some(presets);
                                    self.workbench.configuration_creation_baseline = Some(self.host.creation_defaults());
                                    self.workbench.configuration_error.clear();
                                }
                                Err(error) => self.workbench.configuration_error = error,
                            }
                        }
                        if let Some(presets) = &mut profile.creation_presets {
                            for (label, value) in [("New concept mass height", &mut presets.mass_height_mm), ("New Zone clear height", &mut presets.zone_height_mm)] {
                                ui.horizontal(|ui| {
                                    let label = ui.label(language.translate(label));
                                    ui.add(egui::DragValue::new(value).range(10..=100_000).suffix(" mm")).labelled_by(label.id);
                                });
                            }
                            ui.horizontal(|ui| {
                                let label = ui.label(language.translate("New Zone purpose"));
                                ui.add(TextEdit::singleline(&mut presets.zone_purpose).id_salt("environment_zone_purpose")).labelled_by(label.id);
                            });
                            if ui.button(language.translate("Keep current creation defaults")).clicked() { profile.creation_presets = None; }
                        }
                    });
                ui.horizontal_wrapped(|ui| {
                    ui.checkbox(&mut profile.panels.standard_toolbar, language.translate("Standard toolbar"));
                    ui.checkbox(&mut profile.panels.edit_toolbar, language.translate("Edit toolbar"));
                    ui.checkbox(&mut profile.panels.toolbox, language.translate("Toolbox"));
                    ui.checkbox(
                        &mut profile.panels.navigator,
                        language.translate("Navigator"),
                    );
                    ui.checkbox(&mut profile.panels.info_box, language.translate("Info Box"));
                    ui.checkbox(
                        &mut profile.panels.quick_options,
                        language.translate("Quick Options"),
                    );
                    ui.checkbox(
                        &mut profile.panels.quick_palette,
                        language.translate("Quick Options Palette"),
                    );
                });
                if ui
                    .button(language.translate("Capture current workspace layout"))
                    .clicked()
                {
                    profile.workspace = self.workbench.workspace.clone();
                    profile.panels = self.current_environment_panels();
                    // This existing explicit user action acknowledges the live layout;
                    // merely reading a fresh revision never does.
                    self.workbench.configuration_live_baseline = Some((
                        self.current_environment_panels(),
                        self.workbench.workspace.clone(),
                    ));
                }
                ui.label(language.translate("Layout changes are session-only until saved."));
                ui.label(language.translate("Task commands in display order"));
                Self::environment_command_order_ui(ui, language, &mut profile.commands, false);
                ui.collapsing(language.translate("Project menu"), |ui| {
                    ui.label(language.translate("Explore and Project conditions remain available."));
                    Self::environment_command_order_ui(ui, language, &mut profile.project_menu, true);
                });
                ui.collapsing(
                    language.translate("Configuration source and differences"),
                    |ui| {
                        ui.label(language.translate(profile.base_mode.label()));
                        let layer = profile.layer();
                        ui.label(if layer.commands.is_some() {
                            language.translate("Command order: personal override")
                        } else {
                            language.translate("Command order: bundled default")
                        });
                        ui.label(if layer.panels.is_some() {
                            language.translate("Panels: personal override")
                        } else {
                            language.translate("Panels: bundled default")
                        });
                        if ui
                            .button(language.translate("Reset draft to bundled defaults"))
                            .clicked()
                        {
                            let defaults = Profile::builtin(profile.base_mode);
                            profile.project_menu = defaults.project_menu;
                            profile.shortcuts = defaults.shortcuts;
                            profile.commands = defaults.commands;
                            profile.panels = defaults.panels;
                            profile.workspace = defaults.workspace;
                            profile.creation_presets = defaults.creation_presets;
                            // This explicit reset clears inheritance overrides; merely
                            // reviewing or editing an unrelated field must not do so.
                            configuration.put(profile.layer());
                        }
                        ui.label(language.translate(
                            "Environmental analysis: pending verified analysis providers",
                        ));
                        ui.label(
                            language
                                .translate("Reports and drawings: model-linked figures and ordered report templates"),
                        );
                    },
                );
            });
        self.workbench.configuration_draft = Some((configuration, profile));
        self.workbench.configuration_open = open;
        let result = match operation {
            "save" => self.save_environment_draft(),
            "cancel" => (|| {
                if let Some(id) = self.workbench.reviewed_proposal.as_deref() {
                    let service = self
                        .host
                        .environment
                        .as_ref()
                        .ok_or("Work environment storage is unavailable")?;
                    self.workbench.proposal_record = Some(
                        service
                            .cancel_proposal(id, self.host.document.editor.project_identity())?,
                    );
                }
                self.workbench.reviewed_proposal = None;
                self.workbench.configuration_open = false;
                self.workbench.configuration_draft = None;
                Ok(())
            })(),
            "proposal_cancel" => (|| {
                let service = self
                    .host
                    .environment
                    .as_ref()
                    .ok_or("Work environment storage is unavailable")?;
                let id = self
                    .workbench
                    .proposal_record
                    .as_ref()
                    .ok_or("No configuration proposal for this project")?
                    .request
                    .request_id
                    .clone();
                let record =
                    service.cancel_proposal(&id, self.host.document.editor.project_identity())?;
                if self.workbench.reviewed_proposal.as_deref() == Some(&id) {
                    let config = service.configuration.clone();
                    let profile = config.resolve(&config.active)?;
                    self.set_environment_draft(config.clone(), profile, config);
                    self.workbench.reviewed_proposal = None;
                }
                self.workbench.proposal_record = Some(record);
                self.workbench.configuration_error.clear();
                Ok(())
            })(),
            "proposal" => (|| {
                self.require_clean_environment_draft()?;
                let service = self
                    .host
                    .environment
                    .as_ref()
                    .ok_or("Work environment storage is unavailable")?;
                let project = self.host.document.editor.project_identity();
                let view = service
                    .latest_pending_proposal(project)?
                    .or(service.latest_project_proposal(project)?)
                    .ok_or("No configuration proposal for this project")?;
                let id = view.request.request_id.clone();
                let pending = view.status == "pending";
                self.workbench.proposal_record = Some(view);
                if pending {
                    let draft = service.proposal_draft(&id, project)?;
                    let profile = draft.resolve(&draft.active)?;
                    self.set_environment_draft(draft.clone(), profile, draft);
                    self.workbench.reviewed_proposal = Some(id);
                    self.workbench.configuration_status =
                        "Configuration loaded for review; not applied".into();
                }
                self.workbench.configuration_error.clear();
                Ok(())
            })(),
            "reload" => {
                if let Some(path) = self.host.environment.as_ref().map(|s| s.path().to_owned()) {
                    match EnvironmentService::open(&path, self.workbench.environment.mode) {
                        Ok(service) => {
                            let profile = service
                                .configuration
                                .resolve(&service.configuration.active)
                                .expect("validated profile");
                            self.workbench.active_profile = profile.id.clone();
                            self.workbench.environment.switch_mode(profile.base_mode);
                            self.workbench.configuration_error =
                                service.recovery.clone().unwrap_or_default();
                            self.host.environment = Some(service);
                            self.apply_environment_panels(profile.panels);
                            self.apply_environment_workspace(profile.workspace.clone());
                            self.apply_environment_creation_presets(
                                profile.creation_presets.clone(),
                            );
                            self.open_environment_configuration();
                            Ok(())
                        }
                        Err(e) => Err(e),
                    }
                } else {
                    Err("Work environment storage is unavailable".into())
                }
            }
            "import" => (|| {
                self.require_clean_environment_draft()?;
                let service = self
                    .host
                    .environment
                    .as_ref()
                    .ok_or("Work environment storage is unavailable")?;
                let configuration =
                    service.preview_import(std::path::Path::new(&self.workbench.transfer_path))?;
                let baseline = service.configuration.clone();
                let profile = configuration.resolve(&configuration.active)?;
                self.set_environment_draft(configuration, profile, baseline);
                self.workbench.reviewed_proposal = None;
                self.workbench.proposal_record = None;
                self.workbench.configuration_error.clear();
                self.workbench.configuration_status =
                    "Configuration loaded for review; not applied".into();
                Ok(())
            })(),
            "previous" | "recover" | "defaults" | "proposal_restore" => {
                if operation == "proposal_restore"
                    && self.require_clean_environment_draft().is_err()
                {
                    self.require_clean_environment_draft()
                } else if self.workbench_pointer_active() || self.other_menu_gated() {
                    Err(
                        "Finish or cancel the current model interaction before applying settings"
                            .into(),
                    )
                } else if let Some(service) = self.host.environment.as_mut() {
                    let result = match operation {
                        "proposal_restore" => self
                            .workbench
                            .proposal_record
                            .as_ref()
                            .ok_or_else(|| "No configuration proposal for this project".to_owned())
                            .and_then(|p| {
                                service.restore_proposal(
                                    &p.request.request_id,
                                    self.host.document.editor.project_identity(),
                                )
                            }),
                        "previous" => service.restore_previous(),
                        "defaults" => service.restore_defaults(self.workbench.environment.mode),
                        _ => service.restore_valid(),
                    };
                    match result {
                        Ok(outcome) => {
                            if operation == "proposal_restore" {
                                if let Some(p) = self.workbench.proposal_record.as_ref() {
                                    self.workbench.proposal_record = service
                                        .read_proposal(
                                            &p.request.request_id,
                                            self.host.document.editor.project_identity(),
                                        )
                                        .ok();
                                }
                            }
                            let profile = service
                                .configuration
                                .resolve(&service.configuration.active)
                                .expect("validated profile");
                            self.workbench.active_profile = profile.id.clone();
                            self.workbench.environment.switch_mode(profile.base_mode);
                            self.apply_environment_panels(profile.panels);
                            self.apply_environment_workspace(profile.workspace.clone());
                            self.apply_environment_creation_presets(
                                profile.creation_presets.clone(),
                            );
                            self.open_environment_configuration();
                            self.workbench.configuration_error =
                                outcome.warning.unwrap_or_default();
                            let preserved: Vec<_> = outcome
                                .preserved_file
                                .into_iter()
                                .chain(outcome.preserved_backup_file)
                                .collect();
                            self.workbench.configuration_status = if preserved.is_empty() {
                                "Work environment saved".into()
                            } else {
                                preserved
                                    .into_iter()
                                    .map(|p| {
                                        format!(
                                            "{}: {}",
                                            language.translate("Preserved configuration"),
                                            p.display()
                                        )
                                    })
                                    .collect::<Vec<_>>()
                                    .join("\n")
                            };
                            Ok(())
                        }
                        Err(e) => Err(e),
                    }
                } else {
                    Err("Work environment storage is unavailable".into())
                }
            }
            "export" => self
                .host
                .environment
                .as_ref()
                .ok_or_else(|| "Work environment storage is unavailable".into())
                .and_then(|s| s.export(std::path::Path::new(&self.workbench.transfer_path)))
                .map(|_| self.workbench.configuration_status = "Work environment exported".into()),
            _ => Ok(()),
        };
        if let Err(error) = result {
            self.workbench.configuration_error = error;
        }
    }
}
