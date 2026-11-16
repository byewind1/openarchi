use super::project_sources_ui::SourceAction;
use super::*;
use crate::project_repository::quality::{Counts, Draft, Review, ReviewOperation, Verdict};
#[derive(Clone)]
pub(super) struct ReviewRequest {
    pub operation_id: String,
    pub draft: Draft,
    pub root: PathBuf,
}
#[derive(Default)]
pub(super) struct ReviewUi {
    pub open: bool,
    pub subject: String,
    pub draft: Draft,
    pub records: Vec<Review>,
    pub pending: Option<ReviewRequest>,
    pub operation_result: Option<ReviewOperation>,
    pub(super) counts: Counts,
    pub(super) counts_enabled: bool,
}
impl ReviewUi {
    pub fn choose(&mut self, subject: String) {
        self.open = true;
        self.subject = subject.clone();
        self.records.clear();
        if self.pending.is_none() {
            self.draft = Draft {
                subject_id: subject,
                method: "Declared review".into(),
                ..Default::default()
            };
            self.counts = Default::default();
            self.counts_enabled = false;
        }
    }
    pub fn accept(&mut self, subject: String, records: Vec<Review>) {
        if let Some(pending) = &self.pending
            && let Some(review) = records
                .iter()
                .find(|r| r.operation_id == pending.operation_id && r.draft == pending.draft)
        {
            self.operation_result = Some(ReviewOperation::Published {
                review: review.clone(),
            });
            self.pending = None;
        }
        if self.subject == subject {
            self.records = records;
        }
    }
    pub fn accept_operation(&mut self, id: String, result: ReviewOperation) {
        if let Some(pending) = &self.pending
            && pending.operation_id == id
        {
            match &result {
                ReviewOperation::NotFound => self.pending = None,
                ReviewOperation::Published { review } if review.draft == pending.draft => {
                    self.pending = None
                }
                _ => {}
            }
        }
        self.operation_result = Some(result);
    }
}
fn count(ui: &mut Ui, language: UiLanguage, label: &str, value: &mut u64) {
    ui.horizontal(|ui| {
        ui.label(language.translate(label));
        ui.add(egui::DragValue::new(value).range(0..=u64::MAX));
    });
}
impl AtelierApp {
    pub(super) fn source_review_window(&mut self, ctx: &Context) {
        if !self.source_ui.review.open || self.project_conditions_ui.open {
            return;
        }
        let language = self.language;
        let mut open = true;
        let mut action = None;
        egui::Window::new(language.translate("Source quality reports"))
            .id(Id::new("source_quality_reports"))
            .open(&mut open)
            .default_width(750.)
            .default_height(780.)
            .vscroll(true)
            .show(ctx, |ui| {
                self.source_review_contents(ui, &mut action);
            });
        self.source_ui.review.open = open;
        if let Some(action) = action {
            self.start_source_action(action);
        }
    }
    pub(super) fn source_review_contents(
        &mut self,
        ui: &mut Ui,
        action: &mut Option<SourceAction>,
    ) {
        let language = self.language;
        ui.label(language.translate(
            "Stored reviews are declarations, not approval or independent geometric validation",
        ));
        let Some(repository) = &self.source_ui.repository else {
            ui.label(language.translate("Open a project repository first"));
            return;
        };
        ui.monospace(&self.source_ui.review.subject);
        for review in &self.source_ui.review.records {
            egui::CollapsingHeader::new(format!(
                "{} · {}",
                language.translate(match review.draft.verdict {
                    Verdict::Unverified => "Unverified review",
                    Verdict::Rejected => "Rejected review",
                }),
                review.draft.method
            ))
            .id_salt(&review.operation_id)
            .show(ui, |ui| {
                ui.monospace(&review.operation_id);
                ui.label(&review.draft.scope);
                ui.label(&review.draft.summary);
                if let Some(c) = &review.draft.comparison {
                    ui.label(format!(
                        "{}: {} / {} / {} / {} / {}",
                        language
                            .translate("Expected / observed / missing / extra / duplicate handles"),
                        c.expected_unique_handles,
                        c.observed_unique_handles,
                        c.missing_handles,
                        c.extra_handles,
                        c.duplicate_handles
                    ));
                }
                ui.label(language.translate("Limits and unknowns"));
                ui.label(&review.draft.limits);
                for id in &review.draft.evidence_source_ids {
                    if let Some(source) = repository.manifest.sources.iter().find(|s| &s.id == id) {
                        ui.label(source.names.join(" / "));
                        ui.monospace(id);
                    }
                }
            });
        }
        if let Some(result) = &self.source_ui.review.operation_result {
            ui.label(language.translate(match result {
                ReviewOperation::Published { .. } => "Review operation published",
                ReviewOperation::Unpublished { .. } => "Review artifact unpublished; do not replay",
                ReviewOperation::NotFound => "Review operation not found; no stored report",
            }));
        }
        if let Some(pending) = &self.source_ui.review.pending {
            ui.label(language.translate("Inspect the original operation before retrying"));
            ui.monospace(&pending.operation_id);
            if ui
                .add_enabled(
                    pending.root == repository.root(),
                    egui::Button::new(language.translate("Inspect review operation")),
                )
                .clicked()
            {
                *action = Some(SourceAction::ReviewOperation(pending.operation_id.clone()));
            }
            if pending.root != repository.root() {
                ui.label(
                    language
                        .translate("Reopen the original project folder to inspect this operation"),
                );
            }
        }
        let bound = repository.manifest.document_id == self.host.document.editor.project_identity();
        ui.add_enabled_ui(
            bound && self.source_ui.review.pending.is_none() && !self.source_ui.pending_for_ui(),
            |ui| {
                let state = &mut self.source_ui.review;
                let draft = &mut state.draft;
                ui.separator();
                ui.heading(language.translate("New quality report"));
                ui.horizontal(|ui| {
                    ui.label(language.translate("Review method"));
                    ui.text_edit_singleline(&mut draft.method);
                });
                ui.label(language.translate("Checked scope"));
                ui.text_edit_multiline(&mut draft.scope);
                ui.horizontal(|ui| {
                    ui.radio_value(
                        &mut draft.verdict,
                        Verdict::Unverified,
                        language.translate("Unverified review"),
                    );
                    ui.radio_value(
                        &mut draft.verdict,
                        Verdict::Rejected,
                        language.translate("Rejected review"),
                    );
                });
                ui.label(language.translate("Review summary"));
                ui.text_edit_multiline(&mut draft.summary);
                ui.label(language.translate("Limits and unknowns"));
                ui.text_edit_multiline(&mut draft.limits);
                egui::ComboBox::from_id_salt("review_reference")
                    .selected_text(language.translate("Reference source (optional)"))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut draft.reference_source_id,
                            None,
                            language.translate("Not mapped"),
                        );
                        for source in &repository.manifest.sources {
                            if source.id != draft.subject_id {
                                ui.selectable_value(
                                    &mut draft.reference_source_id,
                                    Some(source.id.clone()),
                                    source.names.join(" / "),
                                );
                            }
                        }
                    });
                if let Some(id) = &draft.reference_source_id {
                    ui.monospace(id);
                }
                ui.checkbox(
                    &mut state.counts_enabled,
                    language.translate("Record explicit unique-handle comparison"),
                );
                if state.counts_enabled {
                    count(
                        ui,
                        language,
                        "Expected unique handles",
                        &mut state.counts.expected_unique_handles,
                    );
                    count(
                        ui,
                        language,
                        "Observed unique handles",
                        &mut state.counts.observed_unique_handles,
                    );
                    count(
                        ui,
                        language,
                        "Missing handles",
                        &mut state.counts.missing_handles,
                    );
                    count(
                        ui,
                        language,
                        "Extra handles",
                        &mut state.counts.extra_handles,
                    );
                    count(
                        ui,
                        language,
                        "Duplicate handles",
                        &mut state.counts.duplicate_handles,
                    );
                }
                draft.comparison = state.counts_enabled.then(|| state.counts.clone());
                egui::CollapsingHeader::new(language.translate("Supporting copied sources")).show(
                    ui,
                    |ui| {
                        for source in &repository.manifest.sources {
                            if source.id != draft.subject_id {
                                let mut selected = draft.evidence_source_ids.contains(&source.id);
                                if ui
                                    .checkbox(&mut selected, source.names.join(" / "))
                                    .changed()
                                {
                                    if selected {
                                        draft.evidence_source_ids.push(source.id.clone());
                                    } else {
                                        draft.evidence_source_ids.retain(|id| id != &source.id);
                                    }
                                }
                            }
                        }
                    },
                );
                let validation = draft.validate(&repository.manifest);
                if let Err(e) = &validation {
                    ui.label(language.translate(e));
                }
                if ui
                    .add_enabled(
                        validation.is_ok(),
                        egui::Button::new(language.translate("Save quality report")),
                    )
                    .clicked()
                {
                    let request = ReviewRequest {
                        operation_id: storage::new_document_identity(),
                        draft: draft.clone(),
                        root: repository.root().to_owned(),
                    };
                    state.pending = Some(request.clone());
                    *action = Some(SourceAction::RecordReview(request));
                }
            },
        );
        if !self.source_ui.error.is_empty() {
            ui.colored_label(
                Color32::LIGHT_RED,
                language.translate(&self.source_ui.error),
            );
        }
    }
}
