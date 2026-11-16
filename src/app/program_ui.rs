use super::project_sources_ui::SourceAction;
use super::*;
use crate::{
    application_host::{CommandContext, HostCommand},
    program::{
        self, Area, AreaBasis, AreaUnit, Mapping, ProgramEdit, ProgramSheet, Values,
        workbook::Workbook,
    },
};
#[derive(Clone)]
pub(super) struct ProgramPreview {
    pub proposal: program::ImportProposal,
    pub base: CommandContext,
}
pub(super) struct RowDraft {
    pub sheet: String,
    pub key: String,
    pub expected: Values,
    pub values: Values,
    pub base: CommandContext,
}
pub(super) struct ProgramUi {
    pub open: bool,
    pub workbook: Option<(String, Workbook)>,
    pub sheet: String,
    pub target: Option<String>,
    pub name: String,
    pub first: u32,
    pub last: u32,
    pub key: u16,
    pub label: u16,
    pub area: u16,
    pub quantity: u16,
    pub scope: u16,
    pub note: u16,
    pub unit: Option<AreaUnit>,
    pub basis: Option<AreaBasis>,
    pub preview: Option<ProgramPreview>,
    pub draft: Option<RowDraft>,
    cell_row: u32,
    cell_column: u16,
    document_paragraph: u32,
    remove: Option<String>,
}
impl Default for ProgramUi {
    fn default() -> Self {
        Self {
            open: false,
            workbook: None,
            sheet: String::new(),
            target: None,
            name: String::new(),
            first: 2,
            last: 2,
            key: 1,
            label: 2,
            area: 3,
            quantity: 0,
            scope: 0,
            note: 0,
            unit: None,
            basis: None,
            preview: None,
            draft: None,
            cell_row: 1,
            cell_column: 1,
            document_paragraph: 1,
            remove: None,
        }
    }
}
impl ProgramUi {
    pub fn accept_workbook(&mut self, source: String, book: Workbook) {
        if !book.sheets.iter().any(|s| s.name == self.sheet) {
            self.sheet = book
                .sheets
                .first()
                .map_or(String::new(), |s| s.name.clone());
        }
        if let Some(s) = book.sheets.iter().find(|s| s.name == self.sheet) {
            self.last = s.last_row.max(self.first);
        }
        if self.name.is_empty() {
            self.name = self.sheet.clone();
        }
        self.workbook = Some((source, book));
        self.preview = None;
    }
    pub fn load_mapping(&mut self, s: &ProgramSheet) {
        self.target = Some(s.id.clone());
        self.name = s.name.clone();
        let m = &s.mapping;
        self.sheet = m.sheet.clone();
        self.first = m.first_row;
        self.last = m.last_row;
        self.key = m.key_column;
        self.label = m.label_column;
        self.area = m.area_column.unwrap_or(0);
        self.quantity = m.quantity_column.unwrap_or(0);
        self.scope = m.scope_column.unwrap_or(0);
        self.note = m.note_column.unwrap_or(0);
        self.unit = Some(m.area_unit);
        self.basis = Some(m.area_basis);
        self.preview = None;
    }
    pub fn mapping_changed(&self, program: &program::Program) -> bool {
        let Some((source, _)) = &self.workbook else {
            return false;
        };
        let (Some(unit), Some(basis)) = (self.unit, self.basis) else {
            return false;
        };
        let Some(sheet) = self
            .target
            .as_ref()
            .and_then(|id| program.sheets.iter().find(|s| &s.id == id))
        else {
            return true;
        };
        let m = &sheet.mapping;
        sheet.name != self.name
            || sheet.source_sha256 != *source
            || m.sheet != self.sheet
            || m.first_row != self.first
            || m.last_row != self.last
            || m.key_column != self.key
            || m.label_column != self.label
            || m.area_column.unwrap_or(0) != self.area
            || m.quantity_column.unwrap_or(0) != self.quantity
            || m.scope_column.unwrap_or(0) != self.scope
            || m.note_column.unwrap_or(0) != self.note
            || m.area_unit != unit
            || m.area_basis != basis
    }
    pub fn mapping_request(
        &self,
        program: &program::Program,
    ) -> Result<(Mapping, String, String, Option<ProgramSheet>), String> {
        let mapping = Mapping {
            sheet: self.sheet.clone(),
            first_row: self.first,
            last_row: self.last,
            key_column: self.key,
            label_column: self.label,
            area_column: (self.area > 0).then_some(self.area),
            quantity_column: (self.quantity > 0).then_some(self.quantity),
            scope_column: (self.scope > 0).then_some(self.scope),
            note_column: (self.note > 0).then_some(self.note),
            area_unit: self.unit.ok_or("Choose the program area unit explicitly")?,
            area_basis: self
                .basis
                .ok_or("Choose total or per-unit area explicitly")?,
        };
        mapping.validate()?;
        let expected = self
            .target
            .as_ref()
            .map(|id| {
                program
                    .sheets
                    .iter()
                    .find(|s| &s.id == id)
                    .cloned()
                    .ok_or("Program sheet no longer exists")
            })
            .transpose()?;
        Ok((
            mapping,
            self.target
                .clone()
                .unwrap_or_else(storage::new_document_identity),
            self.name.clone(),
            expected,
        ))
    }
}
fn column(ui: &mut Ui, language: UiLanguage, label: &str, value: &mut u16, optional: bool) {
    ui.horizontal(|ui| {
        ui.label(language.translate(label));
        ui.add(egui::DragValue::new(value).range(if optional { 0..=16384 } else { 1..=16384 }));
        if *value > 0 {
            ui.monospace(program::workbook::address(1, *value).trim_end_matches('1'));
        } else {
            ui.label(language.translate("Not mapped"));
        }
    });
}
impl AtelierApp {
    pub(super) fn apply_program_edit(&mut self, base: CommandContext, edit: ProgramEdit) {
        if self.menu_gated() || self.workbench_pointer_active() {
            self.source_ui.error =
                "Finish the active interaction before importing requirements".into();
            return;
        }
        match self.dispatch_project_preparation(
            base,
            HostCommand::Apply {
                request_id: storage::new_document_identity(),
                commands: vec![crate::command::ModelCommand::Program { edit }],
            },
        ) {
            Ok(_) => {
                self.source_ui.error.clear();
                self.program_ui.draft = None;
            }
            Err(e) => self.source_ui.error = e,
        }
    }
    pub(super) fn program_content(
        &mut self,
        ui: &mut Ui,
        language: UiLanguage,
        bound: bool,
        action: &mut Option<SourceAction>,
        edit: &mut Option<(CommandContext, ProgramEdit)>,
    ) {
        ui.label(language.translate("Functional requirements are separate from graphical drawing worksheets. Source declarations are unapproved."));

        if ui.button(language.translate("Project sources")).clicked() {
            self.project_conditions_ui.navigation = Some(ConditionsGroup::Sources);
        }

        if !bound {
            ui.label(language.translate("Connect sources for the current project to use source-backed conditions. You can continue designing without them."));
        }

        if let Some(repo) = &self.source_ui.repository {
            ui.horizontal_wrapped(|ui| {
                for source in &repo.manifest.sources {
                    if matches!(source.format.as_str(), "csv" | "xlsx" | "docx")
                        && ui
                            .add_enabled(
                                bound,
                                egui::Button::new(format!(
                                    "{}: {}",
                                    language.translate("Inspect functional worksheet"),
                                    source.names.join(", ")
                                )),
                            )
                            .clicked()
                    {
                        *action = Some(SourceAction::InspectProgram(source.id.clone()));
                    }
                }
            });
        }

        if let Some(preview) = &self.program_ui.preview {
            ui.separator();
            ui.strong(format!(
                "{} · {}",
                language.translate("Requirement preview"),
                preview.proposal.sheet.name
            ));
            ui.monospace(format!(
                "{}: {}..{}",
                preview.proposal.sheet.mapping.sheet,
                preview.proposal.sheet.mapping.first_row,
                preview.proposal.sheet.mapping.last_row
            ));
            ui.label(format!(
                "{}: {} · {}: {}",
                language.translate("Requirement rows"),
                preview.proposal.sheet.rows.len(),
                language.translate("Blank rows accounted for"),
                preview.proposal.sheet.blank_rows.len()
            ));
            let p = program::Program {
                sheets: vec![preview.proposal.sheet.clone()],
            };
            let summary = p.summary();
            ui.label(format!(
                "{}: {}",
                language.translate("Unknown area rows"),
                summary["unknown_area_rows"]
            ));
            ui.label(format!(
                "{}: {}",
                language.translate("Known area by source scope (m²)"),
                summary["known_area_by_scope_m2"]
            ));

            egui::ScrollArea::vertical()
                .id_salt("program_preview_rows")
                .max_height(160.)
                .show(ui, |ui| {
                    for r in preview.proposal.sheet.rows.iter().take(100) {
                        ui.label(format!(
                            "{} · {} · {} · {}",
                            r.key,
                            r.values.label,
                            r.values.scope,
                            match &r.values.area {
                                Area::Known { m2 } => format!("{m2:.3} m²"),
                                Area::Unknown { reason } => format!(
                                    "{}: {}",
                                    language.translate("Unknown"),
                                    language.translate(reason)
                                ),
                            }
                        ));
                    }
                });

            if preview.proposal.sheet.rows.len() > 100 {
                ui.label(
                    language.translate(
                        "Preview shows the first 100 rows; all mapped rows are retained",
                    ),
                );
            }

            if ui
                .button(language.translate(if self.project_conditions_ui.open {
                    "Stage condition change"
                } else {
                    "Import requirement preview"
                }))
                .clicked()
            {
                *action = Some(SourceAction::CommitProgram(preview.clone()));
            }
            if ui
                .button(language.translate("Discard requirement preview"))
                .clicked()
            {
                self.program_ui.preview = None;
            }
        }

        if let Some((source, book)) = &self.program_ui.workbook {
            if let Some(document) = &book.document {
                ui.label(language.translate("DOCX body XML text only; headers, footers, comments, numbering and page layout are not interpreted"));
                egui::CollapsingHeader::new(language.translate("Taskbook source paragraphs")).show(
                    ui,
                    |ui| {
                        ui.horizontal(|ui| {
                            ui.label(language.translate("Source paragraph"));
                            ui.add(
                                egui::DragValue::new(&mut self.program_ui.document_paragraph)
                                    .range(1..=document.paragraphs.len().max(1) as u32),
                            );
                        });
                        if let Some(p) = document
                            .paragraphs
                            .get(self.program_ui.document_paragraph.saturating_sub(1) as usize)
                        {
                            ui.monospace(format!("{source} / {}", p.locator));
                            ui.label(&p.text);
                            if !p.text.trim().is_empty()
                                && ui
                                    .button(language.translate("Create source note from paragraph"))
                                    .clicked()
                            {
                                self.evidence_ui.from_quote(
                                    crate::evidence::Quote {
                                        source_sha256: source.clone(),
                                        locator: p.locator.clone(),
                                        text: p.text.clone(),
                                    },
                                    self.host.context(),
                                );
                            }
                        }
                    },
                );
            } else {
                ui.label(language.translate(
                    "CSV rows are logical records; quoted line breaks stay within a cell",
                ));
            }

            let source = source.clone();
            let names = book
                .sheets
                .iter()
                .map(|s| s.name.clone())
                .collect::<Vec<_>>();

            ui.separator();
            ui.horizontal(|ui| {
                ui.label(language.translate("Source sheet"));
                egui::ComboBox::from_id_salt("program_source_sheet")
                    .selected_text(&self.program_ui.sheet)
                    .show_ui(ui, |ui| {
                        for name in names {
                            if ui
                                .selectable_value(&mut self.program_ui.sheet, name.clone(), name)
                                .changed()
                            {
                                self.program_ui.preview = None;
                            }
                        }
                    });
            });

            egui::CollapsingHeader::new(language.translate("Source cells"))
                .default_open(false)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(language.translate("Source row"));
                        ui.add(
                            egui::DragValue::new(&mut self.program_ui.cell_row)
                                .range(1..=1_048_576),
                        );
                        ui.label(language.translate("Source column"));
                        ui.add(
                            egui::DragValue::new(&mut self.program_ui.cell_column).range(1..=16384),
                        );
                    });

