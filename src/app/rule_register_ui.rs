use super::*;
use super::{project_sources_ui::SourceAction, rules_ui::RulePreview};
use crate::{application_host::CommandContext, rules::*};
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum RegisterDraft {
    Lifecycle {
        value: LifecycleRecord,
        expected: Option<LifecycleRecord>,
        base: CommandContext,
    },
    Condition {
        value: ConditionNeed,
        expected: Option<ConditionNeed>,
        base: CommandContext,
    },
}
#[derive(Default)]
pub(super) struct RegisterUi {
    pub open: bool,
    pub draft: Option<RegisterDraft>,
}
impl RegisterUi {
    pub fn lifecycle(&mut self, citation: Citation, base: CommandContext) {
        self.open = true;
        self.draft = Some(RegisterDraft::Lifecycle {
            value: LifecycleRecord {
                id: storage::new_document_identity(),
                title: String::new(),
                subject: DocumentRef::default(),
                successor: None,
                kind: LifecycleKind::Withdrawn,
                scope: LifecycleScope::Unresolved,
                effective_on: String::new(),
                role: EvidenceRole::Report,
                citation,
                corroboration: vec![],
                scope_note: String::new(),
                missing: String::new(),
            },
            expected: None,
            base,
        });
    }
    pub fn condition(&mut self, book: &RuleBook, base: CommandContext) {
        self.open = true;
        let mut value = ConditionNeed {
            id: storage::new_document_identity(),
            title: String::new(),
            topic: Topic::Planning,
            question: String::new(),
            originals: vec![],
            requirement_ids: vec![],
            citation: None,
            status: ConditionStatus::Missing,
            rationale: String::new(),
            missing: String::new(),
            basis_sha256: String::new(),
        };
        value.basis_sha256 = book.condition_basis(&value).unwrap_or_default();
        self.draft = Some(RegisterDraft::Condition {
            value,
            expected: None,
            base,
        });
    }
}
pub(super) fn register_edit(edit: &RuleEdit) -> bool {
    matches!(
        edit,
        RuleEdit::PutLifecycle { .. }
            | RuleEdit::RemoveLifecycle { .. }
            | RuleEdit::PutCondition { .. }
            | RuleEdit::RemoveCondition { .. }
    )
}
fn field(ui: &mut Ui, lang: UiLanguage, key: &str, value: &mut String) {
    ui.horizontal(|ui| {
        ui.label(lang.translate(key));
        ui.text_edit_singleline(value);
    });
}
fn reference(ui: &mut Ui, lang: UiLanguage, key: &str, value: &mut DocumentRef, book: &RuleBook) {
    ui.push_id(key, |ui| {
        let mut missing = value.is_missing();
        if ui
            .checkbox(&mut missing, lang.translate("Original not acquired"))
            .changed()
        {
            *value = if missing {
                DocumentRef::default()
            } else {
                DocumentRef::Recorded {
                    source_id: String::new(),
                }
            };
        }
        match value {
            DocumentRef::Recorded { source_id } => {
                egui::ComboBox::from_id_salt("registered_original")
                    .selected_text(
                        book.source(source_id)
                            .map(|s| s.title.as_str())
                            .unwrap_or(lang.translate("Choose recorded original")),
                    )
                    .show_ui(ui, |ui| {
                        for source in &book.sources {
                            ui.selectable_value(
                                source_id,
                                source.id.clone(),
                                format!("{} · {} · {}", source.code, source.edition, source.title),
                            );
                        }
                    });
            }
            DocumentRef::Missing {
                title,
                code_hint,
                edition_hint,
                jurisdiction_hint,
            } => {
                field(ui, lang, "Requested original title", title);
                field(ui, lang, "Document code hint", code_hint);
                field(ui, lang, "Edition hint", edition_hint);
                field(ui, lang, "Jurisdiction hint", jurisdiction_hint);
                ui.label(lang.translate("Hints do not substitute for an acquired original"));
            }
        }
    });
}
fn show_reference(ui: &mut Ui, lang: UiLanguage, book: &RuleBook, value: &DocumentRef) {
    ui.label(format!(
        "{}: {}",
        lang.translate(if value.is_missing() {
            "Original not acquired"
        } else {
            "Recorded original"
        }),
        value.label(book)
    ));
}
fn quote(ui: &mut Ui, lang: UiLanguage, citation: &Citation) {
    ui.label(lang.translate("Exact original quote"));
    ui.monospace(format!("{} / {}", citation.source_id, citation.locator));
    ui.label(&citation.text);
}
impl AtelierApp {
    pub(super) fn rule_register_window(&mut self, ctx: &Context) {
        if !self.rules_ui.register.open || self.project_conditions_ui.open {
            return;
        }
        let lang = self.language;
        let book = self.host.document.editor.project.rules.clone();
        let base = self.host.context();
        let mut open = true;
        let mut intent = None;
        let mut action = None;
        egui::Window::new(lang.translate("Lifecycle and condition register"))
            .id(Id::new("rule_register"))
            .default_pos(pos2(40., 40.))
            .open(&mut open)
            .default_width(850.)
            .default_height(760.)
            .vscroll(true)
            .show(ctx, |ui| {
                self.rule_register_embedded(ui, &book, base, &mut intent, &mut action);
            });
        self.rules_ui.register.open = open;
        if !open {
            self.rules_ui.register.draft = None;
            if self
                .rules_ui
                .request
                .as_ref()
                .is_some_and(|p| register_edit(&p.edit))
            {
                self.rules_ui.request = None;
                self.rules_ui.preview = None;
            }
        }
        if !open {
            return;
        }
        if let Some(preview) = intent {
            self.rules_ui.request = Some(preview.clone());
            self.rules_ui.preview = None;
            action = Some(SourceAction::PreviewRule(preview));
        }
        if let Some(action) = action {
            self.start_source_action(action);
        }
    }
    pub(super) fn rule_register_embedded(
        &mut self,
        ui: &mut Ui,
        book: &RuleBook,
        base: CommandContext,
        intent: &mut Option<RulePreview>,
        action: &mut Option<SourceAction>,
    ) {
        let lang = self.language;
        let before = self.rules_ui.register.draft.clone();
        self.rule_register_contents(ui, &book, base, intent, action);
        if before != self.rules_ui.register.draft {
            self.rules_ui.request = None;
            self.rules_ui.preview = None;
        }
        if let Some(preview) = self
            .rules_ui
            .preview
            .clone()
            .filter(|p| register_edit(&p.edit))
        {
            ui.separator();
            ui.heading(lang.translate("Rule edit preview"));
            ui.label(
                lang.translate("Edit validated; source checks do not prove legal applicability"),
            );
            self.register_preview(ui, &book, &preview.edit);
            if ui
                .button(lang.translate("Return to rule editing"))
                .clicked()
            {
                self.rules_ui.preview = None;
                self.rules_ui.request = None;
            }
            if ui
                .button(lang.translate(if self.project_conditions_ui.open {
                    "Stage condition change"
                } else {
                    "Confirm register edit"
                }))
                .clicked()
            {
                *action = Some(SourceAction::CommitRule(preview));
            }
        }
        if self.rules_ui.register.draft.is_some()
            || self
                .rules_ui
                .preview
                .as_ref()
                .is_some_and(|p| register_edit(&p.edit))
        {
            if ui.button(lang.translate("Cancel register edit")).clicked() {
                self.rules_ui.register.draft = None;
                self.rules_ui.request = None;
                self.rules_ui.preview = None;
            }
        }
        if !self.source_ui.error.is_empty() {
            ui.colored_label(Color32::LIGHT_RED, lang.translate(&self.source_ui.error));
        }
        if self.source_ui.pending_for_ui() {
            ui.spinner();
        }
    }
    fn rule_register_contents(
        &mut self,
        ui: &mut Ui,
        book: &RuleBook,
        base: CommandContext,
        intent: &mut Option<RulePreview>,
        action: &mut Option<SourceAction>,
    ) {
        let lang = self.language;
        ui.label(lang.translate("Lifecycle and condition entries are declarations; coverage and legal effect remain unverified"));
        ui.horizontal(|ui| {
            if ui
                .button(lang.translate("Inspect source block for lifecycle evidence"))
                .clicked()
            {
                self.rules_ui.open = true;
            }
            if ui
                .button(lang.translate("Add condition to verify"))
                .clicked()
            {
                self.rules_ui.register.condition(book, base);
                self.rules_ui.request = None;
                self.rules_ui.preview = None;
            }
        });
        if self
            .rules_ui
            .preview
            .as_ref()
            .is_some_and(|p| register_edit(&p.edit))
        {
            return;
        }
        if self.rules_ui.register.draft.is_none() {
            for record in &book.lifecycle {
                egui::CollapsingHeader::new(format!(
                    "{} · {}",
                    lang.translate(record.kind.label()),
                    record.title
                ))
                .id_salt(("lifecycle_record", &record.id))
                .show(ui, |ui| {
                    ui.colored_label(
                        Color32::YELLOW,
                        lang.translate(book.lifecycle_state(record)),
                    );
                    show_reference(ui, lang, book, &record.subject);
                    if let Some(d) = &record.successor {
                        show_reference(ui, lang, book, d)
                    }
                    ui.label(lang.translate(record.role.label()));
                    ui.label(lang.translate(record.scope.label()));
                    ui.label(&record.effective_on);
                    ui.label(&record.scope_note);
                    ui.label(&record.missing);
                    quote(ui, lang, &record.citation);
                    for c in &record.corroboration {
                        quote(ui, lang, c);
                    }
                    ui.horizontal(|ui| {
                        if ui
                            .button(lang.translate("Locate original clause"))
                            .clicked()
                        {
                            *action = self.rules_ui.request_original(&record.citation, book);
                        }
                        if ui.button(lang.translate("Edit lifecycle record")).clicked() {
                            self.rules_ui.register.draft = Some(RegisterDraft::Lifecycle {
                                value: record.clone(),
                                expected: Some(record.clone()),
                                base,
                            });
                        }
                        if ui
                            .button(lang.translate("Remove lifecycle record"))
                            .clicked()
                        {
                            *intent = Some(RulePreview {
                                base,
                                edit: RuleEdit::RemoveLifecycle {
                                    expected: record.clone(),
                                },
                            });
                        }
                    });
                });
            }
            for need in &book.conditions {
                egui::CollapsingHeader::new(format!(
                    "{} · {}",
                    lang.translate(need.topic.label()),
                    need.title
                ))
                .id_salt(("condition_need", &need.id))
                .show(ui, |ui| {
                    ui.colored_label(
                        Color32::YELLOW,
                        lang.translate(
                            book.condition_state(&self.host.document.editor.project, need),
                        ),
                    );
                    ui.label(&need.question);
                    ui.label(&need.rationale);
                    ui.label(&need.missing);
                    for d in &need.originals {
                        show_reference(ui, lang, book, d)
                    }
                    for id in &need.requirement_ids {
                        if let Some(r) = book.requirements.iter().find(|r| &r.id == id) {
                            ui.label(format!(
                                "{} · {}",
                                r.title,
                                lang.translate(
                                    book.requirement_state(&self.host.document.editor.project, r)
                                )
                            ));
                        }
                    }
                    if let Some(c) = &need.citation {
                        quote(ui, lang, c);
                        if ui
                            .button(lang.translate("Locate original clause"))
                            .clicked()
                        {
                            *action = self.rules_ui.request_original(c, book);
                        }
                    }
                    ui.horizontal(|ui| {
                        if ui.button(lang.translate("Edit condition need")).clicked() {
                            self.rules_ui.register.draft = Some(RegisterDraft::Condition {
                                value: need.clone(),
                                expected: Some(need.clone()),
                                base,
                            });
                        }
                        if ui.button(lang.translate("Remove condition need")).clicked() {
                            *intent = Some(RulePreview {
                                base,
                                edit: RuleEdit::RemoveCondition {
                                    expected: need.clone(),
                                },
                            });
                        }
                    });
                });
            }
        }
        if let Some(draft) = &mut self.rules_ui.register.draft {
            ui.separator();
            match draft {
                RegisterDraft::Lifecycle {
                    value: r,
                    expected,
                    base: captured,
                } => {
                    field(ui, lang, "Lifecycle record title", &mut r.title);
                    quote(ui, lang, &r.citation);
                    for c in &r.corroboration {
                        quote(ui, lang, c);
                    }
                    let mut remove = None;
                    for (i, c) in r.corroboration.iter().enumerate() {
                        ui.push_id(("corroborating", i), |ui| {
                            ui.monospace(&c.locator);
                            if ui
                                .button(lang.translate("Remove corroborating block"))
                                .clicked()
                            {
                                remove = Some(i);
                            }
                        });
                    }
                    if let Some(i) = remove {
                        r.corroboration.remove(i);
                    }
                    ui.heading(lang.translate("Lifecycle subject"));
                    reference(ui, lang, "subject", &mut r.subject, book);
                    egui::ComboBox::from_id_salt("lifecycle_kind")
                        .selected_text(lang.translate(r.kind.label()))
                        .show_ui(ui, |ui| {
                            for value in LifecycleKind::ALL {
                                ui.selectable_value(
                                    &mut r.kind,
                                    value,
                                    lang.translate(value.label()),
                                );
                            }
                        });
                    let mut has_successor = r.successor.is_some();
                    if ui
                        .checkbox(
                            &mut has_successor,
                            lang.translate("Record successor original"),
                        )
                        .changed()
                    {
                        r.successor = has_successor.then(DocumentRef::default)
                    }
                    if let Some(d) = &mut r.successor {
                        reference(ui, lang, "successor", d, book);
                    }
                    egui::ComboBox::from_id_salt("lifecycle_scope")
                        .selected_text(lang.translate(r.scope.label()))
                        .show_ui(ui, |ui| {
                            for value in LifecycleScope::ALL {
                                ui.selectable_value(
                                    &mut r.scope,
                                    value,
                                    lang.translate(value.label()),
                                );
                            }
                        });
                    egui::ComboBox::from_id_salt("lifecycle_role")
                        .selected_text(lang.translate(r.role.label()))
                        .show_ui(ui, |ui| {
                            for value in EvidenceRole::ALL {
                                ui.selectable_value(
                                    &mut r.role,
                                    value,
                                    lang.translate(value.label()),
                                );
                            }
                        });
                    field(ui, lang, "Effective on (YYYY-MM-DD)", &mut r.effective_on);
                    ui.label(lang.translate(
                        "Leave unknown dates empty; never infer a day from a month-only statement",
                    ));
                    ui.label(lang.translate("Lifecycle scope and clause limits"));
                    ui.text_edit_multiline(&mut r.scope_note);
                    ui.label(lang.translate("Missing information and conflicts"));
                    ui.text_edit_multiline(&mut r.missing);
                    if ui
                        .button(lang.translate("Recheck lifecycle edit against current project"))
                        .clicked()
                    {
                        if captured.session == base.session {
                            *captured = base;
                        } else {
                            self.source_ui.error="Document session changed; reopen the record in the intended project".into();
                        }
                    }
                    if ui
                        .button(lang.translate("Preview lifecycle record"))
                        .clicked()
                    {
                        *intent = Some(RulePreview {
                            base: *captured,
                            edit: RuleEdit::PutLifecycle {
                                expected: expected.clone(),
                                record: r.clone(),
                            },
                        });
                    }
                }
                RegisterDraft::Condition {
                    value: r,
                    expected,
                    base: captured,
                } => {
                    field(ui, lang, "Condition title", &mut r.title);
                    egui::ComboBox::from_id_salt("condition_topic")
                        .selected_text(lang.translate(r.topic.label()))
                        .show_ui(ui, |ui| {
                            for value in Topic::ALL {
                                ui.selectable_value(
                                    &mut r.topic,
                                    value,
                                    lang.translate(value.label()),
                                );
                            }
                        });
                    ui.label(lang.translate("Question and verification scope"));
                    ui.text_edit_multiline(&mut r.question);
                    let mut remove = None;
                    for (index, d) in r.originals.iter_mut().enumerate() {
                        reference(ui, lang, &format!("needed-{index}"), d, book);
                        if ui
                            .push_id(index, |ui| {
                                ui.button(lang.translate("Remove requested original"))
                            })
                            .inner
                            .clicked()
                        {
                            remove = Some(index)
                        }
                    }
                    if let Some(i) = remove {
                        r.originals.remove(i);
                    }
                    if ui
                        .button(lang.translate("Add requested original"))
                        .clicked()
                    {
                        r.originals.push(DocumentRef::default());
                    }
                    ui.label(lang.translate("Linked cited requirements"));
                    for req in &book.requirements {
                        let mut chosen = r.requirement_ids.contains(&req.id);
                        if ui.checkbox(&mut chosen, &req.title).changed() {
                            if chosen {
                                r.requirement_ids.push(req.id.clone())
                            } else {
                                r.requirement_ids.retain(|id| id != &req.id)
                            }
                        }
                    }
                    if let Some(c) = &r.citation {
                        quote(ui, lang, c)
                    }
                    egui::ComboBox::from_id_salt("condition_status")
                        .selected_text(lang.translate(r.status.label()))
                        .show_ui(ui, |ui| {
                            for value in ConditionStatus::ALL {
                                ui.selectable_value(
                                    &mut r.status,
                                    value,
                                    lang.translate(value.label()),
                                );
                            }
                        });
                    ui.label(lang.translate("Condition review rationale"));
                    ui.text_edit_multiline(&mut r.rationale);
                    ui.label(lang.translate("Missing information and conflicts"));
                    ui.text_edit_multiline(&mut r.missing);
                    if ui
                        .button(lang.translate("Explicitly refresh condition review basis"))
                        .clicked()
                    {
                        if captured.session == base.session {
                            r.basis_sha256 = book.condition_basis(r).unwrap_or_default();
                            *captured = base;
                        } else {
                            self.source_ui.error="Document session changed; reopen the record in the intended project".into();
                        }
                    }
                    if ui
                        .button(lang.translate("Preview condition need"))
                        .clicked()
                    {
                        if expected.is_none() {
                            r.basis_sha256 = book.condition_basis(r).unwrap_or_default();
                        }
                        *intent = Some(RulePreview {
                            base: *captured,
                            edit: RuleEdit::PutCondition {
                                expected: expected.clone(),
                                condition: r.clone(),
                            },
                        });
                    }
                }
            }
        }
    }
    fn register_preview(&self, ui: &mut Ui, book: &RuleBook, edit: &RuleEdit) {
        let lang = self.language;
        match edit {
            RuleEdit::PutLifecycle { record: r, .. } => {
                ui.label(&r.title);
                show_reference(ui, lang, book, &r.subject);
                if let Some(d) = &r.successor {
                    show_reference(ui, lang, book, d)
                }
                ui.label(lang.translate(r.kind.label()));
                ui.label(lang.translate(r.scope.label()));
                ui.label(lang.translate(r.role.label()));
                ui.label(&r.effective_on);
                ui.label(&r.scope_note);
                ui.label(&r.missing);
                quote(ui, lang, &r.citation);
                for c in &r.corroboration {
                    quote(ui, lang, c);
                }
            }
            RuleEdit::PutCondition { condition: r, .. } => {
                ui.label(&r.title);
                ui.label(&r.question);
                ui.label(lang.translate(r.status.label()));
                for d in &r.originals {
                    show_reference(ui, lang, book, d)
                }
                for id in &r.requirement_ids {
                    ui.label(id);
                }
                if let Some(c) = &r.citation {
                    quote(ui, lang, c)
                }
                ui.label(&r.rationale);
                ui.label(&r.missing);
            }
            RuleEdit::RemoveLifecycle { expected } => {
                ui.label(format!(
                    "{}: {}",
                    lang.translate("Remove lifecycle record"),
                    expected.title
                ));
            }
            RuleEdit::RemoveCondition { expected } => {
                ui.label(format!(
                    "{}: {}",
                    lang.translate("Remove condition need"),
                    expected.title
                ));
            }
            _ => {}
        }
    }
}
