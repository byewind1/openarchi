use super::*;

impl AtelierApp {
    pub(super) fn inspector(&mut self, ui: &mut Ui) {
        egui::Panel::right("inspector")
            .default_size(318.)
            .size_range(285.0..=440.)
            .frame(
                Frame::new()
                    .fill(PANEL)
                    .inner_margin(Margin::symmetric(9, 8)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    Self::label(ui, &Self::t(self.language, "NAVIGATOR"));
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
                match self.navigator_view {
                    NavigatorView::ProjectMap => {
                        ui.horizontal(|ui| {
                            Self::inline_vector_icon(ui, "Home", ACCENT);
                            ui.strong(&self.editor.project.name);
                        });
                        CollapsingHeader::new(Self::t(self.language, "Stories"))
                            .default_open(true)
                            .show(ui, |ui| {
                                for story in self.editor.project.stories.clone() {
                                    let selected = self.editor.story == story.id;
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
                                        self.editor.switch_story(story.id);
                                        self.draft_id = None;
                                    }
                                }
                            });
                        for node in [
                            "Sections",
                            "Elevations",
                            "Interior Elevations",
                            "Worksheets",
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
                                    "Project Indexes" => {
                                        &["Change List", "Drawing List", "Sheet Index"]
                                    }
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
                                        self.editor.status = format!("Navigator item · {entry}");
                                    }
                                }
                            });
                        }
                    }
                    NavigatorView::ViewMap => {
                        ui.strong(Self::t(self.language, "View Map"));
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
                                            self.editor.status = format!("View Map · {entry}");
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
                                    for entry in
                                        ["A-001 Cover", "A-101 Floor Plan", "A-201 Elevations"]
                                    {
                                        if Self::navigator_item(
                                            ui,
                                            "Layout Book",
                                            &Self::t(self.language, entry),
                                        ) {
                                            self.editor.status = format!("Layout · {entry}");
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
                                    for entry in
                                        ["Current Project", "Documentation Set", "3D Model"]
                                    {
                                        if Self::navigator_item(
                                            ui,
                                            "Publisher",
                                            &Self::t(self.language, entry),
                                        ) {
                                            self.editor.status =
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
                }
                ui.checkbox(
                    &mut self.all_stories,
                    Self::t(self.language, "Show all stories in 3D"),
                );
                ui.separator();
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
                ScrollArea::vertical()
                    .id_salt("inspector_scroll")
                    .show(ui, |ui| match self.tab {
                        Tab::Properties => self.properties(ui),
                        Tab::Library => self.library(ui),
                        Tab::Assistant => self.assistant(ui),
                    });
            });
    }
    pub(super) fn properties(&mut self, ui: &mut Ui) {
        if self.draft_id != self.editor.selected || self.editor.selected.is_none() {
            self.draft_id = self.editor.selected;
            self.draft = self
                .editor
                .project
                .elements
                .iter()
                .find(|e| Some(e.id) == self.editor.selected)
                .cloned();
        }
        if let Some(mut e) = self.draft.clone() {
            Self::label(ui, &Self::t(self.language, "ELEMENT SETTINGS"));
            ui.add_space(8.);
            ui.horizontal(|ui| {
                ui.heading(format!("{} {:03}", e.kind(), e.id));
            });
            let measure = if matches!(e.shape, Shape::Polyline { .. }) {
                format!("{} · {:.2} m", e.material.name(), e.area())
            } else {
                format!("{} · {:.2} m²", e.material.name(), e.area())
            };
            ui.label(RichText::new(measure).color(MUTED));
            ui.add_space(10.);
            let stories = self.editor.project.stories.clone();
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
                        Button::new(Self::t(self.language, "Edit Wall Settings  ⌘T")),
                    )
                    .clicked()
                {
                    self.wall_settings_dialog = WallSettingsDialog::selected(&e);
                    self.element_settings = None;
                    self.element_settings_error.clear();
                }
            }
            ui.add_space(14.);
            if ui
                .add_sized(
                    [ui.available_width(), 34.],
                    Button::new(
                        RichText::new(Self::t(self.language, "Apply changes"))
                            .color(Color32::WHITE),
                    )
                    .fill(ACCENT),
                )
                .clicked()
            {
                match self.editor.replace_element(e.clone()) {
                    Ok(()) => {
                        self.editor.status = "Properties updated · Both views synchronized".into()
                    }
                    Err(err) => self.editor.status = err,
                }
            }
            ui.horizontal(|ui| {
                if ui.button(Self::t(self.language, "Duplicate")).clicked() {
                    self.editor.duplicate();
                }
                if ui.button(Self::t(self.language, "Delete")).clicked() {
                    self.editor.delete();
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
            let p = &self.editor.project;
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
            let area: f32 = p
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
                    .editor
                    .project
                    .elements
                    .clone()
                    .iter()
                    .filter(|e| e.story == self.editor.story)
                {
                    if ui
                        .selectable_label(
                            self.editor.selected == Some(e.id),
                            format!("{}  ·  {}", e.name, e.material.name()),
                        )
                        .clicked()
                    {
                        self.editor.selected = Some(e.id);
                    }
                }
            },
        );
    }
    pub(super) fn number(ui: &mut Ui, language: UiLanguage, label: &str, n: &mut f32) {
        ui.label(language.translate(label));
        ui.add(DragValue::new(n).speed(0.01).fixed_decimals(3).suffix(" m"));
        ui.end_row();
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
                ui.label(language.translate("Surface"));
                ComboBox::from_id_salt(("element_material", namespace, element.id))
                    .selected_text(element.material.name())
                    .width(180.)
                    .show_ui(ui, |ui| {
                        for material in Material::ALL {
                            ui.selectable_value(&mut element.material, material, material.name());
                        }
                    });
                ui.end_row();
                match &mut element.shape {
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
                        Self::number(ui, language, "Min X", &mut min.x);
                        Self::number(ui, language, "Min Y", &mut min.y);
                        Self::number(ui, language, "Max X", &mut max.x);
                        Self::number(ui, language, "Max Y", &mut max.y);
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
                                &format!("Node {} X", index + 1),
                                &mut point.x,
                            );
                            Self::number(
                                ui,
                                language,
                                &format!("Node {} Y", index + 1),
                                &mut point.y,
                            );
                        }
                    }
                    Shape::Opening {
                        host_wall,
                        width,
                        height,
                        sill,
                        offset,
                        ..
                    } => {
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
                        self.editor.set_tool(tool);
                        self.preview_tool = None;
                        self.editor.material = material;
                        self.editor.thickness = t;
                        self.editor.height = h;
                        self.editor.status =
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
            let p = &self.editor.project;
            let answer = if lower.contains("check") || lower.contains("检查") {
                match p.validate() {
                    Ok(()) => format!(
                        "Geometry validation passed for {} elements.\n\nChecks: finite coordinates, positive dimensions, unique IDs and story references.\n\nThis is not a building-code or clash check.",
                        p.elements.len()
                    ),
                    Err(e) => e,
                }
            } else if lower.contains("quant") || lower.contains("数量") {
                let area: f32 = p
                    .elements
                    .iter()
                    .filter(|e| matches!(e.shape, crate::model::Shape::Slab { .. }))
                    .map(Element::area)
                    .sum();
                let length: f32 = p
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
                    self.editor.story
                )
            } else {
                "Available local commands: Summary, Quantities, Check model.\n\nFree-form AI design and model generation require a future provider connection.".into()
            };
            self.messages.push((true, q));
            self.messages.push((false, answer));
        }
    }
}
