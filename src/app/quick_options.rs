use super::*;

impl AtelierApp {
    pub(super) fn set_plan_zoom_percent(&mut self, percent: f32) {
        let percent = percent.clamp(1.0, 1000.0);
        self.plan_scale = (40.0 * percent / 100.0).clamp(0.4, 400.0);
        self.editor.status = format!("{} · {percent:.1}%", self.language.translate("Zoom"));
    }

    pub(super) fn plan_zoom_percent(&self) -> f32 {
        self.plan_scale / 40.0 * 100.0
    }

    pub(super) fn toggle_grid_snap(&mut self) {
        let enabled = !self.editor.project.grid_settings.grid_snap_enabled;
        self.set_grid_snap_enabled(enabled);
    }

    pub(super) fn set_grid_snap_enabled(&mut self, enabled: bool) {
        let settings = &mut self.editor.project.grid_settings;
        settings.set_snap_enabled(enabled);
        self.editor.status = if settings.grid_snap_enabled {
            format!(
                "{} · {}",
                self.language.translate("Grid Snap on"),
                self.language.translate(settings.snap_mode.label_key())
            )
        } else {
            self.language.translate("Grid Snap off").into()
        };
    }

    pub(super) fn modify_plan_options<F>(&mut self, change: F, status: impl Into<String>) -> bool
    where
        F: FnOnce(&mut PlanOptions),
    {
        let before = self.editor.project.clone();
        change(&mut self.editor.project.plan_options);
        if let Err(error) = self.editor.project.plan_options.validate() {
            self.editor.project.plan_options = before.plan_options;
            self.editor.status = error;
            return false;
        }
        if self.editor.project.plan_options == before.plan_options {
            return false;
        }
        self.editor.commit(before);
        self.editor.status = status.into();
        true
    }

    pub(super) fn set_layer_combination(&mut self, combination: LayerCombination) {
        let status = self.option_status("Layer Combination", combination.name());
        self.modify_plan_options(
            |options| {
                options.layer_combination = combination;
                options.show_all_layers = false;
            },
            status,
        );
    }

    pub(super) fn set_show_all_layers(&mut self, show: bool) {
        let status = self.option_status(
            "Show All Layers",
            if show {
                self.language.translate("On")
            } else {
                self.language.translate("Off")
            },
        );
        self.modify_plan_options(|options| options.show_all_layers = show, status);
    }

    pub(super) fn set_pen_set(&mut self, pen_set: PenSet) {
        let status = self.option_status("Pen Set", pen_set.name());
        self.modify_plan_options(|options| options.pen_set = pen_set, status);
    }

    pub(super) fn set_partial_structure(&mut self, display: PartialStructureDisplay) {
        let status = self.option_status("Partial Structure", display.name());
        self.modify_plan_options(|options| options.partial_structure = display, status);
    }

    pub(super) fn set_model_view(&mut self, model_view: ModelViewOptions) {
        let status = self.option_status("Model View Options", model_view.name());
        self.modify_plan_options(|options| options.model_view = model_view, status);
    }

    pub(super) fn set_graphic_overrides(&mut self, overrides: GraphicOverrides) {
        let status = self.option_status("Graphic Overrides", overrides.name());
        self.modify_plan_options(|options| options.graphic_overrides = overrides, status);
    }

    pub(super) fn set_renovation_filter(&mut self, filter: RenovationFilter) {
        let status = self.option_status("Renovation Filter", filter.name());
        self.modify_plan_options(|options| options.renovation_filter = filter, status);
    }

    pub(super) fn set_dimension_style(&mut self, style: DimensionStyle) {
        let status = self.option_status("Dimensions", style.name());
        self.modify_plan_options(|options| options.dimension_style = style, status);
    }

    pub(super) fn set_drawing_scale(&mut self, denominator: u32) {
        let status = format!(
            "{} · 1:{denominator}",
            self.language.translate("Floor Plan Scale")
        );
        self.modify_plan_options(
            |options| options.drawing_scale = denominator.clamp(1, 10_000),
            status,
        );
    }

