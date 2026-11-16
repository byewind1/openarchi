use super::*;

impl AtelierApp {
    pub(super) fn automation_window(&mut self, ctx: &Context) {
        if !self.automation_window {
            return;
        }
        let language = self.language;
        let mut open = true;
        Window::new(language.translate("AI Operations…"))
            .id(Id::new("automation_operations"))
            .open(&mut open).default_width(540.).resizable(true)
            .show(ctx, |ui| {
                ui.label(language.translate("Native and automation operations share this project history. Safe Undo preserves later user edits."));
                {
                    let operations = self.host.operations.clone();
                    let mut changes = operations.borrow_mut();
                    match changes.operation_rows(&self.host.document.editor) {
                        Err(e) => { self.automation_feedback = e["error"]["code"].as_str().unwrap_or("UNKNOWN").into(); }
                        Ok(rows) => {
                            if rows.previews.is_empty() && rows.operations.is_empty() {
                                ui.label(language.translate("No operations recorded for this project."));
                            }
                            ScrollArea::vertical().max_height(420.).show(ui, |ui| {
                                for preview_id in rows.previews {
                                    ui.horizontal_wrapped(|ui| {
                                        ui.label(language.translate("Pending preview"));
                                        ui.monospace(&preview_id);
                                        if ui.button(language.translate("Cancel preview")).clicked() {
                                            // Current target comes from the same service, never from UI text.
                                            let response = changes.cancel_current(&self.host.document.editor, preview_id.clone());
                                            self.automation_feedback = automation_result(&response);
                                        }
                                    });
                                }
                                for row in rows.operations {
                                    ui.separator();
                                    ui.label(RichText::new(&row.request_id).strong());
                                    ui.horizontal_wrapped(|ui| {
                                        ui.label(language.translate(phase_label(&row.phase)));
                                    ui.label(language.translate(if row.native_confirmation {"Native confirmation"} else {"Automation operation"}));
                                        ui.monospace(format!("{}:{}", row.original_target.instance_id, row.original_target.document_session_id));
                                    });
                                    ui.label(format!("{}: {:?}", language.translate("Changed elements"), row.changed_ids));
                                    ui.horizontal_wrapped(|ui| {
                                        if ui.button(language.translate("Inspect operation")).clicked() {
                                            let response = changes.inspect_current(&self.host.document.editor, row.request_id.clone(), row.original_target.clone());
                                            self.automation_feedback = automation_result(&response);
                                        }
                                        if row.phase == "committed" && ui.button(language.translate("Safe Undo")).clicked() {
                                            let response = changes.undo_current(&mut self.host.document.editor, row.request_id.clone(), row.original_target.clone());
                                            self.automation_feedback = automation_result(&response);
                                        }
                                        if row.native_confirmation && row.phase == "intent" && ui.button(language.translate("Cancel unapplied native intent")).clicked() {
                                            let response = changes.cancel_native_intent(&self.host.document.editor, row.original_target.clone(), &row.request_id);
                                            self.automation_feedback = automation_result(&response);
                                        }
                                        if matches!(row.phase.as_str(), "intent" | "undo_intent" | "unknown") && ui.button(language.translate("Reconcile operation")).clicked() {
                                            let response = changes.reconcile_current(&mut self.host.document.editor, row.request_id.clone(), row.original_target.clone());
                                            self.automation_feedback = automation_result(&response);
                                        }
                                    });
                                }
                            });
                        }
                    }
                }
                if !self.automation_feedback.is_empty() {
                    ui.separator();
                    ui.label(language.translate("Operation result"));
                    ui.monospace(&self.automation_feedback);
                    ui.label(language.translate("Recovery conflicts require an explicit recovered document and Save As; the current file is preserved."));
                }
            });
        self.automation_window = open;
    }
}
fn phase_label(phase: &str) -> &str {
    match phase {
        "intent" => "Commit pending",
        "committed" => "Committed",
        "undo_intent" => "Undo pending",
        "undone" => "Undone",
        "cancelled" => "Cancelled",
        _ => "Outcome unknown",
    }
}
pub(super) fn automation_result(value: &serde_json::Value) -> String {
    // Only protocol status codes, never model snapshots or arbitrary imported text.
    let source = value.get("error").unwrap_or(&value["result"]);
    let code = source
        .get("code")
        .or_else(|| source.get("status"))
        .and_then(|v| v.as_str())
        .unwrap_or("UNKNOWN");
    match value["result"]["recovery_required"].as_bool() {
        Some(true) => format!("{code} · recovery_required"),
        _ => code.into(),
    }
}
