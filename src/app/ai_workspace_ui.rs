//! Presentation over the existing document, command registry and operation service.
use super::*;
use crate::workbench::WorkbenchCommand;

#[cfg(test)]
#[path = "canvas_navigation_ui_tests.rs"]
mod canvas_navigation_tests;

#[derive(Default)]
pub(super) struct AiWorkspaceUi {
    pub enabled: bool,
    details: bool,
    operations: bool,
    feedback: String,
    document_session: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ai_workspace_takeover_keeps_document_and_does_not_reauthorize() {
        let mut app = AtelierApp::from_renderer(None, "test".into());
        let before = app.host.document.editor.project.elements.clone();
        let revision = app.host.document.editor.revision;
        app.host
            .operations
            .borrow_mut()
            .authorize_current(&app.host.document.editor);
        app.set_ai_workspace(false);
        assert_eq!(
            app.host
                .operations
                .borrow()
                .write_authorization(&app.host.document.editor)
                .unwrap_err()["error"]["code"],
            "REMOTE_WRITES_PAUSED"
        );
        app.host.set_remote_interaction_blocked(false);
        assert!(
            app.host
                .operations
                .borrow()
                .write_authorization(&app.host.document.editor)
                .is_err()
        );
        app.set_ai_workspace(true);
        assert!(
            app.host
                .operations
                .borrow()
                .write_authorization(&app.host.document.editor)
                .is_ok()
        );
        assert_eq!(app.host.document.editor.project.elements, before);
        assert_eq!(app.host.document.editor.revision, revision);
        app.set_ai_workspace(false);
        app.host.document.editor.revision += 1;
        app.set_ai_workspace(true);
        assert_eq!(
            app.host
                .operations
                .borrow()
                .write_authorization(&app.host.document.editor)
                .unwrap_err()["error"]["code"],
            "WRITE_AUTHORIZATION_SUSPENDED"
        );
    }

    #[test]
    fn ai_workspace_does_not_discard_active_drawing() {
        let mut app = AtelierApp::from_renderer(None, "test".into());
        app.host.document.editor.anchor = Some(Point::new(1., 2.));
        app.set_ai_workspace(true);
        assert!(!app.ai_workspace.enabled);
        assert_eq!(app.host.document.editor.anchor, Some(Point::new(1., 2.)));
    }

    #[test]
    fn ai_workspace_document_replacement_clears_old_feedback() {
        let mut app = AtelierApp::from_renderer(None, "test".into());
        app.refresh_ai_workspace_document();
        app.ai_workspace.feedback = "undone".into();
        app.refresh_ai_workspace_document();
        assert_eq!(app.ai_workspace.feedback, "undone");
        app.host.document.editor = Editor::new(Project::default());
        app.refresh_ai_workspace_document();
        assert!(app.ai_workspace.feedback.is_empty());
    }
}

