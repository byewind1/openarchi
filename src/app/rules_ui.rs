use super::project_sources_ui::SourceAction;
use super::*;
use crate::{application_host::CommandContext, rules::*};
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct RulePreview {
    pub base: CommandContext,
    pub edit: RuleEdit,
}
#[derive(Default)]
pub(super) struct RulesUi {
    pub open: bool,
    pub request: Option<RulePreview>,
    pub preview: Option<RulePreview>,
    pub original: Option<crate::project_repository::rules::RuleOriginal>,
    pub context_draft: Option<(ProjectContext, ProjectContext, CommandContext)>,
    pub source_draft: Option<(RuleSource, CommandContext)>,
    pub requirement_draft: Option<(Requirement, Option<Requirement>, CommandContext)>,
    pub source_id: String,
    pub register: super::rule_register_ui::RegisterUi,
    filter: String,
    paragraph: usize,
    selected_only: bool,
    wanted_locator: Option<String>,
}
impl RulesUi {
    pub(super) fn request_original(
        &mut self,
        citation: &Citation,
        book: &RuleBook,
    ) -> Option<SourceAction> {
        let source = book.source(&citation.source_id)?;
        self.open = true;
        self.source_id = source.id.clone();
        self.filter = citation.text.clone();
        self.wanted_locator = Some(citation.locator.clone());
        self.original = None;
        Some(SourceAction::InspectRuleOriginal(
            source.original_sha256.clone(),
        ))
    }

