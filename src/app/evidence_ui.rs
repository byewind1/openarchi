use super::project_sources_ui::SourceAction;
use super::*;
use crate::{
    application_host::{CommandContext, HostCommand},
    evidence::{Draft, EvidenceEdit, Kind, Note, Quote, Target},
};
#[derive(Clone, PartialEq, Eq)]
pub(super) struct DraftState {
    pub draft: Draft,
    pub expected: Option<Note>,
    pub base: CommandContext,
}
#[derive(Clone)]
pub(super) struct EvidencePreview {
    pub state: DraftState,
    pub prepared: Note,
}
#[derive(Default)]
pub(super) struct EvidenceUi {
    pub open: bool,
    pub draft: Option<DraftState>,
    pub preview: Option<EvidencePreview>,
    remove: Option<(Note, CommandContext)>,
    target_sheet: String,
    target_key: String,
    status_cache: Option<(CommandContext, serde_json::Value)>,
}
impl EvidenceUi {
    pub fn from_quote(&mut self, quote: Quote, base: CommandContext) {
        self.open = true;
        self.preview = None;
        self.draft = Some(DraftState {
            draft: Draft {
                id: storage::new_document_identity(),
                title: String::new(),
                kind: Kind::Question,
                quote,
                annotation: String::new(),
                targets: vec![],
                refresh_links: false,
            },
            expected: None,
            base,
        });
    }
    pub fn edit(&mut self, note: Note, base: CommandContext) {
        self.open = true;
        self.preview = None;
        self.draft = Some(DraftState {
            draft: Draft {
                id: note.id.clone(),
                title: note.title.clone(),
                kind: note.kind,
                quote: note.quote.clone(),
                annotation: note.annotation.clone(),
                targets: note.links.iter().map(|l| l.target.clone()).collect(),
                refresh_links: false,
            },
            expected: Some(note),
            base,
        });
    }
}
fn target_label(target: &Target) -> String {
    match target {
        Target::Element { id } => format!("#{id}"),
        Target::ProgramRow { sheet_id, key } => format!("{sheet_id} / {key}"),
    }
}
fn kind_label(kind: Kind) -> &'static str {
    match kind {
        Kind::Question => "Question",
        Kind::Requirement => "Requirement",
        Kind::Observation => "Observation",
    }
}
impl AtelierApp {
    pub(super) fn apply_evidence_edit(&mut self, base: CommandContext, edit: EvidenceEdit) {
        if self.menu_gated() || self.workbench_pointer_active() {
            self.source_ui.error =
                "Finish the active interaction before editing source notes".into();
            return;
        }
        match self.dispatch_project_preparation(
            base,
            HostCommand::Apply {
                request_id: storage::new_document_identity(),
                commands: vec![crate::command::ModelCommand::Evidence { edit }],
            },
        ) {
            Ok(_) => {
                self.source_ui.error.clear();
                self.evidence_ui.draft = None;
                self.evidence_ui.preview = None;
                self.evidence_ui.remove = None;
            }
            Err(e) => self.source_ui.error = e,
        }
    }
    pub(super) fn evidence_window(&mut self, ctx: &Context) {
        if !self.evidence_ui.open || self.project_conditions_ui.open {
            return;
        }
        let language = self.language;
        let mut open = true;
        egui::Window::new(language.translate("Source evidence"))
            .id(Id::new("source_evidence"))
            .open(&mut open)
            .default_width(780.)
            .default_height(800.)
            .vscroll(true)
            .show(ctx, |ui| self.evidence_conditions_contents(ui));
        self.evidence_ui.open = open;
    }
    pub(super) fn evidence_conditions_contents(&mut self, ui: &mut Ui) {
        let base = self.host.context();
        let language = self.language;
        if self
            .evidence_ui
            .status_cache
            .as_ref()
            .is_none_or(|(old, _)| *old != base)
        {
            self.evidence_ui.status_cache = Some((
                base,
                self.host
                    .document
                    .editor
                    .project
                    .evidence
                    .status(&self.host.document.editor.project),
            ));
        }
        let mut source_action = None;
        let mut model_edit = None;

        ui.label(language.translate("Source quotes and annotations are declarations, not client approval or measured survey"));
        ui.horizontal(|ui| {
            if ui.button(language.translate("Project sources")).clicked() {
                self.project_conditions_ui.navigation = Some(ConditionsGroup::Sources);
            }
            if ui
                .button(language.translate("Inspect taskbook sources"))
                .clicked()
            {
                self.project_conditions_ui.navigation = Some(ConditionsGroup::Requirements);
            }
        });
        let notes = &self.host.document.editor.project.evidence.notes;
        if notes.is_empty() {
            ui.label(
                language.translate("Choose a taskbook paragraph or cell to create a source note"),
            );
        }
        for note in notes {
            egui::CollapsingHeader::new(format!(
                "{} · {}",
                language.translate(kind_label(note.kind)),
                note.title
            ))
            .id_salt(&note.id)
            .show(ui, |ui| {
                ui.monospace(format!(
                    "{} / {}",
                    note.quote.source_sha256, note.quote.locator
                ));
                ui.label(&note.quote.text);
                ui.label(language.translate("Authored annotation"));
                ui.label(&note.annotation);
                for (index, link) in note.links.iter().enumerate() {
                    let status = self
                        .evidence_ui
                        .status_cache
                        .as_ref()
                        .and_then(|(_, v)| v["notes"].as_array())
                        .and_then(|rows| rows.iter().find(|v| v["id"] == note.id))
                        .and_then(|v| v["links"][index]["status"].as_str())
                        .unwrap_or("missing");
                    let label = match status {
                        "unchanged" => "Link unchanged",
                        "changed" => "Link changed",
                        _ => "Link missing",
                    };
                    ui.label(format!(
                        "{}: {}",
                        language.translate(label),
                        target_label(&link.target)
                    ));
                }
                ui.horizontal(|ui| {
                    if ui.button(language.translate("Edit source note")).clicked() {
                        self.evidence_ui.edit(note.clone(), base);
                    }
                    if ui
                        .button(language.translate("Remove source note"))
                        .clicked()
                    {
                        self.evidence_ui.remove = Some((note.clone(), base));
                    }
                });
            });
        }
        if let Some((note, captured)) = self.evidence_ui.remove.clone() {
            ui.separator();
            ui.label(format!(
                "{}: {}",
                language.translate("Confirm source note removal"),
                note.title
            ));
            if ui
                .button(language.translate(if self.project_conditions_ui.open {
                    "Stage condition change"
                } else {
                    "Confirm removal"
                }))
                .clicked()
            {
                model_edit = Some((captured, EvidenceEdit::Remove { expected: note }));
            }
            if ui.button(language.translate("Cancel removal")).clicked() {
                self.evidence_ui.remove = None;
            }
        }
        if let Some(preview) = &self.evidence_ui.preview {
            ui.separator();
            ui.heading(language.translate("Source note preview"));
            ui.label(&preview.prepared.title);
            ui.label(&preview.prepared.quote.text);
            ui.label(&preview.prepared.annotation);
            ui.label(language.translate(
                "Text matched stored original; authority and approval remain unverified",
            ));
            for link in &preview.prepared.links {
                ui.monospace(target_label(&link.target));
            }
            if ui
                .button(language.translate("Commit source note"))
                .clicked()
            {
                source_action = Some(SourceAction::CommitEvidence(preview.clone()));
            }
            if ui
                .button(language.translate("Cancel source note preview"))
                .clicked()
            {
                self.evidence_ui.preview = None;
            }
        } else if let Some(state) = &mut self.evidence_ui.draft {
            let draft = &mut state.draft;
            ui.separator();
            ui.heading(language.translate("Source note draft"));
            ui.monospace(format!(
                "{} / {}",
                draft.quote.source_sha256, draft.quote.locator
            ));
            ui.label(&draft.quote.text);
            ui.horizontal(|ui| {
                ui.label(language.translate("Title"));
                ui.text_edit_singleline(&mut draft.title);
            });
            egui::ComboBox::from_id_salt("source_note_kind")
                .selected_text(language.translate(kind_label(draft.kind)))
                .show_ui(ui, |ui| {
                    for kind in [Kind::Question, Kind::Requirement, Kind::Observation] {
                        ui.selectable_value(
                            &mut draft.kind,
                            kind,
                            language.translate(kind_label(kind)),
                        );
                    }
                });
            ui.label(language.translate("Authored annotation"));
            ui.text_edit_multiline(&mut draft.annotation);
            if ui
                .button(language.translate("Link selected model elements"))
                .clicked()
            {
                for id in &self.host.document.editor.selected_ids {
                    let target = Target::Element { id: *id };
                    if !draft.targets.contains(&target) {
                        draft.targets.push(target);
                    }
                }
            }
            ui.horizontal(|ui| {
                egui::ComboBox::from_id_salt("evidence_program_sheet")
                    .selected_text(if self.evidence_ui.target_sheet.is_empty() {
                        language.translate("Program sheet")
                    } else {
                        &self.evidence_ui.target_sheet
                    })
                    .show_ui(ui, |ui| {
                        for s in &self.host.document.editor.project.program.sheets {
                            ui.selectable_value(
                                &mut self.evidence_ui.target_sheet,
                                s.id.clone(),
                                &s.name,
                            );
                        }
                    });
                ui.label(language.translate("Requirement key"));
                ui.text_edit_singleline(&mut self.evidence_ui.target_key);
                if ui.button(language.translate("Link program row")).clicked() {
                    let target = Target::ProgramRow {
                        sheet_id: self.evidence_ui.target_sheet.clone(),
                        key: self.evidence_ui.target_key.clone(),
                    };
                    if !draft.targets.contains(&target) {
                        draft.targets.push(target);
                    }
                }
            });
            let mut remove = None;
            for (index, target) in draft.targets.iter().enumerate() {
                ui.horizontal(|ui| {
                    ui.monospace(target_label(target));
                    if ui
                        .small_button(language.translate("Unlink target"))
                        .clicked()
                    {
                        remove = Some(index);
                    }
                });
            }
            if let Some(index) = remove {
                draft.targets.remove(index);
            }
            ui.checkbox(
                &mut draft.refresh_links,
                language.translate("Explicitly refresh linked object baselines"),
            );
            if ui
                .button(language.translate("Preview source note"))
                .clicked()
            {
                source_action = Some(SourceAction::PreviewEvidence(state.clone()));
            }
            if ui
                .button(language.translate("Cancel source note draft"))
                .clicked()
            {
                self.evidence_ui.draft = None;
            }
        }
        if self.source_ui.pending_for_ui() {
            ui.spinner();
        }
        if !self.source_ui.error.is_empty() {
            ui.colored_label(
                Color32::LIGHT_RED,
                language.translate(&self.source_ui.error),
            );
        }

        if let Some((base, edit)) = model_edit {
            self.apply_evidence_edit(base, edit);
        }
        if let Some(action) = source_action {
            self.start_source_action(action);
        }
    }
}