    pub(super) fn set_orientation_degrees(&mut self, degrees: f32) {
        let degrees = degrees.rem_euclid(360.0);
        let status = format!("{} · {degrees:.2}°", self.language.translate("Orientation"));
        self.modify_plan_options(|options| options.orientation_degrees = degrees, status);
    }

    pub(super) fn set_active_design_option(&mut self, option: Option<u32>) {
        let label = option
            .and_then(|id| self.editor.project.design_options.name(id))
            .map_or_else(
                || self.language.translate("Main Model Only").to_owned(),
                str::to_owned,
            );
        let status = format!("{} · {label}", self.language.translate("Design Options"));
        self.modify_plan_options(|options| options.active_design_option = option, status);
    }

    pub(super) fn begin_orientation_pick(&mut self) {
        self.orientation_pick_active = true;
        self.orientation_pick_start = None;
        self.editor.status = self
            .language
            .translate("Set Orientation Graphically · click two points")
            .into();
    }

    pub(super) fn cancel_orientation_pick(&mut self) {
        self.orientation_pick_active = false;
        self.orientation_pick_start = None;
        self.editor.status = self.language.translate("Set Orientation cancelled").into();
    }

    pub(super) fn reset_renovation_status(&mut self) {
        let before = self.editor.project.clone();
        let selection: Vec<u64> = self.editor.selection_ids();
        let mut changed = 0;
        for element in &mut self.editor.project.elements {
            if (selection.is_empty() || selection.contains(&element.id))
                && element.renovation != RenovationStatus::Existing
            {
                element.renovation = RenovationStatus::Existing;
                changed += 1;
            }
        }
        if changed == 0 {
            self.editor.status = self
                .language
                .translate("Renovation status already Existing")
                .into();
            return;
        }
        self.editor.commit(before);
        self.editor.status = format!(
            "{} · {} {}",
            self.language.translate("Renovation status reset"),
            changed,
            self.language
                .translate(if changed == 1 { "element" } else { "elements" })
        );
    }

    pub(super) fn open_plan_option_dialog(&mut self, kind: PlanOptionKind) {
        self.plan_dialog_options = self.editor.project.plan_options.clone();
        self.plan_option_dialog = Some(kind);
    }