    pub(super) fn accept_original(
        &mut self,
        original: crate::project_repository::rules::RuleOriginal,
    ) -> Result<(), String> {
        self.paragraph = if let Some(locator) = &self.wanted_locator {
            original
                .document
                .paragraphs
                .iter()
                .position(|p| &p.locator == locator && p.text == self.filter)
                .ok_or("Rule citation differs from its stored original locator/text")?
        } else {
            0
        };
        self.original = Some(original);
        Ok(())
    }
}
fn rule_text(ui: &mut Ui, lang: UiLanguage, key: &str, value: &mut String) {
    ui.horizontal(|ui| {
        let label = ui.label(lang.translate(key));
        let response = ui.text_edit_singleline(value).labelled_by(label.id);
        #[cfg(test)]
        ui.ctx()
            .data_mut(|data| data.insert_temp(Id::new(("rule_context_field", key)), response.rect));
        let _ = response;
    });
}
impl AtelierApp {
    pub(super) fn apply_rule_edit(
        &mut self,
        preview: RulePreview,
        admission: crate::project_repository::rules::RuleCommitGuard,
    ) {
        if self.menu_gated() || self.workbench_pointer_active() {
            self.source_ui.error = "Finish the active interaction before editing rules".into();
            return;
        }
        if self.project_conditions_ui.open {
            match self.stage_condition_commands(
                preview.base,
                vec![crate::command::ModelCommand::Rules { edit: preview.edit }],
            ) {
                Ok(()) => {
                    self.rules_ui.request = None;
                    self.rules_ui.preview = None;
                    self.rules_ui.source_draft = None;
                    self.rules_ui.context_draft = None;
                    self.rules_ui.requirement_draft = None;
                    self.rules_ui.register.draft = None;
                }
                Err(e) => self.source_ui.error = e,
            }
            return;
        }
        match self.host.apply_rule_edit(
            preview.base,
            &storage::new_document_identity(),
            preview.edit,
            admission,
        ) {
            Ok(_) => {
                self.rules_ui.request = None;
                self.rules_ui.preview = None;
                self.rules_ui.source_draft = None;
                self.rules_ui.context_draft = None;
                self.rules_ui.requirement_draft = None;
                self.rules_ui.register.draft = None;
                self.source_ui.error.clear();
            }
            Err(e) => self.source_ui.error = e,
        }
    }
    pub(super) fn rules_window(&mut self, ctx: &Context) {
        if !self.rules_ui.open || self.project_conditions_ui.open {
            return;
        }
        let lang = self.language;
        let base = self.host.context();
        let mut open = true;
        let mut action = None;
        let mut intent = None;
        let book = self.host.document.editor.project.rules.clone();
        egui::Window::new(lang.translate("Conditions and rules"))
            .id(egui::Id::new("conditions_rules"))
            .open(&mut open)
            .default_width(850.)
            .default_height(760.)
            .vscroll(true)
            .show(ctx, |ui| {
                self.rules_contents(ui, &book, base, &mut action, &mut intent)
            });
        self.rules_ui.open = open;
        if !open
            && !(self.rules_ui.register.open
                && self
                    .rules_ui
                    .request
                    .as_ref()
                    .is_some_and(|p| super::rule_register_ui::register_edit(&p.edit)))
        {
            self.rules_ui.request = None;
            self.rules_ui.preview = None;
        }
        if let Some(preview) = intent {
            self.rules_ui.preview = None;
            self.rules_ui.request = Some(preview.clone());
            action = Some(SourceAction::PreviewRule(preview));
        }
        if let Some(action) = action {
            self.start_source_action(action);
        }
    }
    pub(super) fn rules_contents(
        &mut self,
        ui: &mut Ui,
        book: &RuleBook,
        base: CommandContext,
        action: &mut Option<SourceAction>,
        intent: &mut Option<RulePreview>,
    ) {
        let lang = self.language;
        let before = (
            self.rules_ui.context_draft.clone(),
            self.rules_ui.source_draft.clone(),
            self.rules_ui.requirement_draft.clone(),
        );
        ui.label(lang.translate(
            "Cited declarations only; no compliance check or legal approval has been performed",
        ));
        ui.horizontal_wrapped(|ui| {
            if ui
                .button(lang.translate("Lifecycle and condition register"))
                .clicked()
            {
                self.rules_ui.register.open = true;
            }
            if ui.button(lang.translate("Project sources")).clicked() {
                self.project_conditions_ui.navigation = Some(ConditionsGroup::Sources);
            }
            if ui
                .button(lang.translate("Edit applicability context"))
                .clicked()
            {
                self.rules_ui.context_draft =
                    Some((book.context.clone(), book.context.clone(), base));
            }
            if ui
                .button(lang.translate("Record original revision"))
                .clicked()
            {
                self.rules_ui.source_draft = Some((
                    RuleSource {
                        id: storage::new_document_identity(),
                        title: String::new(),
                        code: String::new(),
                        edition: String::new(),
                        publisher: String::new(),
                        jurisdiction: book.context.jurisdiction.clone(),
                        published_on: String::new(),
                        effective_on: String::new(),
                        original_sha256: String::new(),
                        origin_url: String::new(),
                        origin: Origin::Unverified,
                        verification_note: String::new(),
                        replaces: vec![],
                        same_edition_as: None,
                    },
                    base,
                ));
            }
        });
        if self.rules_ui.preview.is_none()
            && let Some((draft, expected, captured)) = &mut self.rules_ui.context_draft
        {
            ui.separator();
            ui.heading(lang.translate("Edit applicability context"));
            rule_text(ui, lang, "Jurisdiction", &mut draft.jurisdiction);
            rule_text(ui, lang, "Project uses and occupancy", &mut draft.uses);
            rule_text(
                ui,
                lang,
                "Assessment date (YYYY-MM-DD)",
                &mut draft.assessment_date,
            );
            rule_text(ui, lang, "Design stage", &mut draft.stage);
            if ui
                .button(lang.translate("Preview context change"))
                .clicked()
            {
                *intent = Some(RulePreview {
                    base: *captured,
                    edit: RuleEdit::SetContext {
                        expected: expected.clone(),
                        context: draft.clone(),
                    },
                });
            }
        }
        ui.label(format!(
            "{}: {} / {} / {} / {}",
            lang.translate("Project applicability context"),
            book.context.jurisdiction,
            book.context.uses,
            book.context.assessment_date,
            book.context.stage
        ));
        if !book.context.complete() {
            ui.colored_label(
                Color32::YELLOW,
                lang.translate("Project applicability context is incomplete"),
            );
        }
        egui::CollapsingHeader::new(lang.translate("Predesign coverage matrix")).show(ui, |ui| {
            ui.label(lang.translate("Recorded clauses do not prove complete mandatory coverage"));
            for topic in Topic::ALL {
                let count = book
                    .requirements
                    .iter()
                    .filter(|r| r.topic == topic)
                    .count();
                ui.label(format!(
                    "{}: {} · {}",
                    lang.translate(topic.label()),
                    count,
                    lang.translate(if count == 0 {
                        "Missing conditions"
                    } else {
                        "Recorded, not verified"
                    })
                ));
            }
        });
        ui.separator();
        for source in &book.sources {
            egui::CollapsingHeader::new(format!(
                "{} · {} · {}",
                source.code, source.edition, source.title
            ))
            .id_salt(("rule_source", &source.id))
            .show(ui, |ui| {
                ui.monospace(format!("{} / {}", source.id, source.original_sha256));
                ui.label(format!(
                    "{} · {}",
                    lang.translate(source.origin.label()),
                    source.publisher
                ));
                ui.label(&source.origin_url);
                ui.label(format!("{} / {}", source.published_on, source.effective_on));
                ui.label(&source.verification_note);
                if !source.replaces.is_empty() {
                    ui.label(format!(
                        "{}: {}",
                        lang.translate("Replaces recorded revisions"),
                        source.replaces.join(", ")
                    ));
                }
                if ui.button(lang.translate("Inspect rule original")).clicked() {
                    self.rules_ui.source_id = source.id.clone();
                    self.rules_ui.original = None;
                    self.rules_ui.paragraph = 0;
                    self.rules_ui.filter.clear();
                    self.rules_ui.wanted_locator = None;
                    *action = Some(SourceAction::InspectRuleOriginal(
                        source.original_sha256.clone(),
                    ));
                }
                if ui
                    .button(lang.translate("Remove original revision"))
                    .clicked()
                {
                    *intent = Some(RulePreview {
                        base,
                        edit: RuleEdit::RemoveSource {
                            expected: source.clone(),
                        },
                    });
                }
            });
        }
        if self.rules_ui.preview.is_none()
            && let Some(original) = &self.rules_ui.original
        {
            ui.separator();
            ui.heading(lang.translate("Stored original text"));
            ui.monospace(&original.source_sha256);
            ui.label(
                lang.translate(
                    "Static text only; not rendered layout, visibility or legal authority",
                ),
            );
            if let Some(pdf) = &original.pdf {
                ui.label(format!(
                    "{}: {} · {}: {}",
                    lang.translate("Physical PDF pages"),
                    pdf.physical_pages,
                    lang.translate("Pages without extracted text"),
                    pdf.empty_pages
                        .iter()
                        .map(|p| p.to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
                ui.label(lang.translate("PDF blocks preserve physical pages and line wraps; tables, reading order and OCR are unverified"));
                egui::CollapsingHeader::new(lang.translate("Local PDF text provider")).show(
                    ui,
                    |ui| {
                        ui.label(&pdf.tool.executable);
                        ui.label(&pdf.tool.version);
                        ui.monospace(&pdf.tool.sha256);
                        ui.monospace(&pdf.layout_sha256);
                        ui.label(&pdf.recipe);
                    },
                );
            }
            rule_text(ui, lang, "Find original text", &mut self.rules_ui.filter);
            egui::ScrollArea::vertical()
                .id_salt("rule_original_paragraphs")
                .max_height(200.)
                .show(ui, |ui| {
                    let mut shown = 0;
                    for (index, p) in
                        original
                            .document
                            .paragraphs
                            .iter()
                            .enumerate()
                            .filter(|(_, p)| {
                                self.rules_ui.filter.is_empty()
                                    || p.text.contains(&self.rules_ui.filter)
                            })
                    {
                        if shown >= 100 {
                            ui.label(
                                lang.translate("Showing first 100 matches; refine the search"),
                            );
                            break;
                        }
                        shown += 1;
                        if ui
                            .selectable_label(
                                self.rules_ui.paragraph == index,
                                format!(
                                    "{} · {}",
                                    p.locator,
                                    p.text.chars().take(160).collect::<String>()
                                ),
                            )
                            .clicked()
                        {
                            self.rules_ui.paragraph = index;
                        }
                    }
                });
            if let Some(p) = original.document.paragraphs.get(self.rules_ui.paragraph) {
                ui.monospace(&p.locator);
                ui.label(&p.text);
                if matches!(
                    self.rules_ui.register.draft,
                    Some(super::rule_register_ui::RegisterDraft::Lifecycle { .. })
                ) && ui
                    .button(lang.translate("Add block to lifecycle evidence"))
                    .clicked()
                {
                    if let Some(source) = book
                        .source(&self.rules_ui.source_id)
                        .filter(|s| s.original_sha256 == original.source_sha256)
                        && let Some(super::rule_register_ui::RegisterDraft::Lifecycle {
                            value, ..
                        }) = &mut self.rules_ui.register.draft
                    {
                        let c = Citation {
                            source_id: source.id.clone(),
                            locator: p.locator.clone(),
                            text: p.text.clone(),
                        };
                        if c != value.citation && !value.corroboration.contains(&c) {
                            value.corroboration.push(c);
                            self.rules_ui.preview = None;
                            self.rules_ui.request = None;
                        }
                    }
                }
                if ui
                    .button(lang.translate("Record lifecycle from this block"))
                    .clicked()
                {
                    if let Some(source) = book
                        .source(&self.rules_ui.source_id)
                        .filter(|s| s.original_sha256 == original.source_sha256)
                    {
                        self.rules_ui.register.lifecycle(
                            Citation {
                                source_id: source.id.clone(),
                                locator: p.locator.clone(),
                                text: p.text.clone(),
                            },
                            base,
                        );
                        self.rules_ui.request = None;
                        self.rules_ui.preview = None;
                    }
                }
                if ui
                    .button(lang.translate("Use block as condition basis"))
                    .clicked()
                {
                    if let Some(source) = book
                        .source(&self.rules_ui.source_id)
                        .filter(|s| s.original_sha256 == original.source_sha256)
                    {
                        self.rules_ui.register.condition(book, base);
                        if let Some(super::rule_register_ui::RegisterDraft::Condition {
                            value,
                            ..
                        }) = &mut self.rules_ui.register.draft
                        {
                            value.citation = Some(Citation {
                                source_id: source.id.clone(),
                                locator: p.locator.clone(),
                                text: p.text.clone(),
                            });
                        }
                        self.rules_ui.request = None;
                        self.rules_ui.preview = None;
                    }
                }
                if ui.button(lang.translate("Draft cited condition")).clicked() {
                    if let Some(source) = book
                        .source(&self.rules_ui.source_id)
                        .filter(|s| s.original_sha256 == original.source_sha256)
                    {
                        self.rules_ui.requirement_draft = Some((
                            Requirement {
                                id: storage::new_document_identity(),
                                title: String::new(),
                                topic: Topic::Planning,
                                citation: Citation {
                                    source_id: source.id.clone(),
                                    locator: p.locator.clone(),
                                    text: p.text.clone(),
                                },
                                interpretation: String::new(),
                                decision: Decision {
                                    applicability: Applicability::Unknown,
                                    adoption: Adoption::Pending,
                                    rationale: String::new(),
                                    missing: String::new(),
                                    basis_sha256: book.basis(&source.id).unwrap_or_default(),
                                },
                                links: vec![],
                            },
                            None,
                            base,
                        ));
                    } else {
                        self.source_ui.error =
                            "Inspect the original of a recorded rule revision first".into();
                    }
                }
            }
        }
        ui.separator();
        ui.checkbox(
            &mut self.rules_ui.selected_only,
            lang.translate("Conditions linked to selected objects only"),
        );
        for r in &book.requirements {
            if self.rules_ui.selected_only
                && !r.links.iter().any(|l| {
                    matches!(l.target,crate::evidence::Target::Element{
id}
 if self.host.document.editor.selected_ids.contains(&id))
                })
            {
                continue;
            }
            egui::CollapsingHeader::new(format!(
                "{} · {}",
                lang.translate(r.topic.label()),
                r.title
            ))
            .id_salt(("rule_requirement", &r.id))
            .show(ui, |ui| {
                ui.colored_label(
                    Color32::YELLOW,
                    lang.translate(book.requirement_state(&self.host.document.editor.project, r)),
                );
                ui.label(lang.translate(r.decision.adoption.label()));
                ui.label(&r.decision.rationale);
                ui.label(&r.decision.missing);
                ui.label(&r.interpretation);
                ui.monospace(format!("{} / {}", r.citation.source_id, r.citation.locator));
                ui.label(&r.citation.text);
                for link in &r.links {
                    let target = match &link.target {
                        crate::evidence::Target::Element { id } => {
                            format!("{} {}", lang.translate("Element"), id)
                        }
                        crate::evidence::Target::ProgramRow { sheet_id, key } => {
                            format!("{} {} / {}", lang.translate("Program row"), sheet_id, key)
                        }
                    };
                    let status = match link.status(&self.host.document.editor.project) {
                        crate::evidence::LinkStatus::Unchanged => "Unchanged",
                        crate::evidence::LinkStatus::Changed => "Changed",
                        crate::evidence::LinkStatus::Missing => "Missing",
                    };
                    ui.label(format!("{} · {}", target, lang.translate(status)));
                }
                ui.horizontal(|ui| {
                    if ui.button(lang.translate("Locate linked objects")).clicked() {
                        let ids = r
                            .links
                            .iter()
                            .filter_map(|l| match l.target {
                                crate::evidence::Target::Element { id } => Some(id),
                                _ => None,
                            })
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
                        self.host.document.editor.selected_ids = ids;
                    }
                    if ui
                        .button(lang.translate("Locate original clause"))
                        .clicked()
                    {
                        if let Some(s) = book.source(&r.citation.source_id) {
                            self.rules_ui.source_id = s.id.clone();
                            self.rules_ui.filter = r.citation.text.clone();
                            self.rules_ui.wanted_locator = Some(r.citation.locator.clone());
                            self.rules_ui.paragraph = 0;
                            self.rules_ui.original = None;
                            *action =
                                Some(SourceAction::InspectRuleOriginal(s.original_sha256.clone()));
                        }
                    }
                    if ui.button(lang.translate("Edit cited condition")).clicked() {
                        self.rules_ui.requirement_draft = Some((r.clone(), Some(r.clone()), base));
                    }
                    if ui
                        .button(lang.translate("Remove cited condition"))
                        .clicked()
                    {
                        *intent = Some(RulePreview {
                            base,
                            edit: RuleEdit::RemoveRequirement {
                                expected: r.clone(),
                            },
                        });
                    }
                });
            });
        }
        if self.rules_ui.preview.is_none()
            && let Some((s, captured)) = &mut self.rules_ui.source_draft
        {
            ui.separator();
            ui.heading(lang.translate("Record original revision"));
            egui::ComboBox::from_id_salt("rule_source_blob")
                .selected_text(if s.original_sha256.is_empty() {
                    lang.translate("Choose stored original")
                } else {
                    &s.original_sha256
                })
                .show_ui(ui, |ui| {
                    if let Some(repo) = &self.source_ui.repository {
                        for item in &repo.manifest.sources {
                            ui.selectable_value(
                                &mut s.original_sha256,
                                item.id.clone(),
                                item.names.join(", "),
                            );
                        }
                    }
                });
            rule_text(ui, lang, "Document title", &mut s.title);
            rule_text(ui, lang, "Document code", &mut s.code);
            rule_text(ui, lang, "Source edition", &mut s.edition);
            rule_text(ui, lang, "Publisher", &mut s.publisher);
            rule_text(ui, lang, "Jurisdiction", &mut s.jurisdiction);
            rule_text(ui, lang, "Published on (YYYY-MM-DD)", &mut s.published_on);
            rule_text(ui, lang, "Effective on (YYYY-MM-DD)", &mut s.effective_on);
            rule_text(ui, lang, "Original publication URL", &mut s.origin_url);
            egui::ComboBox::from_id_salt("rule_origin")
                .selected_text(lang.translate(s.origin.label()))
                .show_ui(ui, |ui| {
                    for origin in [
                        Origin::Unverified,
                        Origin::OfficialPublication,
                        Origin::ClientDocument,
                        Origin::Survey,
                    ] {
                        ui.selectable_value(&mut s.origin, origin, lang.translate(origin.label()));
                    }
                });
            ui.label(lang.translate("Origin verification notes"));
            ui.text_edit_multiline(&mut s.verification_note);
            for old in book.sources.iter().filter(|old| {
                old.code == s.code && old.jurisdiction == s.jurisdiction && old.edition != s.edition
            }) {
                let mut chosen = s.replaces.contains(&old.id);
                if ui
                    .checkbox(
                        &mut chosen,
                        format!(
                            "{}: {} / {}",
                            lang.translate("Replaces recorded revisions"),
                            old.edition,
                            old.id
                        ),
                    )
                    .changed()
                {
                    if chosen {
                        s.replaces.push(old.id.clone())
                    } else {
                        s.replaces.retain(|id| id != &old.id)
                    }
                }
            }
            for old in book.sources.iter().filter(|old| {
                old.code == s.code
                    && old.jurisdiction == s.jurisdiction
                    && old.edition == s.edition
                    && old.same_edition_as.is_none()
            }) {
                let mut chosen = s.same_edition_as.as_ref() == Some(&old.id);
                if ui
                    .checkbox(
                        &mut chosen,
                        format!(
                            "{}: {}",
                            lang.translate("Same edition, alternate original"),
                            old.id
                        ),
                    )
                    .changed()
                {
                    s.same_edition_as = chosen.then(|| old.id.clone());
                }
            }
            if ui
                .button(lang.translate("Preview original revision"))
                .clicked()
            {
                *intent = Some(RulePreview {
                    base: *captured,
                    edit: RuleEdit::AddSource { source: s.clone() },
                });
            }
        }
        if self.rules_ui.preview.is_none()
            && let Some((r, expected, captured)) = &mut self.rules_ui.requirement_draft
        {
            ui.separator();
            ui.heading(lang.translate("Edit cited condition"));
            rule_text(ui, lang, "Condition title", &mut r.title);
            egui::ComboBox::from_id_salt("rule_topic")
                .selected_text(lang.translate(r.topic.label()))
                .show_ui(ui, |ui| {
                    for topic in Topic::ALL {
                        ui.selectable_value(&mut r.topic, topic, lang.translate(topic.label()));
                    }
                });
            ui.label(lang.translate("Exact original quote"));
            ui.monospace(&r.citation.locator);
            ui.label(&r.citation.text);
            ui.label(lang.translate("Authored interpretation"));
            ui.text_edit_multiline(&mut r.interpretation);
            egui::ComboBox::from_id_salt("rule_applicability")
                .selected_text(lang.translate(r.decision.applicability.label()))
                .show_ui(ui, |ui| {
                    for value in [
                        Applicability::Unknown,
                        Applicability::Applicable,
                        Applicability::NotApplicable,
                        Applicability::Conflict,
                    ] {
                        ui.selectable_value(
                            &mut r.decision.applicability,
                            value,
                            lang.translate(value.label()),
                        );
                    }
                });
            egui::ComboBox::from_id_salt("rule_adoption")
                .selected_text(lang.translate(r.decision.adoption.label()))
                .show_ui(ui, |ui| {
                    for value in [Adoption::Pending, Adoption::Adopted, Adoption::NotAdopted] {
                        ui.selectable_value(
                            &mut r.decision.adoption,
                            value,
                            lang.translate(value.label()),
                        );
                    }
                });
            ui.label(lang.translate("Applicability and adoption rationale"));
            ui.text_edit_multiline(&mut r.decision.rationale);
            ui.label(lang.translate("Missing information and conflicts"));
            ui.text_edit_multiline(&mut r.decision.missing);
            let mut remove_link = None;
            for (index, link) in r.links.iter().enumerate() {
                ui.push_id(("rule_draft_link", index), |ui| {
                    ui.horizontal(|ui| {
                        ui.label(match &link.target {
                            crate::evidence::Target::Element { id } => {
                                format!("{} {}", lang.translate("Element"), id)
                            }
                            crate::evidence::Target::ProgramRow { sheet_id, key } => {
                                format!("{} {} / {}", lang.translate("Program row"), sheet_id, key)
                            }
                        });
                        if ui.button(lang.translate("Remove link")).clicked() {
                            remove_link = Some(index);
                        }
                    })
                });
            }
            if let Some(index) = remove_link {
                r.links.remove(index);
            }
            if ui.button(lang.translate("Link selected objects")).clicked() {
                for id in &self.host.document.editor.selected_ids {
                    let target = crate::evidence::Target::Element { id: *id };
                    if !r.links.iter().any(|l| l.target == target)
                        && let Some(hash) = target.current_hash(&self.host.document.editor.project)
                    {
                        r.links.push(crate::evidence::Link {
                            target,
                            baseline_sha256: hash,
                        });
                    }
                }
            }
            if ui
                .button(lang.translate("Explicitly refresh decision and object baselines"))
                .clicked()
            {
                r.decision.basis_sha256 = book.basis(&r.citation.source_id).unwrap_or_default();
                for link in &mut r.links {
                    if let Some(hash) = link.target.current_hash(&self.host.document.editor.project)
                    {
                        link.baseline_sha256 = hash;
                    }
                }
                *captured = base;
            }
            if ui
                .button(lang.translate("Preview cited condition"))
                .clicked()
            {
                *intent = Some(RulePreview {
                    base: *captured,
                    edit: RuleEdit::PutRequirement {
                        expected: expected.clone(),
                        requirement: r.clone(),
                    },
                });
            }
        }
        if before
            != (
                self.rules_ui.context_draft.clone(),
                self.rules_ui.source_draft.clone(),
                self.rules_ui.requirement_draft.clone(),
            )
        {
            self.rules_ui.request = None;
            self.rules_ui.preview = None;
        }
        if let Some(preview) = self
            .rules_ui
            .preview
            .clone()
            .filter(|p| !super::rule_register_ui::register_edit(&p.edit))
        {
            ui.separator();
            ui.heading(lang.translate("Rule edit preview"));
            ui.label(
                lang.translate("Edit validated; source checks do not prove legal applicability"),
            );
            match &preview.edit {
                RuleEdit::PutLifecycle { .. }
                | RuleEdit::RemoveLifecycle { .. }
                | RuleEdit::PutCondition { .. }
                | RuleEdit::RemoveCondition { .. } => {}
                RuleEdit::SetContext { context, .. } => {
                    ui.label(format!(
                        "{} / {} / {} / {}",
                        context.jurisdiction, context.uses, context.assessment_date, context.stage
                    ));
                }
                RuleEdit::AddSource { source } => {
                    ui.label(format!(
                        "{} · {} · {}",
                        source.code, source.edition, source.title
                    ));
                    ui.monospace(&source.original_sha256);
                    ui.label(&source.origin_url);
                    ui.label(format!(
                        "{} · {} · {}",
                        source.publisher,
                        source.jurisdiction,
                        lang.translate(source.origin.label())
                    ));
                    ui.label(format!(
                        "{}: {} / {}: {}",
                        lang.translate("Published on (YYYY-MM-DD)"),
                        source.published_on,
                        lang.translate("Effective on (YYYY-MM-DD)"),
                        source.effective_on
                    ));
                    ui.label(&source.verification_note);
                    if let Some(id) = &source.same_edition_as {
                        ui.label(format!(
                            "{}: {}",
                            lang.translate("Same edition, alternate original"),
                            id
                        ));
                    }
                    if !source.replaces.is_empty() {
                        ui.label(format!(
                            "{}: {}",
                            lang.translate("Replaces recorded revisions"),
                            source.replaces.join(", ")
                        ));
                    }
                }
                RuleEdit::PutRequirement { requirement: r, .. } => {
                    ui.label(&r.title);
                    ui.label(&r.citation.text);
                    ui.label(&r.interpretation);
                    ui.label(lang.translate(r.decision.applicability.label()));
                    ui.label(lang.translate(r.decision.adoption.label()));
                    ui.monospace(format!("{} / {}", r.citation.source_id, r.citation.locator));
                    ui.label(format!(
                        "{}: {}",
                        lang.translate("Linked targets"),
                        r.links.len()
                    ));
                    ui.label(&r.decision.rationale);
                    ui.label(&r.decision.missing);
                }
                RuleEdit::RemoveRequirement { expected } => {
                    ui.label(format!(
                        "{}: {}",
                        lang.translate("Remove cited condition"),
                        expected.title
                    ));
                }
                RuleEdit::RemoveSource { expected } => {
                    ui.label(format!(
                        "{}: {} / {}",
                        lang.translate("Remove original revision"),
                        expected.code,
                        expected.edition
                    ));
                }
            }
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
                    "Confirm rule edit"
                }))
                .clicked()
            {
                *action = Some(SourceAction::CommitRule(preview.clone()));
            }
        }
        if self.rules_ui.context_draft.is_some()
            || self.rules_ui.source_draft.is_some()
            || self.rules_ui.requirement_draft.is_some()
            || self
                .rules_ui
                .preview
                .as_ref()
                .is_some_and(|p| !super::rule_register_ui::register_edit(&p.edit))
        {
            if ui.button(lang.translate("Cancel rule edit")).clicked() {
                self.rules_ui.context_draft = None;
                self.rules_ui.source_draft = None;
                self.rules_ui.requirement_draft = None;
                self.rules_ui.request = None;
                self.rules_ui.preview = None;
            }
        }
        if self.source_ui.pending_for_ui() {
            ui.spinner();
        }
        if !self.source_ui.error.is_empty() {
            ui.colored_label(Color32::LIGHT_RED, lang.translate(&self.source_ui.error));
        }
    }
}
