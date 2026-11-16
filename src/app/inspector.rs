use super::*;

impl AtelierApp {
    fn compact_concept_inspector(&self) -> bool {
        let editor = &self.host.document.editor;
        crate::design_process::service::selected_space(editor).is_some()
            || editor.selected.is_some_and(|id| {
                editor.project.elements.iter().any(|e| {
                    e.id == id
                        && (matches!(e.shape, Shape::ConceptMass { .. })
                            || (matches!(e.shape, Shape::Zone { .. })
                                && (editor
                                    .project
                                    .design_process
                                    .mass_spaces
                                    .iter()
                                    .any(|link| link.zone == id)
                                    || (editor
                                        .project
                                        .design_process
                                        .task()
                                        .is_some_and(|task| task.is_mass())
                                        && e.design_option
                                            == editor.project.plan_options.active_design_option))))
                })
            })
    }

    fn navigator_contents(&mut self, ui: &mut Ui, layout_header_height: f32) {
        ui.horizontal(|ui| {
            if self.tab != Tab::Assistant {
                Self::label(ui, &Self::t(self.language, "NAVIGATOR"));
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                Self::vector_icon_button(
                    ui,
                    "Options",
                    &Self::t(self.language, "Navigator options"),
                    false,
                );
            });
        });
        ui.horizontal(|ui| {
            for (view, icon, tip) in [
                (NavigatorView::ProjectMap, "Project Map", "Project Map"),
                (NavigatorView::ViewMap, "View Map", "View Map"),
                (NavigatorView::LayoutBook, "Layout Book", "Layout Book"),
                (NavigatorView::Publisher, "Publisher", "Publisher Sets"),
            ] {
                if Self::vector_icon_button(
                    ui,
                    icon,
                    &Self::t(self.language, tip),
                    self.navigator_view == view,
                ) {
                    self.navigator_view = view;
                }
            }
        });
        ui.horizontal(|ui| {
            Self::inline_vector_icon(ui, "Search", MUTED);
            ui.add_sized(
                [ui.available_width(), 25.],
                TextEdit::singleline(&mut self.navigator_search)
                    .hint_text(Self::t(self.language, "Search navigator")),
            );
        });
        ui.separator();
        // Object questions and parameter drafts share this pane with navigation.
        // Reserve editing space even when opening a saved task without a selection.
        let tree_height = if self.tab == Tab::Assistant {
            (ui.available_height() * 0.2).clamp(32., 100.)
        } else if self.compact_concept_inspector() {
            100.
        } else {
            ((ui.available_height() + layout_header_height) * 0.45).clamp(100., 300.)
        };
        let tree_scroll = ScrollArea::vertical()
            .id_salt("navigator_tree_scroll")
            .max_height(tree_height);
        let tree_scroll = if self.tab == Tab::Assistant {
            tree_scroll.min_scrolled_height(0.)
        } else {
            tree_scroll
        };
        let navigator_scroll = tree_scroll.show(ui, |ui| match self.navigator_view {
            NavigatorView::ProjectMap => {
                ui.horizontal(|ui| {
                    Self::inline_vector_icon(ui, "Home", ACCENT);
                    ui.strong(&self.host.document.editor.project.name);
                });
                CollapsingHeader::new(Self::t(self.language, "Stories"))
                    .default_open(true)
                    .show(ui, |ui| {
                        for story in self.host.document.editor.project.stories.clone() {
                            let selected = self.host.document.editor.story == story.id;
                            let text = format!(
                                "{:02}  {}     {:+.2} m",
                                story.id, story.name, story.elevation
                            );
                            if ui
                                .add_sized(
                                    [ui.available_width(), 25.],
                                    Button::new(text).selected(selected),
                                )
                                .clicked()
                            {
                                self.host.document.editor.switch_story(story.id);
                                self.tracker_locked = false;
                                self.tracker_a = 0.;
                                self.tracker_b = 0.;
                                self.draft_id = None;
                            }
                        }
                    });
                CollapsingHeader::new(Self::t(self.language, "Worksheets"))
                    .default_open(true)
                    .show(ui, |ui| {
                        for sheet in self
                            .host
                            .document
                            .editor
                            .project
                            .drawings
                            .worksheets
                            .clone()
                        {
                            if ui
                                .selectable_label(
                                    self.host
                                        .document
                                        .editor
                                        .project
                                        .plan_options
                                        .active_worksheet
                                        == Some(sheet.id),
                                    sheet.name,
                                )
                                .clicked()
                            {
                                self.open_worksheet(sheet.id);
                            }
                        }
                        if ui
                            .button(self.language.translate("New worksheet"))
                            .clicked()
                            && let Err(e) = self.new_worksheet()
                        {
                            self.host.document.editor.status = e;
                        }
                        for page in self
                            .host
                            .document
                            .editor
                            .project
                            .drawings
                            .pages
                            .clone()
                            .into_iter()
                            .filter(|p| p.placement.is_none())
                        {
                            if ui
                                .selectable_label(
                                    self.host
                                        .document
                                        .editor
                                        .project
                                        .plan_options
                                        .active_worksheet
                                        == Some(page.id),
                                    &page.name,
                                )
                                .clicked()
                            {
                                self.open_worksheet(page.id);
                            }
                        }
                        if ui
                            .button(self.language.translate("Place External Drawing…"))
                            .clicked()
                        {
                            self.drawings_ui.open = true;
                        }
                    });
                for node in [
                    "Sections",
                    "Elevations",
                    "Interior Elevations",
                    "Details",
                    "3D Documents",
                    "3D",
                    "Schedules",
                    "Project Indexes",
                    "Lists",
                    "Info",
                    "Help",
                ] {
                    CollapsingHeader::new(Self::t(self.language, node)).show(ui, |ui| {
                        let entries: &[&str] = match node {
                            "Elevations" => &[
                                "East Elevation (Auto-rebuild Model)",
                                "North Elevation (Auto-rebuild Model)",
                                "South Elevation (Auto-rebuild Model)",
                                "West Elevation (Auto-rebuild Model)",
                            ],
                            "3D" => &["Generic Perspective", "Generic Axonometry"],
                            "Schedules" => &[
                                "Wall Schedule",
                                "Door Schedule",
                                "Window Schedule",
                                "Object Inventory",
                            ],
                            "Project Indexes" => &["Change List", "Drawing List", "Sheet Index"],
                            _ => &["No saved views"],
                        };
                        for entry in entries {
                            if Self::navigator_item(
                                ui,
                                if node == "3D" {
                                    "Perspective"
                                } else {
                                    "Elevation View"
                                },
                                &Self::t(self.language, entry),
                            ) {
                                self.host.document.editor.status =
                                    format!("Navigator item · {entry}");
                            }
                        }
                    });
                }
            }
            NavigatorView::ViewMap => {
                ui.strong(Self::t(self.language, "View Map"));
                let figures: Vec<_> = self
                    .host
                    .document
                    .editor
                    .project
                    .drawings
                    .diagrams
                    .iter()
                    .map(|v| (v.id, v.name.clone()))
                    .collect();
                for (id, name) in figures {
                    if ui
                        .selectable_label(self.view == View::Figure(id), format!("#{id} · {name}"))
                        .clicked()
                    {
                        self.open_figure(id);
                    }
                }
                for group in [
                    "Project Views",
                    "Floor Plans",
                    "Ceiling Plans",
                    "Sections & Elevations",
                    "3D Views",
                    "Schedules",
                ] {
                    CollapsingHeader::new(Self::t(self.language, group))
                        .default_open(group == "Floor Plans")
                        .show(ui, |ui| {
                            for entry in [
                                "Ground Floor",
                                "South Elevation",
                                "Generic Perspective",
                                "Generic Axonometry",
                            ] {
                                if Self::navigator_item(
                                    ui,
                                    if group == "Floor Plans" {
                                        "Floor Plan"
                                    } else {
                                        "View Map"
                                    },
                                    &Self::t(self.language, entry),
                                ) {
                                    self.host.document.editor.status =
                                        format!("View Map · {entry}");
                                }
                            }
                        });
                }
            }
            NavigatorView::LayoutBook => {
                ui.strong(Self::t(self.language, "Layout Book"));
                for group in [
                    "Master Layouts",
                    "Layout Subsets",
                    "Documentation Set",
                    "Plans",
                    "Elevations",
                ] {
                    CollapsingHeader::new(Self::t(self.language, group))
                        .default_open(group == "Documentation Set")
                        .show(ui, |ui| {
                            for entry in ["A-001 Cover", "A-101 Floor Plan", "A-201 Elevations"] {
                                if Self::navigator_item(
                                    ui,
                                    "Layout Book",
                                    &Self::t(self.language, entry),
                                ) {
                                    self.host.document.editor.status = format!("Layout · {entry}");
                                }
                            }
                        });
                }
            }
            NavigatorView::Publisher => {
                ui.strong(Self::t(self.language, "Publisher Sets"));
                for group in [
                    "Publisher Sets",
                    "PDF Issue",
                    "BIMx Hyper-model",
                    "DWG Exchange",
                ] {
                    CollapsingHeader::new(Self::t(self.language, group))
                        .default_open(group == "Publisher Sets")
                        .show(ui, |ui| {
                            for entry in ["Current Project", "Documentation Set", "3D Model"] {
                                if Self::navigator_item(
                                    ui,
                                    "Publisher",
                                    &Self::t(self.language, entry),
                                ) {
                                    self.host.document.editor.status =
                                        format!("Publisher item · {entry}");
                                }
                            }
                        });
                }
                ui.add_sized(
                    [ui.available_width(), 28.],
                    Button::new(self.language.translate("Publish…")),
                );
            }
        });
        #[cfg(test)]
        ui.ctx().data_mut(|data| {
            data.insert_temp(Id::new("navigator_tree_rect"), navigator_scroll.inner_rect);
            data.insert_temp(
                Id::new("navigator_tree_offset"),
                navigator_scroll.state.offset.y,
            );
        });
        #[cfg(not(test))]
        let _ = navigator_scroll;
        ui.checkbox(
            &mut self.all_stories,
            Self::t(self.language, "Show all stories in 3D"),
        );
        ui.separator();
    }

    pub(super) fn inspector_contents(&mut self, ui: &mut Ui, layout_header_height: f32) {
        if self.tab == Tab::Assistant || self.ai_workspace.enabled {
            // Navigation stays beside the task, available on demand. Its disclosure
            // never changes selection, the parameter draft, or the discussion target.
            CollapsingHeader::new(Self::t(self.language, "NAVIGATOR"))
                .id_salt("assistant_navigation")
                .default_open(false)
                .show(ui, |ui| self.navigator_contents(ui, layout_header_height));
        } else {
            self.navigator_contents(ui, layout_header_height);
        }
        ui.horizontal(|ui| {
            ui.selectable_value(
                &mut self.tab,
                Tab::Properties,
                Self::t(self.language, "Properties"),
            );
            ui.selectable_value(
                &mut self.tab,
                Tab::Library,
                Self::t(self.language, "Library"),
            );
            ui.selectable_value(
                &mut self.tab,
                Tab::Assistant,
                Self::t(self.language, "AI Assistant"),
            );
        });
        ui.separator();
        let sticky_apply = self.tab == Tab::Properties && self.compact_concept_inspector();
        let sticky_parameters = self.tab == Tab::Assistant
            && self.research_ui.parameters.is_some()
            && self.research_ui.canvas_move.is_none();
        let body_height = (ui.available_height()
            - if sticky_apply {
                44.
            } else if sticky_parameters {
                56.
            } else {
                0.
            })
        .max(0.);
        let tab_key = match self.tab {
            Tab::Properties => "properties",
            Tab::Library => "library",
            Tab::Assistant => "assistant",
        };
        let context = (self.host.document.editor.document_session(), tab_key);
        let context_id = ui.id().with("inspector_scroll_context");
        let momentum_id = ui.id().with("inspector_old_scroll_momentum");
        let changed = ui.ctx().data_mut(|data| {
            let changed = data.get_temp::<(u64, &'static str)>(context_id) != Some(context);
            data.insert_temp(context_id, context);
            changed
        });
        let focus =
            self.tab == Tab::Assistant && std::mem::take(&mut self.research_ui.focus_question);
        let salt = ("inspector_scroll", context);
        if changed || focus {
            // Stop only this pane's old kinetic scroll. Preserve a tab's position,
            // except when the user explicitly opens an object question or parameters.
            let id = ui.make_persistent_id(salt);
            let mut state = egui::scroll_area::State::default();
            if !focus {
                state.offset = egui::scroll_area::State::load(ui.ctx(), id)
                    .unwrap_or_default()
                    .offset;
            }
            state.store(ui.ctx(), id);
            ui.ctx()
                .data_mut(|data| data.insert_temp(momentum_id, true));
        }
        let new_wheel = ui.input(|input| {
            input
                .events
                .iter()
                .any(|e| matches!(e, egui::Event::MouseWheel { .. }))
        });
        if new_wheel || !ui.input(|input| input.is_scrolling()) {
            ui.ctx()
                .data_mut(|data| data.insert_temp(momentum_id, false));
        }
        let mut scroll = ScrollArea::vertical().id_salt(salt).max_height(body_height);
        if sticky_parameters {
            // egui's default 64pt minimum may exceed the space left below a
            // tall navigator. The body must yield space to its fixed actions.
            scroll = scroll.min_scrolled_height(0.);
        }
        if ui
            .ctx()
            .data(|data| data.get_temp::<bool>(momentum_id).unwrap_or(false))
        {
            // A smoothed wheel tail from the previous view is not a new gesture.
            scroll = scroll.scroll_source(egui::scroll_area::ScrollSource {
                mouse_wheel: false,
                ..Default::default()
            });
        }
        if focus {
            scroll = scroll.vertical_scroll_offset(0.);
        }
        let body_scroll = scroll.show(ui, |ui| match self.tab {
            Tab::Properties => self.properties_in_layout(ui, sticky_apply),
            Tab::Library => self.library(ui),
            Tab::Assistant => self.assistant(ui),
        });
        #[cfg(test)]
        ui.ctx().data_mut(|d| {
            d.insert_temp(Id::new("inspector_scroll_rect"), body_scroll.inner_rect);
            d.insert_temp(
                Id::new("inspector_scroll_offset"),
                body_scroll.state.offset.y,
            );
        });
        #[cfg(not(test))]
        let _ = body_scroll;
        if sticky_apply {
            ui.separator();
            if self.property_apply_button(ui) {
                if let Some(element) = self.draft.clone() {
                    self.apply_property_element(element);
                }
            }
        }
        if sticky_parameters && self.research_ui.parameters.is_some() {
            ui.separator();
            let controls = ui.scope(|ui| self.parametric_draft_actions(ui));
            #[cfg(test)]
            ui.ctx().data_mut(|d| {
                d.insert_temp(Id::new("parametric_actions_rect"), controls.response.rect)
            });
            #[cfg(not(test))]
            let _ = controls;
        } else {
            #[cfg(test)]
            ui.ctx()
                .data_mut(|d| d.remove::<Rect>(Id::new("parametric_actions_rect")));
        }
    }
    pub(super) fn properties(&mut self, ui: &mut Ui) {
        self.properties_in_layout(ui, false);
    }
    fn property_apply_button(&self, ui: &mut Ui) -> bool {
        ui.add_sized(
            [ui.available_width(), 34.],
            Button::new(
                RichText::new(Self::t(self.language, "Apply changes")).color(Color32::WHITE),
            )
            .fill(ACCENT),
        )
        .clicked()
    }
    fn apply_property_element(&mut self, element: Element) {
        match self.host.document.editor.replace_element(element) {
            Ok(()) => {
                self.host.document.editor.status =
                    "Properties updated · Both views synchronized".into()
            }
            Err(error) => self.host.document.editor.status = error,
        }
    }
    fn properties_in_layout(&mut self, ui: &mut Ui, sticky_apply: bool) {
        self.mass_space_controls(ui);
        let source = self
            .host
            .document
            .editor
            .project
            .elements
            .iter()
            .find(|e| Some(e.id) == self.host.document.editor.selected);
        if self.draft_id != self.host.document.editor.selected
            || self.host.document.editor.selected.is_none()
            || self.draft_source.as_ref() != source
        {
            self.draft_id = self.host.document.editor.selected;
            self.draft_source = source.cloned();
            self.draft = self.draft_source.clone();
        }
        if let Some(mut e) = self.draft.clone() {
            Self::label(ui, &Self::t(self.language, "ELEMENT SETTINGS"));
            ui.add_space(8.);
            ui.horizontal(|ui| {
                ui.heading(format!("{} {:03}", self.language.translate(e.kind()), e.id));
            });
            use crate::derivation_cache::NominalMeasure;
            let value = if source == Some(&e) {
                self.host
                    .document
                    .editor
                    .geometry_source(e.id)
                    .map(|h| h.source.nominal_measure())
            } else {
                // A parameter draft is not the committed entity. Keep its
                // existing shared analytic measure until it is accepted.
                Ok(if matches!(e.shape, Shape::Polyline { .. }) {
                    NominalMeasure::LengthM(e.area())
                } else {
                    NominalMeasure::AreaM2(e.area())
                })
            };
            let material = if let Shape::Zone { spec } = &e.shape {
                spec.purpose.as_str()
            } else {
                self.language.translate(e.material.name())
            };
            let measure = match value {
                Ok(NominalMeasure::LengthM(value)) => format!("{material} · {value:.2} m"),
                Ok(NominalMeasure::AreaM2(value)) => format!("{material} · {value:.2} m²"),
                Err(d) => self.language.geometry_diagnostic_message(&d),
            };
            ui.label(RichText::new(measure).color(MUTED));
            ui.add_space(10.);
            let stories = self.host.document.editor.project.stories.clone();
            if matches!(e.shape, Shape::ConceptMass { .. }) {
                ui.label(
                    self.language
                        .translate("Concept envelope; not space or statutory floor area"),
                );
            }
            Self::element_fields(ui, &mut e, &stories, "inspector", self.language);
            if let Shape::Wall {
                thickness, height, ..
            } = e.shape
            {
                let settings: WallSettings = e.wall_settings.clone().unwrap_or_default();
                ui.add_space(14.);
                Self::label(ui, self.language.translate("WALL SETTINGS"));
                Grid::new(("inspector_wall_settings", e.id))
                    .num_columns(2)
                    .spacing([14., 6.])
                    .show(ui, |ui| {
                        Self::property_row(
                            ui,
                            self.language,
                            "Thickness",
                            &format!("{thickness:.3} m"),
                        );
                        Self::property_row(ui, self.language, "Height", &format!("{height:.3} m"));
                        Self::property_row(
                            ui,
                            self.language,
                            "Reference Line",
                            self.language
                                .translate(settings.geometry.reference_line.name()),
                        );
                        Self::property_row(
                            ui,
                            self.language,
                            "Reference Offset",
                            &format!("{:.3} m", settings.geometry.reference_offset),
                        );
                        Self::property_row(
                            ui,
                            self.language,
                            "Top / Bottom Offset",
                            &format!(
                                "{:.3} / {:.3} m",
                                settings.geometry.top_offset, settings.geometry.bottom_offset
                            ),
                        );
                        Self::property_row(ui, self.language, "Structure", &settings.profile.name);
                        Self::property_row(
                            ui,
                            self.language,
                            "Floor Plan Display",
                            self.language
                                .translate(settings.floor_plan.floor_plan_display.name()),
                        );
                        Self::property_row(
                            ui,
                            self.language,
                            "Uncut Lines",
                            self.language
                                .translate(settings.floor_plan.uncut_lines.name()),
                        );
                        Self::property_row(
                            ui,
                            self.language,
                            "Left Surface",
                            &settings.model.left_surface,
                        );
                        Self::property_row(
                            ui,
                            self.language,
                            "Right Surface",
                            &settings.model.right_surface,
                        );
                        Self::property_row(
                            ui,
                            self.language,
                            "End Surface",
                            &settings.model.end_surface,
                        );
                        Self::property_row(
                            ui,
                            self.language,
                            "Structural Function",
                            self.language
                                .translate(settings.structural.structural_function.name()),
                        );
                        Self::property_row(
                            ui,
                            self.language,
                            "Property ID",
                            &settings.classification.property_id,
                        );
                    });
                ui.add_space(8.);
                if ui
                    .add_sized(
                        [ui.available_width(), 32.],
                        Button::new(self.personal_command_label(
                            "edit.tool_settings",
                            "Edit Wall Settings",
                            &Self::t(self.language, "Edit Wall Settings  ⌘T"),
                        )),
                    )
                    .clicked()
                {
                    self.wall_settings_dialog = WallSettingsDialog::selected(&e);
                    self.element_settings = None;
                    self.element_settings_error.clear();
                }
            }
            ui.add_space(14.);
            if !sticky_apply && self.property_apply_button(ui) {
                self.apply_property_element(e.clone());
            }
            ui.horizontal(|ui| {
                if ui.button(Self::t(self.language, "Duplicate")).clicked() {
                    self.host.document.editor.duplicate();
                }
                if ui.button(Self::t(self.language, "Delete")).clicked() {
                    self.host.document.editor.delete();
                }
            });
            self.draft = Some(e);
        } else {
            ui.add_space(12.);
            ui.heading(self.language.translate("Build with precision."));
            ui.label(
                RichText::new(self.language.translate(
                    "Select an element in either view to inspect and edit its properties.",
                ))
                .color(MUTED),
            );
            ui.add_space(20.);
            Self::label(ui, self.language.translate("PROJECT QUANTITIES"));
            let p = &self.host.document.editor.project;
            let walls = p
                .elements
                .iter()
                .filter(|e| matches!(e.shape, crate::model::Shape::Wall { .. }))
                .count();
            let stairs = p
                .elements
                .iter()
                .filter(|e| matches!(e.shape, crate::model::Shape::Stair { .. }))
                .count();
            let area: f64 = p
                .elements
                .iter()
                .filter(|e| matches!(e.shape, crate::model::Shape::Slab { .. }))
                .map(Element::area)
                .sum();
            ui.add_space(8.);
            ui.heading(format!("{}", p.elements.len()));
            ui.label(if self.language == UiLanguage::SimplifiedChinese {
                format!("构件 / {walls} 面墙 / {stairs} 座楼梯")
            } else {
                format!("elements / {walls} walls / {stairs} stairs")
            });
            ui.add_space(12.);
            ui.heading(format!("{area:.1} m²"));
            ui.label(self.language.translate("total slab footprint"));
        }
        ui.add_space(20.);
        ui.separator();
        CollapsingHeader::new(self.language.translate("Elements on current story")).show(
            ui,
            |ui| {
                for e in self
                    .host
                    .document
                    .editor
                    .project
                    .elements
                    .clone()
                    .iter()
                    .filter(|e| e.story == self.host.document.editor.story)
                {
                    if ui
                        .selectable_label(
                            self.host.document.editor.selected == Some(e.id),
                            format!("{}  ·  {}", e.name, e.material.name()),
                        )
                        .clicked()
                    {
                        self.host.document.editor.selected = Some(e.id);
                    }
                }
            },
        );
    }
    pub(super) fn number(ui: &mut Ui, language: UiLanguage, label: &str, n: &mut f64) {
        Self::number_input(ui, language, label, n);
        ui.end_row();
    }
    pub(super) fn number_input(
        ui: &mut Ui,
        language: UiLanguage,
        label: &str,
        n: &mut f64,
    ) -> Response {
        let label = ui.label(language.translate(label));
        // DragValue parses its edit text again on blur, even without typing.
        // Keep the compact idle label, but seed keyboard editing with an exact
        // round-trip value so merely inspecting a coordinate cannot round it.
        let id = ui.next_auto_id();
        let ctx = ui.ctx().clone();
        ui.add(
            DragValue::new(n)
                .speed(0.01)
                .fixed_decimals(3)
                .custom_formatter(move |value, _| {
                    // Tab can grant focus inside DragValue's own layout pass.
                    if ctx.memory(|m| m.has_focus(id)) {
                        value.to_string()
                    } else {
                        format!("{value:.3}")
                    }
                })
                .suffix(" m"),
        )
        .labelled_by(label.id)
    }
    fn bounds_fields(
        ui: &mut Ui,
        language: UiLanguage,
        min: &mut Point,
        max: &mut Point,
        editable: bool,
    ) {
        for (label, value) in [
            ("Min X", &mut min.x),
            ("Min Y", &mut min.y),
            ("Max X", &mut max.x),
            ("Max Y", &mut max.y),
        ] {
            let label = ui.add_enabled(editable, egui::Label::new(language.translate(label)));
            ui.add_enabled(
                editable,
                DragValue::new(value)
                    .speed(0.01)
                    .fixed_decimals(3)
                    .suffix(" m"),
            )
            .labelled_by(label.id);
            ui.end_row();
        }
    }

    pub(super) fn property_row(ui: &mut Ui, language: UiLanguage, label: &str, value: &str) {
        ui.label(RichText::new(language.translate(label)).color(MUTED));
        ui.label(RichText::new(value).color(INK));
        ui.end_row();
    }
    pub(super) fn element_fields(
        ui: &mut Ui,
        element: &mut Element,
        stories: &[Story],
        namespace: &str,
        language: UiLanguage,
    ) {
        let original_joinery = element.opening_joinery;
        ui.label(language.translate("Element name"));
        ui.text_edit_singleline(&mut element.name);
        ui.add_space(8.);
        Grid::new(("element_settings_fields", namespace, element.id))
            .num_columns(2)
            .spacing([16., 10.])
            .show(ui, |ui| {
                ui.label(language.translate("Home Story"));
                ComboBox::from_id_salt(("element_story", namespace, element.id))
                    .selected_text(
                        stories
                            .iter()
                            .find(|story| story.id == element.story)
                            .map_or_else(
                                || format!("Level {:02}", element.story),
                                |story| story.name.clone(),
                            ),
                    )
                    .width(180.)
                    .show_ui(ui, |ui| {
                        for story in stories {
                            ui.selectable_value(&mut element.story, story.id, &story.name);
                        }
                    });
                ui.end_row();
                if !matches!(element.shape, Shape::Zone { .. }) {
                    ui.label(language.translate("Surface"));
                    ComboBox::from_id_salt(("element_material", namespace, element.id))
                        .selected_text(element.material.name())
                        .width(180.)
                        .show_ui(ui, |ui| {
                            for material in Material::ALL {
                                ui.selectable_value(
                                    &mut element.material,
                                    material,
                                    material.name(),
                                );
                            }
                        });
                    ui.end_row();
                }
                if matches!(element.shape, Shape::Slab { .. }) {
                    let mut elevation = element
                        .slab_contour
                        .as_ref()
                        .map_or(0., |contour| contour.elevation);
                    let label = ui.label(language.translate("Top offset to story"));
                    if ui
                        .add(
                            DragValue::new(&mut elevation)
                                .speed(0.01)
                                .fixed_decimals(3)
                                .suffix(" m"),
                        )
                        .labelled_by(label.id)
                        .on_hover_text(language.translate(
                            "Slab top relative to the home story; thickness extends downward",
                        ))
                        .changed()
                    {
                        let mut contour = crate::slab_geometry::contour(element);
                        contour.elevation = elevation;
                        element.slab_contour = Some(contour);
                    }
                    ui.end_row();
                }
                match &mut element.shape {
                    Shape::ConceptMass { spec } => {
                        Self::number(ui, language, "Height", &mut spec.height);
                        Self::number(ui, language, "Bottom offset", &mut spec.bottom_offset);
                        ui.label(language.translate("Mass footprint"));
                        ui.label(format!("{:.2} m²", spec.area()));
                        ui.end_row();
                        ui.label(language.translate("Concept volume"));
                        ui.label(format!("{:.2} m³", spec.volume()));
                        ui.end_row();

                        ui.collapsing(language.translate("Mass footprint"), |ui| {
                            Grid::new(("mass_vertices", namespace, element.id))
                                .num_columns(2)
                                .show(ui, |ui| {
                                    for (ring, points) in std::iter::once(&mut spec.outline)
                                        .chain(spec.holes.iter_mut())
                                        .enumerate()
                                    {
                                        for (i, p) in points.iter_mut().enumerate() {
                                            Self::number(
                                                ui,
                                                language,
                                                &format!("X {ring}.{}", i + 1),
                                                &mut p.x,
                                            );
                                            Self::number(
                                                ui,
                                                language,
                                                &format!("Y {ring}.{}", i + 1),
                                                &mut p.y,
                                            );
                                        }
                                    }
                                });
                        });
                        ui.end_row();
                    }

                    Shape::Zone { spec } => {
                        ui.label(language.translate("3D surface"));
                        let mut custom = !spec.surface.is_inherit();
                        if ui
                            .checkbox(&mut custom, language.translate("Custom color"))
                            .changed()
                        {
                            spec.surface = if custom {
                                crate::zones::surfaces::ZoneSurface::Solid {
                                    name: "Zone surface".into(),
                                    color: [110, 170, 190],
                                }
                            } else {
                                Default::default()
                            };
                        }
                        ui.end_row();
                        if let crate::zones::surfaces::ZoneSurface::Solid { name, color } =
                            &mut spec.surface
                        {
                            ui.label(language.translate("Surface name"));
                            ui.text_edit_singleline(name);
                            ui.end_row();
                            ui.label(language.translate("Surface color"));
                            ui.color_edit_button_srgb(color);
                            ui.end_row();
                        }
                        ui.label(language.translate("Space purpose"));
                        ui.text_edit_singleline(&mut spec.purpose);
                        ui.end_row();
                        Self::number(ui, language, "Clear height", &mut spec.height);
                        ui.label(language.translate("Space area"));
                        ui.label(format!("{:.2} m²", spec.area()));
                        ui.end_row();
                        ui.label(language.translate("Space volume"));
                        ui.label(format!("{:.2} m³", spec.area() * spec.height));
                        ui.end_row();
                        ui.collapsing(language.translate("Advanced space geometry"), |ui| {
                            Grid::new(("zone_advanced", namespace, element.id))
                                .num_columns(2)
                                .show(ui, |ui| {
                                    Self::number(
                                        ui,
                                        language,
                                        "Bottom offset",
                                        &mut spec.bottom_offset,
                                    );
                                    let mut auto = spec.label_position.is_none();
                                    ui.label(language.translate("Automatic label"));
                                    if ui.checkbox(&mut auto, "").changed() {
                                        spec.label_position =
                                            if auto { None } else { Some(spec.label_point()) };
                                    }
                                    ui.end_row();
                                    if let Some(p) = &mut spec.label_position {
                                        Self::number(ui, language, "Label X", &mut p.x);
                                        Self::number(ui, language, "Label Y", &mut p.y);
                                    }
                                    for (i, p) in spec.outline.iter_mut().enumerate() {
                                        Self::number(
                                            ui,
                                            language,
                                            &format!("X {}", i + 1),
                                            &mut p.x,
                                        );
                                        Self::number(
                                            ui,
                                            language,
                                            &format!("Y {}", i + 1),
                                            &mut p.y,
                                        );
                                    }
                                });
                        });
                        ui.end_row();
                    }
                    Shape::Roof { spec } => {
                        Self::number(ui, language, "Pivot start X", &mut spec.pivot_start.x);
                        Self::number(ui, language, "Pivot start Y", &mut spec.pivot_start.y);
                        Self::number(ui, language, "Pivot end X", &mut spec.pivot_end.x);
                        Self::number(ui, language, "Pivot end Y", &mut spec.pivot_end.y);
                        // Keep each label/value pair in the parent Grid. A child
                        // UI here becomes one oversized cell and expands the panel.
                        Self::bounds_fields(
                            ui,
                            language,
                            &mut spec.min,
                            &mut spec.max,
                            spec.contour.is_none(),
                        );
                        ui.label(language.translate("Slope side: left"));
                        ui.checkbox(&mut spec.rise_left, language.translate("Slope side: left"));
                        ui.end_row();
                        ui.label(language.translate("Roof pitch"));
                        ui.add(DragValue::new(&mut spec.pitch).speed(0.1).suffix("°"));
                        ui.end_row();
                        Self::number(ui, language, "Vertical thickness", &mut spec.thickness);
                        Self::number(ui, language, "Pivot offset", &mut spec.pivot_offset);
                    }
                    Shape::Beam {
                        start,
                        end,
                        width,
                        height,
                        top_offset,
                        slant_angle,
                        ..
                    } => {
                        Self::number(ui, language, "Start X", &mut start.x);
                        Self::number(ui, language, "Start Y", &mut start.y);
                        Self::number(ui, language, "End X", &mut end.x);
                        Self::number(ui, language, "End Y", &mut end.y);
                        Self::number(ui, language, "Width", width);
                        Self::number(ui, language, "Height", height);
                        Self::number(ui, language, "Beam top offset", top_offset);
                        let label = ui.label(language.translate("Slant angle"));
                        ui.add(
                            DragValue::new(slant_angle)
                                .range(-89. ..=89.)
                                .speed(0.5)
                                .fixed_decimals(1)
                                .suffix("°"),
                        )
                        .labelled_by(label.id)
                        .on_hover_text(
                            language.translate(
                                "Angle from the start point; positive rises toward the end",
                            ),
                        );
                        ui.end_row();
                    }
                    Shape::Column {
                        center,
                        section,
                        height,
                        bottom_offset,
                        axis_rotation,
                        inclination,
                        ..
                    } => {
                        Self::number(ui, language, "Center X", &mut center.x);
                        Self::number(ui, language, "Center Y", &mut center.y);
                        match section {
                            crate::column::ColumnSection::Rectangular { width, depth } => {
                                Self::number(ui, language, "Width", width);
                                Self::number(ui, language, "Depth", depth);
                            }
                            crate::column::ColumnSection::Circular { diameter } => {
                                Self::number(ui, language, "Diameter", diameter)
                            }
                        }
                        Self::number(ui, language, "Height", height);
                        Self::number(ui, language, "Bottom offset to story", bottom_offset);
                        let mut slanted = inclination.is_some();
                        let label = ui.label(language.translate("Slanted column"));
                        if ui
                            .checkbox(&mut slanted, "")
                            .labelled_by(label.id)
                            .changed()
                        {
                            *inclination =
                                slanted.then_some(crate::column_inclination::ColumnInclination {
                                    slant_degrees: 90.,
                                    direction_degrees: 0.,
                                });
                        }
                        ui.end_row();
                        if let Some(value) = inclination {
                            for (label, angle) in [
                                ("Column inclination", &mut value.slant_degrees),
                                ("Column direction", &mut value.direction_degrees),
                            ] {
                                let label = ui.label(language.translate(label));
                                ui.add(
                                    DragValue::new(angle)
                                        .speed(1.)
                                        .fixed_decimals(1)
                                        .suffix("°"),
                                )
                                .labelled_by(label.id);
                                ui.end_row();
                            }
                        }
                        if !section.is_circular() {
                            let label = ui.label(language.translate("Axis rotation"));
                            ui.add(
                                DragValue::new(axis_rotation)
                                    .speed(1.)
                                    .fixed_decimals(1)
                                    .suffix("°"),
                            )
                            .labelled_by(label.id);
                            ui.end_row();
                        }
                    }
                    Shape::Wall {
                        start,
                        end,
                        thickness,
                        height,
                    } => {
                        Self::number(ui, language, "Start X", &mut start.x);
                        Self::number(ui, language, "Start Y", &mut start.y);
                        Self::number(ui, language, "End X", &mut end.x);
                        Self::number(ui, language, "End Y", &mut end.y);
                        Self::number(ui, language, "Thickness", thickness);
                        Self::number(ui, language, "Height", height);
                    }
                    Shape::Slab {
                        min,
                        max,
                        thickness,
                    } => {
                        Self::bounds_fields(ui, language, min, max, element.slab_contour.is_none());
                        Self::number(ui, language, "Thickness", thickness);
                    }
                    Shape::Stair { spec } => {
                        Self::number(ui, language, "Start X", &mut spec.start.x);
                        Self::number(ui, language, "Start Y", &mut spec.start.y);
                        Self::number(ui, language, "End X", &mut spec.end.x);
                        Self::number(ui, language, "End Y", &mut spec.end.y);
                        Self::number(ui, language, "Width", &mut spec.width);
                        Self::number(ui, language, "Total rise", &mut spec.total_rise);
                        ui.label(language.translate("Risers"));
                        ui.add(DragValue::new(&mut spec.risers).range(2..=100));
                        ui.end_row();
                        Self::number(ui, language, "Slab thickness", &mut spec.slab_thickness);
                        Self::number(ui, language, "Handrail height", &mut spec.handrail_height);
                    }
                    Shape::Polyline { points, .. } => {
                        ui.label(language.translate("Nodes"));
                        ui.label(points.len().to_string());
                        ui.end_row();
                        for (index, point) in points.iter_mut().enumerate() {
                            Self::number(
                                ui,
                                language,
                                &format!("{} {} X", language.translate("Node"), index + 1),
                                &mut point.x,
                            );
                            Self::number(
                                ui,
                                language,
                                &format!("{} {} Y", language.translate("Node"), index + 1),
                                &mut point.y,
                            );
                        }
                    }
                    Shape::Opening {
                        kind,
                        host_wall,
                        width,
                        height,
                        sill,
                        offset,
                        ..
                    } => {
                        let joinery = element.opening_joinery.get_or_insert_with(Default::default);
                        if *kind == OpeningKind::Door {
                            Self::opening_joinery_controls(ui, language, joinery);
                            ui.end_row();
                        }
                        Self::number(ui, language, "Frame width", &mut joinery.frame_width);
                        Self::number(ui, language, "Frame depth", &mut joinery.frame_depth);
                        if *kind == OpeningKind::Door {
                            Self::number(
                                ui,
                                language,
                                "Leaf thickness",
                                &mut joinery.leaf_thickness,
                            );
                        }
                        ui.label(language.translate("Host Wall"));
                        ui.label(format!("{host_wall:03}"));
                        ui.end_row();
                        Self::number(ui, language, "Width", width);
                        Self::number(ui, language, "Height", height);
                        Self::number(ui, language, "Sill Height", sill);
                        Self::number(ui, language, "Offset", offset);
                    }
                }
            });
        if original_joinery.is_none() && element.opening_joinery == Some(Default::default()) {
            element.opening_joinery = None;
        }
    }
    pub(super) fn library(&mut self, ui: &mut Ui) {
        Self::label(ui, "PARAMETRIC PRESETS");
        ui.add_space(8.);
        ui.add(
            TextEdit::singleline(&mut self.library_search)
                .hint_text(self.language.translate("Search wall or slab…"))
                .desired_width(f32::INFINITY),
        );
        ui.add_space(10.);
        for (name, desc, tool, material, t, h) in [
            (
                "Exterior / plaster",
                "220 mm · 3.00 m high",
                Tool::Wall,
                Material::Plaster,
                0.22,
                3.,
            ),
            (
                "Partition / plaster",
                "120 mm · 2.80 m high",
                Tool::Wall,
                Material::Plaster,
                0.12,
                2.8,
            ),
            (
                "Structure / concrete",
                "300 mm · 3.20 m high",
                Tool::Wall,
                Material::Concrete,
                0.3,
                3.2,
            ),
            (
                "Floor / concrete",
                "280 mm structural slab",
                Tool::Slab,
                Material::Concrete,
                0.28,
                3.,
            ),
            (
                "Terrace / timber",
                "160 mm deck slab",
                Tool::Slab,
                Material::Timber,
                0.16,
                3.,
            ),
        ] {
            if !format!(
                "{name} {desc} {tool:?} {} {}",
                self.language.translate(name),
                self.language.translate(desc)
            )
            .to_lowercase()
            .contains(&self.library_search.to_lowercase())
            {
                continue;
            }
            Frame::new()
                .fill(PANEL_ALT)
                .corner_radius(6)
                .inner_margin(12)
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    let (r, _) =
                        ui.allocate_exact_size(vec2(ui.available_width(), 48.), Sense::hover());
                    let c = material.color();
                    let col = Color32::from_rgb(
                        (c[0] * 255.) as u8,
                        (c[1] * 255.) as u8,
                        (c[2] * 255.) as u8,
                    );
                    let r = Rect::from_center_size(
                        r.center(),
                        if tool == Tool::Wall {
                            vec2(96., 32.)
                        } else {
                            vec2(140., 15.)
                        },
                    );
                    ui.painter().rect_filled(r, 1., col);
                    ui.painter()
                        .rect_stroke(r, 1., Stroke::new(1., INK), StrokeKind::Inside);
                    ui.strong(self.language.translate(name));
                    ui.label(
                        RichText::new(self.language.translate(desc))
                            .small()
                            .color(MUTED),
                    );
                    if ui.button(self.language.translate("Use preset")).clicked() {
                        self.host.document.editor.set_tool(tool);
                        self.preview_tool = None;
                        self.host.document.editor.material = material;
                        self.host.document.editor.thickness = t;
                        self.host.document.editor.height = h;
                        self.host.document.editor.status =
                            format!("{name} preset active · Click in plan to place");
                    }
                });
            ui.add_space(8.);
        }
        ui.label(
            RichText::new(self.language.translate(
                "Built-in wall / slab presets. External object catalogs are not connected.",
            ))
            .small()
            .color(MUTED),
        );
    }
    pub(super) fn assistant(&mut self, ui: &mut Ui) {
        if self
            .research_ui
            .review
            .as_ref()
            .is_some_and(|r| r.session != self.host.document.editor.document_session())
        {
            self.research_ui.review = None;
        }
        let object_interaction = self.research_ui.draft.is_some()
            || self.research_ui.parameters.is_some()
            || self.research_ui.canvas_move.is_some()
            || self.object_review_active()
            || !self.research_ui.error.is_empty();
        if object_interaction {
            self.object_research_contents(ui);
            self.drawing_input_selector(ui);
        } else {
            self.drawing_input_selector(ui);
            self.object_research_contents(ui);
        }
        ui.collapsing(
            self.language.translate("Global local-demo commands"),
            |ui| self.legacy_assistant(ui),
        );
    }
    fn legacy_assistant(&mut self, ui: &mut Ui) {
        Self::label(
            ui,
            self.language.translate("DESIGN ASSISTANT  /  LOCAL DEMO"),
        );
        ui.add_space(8.);
        let mut prompt = None;
        ui.horizontal_wrapped(|ui| {
            for label in ["Summary", "Quantities", "Check model"] {
                if ui.small_button(self.language.translate(label)).clicked() {
                    prompt = Some(label.to_string());
                }
            }
        });
        ui.add_space(8.);
        for (user, text) in &self.messages {
            Frame::new()
                .fill(if *user {
                    Color32::from_rgb(45, 83, 99)
                } else {
                    PANEL_ALT
                })
                .corner_radius(6)
                .inner_margin(12)
                .show(ui, |ui| {
                    Self::label(
                        ui,
                        if *user {
                            "YOU"
                        } else {
                            self.language.translate("OPENARCHI · LOCAL RULES")
                        },
                    );
                    ui.label(text);
                });
            ui.add_space(8.);
        }
        ui.add(
            TextEdit::multiline(&mut self.assistant_input)
                .desired_rows(3)
                .desired_width(f32::INFINITY)
                .hint_text(self.language.translate("Ask about this model…")),
        );
        if ui.button(self.language.translate("Send")).clicked()
            && !self.assistant_input.trim().is_empty()
        {
            prompt = Some(std::mem::take(&mut self.assistant_input));
        }
        if let Some(q) = prompt {
            let lower = q.to_lowercase();
            let p = &self.host.document.editor.project;
            let answer = if lower.contains("check") || lower.contains("检查") {
                match p.validate() {
                    Ok(()) => format!(
                        "Geometry validation passed for {} elements.\n\nChecks: finite coordinates, positive dimensions, unique IDs and story references.\n\nThis is not a building-code or clash check.",
                        p.elements.len()
                    ),
                    Err(e) => e,
                }
            } else if lower.contains("quant") || lower.contains("数量") {
                let area: f64 = p
                    .elements
                    .iter()
                    .filter(|e| matches!(e.shape, crate::model::Shape::Slab { .. }))
                    .map(Element::area)
                    .sum();
                let length: f64 = p
                    .elements
                    .iter()
                    .map(|e| match e.shape {
                        crate::model::Shape::Wall { start, end, .. } => start.distance(end),
                        _ => 0.,
                    })
                    .sum();
                format!(
                    "Wall centerline length: {length:.2} m\nSlab footprint total: {area:.2} m²\n\nGross quantities; intersections and openings are not deducted."
                )
            } else if lower.contains("summary") || lower.contains("总结") {
                format!(
                    "{}\n{} stories · {} model elements\n\nSelected story: Level {:02}\nBoth views share one in-memory model. Save JSON to preserve this design.",
                    p.name,
                    p.stories.len(),
                    p.elements.len(),
                    self.host.document.editor.story
                )
            } else {
                "Available local commands: Summary, Quantities, Check model.\n\nFree-form AI design and model generation require a future provider connection.".into()
            };
            self.messages.push((true, q));
            self.messages.push((false, answer));
        }
    }
}