                    if let Some(s) = book.sheets.iter().find(|s| s.name == self.program_ui.sheet) {
                        let row = self.program_ui.cell_row;
                        let col = self.program_ui.cell_column;
                        ui.monospace(format!(
                            "{}!{}",
                            s.name,
                            program::workbook::address(row, col)
                        ));
                        if let Some(document) = &book.document {
                            if let Some(index) =
                                book.sheets.iter().position(|sheet| sheet.name == s.name)
                            {
                                if let Some(table) = document.tables.get(index) {
                                    ui.monospace(format!(
                                        "{}/row[{row}]/cell[{col}]",
                                        table.locator
                                    ));
                                    let locator =
                                        format!("{}/row[{row}]/cell[{col}]", table.locator);
                                    if let Some(text) =
                                        document.quote(&locator).filter(|t| !t.trim().is_empty())
                                    {
                                        if ui
                                            .button(
                                                language.translate("Create source note from cell"),
                                            )
                                            .clicked()
                                        {
                                            self.evidence_ui.from_quote(
                                                crate::evidence::Quote {
                                                    source_sha256: source.clone(),
                                                    locator,
                                                    text: text.to_owned(),
                                                },
                                                self.host.context(),
                                            );
                                        }
                                    }
                                }
                            }
                        }
                        if let Some(c) = s.cell(row, col) {
                            ui.label(&c.value);
                            if c.formula {
                                ui.colored_label(
                                    Color32::YELLOW,
                                    language.translate(
                                        "Formula cache only; not recalculated or admitted as area",
                                    ),
                                );
                            }
                        } else {
                            ui.label(language.translate("Empty source cell"));
                        }

                        egui::ScrollArea::both()
                            .id_salt("program_source_cells")
                            .max_height(130.)
                            .show(ui, |ui| {
                                egui::Grid::new("program_cell_grid")
                                    .striped(true)
                                    .show(ui, |ui| {
                                        for r in row..=(row.saturating_add(7).min(s.last_row)) {
                                            ui.monospace(r.to_string());
                                            for c in col..=col.saturating_add(7).min(16384) {
                                                let text = s
                                                    .cell(r, c)
                                                    .map_or("", |cell| cell.value.as_str());
                                                let display =
                                                    text.chars().take(45).collect::<String>();
                                                if ui
                                                    .small_button(display)
                                                    .on_hover_text(format!(
                                                        "{}!{}\n{}",
                                                        s.name,
                                                        program::workbook::address(r, c),
                                                        text
                                                    ))
                                                    .clicked()
                                                {
                                                    self.program_ui.cell_row = r;
                                                    self.program_ui.cell_column = c;
                                                }
                                            }
                                            ui.end_row();
                                        }
                                    });
                            });
                    }
                });

            let old = self.host.document.editor.project.program.sheets.clone();

            ui.horizontal(|ui| {
                ui.label(language.translate("Import target"));
                egui::ComboBox::from_id_salt("program_target")
                    .selected_text(
                        self.program_ui
                            .target
                            .as_ref()
                            .and_then(|id| old.iter().find(|s| &s.id == id))
                            .map_or(language.translate("New program sheet"), |s| s.name.as_str()),
                    )
                    .show_ui(ui, |ui| {
                        if ui
                            .selectable_value(
                                &mut self.program_ui.target,
                                None,
                                language.translate("New program sheet"),
                            )
                            .changed()
                        {
                            self.program_ui.preview = None;
                        }
                        for s in &old {
                            if ui
                                .selectable_label(
                                    self.program_ui.target.as_ref() == Some(&s.id),
                                    &s.name,
                                )
                                .clicked()
                            {
                                self.program_ui.load_mapping(s);
                            }
                        }
                    });
            });

            ui.horizontal(|ui| {
                ui.label(language.translate("Program sheet name"));
                ui.text_edit_singleline(&mut self.program_ui.name);
            });

            ui.horizontal(|ui| {
                ui.label(language.translate("First source row"));
                ui.add(egui::DragValue::new(&mut self.program_ui.first).range(1..=1_048_576));
                ui.label(language.translate("Last source row"));
                ui.add(egui::DragValue::new(&mut self.program_ui.last).range(1..=1_048_576));
            });

            egui::Grid::new("program_mapping_columns")
                .num_columns(2)
                .show(ui, |ui| {
                    column(
                        ui,
                        language,
                        "Requirement key column",
                        &mut self.program_ui.key,
                        false,
                    );
                    column(
                        ui,
                        language,
                        "Requirement name column",
                        &mut self.program_ui.label,
                        false,
                    );
                    ui.end_row();
                    column(ui, language, "Area column", &mut self.program_ui.area, true);
                    column(
                        ui,
                        language,
                        "Quantity column",
                        &mut self.program_ui.quantity,
                        true,
                    );
                    ui.end_row();
                    column(
                        ui,
                        language,
                        "Area scope column",
                        &mut self.program_ui.scope,
                        true,
                    );
                    column(ui, language, "Note column", &mut self.program_ui.note, true);
                    ui.end_row();
                });

            if self.program_ui.quantity == 0 {
                ui.label(language.translate("No quantity column: explicitly use one unit per row"));
            }

            ui.horizontal(|ui| {
                egui::ComboBox::from_id_salt("program_area_unit")
                    .selected_text(match self.program_ui.unit {
                        None => language.translate("Choose program area unit"),
                        Some(AreaUnit::SquareMetre) => "m²",
                        Some(AreaUnit::SquareFoot) => "ft²",
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut self.program_ui.unit,
                            Some(AreaUnit::SquareMetre),
                            "m²",
                        );
                        ui.selectable_value(
                            &mut self.program_ui.unit,
                            Some(AreaUnit::SquareFoot),
                            "ft²",
                        );
                    });
                egui::ComboBox::from_id_salt("program_area_basis")
                    .selected_text(language.translate(match self.program_ui.basis {
                        None => "Choose program area basis",
                        Some(AreaBasis::RowTotal) => "Area is row total",
                        Some(AreaBasis::PerUnit) => "Area is per unit",
                    }))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut self.program_ui.basis,
                            Some(AreaBasis::RowTotal),
                            language.translate("Area is row total"),
                        );
                        ui.selectable_value(
                            &mut self.program_ui.basis,
                            Some(AreaBasis::PerUnit),
                            language.translate("Area is per unit"),
                        );
                    });
            });

            if ui
                .add_enabled(
                    bound,
                    egui::Button::new(language.translate("Preview requirements")),
                )
                .clicked()
            {
                *action = Some(SourceAction::PreviewProgram(source));
            }
        }

        ui.separator();
        if let Some(draft) = &mut self.program_ui.draft {
            ui.separator();
            ui.strong(format!(
                "{} · {}",
                language.translate("Edit requirement"),
                draft.key
            ));
            ui.horizontal(|ui| {
                ui.label(language.translate("Name"));
                ui.text_edit_singleline(&mut draft.values.label);
            });
            ui.horizontal(|ui| {
                ui.label(language.translate("Area scope"));
                ui.text_edit_singleline(&mut draft.values.scope);
            });
            ui.horizontal(|ui| {
                ui.label(language.translate("Quantity"));
                ui.add(egui::DragValue::new(&mut draft.values.quantity).range(1..=1_000_000));
            });
            let mut known = matches!(draft.values.area, Area::Known { .. });
            if ui
                .checkbox(&mut known, language.translate("Area is known (m²)"))
                .changed()
            {
                draft.values.area = if known {
                    Area::Known { m2: 0. }
                } else {
                    Area::Unknown {
                        reason: "User marked area unknown".into(),
                    }
                };
            }
            match &mut draft.values.area {
                Area::Known { m2 } => {
                    ui.add(egui::DragValue::new(m2).range(0.0..=1e9));
                }
                Area::Unknown { reason } => {
                    ui.text_edit_singleline(reason);
                }
            }
            ui.label(language.translate("Requirement note"));
            ui.text_edit_multiline(&mut draft.values.note);

            if ui
                .button(language.translate(if self.project_conditions_ui.open {
                    "Stage condition change"
                } else {
                    "Apply requirement edit"
                }))
                .clicked()
            {
                *edit = Some((
                    draft.base,
                    ProgramEdit::UpdateRow {
                        sheet_id: draft.sheet.clone(),
                        key: draft.key.clone(),
                        expected: draft.expected.clone(),
                        values: draft.values.clone(),
                    },
                ));
            }
            if ui
                .button(language.translate("Cancel requirement edit"))
                .clicked()
            {
                self.program_ui.draft = None;
            }
        }

        let sheets = self.host.document.editor.project.program.sheets.clone();
        for s in sheets {
            ui.collapsing(format!("{} ({})", s.name, s.rows.len()), |ui| {
                ui.label(format!(
                    "{}: {}",
                    language.translate("Source sheet"),
                    s.mapping.sheet
                ));
                if s.user_modified() {
                    ui.label(
                        language.translate("Contains user edits; automatic reimport is refused"),
                    );
                }

                egui::ScrollArea::vertical()
                    .id_salt(("program_rows", &s.id))
                    .max_height(220.)
                    .show(ui, |ui| {
                        for r in &s.rows {
                            ui.horizontal(|ui| {
                                ui.label(format!(
                                    "{} · {} · {}",
                                    r.key, r.values.label, r.values.scope
                                ));
                                ui.label(match &r.values.area {
                                    Area::Known { m2 } => format!("{m2:.3} m²"),
                                    Area::Unknown { .. } => language.translate("Unknown").into(),
                                });
                                if ui
                                    .small_button(language.translate("Discuss requirement"))
                                    .clicked()
                                {
                                    self.begin_object_research(
                                        vec![crate::evidence::Target::ProgramRow {
                                            sheet_id: s.id.clone(),
                                            key: r.key.clone(),
                                        }],
                                        None,
                                    );
                                }
                                if ui
                                    .small_button(language.translate("Edit requirement"))
                                    .clicked()
                                {
                                    self.program_ui.draft = Some(RowDraft {
                                        sheet: s.id.clone(),
                                        key: r.key.clone(),
                                        expected: r.values.clone(),
                                        values: r.values.clone(),
                                        base: self.host.context(),
                                    });
                                }
                                if r.values != r.imported
                                    && ui
                                        .small_button(language.translate("Restore imported values"))
                                        .clicked()
                                {
                                    *edit = Some((
                                        self.host.context(),
                                        ProgramEdit::UpdateRow {
                                            sheet_id: s.id.clone(),
                                            key: r.key.clone(),
                                            expected: r.values.clone(),
                                            values: r.imported.clone(),
                                        },
                                    ));
                                }
                                if ui
                                    .small_button(format!(
                                        "{}!{}",
                                        s.mapping.sheet,
                                        program::workbook::address(
                                            r.source_row,
                                            s.mapping.key_column
                                        )
                                    ))
                                    .on_hover_text(
                                        language.translate("Locate original source cell"),
                                    )
                                    .clicked()
                                {
                                    self.program_ui.cell_row = r.source_row;
                                    self.program_ui.cell_column = s.mapping.key_column;
                                    self.program_ui.sheet = s.mapping.sheet.clone();
                                    *action =
                                        Some(SourceAction::InspectProgram(s.source_sha256.clone()));
                                }
                            });
                        }
                    });

                if ui
                    .button(language.translate("Remove program sheet…"))
                    .clicked()
                {
                    self.program_ui.remove = Some(s.id.clone());
                }
                if self.program_ui.remove.as_ref() == Some(&s.id) {
                    ui.label(language.translate(
                        "Remove these requirements? Original source files are retained.",
                    ));
                    if ui
                        .button(language.translate(if self.project_conditions_ui.open {
                            "Stage condition change"
                        } else {
                            "Confirm remove requirements"
                        }))
                        .clicked()
                    {
                        *edit = Some((
                            self.host.context(),
                            ProgramEdit::RemoveSheet {
                                expected: s.clone(),
                            },
                        ));
                        self.program_ui.remove = None;
                    }
                    if ui.button(language.translate("Cancel")).clicked() {
                        self.program_ui.remove = None;
                    }
                }
            });
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
    }
    pub(super) fn program_window(&mut self, ctx: &Context) {
        if !self.program_ui.open || self.project_conditions_ui.open {
            return;
        }
        let language = self.language;
        let mut open = true;
        let mut action = None;
        let mut edit = None;
        let bound = self.source_ui.repository.as_ref().is_some_and(|r| {
            r.manifest.document_id == self.host.document.editor.project_identity()
        });
        egui::Window::new(language.translate("Functional program"))
            .id(egui::Id::new("functional_program"))
            .open(&mut open)
            .default_width(820.)
            .default_height(850.)
            .vscroll(true)
            .show(ctx, |ui| {
                self.program_content(ui, language, bound, &mut action, &mut edit)
            });

        self.program_ui.open = open;

        if let Some((base, e)) = edit {
            self.apply_program_edit(base, e);
        }

        if let Some(a) = action {
            self.start_source_action(a);
        }
    }
}
