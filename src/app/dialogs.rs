use super::*;

impl AtelierApp {
    pub(super) fn element_settings_dialog(&mut self, ctx: &Context) {
        let Some(mut element) = self.element_settings.clone() else {
            return;
        };
        let stories = self.editor.project.stories.clone();
        let mut apply = false;
        let mut accept = false;
        let mut cancel = false;
        let modal =
            Modal::new(Id::new("element_settings_dialog")).show(ctx, |ui| {
                ui.set_min_width(470.);
                ui.heading(format!(
                    "{} {}  ·  {:03}",
                    self.language.translate(element.kind()),
                    self.language.translate("Settings"),
                    element.id
                ));
                ui.label(
                    RichText::new(self.language.translate(
                        "Changes preview after applying and synchronize across all views.",
                    ))
                    .color(MUTED),
                );
                ui.separator();
                Self::element_fields(ui, &mut element, &stories, "dialog", self.language);
                if !self.element_settings_error.is_empty() {
                    ui.colored_label(
                        Color32::from_rgb(224, 125, 105),
                        &self.element_settings_error,
                    );
                }
                ui.add_space(12.);
                ui.separator();
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui
                        .add_sized(
                            [94., 32.],
                            Button::new(Self::t(self.language, "OK")).fill(ACCENT),
                        )
                        .clicked()
                    {
                        accept = true;
                    }
                    if ui.button(Self::t(self.language, "Apply")).clicked() {
                        apply = true;
                    }
                    if ui.button(Self::t(self.language, "Cancel")).clicked() {
                        cancel = true;
                    }
                });
            });

        self.element_settings = Some(element);
        if apply || accept {
            match self.apply_element_settings() {
                Ok(()) if accept => self.element_settings = None,
                Ok(()) => {}
                Err(error) => self.element_settings_error = error,
            }
        }
        if cancel || modal.should_close() {
            self.element_settings = None;
            self.element_settings_error.clear();
        }
    }
    pub(super) fn wall_settings_dialog(&mut self, ctx: &Context) {
        let Some(mut dialog) = self.wall_settings_dialog.take() else {
            return;
        };
        let stories = self.editor.project.stories.clone();
        let action = dialog.show(ctx, self.language, &stories);
        if action == WallDialogAction::None {
            self.wall_settings_dialog = Some(dialog);
            return;
        }
        if action == WallDialogAction::Cancel {
            self.editor.status = self.language.translate("Wall Settings cancelled").into();
            return;
        }
        let result = match dialog.mode {
            crate::wall_ui::WallSettingsMode::Selected(id) => {
                let Some(mut element) = self
                    .editor
                    .project
                    .elements
                    .iter()
                    .find(|element| element.id == id)
                    .cloned()
                else {
                    self.editor.status = self
                        .language
                        .translate("Wall Settings · The selected wall no longer exists")
                        .into();
                    return;
                };
                dialog
                    .draft
                    .apply_to_element(&mut element)
                    .and_then(|()| self.editor.replace_element(element))
            }
            crate::wall_ui::WallSettingsMode::Default => dialog
                .draft
                .validate()
                .map(|()| self.apply_wall_default_draft(&dialog.draft)),
        };
        match result {
            Ok(()) => {
                match dialog.mode {
                    crate::wall_ui::WallSettingsMode::Selected(_) => {
                        self.editor.status =
                            "Wall settings applied · Plan and 3D synchronized".into()
                    }
                    crate::wall_ui::WallSettingsMode::Default => {
                        self.editor.status =
                            "Wall default settings applied · New walls use these values".into()
                    }
                }
                dialog.error.clear();
                if action != WallDialogAction::Ok {
                    self.wall_settings_dialog = Some(dialog);
                }
            }
            Err(error) => {
                dialog.error = error;
                self.wall_settings_dialog = Some(dialog);
            }
        }
    }
    pub(super) fn stair_settings_dialog(&mut self, ctx: &Context) {
        if !self.stair_settings_open {
            return;
        }
        let mut apply = false;
        let mut close = false;
        let mut cancel = false;
        let modal = Modal::new(Id::new("stair_settings_dialog")).show(ctx, |ui| {
            ui.set_min_width(430.);
            ui.heading(self.language.translate("Stair Settings"));
            ui.label(
                RichText::new(
                    self.language.translate(
                        "Straight flight · click two plan points after applying settings.",
                    ),
                )
                .color(MUTED),
            );
            ui.separator();
            Grid::new("stair_settings_grid")
                .num_columns(2)
                .spacing([18., 11.])
                .show(ui, |ui| {
                    Self::number(
                        ui,
                        self.language,
                        "Width",
                        &mut self.stair_settings_draft.width,
                    );
                    Self::number(
                        ui,
                        self.language,
                        "Total rise",
                        &mut self.stair_settings_draft.total_rise,
                    );
                    ui.label(self.language.translate("Risers"));
                    ui.add(DragValue::new(&mut self.stair_settings_draft.risers).range(2..=100));
                    ui.end_row();
                    Self::number(
                        ui,
                        self.language,
                        "Slab thickness",
                        &mut self.stair_settings_draft.slab_thickness,
                    );
                    Self::number(
                        ui,
                        self.language,
                        "Handrail height",
                        &mut self.stair_settings_draft.handrail_height,
                    );
                });
            ui.label(format!(
                "Rise per step: {:.0} mm",
                self.stair_settings_draft.total_rise * 1000.
                    / f32::from(self.stair_settings_draft.risers.max(1))
            ));
            if !self.stair_settings_error.is_empty() {
                ui.colored_label(Color32::from_rgb(224, 125, 105), &self.stair_settings_error);
            }
            ui.add_space(10.);
            ui.separator();
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui
                    .add_sized(
                        [112., 32.],
                        Button::new(self.language.translate("Start Drawing")).fill(ACCENT),
                    )
                    .clicked()
                {
                    apply = true;
                    close = true;
                }
                if ui.button(self.language.translate("Apply")).clicked() {
                    apply = true;
                }
                if ui.button(self.language.translate("Cancel")).clicked() {
                    cancel = true;
                }
            });
        });
        if apply {
            match self.stair_settings_draft.validate() {
                Ok(()) => {
                    self.editor.stair_spec = self.stair_settings_draft.clone();
                    self.stair_settings_error.clear();
                    self.editor.status =
                        "Stair settings applied · click start and end points in plan".into();
                }
                Err(error) => self.stair_settings_error = error.into(),
            }
        }
        let dismissed = cancel || modal.should_close();
        if (close && self.stair_settings_error.is_empty()) || dismissed {
            self.stair_settings_open = false;
            if dismissed {
                self.editor.set_tool(Tool::Select);
            }
        }
    }
    pub(super) fn command_preview_dialog(&mut self, ctx: &Context) {
        let Some(title) = self.command_dialog.clone() else {
            return;
        };
        let mut close = false;
        let mut applied = false;
        let modal = Modal::new(Id::new("command_preview_dialog")).show(ctx, |ui| {
            ui.set_min_width(610.);
            ui.set_min_height(430.);
            ui.heading(&title);
            ui.label(RichText::new(self.language.translate("Configuration interface preview")).small().color(MUTED));
            ui.separator();
            ui.horizontal(|ui| {
                for (index, tab) in [(0, "General"), (1, "Geometry"), (2, "Representation"), (3, "Classification") ] {
                    ui.selectable_value(&mut self.command_dialog_tab, index, tab);
                }
            });
            ui.separator();
            ScrollArea::vertical().max_height(290.).show(ui, |ui| {
                match self.command_dialog_tab {
                    0 => {
                        Grid::new("command_preview_general").num_columns(2).spacing([14., 9.]).show(ui, |ui| {
                            ui.label(self.language.translate("Name")); ui.add(TextEdit::singleline(&mut self.command_dialog_name).desired_width(360.)); ui.end_row();
                            ui.label(self.language.translate("Element ID")); ui.add(TextEdit::singleline(&mut self.preview_element_id).desired_width(180.)); ui.end_row();
                            ui.label(self.language.translate("Home Story")); ComboBox::from_id_salt("command_preview_story").selected_text(self.language.translate("Current Story")).show_ui(ui, |ui| { for story in self.editor.project.stories.clone() { let _ = ui.selectable_label(story.id == self.editor.story, format!("{} · {}", story.id, story.name)); } }); ui.end_row();
                            ui.label(self.language.translate("Layer")); ComboBox::from_id_salt("command_preview_layer").selected_text(&self.preview_layer).show_ui(ui, |ui| { for layer in ["01 Site", "02 Drafting", "03 Plans - Preliminary", "04 Plans - Approval", "05 Plans - Detailed", "06 Plans - Mechanical", "07 Plans - Structural", "08 Rendering", "09 Layouting"] { ui.selectable_value(&mut self.preview_layer, layer.to_owned(), layer); } }); ui.end_row();
                            ui.label(self.language.translate("State")); ui.checkbox(&mut self.command_dialog_enabled, self.language.translate("Visible and enabled")); ui.end_row();
                        });
                    }
                    1 => {
                        Grid::new("command_preview_geometry").num_columns(2).spacing([14., 9.]).show(ui, |ui| {
                            ui.label(self.language.translate("Length / Width")); ui.add(DragValue::new(&mut self.command_dialog_number).speed(0.05).fixed_decimals(3).suffix(" m")); ui.end_row();
                            ui.label(self.language.translate("Height / Offset")); ui.add(DragValue::new(&mut self.preview_offset).speed(0.05).fixed_decimals(3).suffix(" m")); ui.end_row();
                            ui.label(self.language.translate("Reference")); ComboBox::from_id_salt("command_preview_reference").selected_text(&self.command_dialog_reference).show_ui(ui, |ui| { for value in ["Center", "Inside", "Outside"] { ui.selectable_value(&mut self.command_dialog_reference, value.into(), value); } }); ui.end_row();
                            ui.label(self.language.translate("Method")); ComboBox::from_id_salt("command_preview_method").selected_text(&self.command_dialog_method).show_ui(ui, |ui| { for value in ["Default", "Single", "Continuous", "Polygonal", "Rectangular"] { ui.selectable_value(&mut self.command_dialog_method, value.into(), value); } }); ui.end_row();
                        });
                    }
                    2 => {
                        for value in ["Model View Options", "Pen Set", "Surface Override", "Floor Plan Display", "3D Representation", "Cut Surface", "Line Type"] {
                            ui.horizontal(|ui| { ui.label(value); ComboBox::from_id_salt(value).selected_text(&self.command_dialog_representation).show_ui(ui, |ui| { for option in ["By Element", "By Building Material", "Override", "Hidden", "Visible"] { ui.selectable_value(&mut self.command_dialog_representation, option.into(), option); } }); });
                        }
                    }
                    _ => {
                        for value in ["Classification", "Property Set", "Building Material", "Zone Category", "Renovation Status", "Structural Function"] {
                            ui.horizontal(|ui| { ui.label(value); ComboBox::from_id_salt(value).selected_text(&self.command_dialog_classification).show_ui(ui, |ui| { for option in ["Not assigned", "Architecture", "Structure", "Existing", "New"] { ui.selectable_value(&mut self.command_dialog_classification, option.into(), option); } }); });
                        }
                    }
                }
                ui.separator();
                ui.label(RichText::new(self.language.translate("Preview values are local UI state. This command has no connected model backend.")).small().color(MUTED));
            });
            ui.separator();
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.add_sized([90., 30.], Button::new(self.language.translate("OK")).fill(ACCENT)).clicked() { close = true; applied = true; }
                if ui.button(self.language.translate("Apply")).clicked() { applied = true; }
                if ui.button(self.language.translate("Cancel")).clicked() { close = true; }
            });
        });
        if applied {
            self.editor.status = format!("{} · preview values applied locally", title);
        }
        if close || modal.should_close() {
            self.command_dialog = None;
        }
    }
    pub(super) fn plan_options_dialog(&mut self, ctx: &Context) {
        let Some(kind) = self.plan_option_dialog else {
            return;
        };
        if kind == PlanOptionKind::DesignOptions {
            self.design_options_dialog(ctx);
            return;
        }
        let language = self.language;
        let mut options = self.plan_dialog_options.clone();
        let mut apply = false;
        let mut cancel = false;
        let modal = Modal::new(Id::new(("plan_options_dialog", kind as u8))).show(ctx, |ui| {
            ui.set_min_width(430.);
            ui.heading(Self::plan_option_title(language, kind));
            ui.separator();
            match kind {
                PlanOptionKind::Scale => {
                    ui.horizontal(|ui| {
                        ui.label("1 :");
                        ui.add(
                            DragValue::new(&mut options.drawing_scale)
                                .range(1..=10_000)
                                .speed(1.0),
                        );
                    });
                    ui.label(
                        RichText::new(
                            language
                                .translate("Plan scale controls pen widths and annotation size."),
                        )
                        .small()
                        .color(MUTED),
                    );
                    ui.add_space(6.);
                    ui.horizontal_wrapped(|ui| {
                        for preset in SCALE_PRESETS {
                            if ui
                                .selectable_label(
                                    options.drawing_scale == preset,
                                    format!("1:{preset}"),
                                )
                                .clicked()
                            {
                                options.drawing_scale = preset;
                            }
                        }
                    });
                }
                PlanOptionKind::Layers => {
                    ui.checkbox(
                        &mut options.show_all_layers,
                        language.translate("Show All Layers"),
                    );
                    ui.add_space(4.);
                    for combination in LayerCombination::ALL {
                        ui.radio_value(
                            &mut options.layer_combination,
                            combination,
                            language.translate(combination.name()),
                        );
                    }
                }
                PlanOptionKind::PenSets => {
                    for pen_set in PenSet::ALL {
                        ui.horizontal(|ui| {
                            let (rect, _) = ui.allocate_exact_size(vec2(18., 18.), Sense::hover());
                            ui.painter().rect_filled(
                                rect,
                                3.,
                                Self::plan_color(pen_set.palette().line),
                            );
                            ui.radio_value(
                                &mut options.pen_set,
                                pen_set,
                                language.translate(pen_set.name()),
                            );
                        });
                    }
                }
                PlanOptionKind::PartialStructure => {
                    for display in PartialStructureDisplay::ALL {
                        ui.radio_value(
                            &mut options.partial_structure,
                            display,
                            language.translate(display.name()),
                        );
                    }
                }
                PlanOptionKind::ModelView => {
                    for model_view in ModelViewOptions::ALL {
                        ui.radio_value(
                            &mut options.model_view,
                            model_view,
                            language.translate(model_view.name()),
                        );
                    }
                }
                PlanOptionKind::GraphicOverrides => {
                    for overrides in GraphicOverrides::ALL {
                        ui.radio_value(
                            &mut options.graphic_overrides,
                            overrides,
                            language.translate(overrides.name()),
                        );
                    }
                }
                PlanOptionKind::Renovation => {
                    for filter in RenovationFilter::ALL {
                        ui.radio_value(
                            &mut options.renovation_filter,
                            filter,
                            language.translate(filter.name()),
                        );
                    }
                }
                PlanOptionKind::Dimensions => {
                    for style in DimensionStyle::ALL {
                        ui.radio_value(
                            &mut options.dimension_style,
                            style,
                            language.translate(style.name()),
                        );
                    }
                    ui.separator();
                    ui.label(format!(
                        "{} · 3.200 m → {}",
                        language.translate("Preview"),
                        options.dimension_style.format_length(3.2)
                    ));
                }
                PlanOptionKind::DesignOptions => {}
            }
            ui.add_space(12.);
            ui.separator();
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui
                    .add_sized(
                        [94., 32.],
                        Button::new(Self::t(self.language, "OK")).fill(ACCENT),
                    )
                    .clicked()
                {
                    apply = true;
                }
                if ui.button(Self::t(self.language, "Cancel")).clicked() {
                    cancel = true;
                }
            });
        });
        self.plan_dialog_options = options;
        if apply {
            let before = self.editor.project.clone();
            self.editor.project.plan_options = self.plan_dialog_options.clone();
            match self.editor.project.plan_options.validate() {
                Ok(()) => {
                    if self.editor.project.plan_options != before.plan_options {
                        self.editor.commit(before);
                    }
                    self.editor.status = format!(
                        "{} {}",
                        Self::plan_option_title(self.language, kind),
                        self.language.translate("updated")
                    );
                    self.plan_option_dialog = None;
                }
                Err(error) => {
                    self.editor.project.plan_options = before.plan_options;
                    self.editor.status = error;
                }
            }
        }
        if cancel || modal.should_close() {
            self.plan_option_dialog = None;
        }
    }

    pub(super) fn design_options_dialog(&mut self, ctx: &Context) {
        #[derive(Clone, Copy)]
        enum DesignAction {
            SetActive(Option<u32>),
            Delete(u32),
            Create,
            AssignSelected,
            MoveSelectedToMain,
        }
        let mut actions = Vec::new();
        let mut create_name = self.design_option_new_name.clone();
        let mut close = false;
        let language = self.language;
        let modal = Modal::new(Id::new("design_options_manager")).show(ctx, |ui| {
            ui.set_min_width(500.);
            ui.heading(language.translate("Design Options"));
            ui.label(
                RichText::new(language.translate(
                    "Main Model Only shows elements that are not assigned to a design option.",
                ))
                .small()
                .color(MUTED),
            );
            ui.separator();
            let active = self.editor.project.plan_options.active_design_option;
            if ui
                .radio(active.is_none(), language.translate("Main Model Only"))
                .clicked()
            {
                actions.push(DesignAction::SetActive(None));
            }
            ui.add_space(4.);
            let options: Vec<(u32, String)> = self
                .editor
                .project
                .design_options
                .options
                .iter()
                .map(|option| (option.id, option.name.clone()))
                .collect();
            for (id, name) in options {
                let count = self
                    .editor
                    .project
                    .elements
                    .iter()
                    .filter(|element| element.design_option == Some(id))
                    .count();
                ui.horizontal(|ui| {
                    if ui.radio(active == Some(id), "").clicked() {
                        actions.push(DesignAction::SetActive(Some(id)));
                    }
                    ui.label(name);
                    ui.label(
                        RichText::new(format!(
                            "{count} {}",
                            language.translate(if count == 1 { "element" } else { "elements" })
                        ))
                        .small()
                        .color(MUTED),
                    );
                    if ui.small_button(language.translate("Delete")).clicked() {
                        actions.push(DesignAction::Delete(id));
                    }
                });
            }
            ui.separator();
            ui.horizontal(|ui| {
                ui.add(
                    TextEdit::singleline(&mut create_name)
                        .hint_text(language.translate("New design option name"))
                        .desired_width(250.),
                );
                if ui.button(language.translate("New Design Option")).clicked() {
                    actions.push(DesignAction::Create);
                }
            });
            ui.add_space(6.);
            ui.horizontal(|ui| {
                let assign_enabled = active.is_some();
                if ui
                    .add_enabled(
                        assign_enabled,
                        Button::new(language.translate("Assign Selected to Active Option")),
                    )
                    .clicked()
                {
                    actions.push(DesignAction::AssignSelected);
                }
                if ui
                    .button(language.translate("Move Selected to Main Model"))
                    .clicked()
                {
                    actions.push(DesignAction::MoveSelectedToMain);
                }
            });
            ui.add_space(12.);
            ui.separator();
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui
                    .add_sized(
                        [94., 32.],
                        Button::new(Self::t(self.language, "Close")).fill(ACCENT),
                    )
                    .clicked()
                {
                    close = true;
                }
            });
        });
        self.design_option_new_name = create_name;
        for action in actions {
            match action {
                DesignAction::SetActive(option) => self.set_active_design_option(option),
                DesignAction::Delete(id) => {
                    let before = self.editor.project.clone();
                    self.editor.project.design_options.remove(id);
                    if self.editor.project.plan_options.active_design_option == Some(id) {
                        self.editor.project.plan_options.active_design_option = None;
                    }
                    for element in &mut self.editor.project.elements {
                        if element.design_option == Some(id) {
                            element.design_option = None;
                        }
                    }
                    self.editor.commit(before);
                    self.editor.status = language.translate("Design option deleted").into();
                }
                DesignAction::Create => {
                    let before = self.editor.project.clone();
                    let next = self.editor.project.design_options.next_id.max(1);
                    let name = if self.design_option_new_name.trim().is_empty() {
                        format!("{} {next}", language.translate("Design Option"))
                    } else {
                        self.design_option_new_name.trim().to_owned()
                    };
                    let id = self.editor.project.design_options.create(name.clone());
                    self.editor.project.plan_options.active_design_option = Some(id);
                    self.editor.commit(before);
                    self.design_option_new_name.clear();
                    self.editor.status =
                        format!("{} · {name}", language.translate("Design option created"));
                }
                DesignAction::AssignSelected => {
                    let Some(active) = self.editor.project.plan_options.active_design_option else {
                        self.editor.status =
                            language.translate("Select a design option first").into();
                        continue;
                    };
                    let selection: Vec<u64> = self.editor.selection_ids();
                    if selection.is_empty() {
                        self.editor.status = language.translate("Select elements to assign").into();
                        continue;
                    }
                    let before = self.editor.project.clone();
                    for element in &mut self.editor.project.elements {
                        if selection.contains(&element.id) {
                            element.design_option = Some(active);
                        }
                    }
                    self.editor.commit(before);
                    self.editor.status = language
                        .translate("Selected elements assigned to design option")
                        .into();
                }
                DesignAction::MoveSelectedToMain => {
                    let selection: Vec<u64> = self.editor.selection_ids();
                    if selection.is_empty() {
                        self.editor.status = language.translate("Select elements to move").into();
                        continue;
                    }
                    let before = self.editor.project.clone();
                    for element in &mut self.editor.project.elements {
                        if selection.contains(&element.id) {
                            element.design_option = None;
                        }
                    }
                    self.editor.commit(before);
                    self.editor.status = language
                        .translate("Selected elements moved to Main Model")
                        .into();
                }
            }
        }
        if close || modal.should_close() {
            self.plan_option_dialog = None;
        }
    }
}