impl AtelierApp {
    // The collaboration workspace hides the native quick-options strip. Keep
    // its view commands available at the canvas instead of duplicating them.
    pub(super) fn ai_canvas_navigation(
        &mut self,
        ui: &mut Ui,
        r: Rect,
        pane: viewport_navigation::Pane,
    ) -> Option<Rect> {
        self.navigation_canvas_rect(pane, r);
        if !self.ai_workspace.enabled || r.width() < 1. || r.height() < 1. {
            return None;
        }
        let index = match pane {
            viewport_navigation::Pane::Plan => 0,
            viewport_navigation::Pane::Model => 1,
        };
        let area = egui::Area::new(Id::new(("ai_canvas_navigation", index)))
            .order(Order::Foreground)
            .movable(false)
            .pivot(Align2::RIGHT_BOTTOM)
            .fixed_pos(r.right_bottom() - vec2(8., 8.))
            .constrain_to(r)
            .show(ui.ctx(), |ui| {
                ui.set_max_width((r.width() - 32.).max(1.));
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.add_enabled_ui(!self.menu_gated(), |ui| {
                        self.navigation_bar_for_pane(ui, pane);
                    });
                });
            });
        Some(area.response.rect)
    }

    fn refresh_ai_workspace_document(&mut self) {
        let session = self.host.document.editor.document_session();
        if self.ai_workspace.document_session != Some(session) {
            self.ai_workspace.feedback.clear();
            self.ai_workspace.document_session = Some(session);
        }
    }
    pub(super) fn ai_view_bar(&mut self, ui: &mut Ui) {
        let language = self.language;
        let idle = self.collaboration_switch_available();
        ui.horizontal_wrapped(|ui| {
            let stories = self.host.document.editor.project.stories.clone();
            let current = stories
                .iter()
                .find(|s| s.id == self.host.document.editor.story)
                .map(|s| s.name.clone())
                .unwrap_or_default();
            ui.add_enabled_ui(idle, |ui| {
                egui::ComboBox::from_id_salt("ai_story")
                    .selected_text(current)
                    .show_ui(ui, |ui| {
                        for story in stories {
                            if ui
                                .selectable_label(
                                    self.host.document.editor.story == story.id,
                                    &story.name,
                                )
                                .clicked()
                            {
                                self.host.document.editor.switch_story(story.id);
                                self.fit();
                            }
                        }
                    });
                for (view, label) in [
                    (View::Plan, "Floor Plan"),
                    (View::Model, "3D Perspective"),
                    (View::Split, "Split view"),
                ] {
                    if ui
                        .selectable_label(self.view == view, language.translate(label))
                        .clicked()
                    {
                        match view {
                            View::Plan => self.dispatch_workbench(WorkbenchCommand::Plan),
                            View::Model => self.dispatch_workbench(WorkbenchCommand::Model),
                            _ => self.view = view,
                        }
                    }
                }
                if ui.button(language.translate("Fit")).clicked() {
                    self.navigation_fit();
                }
            });
        });
        ui.separator();
    }

    fn collaboration_switch_available(&self) -> bool {
        !self.workbench_pointer_active()
            && !self.menu_gated()
            && !self.pending_design_parameters()
            && !self.project_conditions_has_draft()
            && self.drawings_ui.report_draft.is_none()
    }

    pub(super) fn set_ai_workspace(&mut self, enabled: bool) {
        if !self.collaboration_switch_available() {
            return;
        }
        // Polling and model commits run serially on the UI thread. Once this
        // returns, queued remote writes must pass the service admission gate.
        self.host.pause_remote_writes(!enabled);
        if !enabled {
            let rows = self
                .host
                .operations
                .borrow_mut()
                .operation_rows(&self.host.document.editor);
            match rows {
                Ok(rows)
                    if rows.operations.iter().any(|r| {
                        matches!(r.phase.as_str(), "intent" | "undo_intent" | "unknown")
                    }) =>
                {
                    self.ai_workspace.feedback =
                        "Inspect pending operations before manual takeover".into();
                    self.automation_window = true;
                    return;
                }
                Err(_) => {
                    self.ai_workspace.feedback =
                        "Operation history unavailable; remote writes remain paused".into();
                    self.automation_window = true;
                    return;
                }
                _ => {}
            }
        }
        self.ai_workspace.enabled = enabled;
    }

    pub(super) fn ai_workspace_header(&mut self, ui: &mut Ui) {
        self.refresh_ai_workspace_document();
        if !self.ai_workspace.enabled {
            return;
        }
        let language = self.language;
        let idle = self.collaboration_switch_available();
        egui::Panel::top("ai_project_header")
            .frame(
                Frame::new()
                    .fill(PANEL)
                    .inner_margin(Margin::symmetric(12, 6)),
            )
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.strong("OpenArchi");
                    if self.host.document.editor.dirty() {
                        ui.small(language.translate("Unsaved changes"));
                    }
                    ui.separator();
                    for (enabled, label) in [(true, "AI collaboration"), (false, "Manual editing")]
                    {
                        if ui
                            .add_enabled(
                                idle,
                                Button::new(language.translate(label))
                                    .selected(self.ai_workspace.enabled == enabled),
                            )
                            .clicked()
                        {
                            self.set_ai_workspace(enabled);
                        }
                    }
                    if ui.button(language.translate("Save")).clicked() {
                        self.dispatch_workbench(WorkbenchCommand::Save);
                    }
                    if ui.button(language.translate("Find command…")).clicked() {
                        self.workbench.search_open = true;
                    }
                    ui.menu_button(language.translate("Workspace"), |ui| {
                        for command in [
                            WorkbenchCommand::Conditions,
                            WorkbenchCommand::Checks,
                            WorkbenchCommand::Explore,
                            WorkbenchCommand::Drawings,
                            WorkbenchCommand::Reports,
                            WorkbenchCommand::Environment,
                        ] {
                            if ui.button(language.translate(command.label())).clicked() {
                                self.dispatch_workbench(command);
                                ui.close();
                            }
                        }
                        if ui.button(language.translate("AI Operations…")).clicked() {
                            self.automation_window = true;
                            ui.close();
                        }
                        if ui.button(language.translate("MCP Settings…")).clicked() {
                            self.mcp_settings.open = true;
                            ui.close();
                        }
                        ui.separator();
                        ui.menu_button(language.translate("Language"), |ui| {
                            for value in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
                                let label = match value {
                                    UiLanguage::English => "English",
                                    UiLanguage::SimplifiedChinese => "简体中文",
                                };
                                if ui.selectable_label(self.language == value, label).clicked() {
                                    self.language = value;
                                    self.language.save_preference();
                                    ui.close();
                                }
                            }
                        });
                    });
                    if self.ai_workspace.enabled {
                        if ui
                            .toggle_value(
                                &mut self.ai_workspace.operations,
                                language.translate("Changes"),
                            )
                            .clicked()
                            && self.ai_workspace.operations
                        {
                            self.ai_workspace.details = false;
                        }
                        if ui
                            .toggle_value(
                                &mut self.ai_workspace.details,
                                language.translate("Object details"),
                            )
                            .clicked()
                            && self.ai_workspace.details
                        {
                            self.ai_workspace.operations = false;
                        }
                    }
                    if self.automation_window && !self.ai_workspace.feedback.is_empty() {
                        ui.small(language.translate(&self.ai_workspace.feedback));
                    }
                });
            });
    }

    pub(super) fn ai_workspace_panel(&mut self, ui: &mut Ui) {
        let language = self.language;
        // An idle canvas remains unobstructed. The user opens records or object
        // details explicitly; no persistent empty inspector or tool palette.
        if !self.ai_workspace.operations && !self.ai_workspace.details {
            return;
        }
        let width = (ui.available_width() * 0.32).clamp(220., 360.);
        egui::Panel::right("ai_context_panel")
            .default_size(width)
            .size_range(200.0..=420.0)
            .frame(Frame::new().fill(PANEL).inner_margin(12))
            .show(ui, |ui| {
                if self.ai_workspace.details {
                    self.inspector_contents(ui, 0.);
                    return;
                }
                ui.heading(language.translate("Changes"));
                let status = self
                    .host
                    .operations
                    .borrow()
                    .authorization_status(&self.host.document.editor);
                let label = match status["status"].as_str().unwrap_or("") {
                    "authorized" => "Remote writes authorized",
                    "paused_by_user" => "Remote writes paused",
                    "read_only" => "Read-only collaboration",
                    _ => "Remote writes need attention",
                };
                ui.label(language.translate(label));
                ui.small(
                    language.translate("Endpoint permission is not an Agent connection status."),
                );
                let result = self
                    .host
                    .operations
                    .borrow_mut()
                    .operation_rows(&self.host.document.editor);
                match result {
                    Err(error) => {
                        ui.label(error.to_string());
                    }
                    Ok(rows) => {
                        if rows.operations.is_empty() && rows.previews.is_empty() {
                            ui.label(
                                language.translate("No operations recorded for this project."),
                            );
                        }
                        if !rows.previews.is_empty() {
                            ui.label(format!(
                                "{}: {}",
                                language.translate("Pending preview"),
                                rows.previews.len()
                            ));
                        }
                        ScrollArea::vertical().show(ui, |ui| {
                            for row in rows.operations {
                                ui.push_id(
                                    (
                                        &row.original_target.instance_id,
                                        row.original_target.document_session_id,
                                        &row.request_id,
                                    ),
                                    |ui| {
                                        ui.separator();
                                        ui.strong(language.translate(if row.native_confirmation {
                                            "Native confirmation"
                                        } else {
                                            "Automation operation"
                                        }));
                                        let phase = match row.phase.as_str() {
                                            "committed" => "Committed",
                                            "undone" => "Undone",
                                            "cancelled" => "Cancelled",
                                            "intent" => "Commit pending",
                                            "undo_intent" => "Undo pending",
                                            _ => "Outcome unknown",
                                        };
                                        ui.label(language.translate(phase));
                                        ui.label(format!(
                                            "{}: {}",
                                            language.translate("Changed elements"),
                                            row.changed_ids.len()
                                        ));
                                        for element in self
                                            .host
                                            .document
                                            .editor
                                            .project
                                            .elements
                                            .iter()
                                            .filter(|e| row.changed_ids.contains(&e.id))
                                            .take(5)
                                        {
                                            ui.label(&element.name);
                                        }
                                        ui.collapsing(
                                            language.translate("Operation details"),
                                            |ui| {
                                                ui.label(&row.request_id);
                                                ui.label(format!("{:?}", row.changed_ids));
                                            },
                                        );
                                        let idle = self.collaboration_switch_available();
                                        ui.horizontal_wrapped(|ui| {
                                            if ui
                                                .add_enabled(
                                                    idle && !row.changed_ids.is_empty(),
                                                    Button::new(
                                                        language.translate("Locate changes"),
                                                    ),
                                                )
                                                .clicked()
                                            {
                                                let ids = row
                                                    .changed_ids
                                                    .iter()
                                                    .copied()
                                                    .filter(|id| {
                                                        self.host
                                                            .document
                                                            .editor
                                                            .project
                                                            .elements
                                                            .iter()
                                                            .any(|e| e.id == *id)
                                                    })
                                                    .collect::<Vec<_>>();
                                                if let Some(first) = ids.first().and_then(|id| {
                                                    self.host
                                                        .document
                                                        .editor
                                                        .project
                                                        .elements
                                                        .iter()
                                                        .find(|e| e.id == *id)
                                                }) {
                                                    let story = first.story;
                                                    self.host.document.editor.story = story;
                                                    self.host.document.editor.set_selection(ids);
                                                    self.fit_selection();
                                                } else {
                                                    self.ai_workspace.feedback =
                                                        "Changed objects are no longer present"
                                                            .into();
                                                }
                                            }
                                            if ui
                                                .add_enabled(
                                                    idle && row.phase == "committed",
                                                    Button::new(language.translate("Safe Undo")),
                                                )
                                                .clicked()
                                            {
                                                let value =
                                                    self.host.operations.borrow_mut().undo_current(
                                                        &mut self.host.document.editor,
                                                        row.request_id.clone(),
                                                        row.original_target.clone(),
                                                    );
                                                self.ai_workspace.feedback =
                                                    super::automation_ui::automation_result(&value);
                                            }
                                            if ui
                                                .button(language.translate("Inspect operation"))
                                                .clicked()
                                            {
                                                let value = self
                                                    .host
                                                    .operations
                                                    .borrow_mut()
                                                    .inspect_current(
                                                        &self.host.document.editor,
                                                        row.request_id.clone(),
                                                        row.original_target.clone(),
                                                    );
                                                self.ai_workspace.feedback =
                                                    super::automation_ui::automation_result(&value);
                                            }
                                        });
                                    },
                                );
                            }
                        });
                    }
                }
                if !self.ai_workspace.feedback.is_empty() {
                    ui.separator();
                    let label = match self.ai_workspace.feedback.as_str() {
                        "committed" => "Committed",
                        "undone" => "Undone",
                        "cancelled" => "Cancelled",
                        value => value,
                    };
                    ui.label(language.translate(label));
                }
            });
    }
}