    pub(super) fn plan_option_title(language: UiLanguage, kind: PlanOptionKind) -> &'static str {
        language.translate(match kind {
            PlanOptionKind::Layers => "Layers",
            PlanOptionKind::PenSets => "Pen Sets",
            PlanOptionKind::PartialStructure => "Partial Structure Display",
            PlanOptionKind::ModelView => "Model View Options",
            PlanOptionKind::GraphicOverrides => "Graphic Overrides",
            PlanOptionKind::Renovation => "Renovation",
            PlanOptionKind::Dimensions => "Dimensions",
            PlanOptionKind::DesignOptions => "Design Options",
            PlanOptionKind::Scale => "Floor Plan Scale",
        })
    }

    pub(super) fn option_status(&self, label: &str, value: &str) -> String {
        format!(
            "{} · {}",
            self.language.translate(label),
            self.language.translate(value)
        )
    }

    pub(super) fn option_menu_list<T: Copy + PartialEq>(
        ui: &mut Ui,
        current: T,
        all: &[T],
        label: impl Fn(T) -> &'static str,
    ) -> Option<T> {
        let mut picked = None;
        for &item in all {
            if ui.selectable_label(current == item, label(item)).clicked() {
                picked = Some(item);
                ui.close();
            }
        }
        picked
    }

    pub(super) fn elide_text(text: &str, max_chars: usize) -> String {
        if text.chars().count() <= max_chars {
            return text.to_owned();
        }
        let mut out: String = text.chars().take(max_chars.saturating_sub(1)).collect();
        out.push('…');
        out
    }

    pub(super) fn quick_option_button(
        ui: &mut Ui,
        icon: &str,
        label: &str,
        min_width: f32,
        max_width: f32,
        tip: &str,
    ) -> Response {
        let font = FontId::proportional(11.0);
        let max_chars = ((max_width - 40.) / 6.0).max(4.) as usize;
        let display = Self::elide_text(label, max_chars);
        let text_width = ui
            .painter()
            .layout_no_wrap(display.clone(), font.clone(), INK)
            .size()
            .x;
        let width = (text_width + 40.).clamp(min_width, max_width);
        let (rect, response) = ui.allocate_exact_size(vec2(width, 26.), Sense::click());
        if response.hovered() {
            ui.painter().rect_filled(rect, 2., PANEL_ALT);
        }
        Self::paint_tool_icon(
            ui.painter(),
            Rect::from_min_size(rect.left_top() + vec2(4., 5.), vec2(16., 16.)),
            icon,
            if response.hovered() { ACCENT } else { INK },
        );
        ui.painter().text(
            rect.left_center() + vec2(23., 0.),
            Align2::LEFT_CENTER,
            display,
            font,
            INK,
        );
        ui.painter().text(
            rect.right_center() - vec2(5., 0.),
            Align2::RIGHT_CENTER,
            "›",
            FontId::proportional(13.),
            MUTED,
        );
        response.on_hover_text(tip)
    }

    pub(super) fn zoom_menu_contents(&mut self, ui: &mut Ui) {
        let language = self.language;
        if ui
            .button(language.translate("Save Current Zoom…"))
            .clicked()
        {
            let name = format!(
                "{} {}",
                language.translate("Zoom"),
                self.saved_zooms.len() + 1
            );
            self.saved_zooms.push((name.clone(), self.plan_scale));
            self.editor.status = format!("{} · {name}", language.translate("Saved zoom"));
            ui.close();
        }
        ui.add_enabled(false, Button::new(language.translate("Rename …")));
        ui.add_enabled(false, Button::new(language.translate("Delete")));
        ui.separator();
        for percent in ZOOM_PRESETS {
            let selected = (self.plan_zoom_percent() - percent as f32).abs() < 0.05;
            if ui
                .selectable_label(selected, format!("{percent}%"))
                .clicked()
            {
                self.set_plan_zoom_percent(percent as f32);
                ui.close();
            }
        }
        if !self.saved_zooms.is_empty() {
            ui.separator();
            let saved = self.saved_zooms.clone();
            for (name, scale) in saved {
                let percent = scale / 40.0 * 100.0;
                if ui
                    .selectable_label(false, format!("{name} · {percent:.1}%"))
                    .clicked()
                {
                    self.plan_scale = scale;
                    self.editor.status = format!("{} · {name}", language.translate("Zoom"));
                    ui.close();
                }
            }
        }
    }

    pub(super) fn orientation_menu_contents(&mut self, ui: &mut Ui) {
        let language = self.language;
        if ui
            .button(language.translate("Set Orientation Graphically…"))
            .clicked()
        {
            self.begin_orientation_pick();
            ui.close();
        }
        ui.separator();
        let current = self.editor.project.plan_options.normalized_orientation();
        for degrees in ORIENTATION_PRESETS {
            if ui
                .selectable_label((current - degrees).abs() < 0.005, format!("{degrees:.2}°"))
                .clicked()
            {
                self.set_orientation_degrees(degrees);
                ui.close();
            }
        }
    }

    pub(super) fn scale_menu_contents(&mut self, ui: &mut Ui) {
        let language = self.language;
        if ui.button(language.translate("Custom Scale…")).clicked() {
            self.open_plan_option_dialog(PlanOptionKind::Scale);
            ui.close();
        }
        ui.separator();
        let current = self.editor.project.plan_options.drawing_scale;
        for preset in SCALE_PRESETS {
            if ui
                .selectable_label(current == preset, format!("1:{preset}"))
                .clicked()
            {
                self.set_drawing_scale(preset);
                ui.close();
            }
        }
    }

    pub(super) fn layers_menu_contents(&mut self, ui: &mut Ui) {
        let language = self.language;
        if ui
            .button(language.translate("Layers (Model Views)…"))
            .clicked()
        {
            self.open_plan_option_dialog(PlanOptionKind::Layers);
            ui.close();
        }
        let mut show_all = self.editor.project.plan_options.show_all_layers;
        if ui
            .checkbox(&mut show_all, language.translate("Show All Layers"))
            .changed()
        {
            self.set_show_all_layers(show_all);
        }
        ui.add_enabled(false, Button::new(language.translate("Layer Extras")));
        ui.separator();
        let current = self.editor.project.plan_options.layer_combination;
        if let Some(combination) =
            Self::option_menu_list(ui, current, &LayerCombination::ALL, |value| {
                language.translate(value.name())
            })
        {
            self.set_layer_combination(combination);
        }
    }

    pub(super) fn partial_structure_menu_contents(&mut self, ui: &mut Ui) {
        let language = self.language;
        if ui
            .button(language.translate("Partial Structure Display…"))
            .clicked()
        {
            self.open_plan_option_dialog(PlanOptionKind::PartialStructure);
            ui.close();
        }
        ui.separator();
        let current = self.editor.project.plan_options.partial_structure;
        for option in PartialStructureDisplay::ALL {
            if ui
                .radio(current == option, language.translate(option.name()))
                .clicked()
            {
                self.set_partial_structure(option);
                ui.close();
            }
        }
    }

    pub(super) fn pen_set_menu_contents(&mut self, ui: &mut Ui) {
        let language = self.language;
        if ui
            .button(language.translate("Pens & Color (Model Views)…"))
            .clicked()
        {
            self.open_plan_option_dialog(PlanOptionKind::PenSets);
            ui.close();
        }
        ui.separator();
        let current = self.editor.project.plan_options.pen_set;
        if let Some(pen_set) = Self::option_menu_list(ui, current, &PenSet::ALL, |value| {
            language.translate(value.name())
        }) {
            self.set_pen_set(pen_set);
        }
    }

    pub(super) fn model_view_menu_contents(&mut self, ui: &mut Ui) {
        let language = self.language;
        if ui
            .button(language.translate("Model View Options…"))
            .clicked()
        {
            self.open_plan_option_dialog(PlanOptionKind::ModelView);
            ui.close();
        }
        ui.separator();
        let current = self.editor.project.plan_options.model_view;
        if let Some(model_view) =
            Self::option_menu_list(ui, current, &ModelViewOptions::ALL, |value| {
                language.translate(value.name())
            })
        {
            self.set_model_view(model_view);
        }
    }

    pub(super) fn graphic_overrides_menu_contents(&mut self, ui: &mut Ui) {
        let language = self.language;
        if ui
            .button(language.translate("Graphic Override Combinations…"))
            .clicked()
        {
            self.open_plan_option_dialog(PlanOptionKind::GraphicOverrides);
            ui.close();
        }
        ui.separator();
        let current = self.editor.project.plan_options.graphic_overrides;
        if let Some(overrides) =
            Self::option_menu_list(ui, current, &GraphicOverrides::ALL, |value| {
                language.translate(value.name())
            })
        {
            self.set_graphic_overrides(overrides);
        }
        ui.separator();
        if ui
            .button(language.translate("Graphic Override Rules…"))
            .clicked()
        {
            self.editor.status = language
                .translate("Graphic Override Rules · interface preview")
                .into();
            ui.close();
        }
    }

    pub(super) fn renovation_menu_contents(&mut self, ui: &mut Ui) {
        let language = self.language;
        if ui
            .button(language.translate("Renovation Filter Options…"))
            .clicked()
        {
            self.open_plan_option_dialog(PlanOptionKind::Renovation);
            ui.close();
        }
        ui.separator();
        let current = self.editor.project.plan_options.renovation_filter;
        if let Some(filter) = Self::option_menu_list(ui, current, &RenovationFilter::ALL, |value| {
            language.translate(value.name())
        }) {
            self.set_renovation_filter(filter);
        }
        ui.separator();
        if ui
            .button(language.translate("Reset Renovation Status…"))
            .clicked()
        {
            self.reset_renovation_status();
            ui.close();
        }
    }

    pub(super) fn design_options_menu_contents(&mut self, ui: &mut Ui) {
        let language = self.language;
        if ui
            .button(language.translate("Design Option Manager"))
            .clicked()
        {
            self.open_plan_option_dialog(PlanOptionKind::DesignOptions);
            ui.close();
        }
        ui.separator();
        let active = self.editor.project.plan_options.active_design_option;
        if ui
            .radio(active.is_none(), language.translate("Main Model Only"))
            .clicked()
        {
            self.set_active_design_option(None);
            ui.close();
        }
        let options: Vec<(u32, String)> = self
            .editor
            .project
            .design_options
            .options
            .iter()
            .map(|option| (option.id, option.name.clone()))
            .collect();
        if !options.is_empty() {
            ui.separator();
            for (id, name) in options {
                if ui.radio(active == Some(id), name).clicked() {
                    self.set_active_design_option(Some(id));
                    ui.close();
                }
            }
        }
    }

    pub(super) fn dimensions_menu_contents(&mut self, ui: &mut Ui) {
        let language = self.language;
        if ui.button(language.translate("Dimensions…")).clicked() {
            self.open_plan_option_dialog(PlanOptionKind::Dimensions);
            ui.close();
        }
        ui.separator();
        let current = self.editor.project.plan_options.dimension_style;
        if let Some(style) = Self::option_menu_list(ui, current, &DimensionStyle::ALL, |value| {
            language.translate(value.name())
        }) {
            self.set_dimension_style(style);
        }
    }

    pub(super) fn toggle_quick_options_bar(&mut self) {
        self.quick_options_visible = !self.quick_options_visible;
        self.editor.status = if self.quick_options_visible {
            self.language.translate("Quick Options bar shown").into()
        } else {
            self.language.translate("Quick Options bar hidden").into()
        };
    }

    pub(super) fn toggle_quick_options_palette(&mut self) {
        self.quick_options_palette_visible = !self.quick_options_palette_visible;
        self.editor.status = if self.quick_options_palette_visible {
            self.language
                .translate("Quick Options palette shown")
                .into()
        } else {
            self.language
                .translate("Quick Options palette hidden")
                .into()
        };
    }

    pub(super) fn quick_options_bar(&mut self, ui: &mut Ui) {
        let language = self.language;
        egui::Panel::bottom("quick_options_bar")
            .frame(
                Frame::new()
                    .fill(CHROME)
                    .inner_margin(Margin::symmetric(2, 1)),
            )
            .show(ui, |ui| {
                ScrollArea::horizontal()
                    .id_salt("quick_options_scroll")
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 1.0;
                            let zoom = format!("{:.1}%", self.plan_zoom_percent());
                            let response = Self::quick_option_button(
                                ui,
                                "Zoom In",
                                &zoom,
                                64.,
                                84.,
                                &format!("{} · {zoom}", language.translate("Zoom")),
                            );
                            Popup::menu(&response).show(|ui| self.zoom_menu_contents(ui));
                            ui.separator();
                            let orientation = format!(
                                "{:.2}°",
                                self.editor.project.plan_options.normalized_orientation()
                            );
                            let response = Self::quick_option_button(
                                ui,
                                "Orientation",
                                &orientation,
                                78.,
                                96.,
                                &format!("{} · {orientation}", language.translate("Orientation")),
                            );
                            Popup::menu(&response).show(|ui| self.orientation_menu_contents(ui));
                            ui.separator();
                            let scale =
                                format!("1:{}", self.editor.project.plan_options.drawing_scale);
                            let response = Self::quick_option_button(
                                ui,
                                "Scale",
                                &scale,
                                62.,
                                80.,
                                &format!("{} · {scale}", language.translate("Scale")),
                            );
                            Popup::menu(&response).show(|ui| self.scale_menu_contents(ui));
                            ui.separator();
                            let layer_label = if self.editor.project.plan_options.show_all_layers {
                                language.translate("All Layers").to_owned()
                            } else {
                                let name =
                                    self.editor.project.plan_options.layer_combination.name();
                                language.translate(name).to_owned()
                            };
                            let response = Self::quick_option_button(
                                ui,
                                "Layer Combination",
                                &layer_label,
                                120.,
                                158.,
                                &format!(
                                    "{} · {layer_label}",
                                    language.translate("Layer Combination")
                                ),
                            );
                            Popup::menu(&response).show(|ui| self.layers_menu_contents(ui));
                            ui.separator();
                            let partial = language.translate(
                                self.editor.project.plan_options.partial_structure.name(),
                            );
                            let response = Self::quick_option_button(
                                ui,
                                "Partial Structure",
                                partial,
                                104.,
                                132.,
                                &format!("{} · {partial}", language.translate("Partial Structure")),
                            );
                            Popup::menu(&response)
                                .show(|ui| self.partial_structure_menu_contents(ui));
                            ui.separator();
                            let pen =
                                language.translate(self.editor.project.plan_options.pen_set.name());
                            let response = Self::quick_option_button(
                                ui,
                                "Pen Set",
                                pen,
                                116.,
                                148.,
                                &format!("{} · {pen}", language.translate("Pen Set")),
                            );
                            Popup::menu(&response).show(|ui| self.pen_set_menu_contents(ui));
                            ui.separator();
                            let model_view = language
                                .translate(self.editor.project.plan_options.model_view.name());
                            let response = Self::quick_option_button(
                                ui,
                                "Model View",
                                model_view,
                                112.,
                                142.,
                                &format!(
                                    "{} · {model_view}",
                                    language.translate("Model View Options")
                                ),
                            );
                            Popup::menu(&response).show(|ui| self.model_view_menu_contents(ui));
                            ui.separator();
                            let overrides = language.translate(
                                self.editor.project.plan_options.graphic_overrides.name(),
                            );
                            let response = Self::quick_option_button(
                                ui,
                                "Graphic Overrides",
                                overrides,
                                108.,
                                142.,
                                &format!(
                                    "{} · {overrides}",
                                    language.translate("Graphic Overrides")
                                ),
                            );
                            Popup::menu(&response)
                                .show(|ui| self.graphic_overrides_menu_contents(ui));
                            ui.separator();
                            let renovation = language.translate(
                                self.editor.project.plan_options.renovation_filter.name(),
                            );
                            let response = Self::quick_option_button(
                                ui,
                                "Renovation",
                                renovation,
                                112.,
                                146.,
                                &format!(
                                    "{} · {renovation}",
                                    language.translate("Renovation Filter")
                                ),
                            );
                            Popup::menu(&response).show(|ui| self.renovation_menu_contents(ui));
                            ui.separator();
                            let design = self
                                .editor
                                .project
                                .plan_options
                                .active_design_option
                                .and_then(|id| self.editor.project.design_options.name(id))
                                .unwrap_or(language.translate("Main Model Only"));
                            let response = Self::quick_option_button(
                                ui,
                                "Design Options",
                                design,
                                108.,
                                142.,
                                &format!("{} · {design}", language.translate("Design Options")),
                            );
                            Popup::menu(&response).show(|ui| self.design_options_menu_contents(ui));
                            ui.separator();
                            let dimensions = language
                                .translate(self.editor.project.plan_options.dimension_style.name());
                            let response = Self::quick_option_button(
                                ui,
                                "Dimension Style",
                                dimensions,
                                86.,
                                116.,
                                &format!("{} · {dimensions}", language.translate("Dimensions")),
                            );
                            Popup::menu(&response).show(|ui| self.dimensions_menu_contents(ui));
                        });
                    });
            });
    }

    pub(super) fn palette_row(
        ui: &mut Ui,
        label: &str,
        value: String,
        contents: impl FnOnce(&mut Ui),
    ) {
        ui.horizontal(|ui| {
            ui.label(RichText::new(label).size(10.).color(MUTED));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.menu_button(value, contents);
            });
        });
    }

    pub(super) fn quick_options_palette(&mut self, ctx: &Context) {
        if !self.quick_options_palette_visible {
            return;
        }
        let mut open = self.quick_options_palette_visible;
        let language = self.language;
        egui::Window::new(Self::t(language, "Quick Options"))
            .id(Id::new("quick_options_palette_window"))
            .default_pos(pos2(1180., 520.))
            .default_width(268.)
            .resizable(true)
            .open(&mut open)
            .show(ctx, |ui| {
                ui.label(
                    RichText::new(language.translate("Floor Plan"))
                        .size(10.)
                        .strong()
                        .color(MUTED),
                );
                ui.separator();
                let zoom = format!("{:.1}%", self.plan_zoom_percent());
                Self::palette_row(ui, language.translate("Zoom"), zoom, |ui| {
                    self.zoom_menu_contents(ui)
                });
                let orientation = format!(
                    "{:.2}°",
                    self.editor.project.plan_options.normalized_orientation()
                );
                Self::palette_row(ui, language.translate("Orientation"), orientation, |ui| {
                    self.orientation_menu_contents(ui)
                });
                let scale = format!("1:{}", self.editor.project.plan_options.drawing_scale);
                Self::palette_row(ui, language.translate("Scale"), scale, |ui| {
                    self.scale_menu_contents(ui)
                });
                let layer = if self.editor.project.plan_options.show_all_layers {
                    language.translate("All Layers").to_owned()
                } else {
                    let name = self.editor.project.plan_options.layer_combination.name();
                    language.translate(name).to_owned()
                };
                Self::palette_row(ui, language.translate("Layer Combination"), layer, |ui| {
                    self.layers_menu_contents(ui)
                });
                let partial = language
                    .translate(self.editor.project.plan_options.partial_structure.name())
                    .to_owned();
                Self::palette_row(ui, language.translate("Partial Structure"), partial, |ui| {
                    self.partial_structure_menu_contents(ui)
                });
                let pen = language
                    .translate(self.editor.project.plan_options.pen_set.name())
                    .to_owned();
                Self::palette_row(ui, language.translate("Pen Set"), pen, |ui| {
                    self.pen_set_menu_contents(ui)
                });
                let model_view = language
                    .translate(self.editor.project.plan_options.model_view.name())
                    .to_owned();
                Self::palette_row(
                    ui,
                    language.translate("Model View Options"),
                    model_view,
                    |ui| self.model_view_menu_contents(ui),
                );
                let overrides = language
                    .translate(self.editor.project.plan_options.graphic_overrides.name())
                    .to_owned();
                Self::palette_row(
                    ui,
                    language.translate("Graphic Overrides"),
                    overrides,
                    |ui| self.graphic_overrides_menu_contents(ui),
                );
                let renovation = language
                    .translate(self.editor.project.plan_options.renovation_filter.name())
                    .to_owned();
                Self::palette_row(
                    ui,
                    language.translate("Renovation Filter"),
                    renovation,
                    |ui| self.renovation_menu_contents(ui),
                );
                let design = self
                    .editor
                    .project
                    .plan_options
                    .active_design_option
                    .and_then(|id| self.editor.project.design_options.name(id))
                    .unwrap_or(language.translate("Main Model Only"))
                    .to_owned();
                Self::palette_row(ui, language.translate("Design Options"), design, |ui| {
                    self.design_options_menu_contents(ui)
                });
                let dimensions = language
                    .translate(self.editor.project.plan_options.dimension_style.name())
                    .to_owned();
                Self::palette_row(ui, language.translate("Dimensions"), dimensions, |ui| {
                    self.dimensions_menu_contents(ui)
                });
                ui.separator();
                if ui
                    .button(language.translate("Hide Quick Options Bar"))
                    .clicked()
                {
                    self.quick_options_visible = false;
                    self.editor.status = language.translate("Quick Options bar hidden").into();
                }
            });
        self.quick_options_palette_visible = open;
    }
}
