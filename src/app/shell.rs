use super::*;

impl AtelierApp {
    pub(super) fn top(&mut self, ui: &mut Ui) {
        egui::Panel::top("title")
            .frame(
                Frame::new()
                    .fill(CHROME)
                    .inner_margin(Margin::symmetric(14, 7)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(self.language.translate("OpenArchi"))
                            .size(16.)
                            .strong()
                            .color(Color32::WHITE),
                    );
                    ui.add_space(24.);
                    ui.label(
                        RichText::new(&self.editor.project.name)
                            .color(Color32::from_rgb(210, 220, 218)),
                    );
                    if self.editor.dirty() {
                        let (dot, _) = ui.allocate_exact_size(vec2(8., 8.), Sense::hover());
                        ui.painter().circle_filled(dot.center(), 2.5, ACCENT);
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let previous_language = self.language;
                        ComboBox::from_id_salt("ui_language")
                            .selected_text(self.language.label())
                            .width(112.)
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut self.language,
                                    UiLanguage::English,
                                    "English",
                                );
                                ui.selectable_value(
                                    &mut self.language,
                                    UiLanguage::SimplifiedChinese,
                                    "简体中文",
                                );
                            });
                        if self.language != previous_language {
                            self.language.save_preference();
                            self.editor.status = format!(
                                "{} · {}",
                                self.language.translate("Language"),
                                self.language.label()
                            );
                        }
                        ui.label(
                            RichText::new(if self.language == UiLanguage::SimplifiedChinese {
                                "建筑设计工作台   ·   0.1"
                            } else {
                                "ARCHITECTURAL DESIGN   ·   0.1"
                            })
                            .size(10.)
                            .color(Color32::from_rgb(145, 177, 174)),
                        );
                    });
                });
            });
        egui::Panel::top("commands")
            .frame(
                Frame::new()
                    .fill(PANEL)
                    .inner_margin(Margin::symmetric(10, 4)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.menu_button(Self::t(self.language, "OpenArchi"), |ui| {
                        if ui
                            .button(Self::t(self.language, "About OpenArchi"))
                            .clicked()
                        {
                            self.help = true;
                            ui.close();
                        }
                        ui.menu_button(self.language.translate("Project Preferences"), |ui| {
                            if let Some(command) = Self::preview_menu_items(
                                ui,
                                self.language,
                                &[
                                    "Working Units…",
                                    "Dimensions…",
                                    "Calculation Units…",
                                    "Zones…",
                                    "Reference Levels…",
                                    "Location Settings…",
                                    "Project North…",
                                    "Survey Point…",
                                ],
                            ) {
                                self.open_command_preview(command);
                            }
                        });
                        ui.menu_button(self.language.translate("Work Environment"), |ui| {
                            if let Some(command) = Self::preview_menu_items(
                                ui,
                                self.language,
                                &[
                                    "Appearance…",
                                    "Keyboard Shortcuts…",
                                    "Dialog Boxes and Palettes…",
                                    "Toolbox…",
                                    "Info Box…",
                                    "Toolbars…",
                                    "Menus…",
                                ],
                            ) {
                                self.open_command_preview(command);
                            }
                        });
                        ui.menu_button(self.language.translate("Services"), |ui| {
                            if let Some(command) = Self::preview_menu_items(
                                ui,
                                self.language,
                                &[
                                    "Look Up Selection",
                                    "Speech",
                                    "Spelling and Grammar",
                                    "Substitutions",
                                    "Transformations",
                                ],
                            ) {
                                self.open_command_preview(command);
                            }
                        });
                        ui.separator();
                        ui.label(
                            RichText::new(format!(
                                "{} · {}",
                                self.language.translate("Native Rust"),
                                crate::platform::RENDERER_LABEL
                            ))
                            .small()
                            .color(MUTED),
                        );
                    });
                    ui.menu_button(Self::t(self.language, "File"), |ui| {
                        ui.menu_button(self.language.translate("New"), |ui| {
                            if ui
                                .button(self.language.translate("New Empty Project…"))
                                .clicked()
                            {
                                self.file_action = Some(FileAction::New);
                                ui.close();
                            }
                            if ui
                                .button(self.language.translate("New & Reset All…"))
                                .clicked()
                            {
                                self.file_action = Some(FileAction::New);
                                ui.close();
                            }
                            if let Some(command) = Self::preview_menu_items(
                                ui,
                                self.language,
                                &["Share as Teamwork Project…"],
                            ) {
                                self.open_command_preview(command);
                            }
                        });
                        if ui
                            .button(format!("{}     ⌘O", Self::t(self.language, "Open JSON…")))
                            .clicked()
                        {
                            self.file_action = Some(FileAction::Open);
                            ui.close();
                        }
                        if ui
                            .button(format!("{}      ⌘S", Self::t(self.language, "Save JSON…")))
                            .clicked()
                        {
                            self.file_action = Some(FileAction::Save);
                            ui.close();
                        }
                        for command in [
                            "Save As…",
                            "Save as Template…",
                            "Save Selection as Module…",
                            "Save to BIMcloud…",
                            "Publish…",
                        ] {
                            if ui.button(self.language.translate(command)).clicked() {
                                self.open_command_preview(command.to_owned());
                                ui.close();
                            }
                        }
                        ui.menu_button(self.language.translate("Recent Projects"), |ui| {
                            if ui.button(&self.editor.project.name).clicked() {
                                ui.close();
                            }
                            ui.label(
                                RichText::new(
                                    self.language
                                        .translate("No other recent OpenArchi projects"),
                                )
                                .small()
                                .color(MUTED),
                            );
                        });
                        ui.separator();
                        ui.menu_button(self.language.translate("Interoperability"), |ui| {
                            if let Some(command) = Self::preview_menu_items(
                                ui,
                                self.language,
                                &[
                                    "Merge…",
                                    "IFC Project Manager…",
                                    "IFC Translators…",
                                    "Import / Export IFC…",
                                    "SAF Translators…",
                                    "DXF-DWG Translation Setup…",
                                    "Classifications & Properties…",
                                    "Import Point Cloud…",
                                    "Place Mesh from Survey Data…",
                                    "Grasshopper Connection…",
                                    "Bimplus Connection…",
                                ],
                            ) {
                                self.open_command_preview(command);
                            }
                        });
                        ui.menu_button(self.language.translate("External Content"), |ui| {
                            if let Some(command) = Self::preview_menu_items(
                                ui,
                                self.language,
                                &[
                                    "Hotlink Manager…",
                                    "Place External Drawing…",
                                    "Drawing Manager…",
                                    "Xref Manager…",
                                ],
                            ) {
                                self.open_command_preview(command);
                            }
                        });
                        ui.menu_button(self.language.translate("Libraries and Objects"), |ui| {
                            if let Some(command) = Self::preview_menu_items(
                                ui,
                                self.language,
                                &[
                                    "Library Manager…",
                                    "BIMcloud Libraries…",
                                    "New Object…",
                                    "Open Object…",
                                    "Save Selection as Object…",
                                    "Import Blocks…",
                                    "Import RFA…",
                                    "Import Rhino / OBJ / FBX…",
                                ],
                            ) {
                                self.open_command_preview(command);
                            }
                        });
                        ui.menu_button(self.language.translate("Info"), |ui| {
                            if let Some(command) = Self::preview_menu_items(
                                ui,
                                self.language,
                                &[
                                    "Project Info…",
                                    "Action Center…",
                                    "Project Notes…",
                                    "Preview and Credits…",
                                    "Session Report…",
                                ],
                            ) {
                                self.open_command_preview(command);
                            }
                        });
                        ui.menu_button(self.language.translate("Plot and Print"), |ui| {
                            if let Some(command) = Self::preview_menu_items(
                                ui,
                                self.language,
                                &["Plot Setup…", "Plot…", "Page Setup…", "Print…"],
                            ) {
                                self.open_command_preview(command);
                            }
                        });
                        ui.separator();
                        if ui
                            .button(Self::t(self.language, "About OpenArchi"))
                            .clicked()
                        {
                            self.help = true;
                            ui.close();
                        }
                    });
                    ui.menu_button(Self::t(self.language, "Edit"), |ui| {
                        if ui
                            .add_enabled(
                                !self.editor.undo.is_empty(),
                                Button::new(format!("{}  ⌘Z", Self::t(self.language, "Undo"))),
                            )
                            .clicked()
                        {
                            self.editor.undo();
                            ui.close();
                        }
                        if ui
                            .add_enabled(
                                !self.editor.redo.is_empty(),
                                Button::new(format!("{}  ⇧⌘Z", Self::t(self.language, "Redo"))),
                            )
                            .clicked()
                        {
                            self.editor.redo();
                            ui.close();
                        }
                        ui.separator();
                        if ui.button(self.language.translate("Cut  ⌘X")).clicked() {
                            self.editor.cut_selection();
                            ui.close();
                        }
                        if ui.button(self.language.translate("Copy  ⌘C")).clicked() {
                            self.editor.copy_selection();
                            ui.close();
                        }
                        if ui.button(self.language.translate("Paste  ⌘V")).clicked() {
                            self.editor.paste(Point::new(0.5, 0.5));
                            ui.close();
                        }
                        if ui
                            .button(self.language.translate("Duplicate  ⌘D"))
                            .clicked()
                        {
                            self.editor.duplicate();
                            ui.close();
                        }
                        ui.separator();
                        if ui
                            .button(self.language.translate("Element Settings  ⌘T"))
                            .clicked()
                        {
                            self.open_element_settings();
                            ui.close();
                        }
                        ui.separator();
                        for (group, commands) in [
                            (
                                "Groups",
                                &["Group", "Ungroup", "Suspend Groups", "Lock", "Unlock"][..],
                            ),
                            (
                                "Display Order",
                                &[
                                    "Bring to Front",
                                    "Send to Back",
                                    "Bring Forward",
                                    "Send Backward",
                                ][..],
                            ),
                            (
                                "Move",
                                &[
                                    "Drag",
                                    "Rotate",
                                    "Mirror",
                                    "Elevate",
                                    "Multiply",
                                    "Move to Story",
                                ][..],
                            ),
                            (
                                "Align & Distribute",
                                &["Align Elements…", "Distribute…"][..],
                            ),
                            (
                                "Reshape",
                                &[
                                    "Trim",
                                    "Stretch",
                                    "Resize",
                                    "Split",
                                    "Fillet / Chamfer",
                                    "Intersect",
                                    "Adjust",
                                    "Offset",
                                    "Explode",
                                    "Unify",
                                ][..],
                            ),
                            (
                                "Reference Line and Plane",
                                &[
                                    "Modify / Invert Wall Reference Line",
                                    "Modify Slab Reference Plane",
                                ][..],
                            ),
                        ] {
                            ui.menu_button(self.language.translate(group), |ui| {
                                if let Some(command) =
                                    Self::preview_menu_items(ui, self.language, commands)
                                {
                                    self.open_command_preview(command);
                                }
                            });
                        }
                    });
                    ui.menu_button(Self::t(self.language, "View"), |ui| {
                        ui.selectable_value(
                            &mut self.view,
                            View::Plan,
                            format!("{}  F2", Self::t(self.language, "Floor Plan")),
                        );
                        ui.selectable_value(
                            &mut self.view,
                            View::Split,
                            Self::t(self.language, "Split view"),
                        );
                        ui.selectable_value(
                            &mut self.view,
                            View::Model,
                            format!("{}  F3", Self::t(self.language, "3D Perspective")),
                        );
                        if ui
                            .button(
                                self.language
                                    .translate("Show selection / Marquee in 3D  F5"),
                            )
                            .clicked()
                        {
                            if self.editor.marquee.is_some() {
                                self.show_marquee_3d = true;
                                self.marquee_outside = false;
                                self.fit_marquee();
                            } else if !self.editor.selection_ids().is_empty() {
                                self.show_marquee_3d = true;
                                self.fit_selection();
                            }
                        }
                        ui.separator();
                        if ui
                            .checkbox(
                                &mut self.show_marquee_3d,
                                self.language.translate("Show Marquee in 3D"),
                            )
                            .changed()
                            && self.show_marquee_3d
                        {
                            if self.editor.marquee.is_some() {
                                self.fit_marquee();
                            } else {
                                self.fit_selection();
                            }
                        }
                        if self.show_marquee_3d && self.editor.marquee.is_some() {
                            ui.checkbox(
                                &mut self.marquee_outside,
                                self.language.translate("Show Outside Marquee"),
                            );
                        }
                        ui.separator();
                        if ui
                            .checkbox(&mut self.trace.enabled, self.language.translate("Trace"))
                            .changed()
                        {
                            self.trace_toggled();
                        }
                        ui.menu_button(self.language.translate("Trace Options"), |ui| {
                            self.trace_menu(ui)
                        });
                        ui.separator();
                        let before_guides = self.editor.project.clone();
                        ui.menu_button(self.language.translate("Guides"), |ui| {
                            self.guides_menu(ui)
                        });
                        if ui
                            .checkbox(
                                &mut self.editor.project.guide_settings.guide_lines_enabled,
                                self.language.translate("Guide Lines"),
                            )
                            .changed()
                        {
                            self.editor.commit(before_guides);
                        }
                        ui.checkbox(
                            &mut self.editor.project.grid_settings.show_construction_grid,
                            self.language.translate("Construction Grid Display"),
                        );
                        ui.checkbox(
                            &mut self.editor.snap,
                            self.language.translate("Element Snap"),
                        );
                        ui.menu_button(self.language.translate("Snap Options"), |ui| {
                            ui.checkbox(
                                &mut self.editor.snap,
                                self.language.translate("Element Snap"),
                            );
                            let grid_snap = {
                                let enabled =
                                    &mut self.editor.project.grid_settings.grid_snap_enabled;
                                ui.checkbox(enabled, self.language.translate("Grid Snap"))
                                    .changed()
                                    .then_some(*enabled)
                            };
                            if let Some(enabled) = grid_snap {
                                self.set_grid_snap_enabled(enabled);
                            }
                            let before = self.editor.project.clone();
                            if ui
                                .checkbox(
                                    &mut self.editor.project.guide_settings.snap_enabled,
                                    self.language.translate("Snap Guides and Points"),
                                )
                                .changed()
                            {
                                self.editor.commit(before);
                            }
                        });
                        ui.menu_button(
                            self.language.translate("Grid & Editing Plane Options"),
                            |ui| self.grids_menu(ui),
                        );
                        ui.menu_button(self.language.translate("Elements in 3D View"), |ui| {
                            ui.checkbox(
                                &mut self.all_stories,
                                self.language.translate("Show all stories in 3D"),
                            );
                            ui.separator();
                            ui.checkbox(
                                &mut self.cutaway_enabled,
                                self.language.translate("3D Cutaway"),
                            );
                            ui.checkbox(
                                &mut self.show_marquee_3d,
                                self.language.translate("Filter and Cut Elements in 3D"),
                            );
                            if ui
                                .button(self.language.translate("Show Selection / Marquee"))
                                .clicked()
                            {
                                self.show_marquee_3d = true;
                                if self.editor.marquee.is_some() {
                                    self.fit_marquee();
                                } else {
                                    self.fit_selection();
                                }
                            }
                            if ui
                                .button(self.language.translate("Hide Selection"))
                                .clicked()
                            {
                                self.editor.status =
                                    "3D selection visibility · interface preview".into();
                            }
                            if ui.button(self.language.translate("Show All")).clicked() {
                                self.editor.status = "All elements shown in 3D".into();
                            }
                        });
                        ui.menu_button(self.language.translate("3D View Options"), |ui| {
                            for option in [
                                "Projection Settings…",
                                "Perspective",
                                "Axonometry",
                                "Basic",
                                "Vectorial",
                                "Simple Shading",
                                "Technical",
                                "Detailed",
                                "Shadows",
                                "White Model",
                                "Wireframe",
                                "Orbit",
                                "Explore Model",
                            ] {
                                if ui.button(self.language.translate(option)).clicked() {
                                    self.editor.status =
                                        format!("{option} · viewport interface preview");
                                    ui.close();
                                }
                            }
                        });
                        ui.menu_button(self.language.translate("Zoom and Rebuild"), |ui| {
                            if ui
                                .button(self.language.translate("Fit in Window · F"))
                                .clicked()
                            {
                                self.fit();
                                ui.close();
                            }
                            if ui.button(self.language.translate("Rebuild")).clicked() {
                                self.editor.status = "View refreshed".into();
                                ui.close();
                            }
                            if ui.button(self.language.translate("Refresh")).clicked() {
                                self.editor.status = "View refreshed".into();
                            }
                            if ui
                                .button(self.language.translate("Rebuild & Regenerate"))
                                .clicked()
                            {
                                self.editor.status = "Model refresh requested".into();
                            }
                        });
                        ui.separator();
                        ui.menu_button(Self::t(self.language, "Language"), |ui| {
                            let previous = self.language;
                            ui.selectable_value(&mut self.language, UiLanguage::English, "English");
                            ui.selectable_value(
                                &mut self.language,
                                UiLanguage::SimplifiedChinese,
                                "简体中文",
                            );
                            if self.language != previous {
                                self.language.save_preference();
                                self.editor.status = format!(
                                    "{} · {}",
                                    self.language.translate("Language"),
                                    self.language.label()
                                );
                            }
                        });
                    });
                    ui.menu_button(Self::t(self.language, "Design"), |ui| {
                        ui.menu_button(self.language.translate("Architectural Tools"), |ui| {
                            if let Some(command) = Self::preview_menu_items(
                                ui,
                                self.language,
                                &[
                                    "Arrow",
                                    "Marquee",
                                    "Wall",
                                    "Wall End",
                                    "Column",
                                    "Beam",
                                    "Slab",
                                    "Roof",
                                    "Shell",
                                    "Stair",
                                    "Railing",
                                    "Curtain Wall",
                                    "Door",
                                    "Window",
                                    "Corner-Window",
                                    "Skylight",
                                    "Opening",
                                    "Zone",
                                    "Mesh",
                                    "Morph",
                                    "Object",
                                    "Lamp",
                                ],
                            ) {
                                match command.as_str() {
                                    "Wall" => {
                                        self.editor.set_tool(Tool::Wall);
                                        self.preview_tool = None;
                                    }
                                    "Slab" => {
                                        self.editor.set_tool(Tool::Slab);
                                        self.preview_tool = None;
                                    }
                                    "Stair" => self.activate_stair_tool(),
                                    "Polyline" => self.activate_polyline_tool(),
                                    "Marquee" => {
                                        self.editor.set_tool(Tool::Marquee);
                                        self.preview_tool = None;
                                    }
                                    "Arrow" => {
                                        self.editor.set_tool(Tool::Select);
                                        self.preview_tool = None;
                                    }
                                    _ => {
                                        self.preview_tool = Some(command.clone());
                                        self.editor.set_tool(Tool::Select);
                                        self.editor.status =
                                            format!("{command} tool selected · interface preview");
                                    }
                                }
                                self.tracker_locked = false;
                            }
                        });
                        ui.menu_button(
                            self.language.translate("Structural Engineering Tools"),
                            |ui| {
                                if let Some(command) = Self::preview_menu_items(
                                    ui,
                                    self.language,
                                    &["Support", "Link", "Point Load", "Line Load", "Surface Load"],
                                ) {
                                    self.open_command_preview(command);
                                }
                            },
                        );
                        ui.menu_button(self.language.translate("MEP Tools"), |ui| {
                            if let Some(command) = Self::preview_menu_items(
                                ui,
                                self.language,
                                &[
                                    "Duct Route",
                                    "Pipe Route",
                                    "Cable Route",
                                    "Branch",
                                    "Fitting",
                                    "Terminal",
                                    "Accessory",
                                    "Equipment",
                                ],
                            ) {
                                self.open_command_preview(command);
                            }
                        });
                        for (group, commands) in [
                            (
                                "Stories and Connections",
                                &[
                                    "Story Settings…",
                                    "Edit Elements by Stories…",
                                    "Trim Elements",
                                    "Merge Elements",
                                    "Join Wall to Curtain Wall",
                                    "Create Opening",
                                    "Clear Connections",
                                ][..],
                            ),
                            (
                                "Model Operations",
                                &[
                                    "Solid Element Operations…",
                                    "Magic Wand",
                                    "Modify Morph",
                                    "Modify Railing",
                                    "Modify Curtain Wall",
                                ][..],
                            ),
                            (
                                "Design Extras",
                                &["RoofMaker…", "TrussMaker…", "Grid System…", "Update Zones"][..],
                            ),
                            (
                                "Model Check",
                                &[
                                    "Collision Detection…",
                                    "Physical Quality…",
                                    "Model Compare…",
                                    "Structural Analytical Model…",
                                    "Energy Evaluation…",
                                ][..],
                            ),
                        ] {
                            ui.menu_button(self.language.translate(group), |ui| {
                                if let Some(command) =
                                    Self::preview_menu_items(ui, self.language, commands)
                                {
                                    self.open_command_preview(command);
                                }
                            });
                        }
                    });
                    ui.menu_button(Self::t(self.language, "Document"), |ui| {
                        ui.menu_button(self.language.translate("Documenting Tools"), |ui| {
                            for command in [
                                "Section",
                                "Elevation",
                                "Interior Elevation",
                                "Detail",
                                "Worksheet",
                                "Camera",
                                "Dimension",
                                "Level Dimension",
                                "Radial Dimension",
                                "Angle Dimension",
                                "Text",
                                "Label",
                                "Grid Element",
                                "Fill",
                                "Line",
                                "Arc / Circle",
                                "Polyline",
                                "Spline",
                                "Hotspot",
                                "Figure",
                                "Drawing",
                            ] {
                                if ui.button(self.language.translate(command)).clicked() {
                                    if command == "Polyline" {
                                        self.activate_polyline_tool();
                                    } else {
                                        self.open_command_preview(command.to_owned());
                                    }
                                    ui.close();
                                }
                            }
                        });
                        ui.menu_button(self.language.translate("Layers"), |ui| {
                            self.layers_menu_contents(ui);
                        });
                        if ui
                            .button(self.language.translate("Floor Plan Scale…"))
                            .clicked()
                        {
                            self.open_plan_option_dialog(PlanOptionKind::Scale);
                            ui.close();
                        }
                        if ui
                            .button(self.language.translate("Partial Structure Display…"))
                            .clicked()
                        {
                            self.open_plan_option_dialog(PlanOptionKind::PartialStructure);
                            ui.close();
                        }
                        ui.menu_button(self.language.translate("Pen Sets"), |ui| {
                            self.pen_set_menu_contents(ui);
                        });
                        ui.menu_button(self.language.translate("Model View"), |ui| {
                            self.model_view_menu_contents(ui);
                        });
                        ui.menu_button(self.language.translate("Graphic Overrides"), |ui| {
                            self.graphic_overrides_menu_contents(ui);
                        });
                        ui.menu_button(self.language.translate("Design Options"), |ui| {
                            self.design_options_menu_contents(ui);
                        });
                        ui.menu_button(self.language.translate("Renovation"), |ui| {
                            self.renovation_menu_contents(ui);
                        });
                        for (group, commands) in [
                            (
                                "Annotation",
                                &[
                                    "Automatic Dimensioning…",
                                    "Label Selected Elements",
                                    "Create Patch…",
                                    "Spell Checker",
                                ][..],
                            ),
                            (
                                "Schedules and Lists",
                                &[
                                    "Scheme Settings…",
                                    "Wall Schedule",
                                    "Door Schedule",
                                    "Window Schedule",
                                    "Object Inventory",
                                    "Element Lists",
                                    "Component Lists",
                                    "Zone Lists",
                                    "Project Indexes",
                                ][..],
                            ),
                            (
                                "Issues and Changes",
                                &[
                                    "Issue Manager",
                                    "Issue Organizer",
                                    "Change Manager",
                                    "Keynotes",
                                ][..],
                            ),
                            (
                                "Creative Imaging",
                                &[
                                    "PhotoRender Projection",
                                    "Render Marquee Region",
                                    "PhotoRendering Settings…",
                                    "Surface Painter",
                                    "Create Fly-Through…",
                                    "Create Sun Study…",
                                ][..],
                            ),
                        ] {
                            ui.menu_button(self.language.translate(group), |ui| {
                                if let Some(command) =
                                    Self::preview_menu_items(ui, self.language, commands)
                                {
                                    self.open_command_preview(command);
                                }
                            });
                        }
                    });
                    ui.menu_button(Self::t(self.language, "Options"), |ui| {
                        ui.checkbox(
                            &mut self.editor.snap,
                            self.language.translate("Snap to geometry and grid"),
                        );
                        ui.checkbox(
                            &mut self.editor.orthogonal,
                            self.language.translate("Orthogonal drawing"),
                        );
                        ui.separator();
                        for (group, commands) in [
                            (
                                "Element Attributes",
                                &[
                                    "Layers…",
                                    "Lines…",
                                    "Pens & Color…",
                                    "Fills…",
                                    "Surfaces…",
                                    "Building Materials…",
                                    "Composites…",
                                    "Profile Manager…",
                                    "Zone Categories…",
                                    "Attributes…",
                                    "Attribute Manager…",
                                ][..],
                            ),
                            (
                                "Properties and Classification",
                                &[
                                    "Property Manager…",
                                    "Classification Manager…",
                                    "Operation Profiles…",
                                ][..],
                            ),
                            (
                                "Structural Attributes",
                                &[
                                    "Load Manager…",
                                    "Load Combinations…",
                                    "Load Mapping…",
                                    "Load Categories…",
                                ][..],
                            ),
                            (
                                "Project Preferences",
                                &[
                                    "Working Units…",
                                    "Dimensions…",
                                    "Calculation Units…",
                                    "Calculation Rules…",
                                    "Zones…",
                                    "Reference Levels…",
                                    "Legacy…",
                                    "Stair Rules and Standards…",
                                    "Structural Analytical Model Units…",
                                    "Location Settings…",
                                    "Project North…",
                                    "Survey Point…",
                                ][..],
                            ),
                            (
                                "Work Environment",
                                &[
                                    "Dialog Boxes and Palettes…",
                                    "Selection and Element Information…",
                                    "Tracker and Coordinate Input…",
                                    "Input Constraints and Guides…",
                                    "Model Rebuild Options…",
                                    "Appearance…",
                                    "Data Safety and Integrity…",
                                    "Keyboard Shortcuts…",
                                    "Toolbox…",
                                    "Tool Settings Dialog Boxes…",
                                    "Info Box…",
                                    "Toolbars…",
                                    "Menus…",
                                ][..],
                            ),
                        ] {
                            ui.menu_button(self.language.translate(group), |ui| {
                                if let Some(command) =
                                    Self::preview_menu_items(ui, self.language, commands)
                                {
                                    if command == "Layers…" {
                                        self.open_plan_option_dialog(PlanOptionKind::Layers);
                                    } else if command == "Pens & Color…" {
                                        self.open_plan_option_dialog(PlanOptionKind::PenSets);
                                    } else if command == "Dimensions…" {
                                        self.open_plan_option_dialog(PlanOptionKind::Dimensions);
                                    } else {
                                        self.open_command_preview(command);
                                    }
                                }
                            });
                        }
                        ui.separator();
                        if ui
                            .button(self.language.translate("Add-On Manager…"))
                            .clicked()
                        {
                            self.editor.status = "Add-On Manager · interface preview".into();
                        }
                    });
                    ui.menu_button(Self::t(self.language, "Teamwork"), |ui| {
                        ui.menu_button(self.language.translate("Project"), |ui| {
                            if let Some(command) = Self::preview_menu_items(
                                ui,
                                self.language,
                                &[
                                    "Share…",
                                    "Migrate Teamwork Project…",
                                    "Open / Join Teamwork Project…",
                                    "Local Data Manager…",
                                ],
                            ) {
                                self.open_command_preview(command);
                            }
                        });
                        ui.menu_button(self.language.translate("Traveling"), |ui| {
                            if let Some(command) = Self::preview_menu_items(
                                ui,
                                self.language,
                                &[
                                    "Create Travel Pack",
                                    "Import Travel Pack",
                                    "Leave Teamwork Project",
                                ],
                            ) {
                                self.open_command_preview(command);
                            }
                        });
                        ui.menu_button(self.language.translate("Send & Receive"), |ui| {
                            if let Some(command) = Self::preview_menu_items(
                                ui,
                                self.language,
                                &[
                                    "Send & Receive",
                                    "Send Changes",
                                    "Receive Changes",
                                    "Send & Receive, then Compare",
                                    "Reserve Elements…",
                                    "Release All",
                                ],
                            ) {
                                self.open_command_preview(command);
                            }
                        });
                        ui.menu_button(self.language.translate("Messages and Review"), |ui| {
                            if let Some(command) = Self::preview_menu_items(
                                ui,
                                self.language,
                                &[
                                    "Assign Selected Elements",
                                    "New Message…",
                                    "Review Selected Elements…",
                                    "Change Tracking",
                                ],
                            ) {
                                self.open_command_preview(command);
                            }
                        });
                        if ui
                            .button(self.language.translate("Teamwork Palette"))
                            .clicked()
                        {
                            self.editor.status = "Teamwork Palette · standalone preview".into();
                        }
                    });
                    ui.menu_button(Self::t(self.language, "Window"), |ui| {
                        let quick_label = if self.quick_options_visible {
                            "Hide Quick Options Bar"
                        } else {
                            "Show Quick Options Bar"
                        };
                        if ui.button(self.language.translate(quick_label)).clicked() {
                            self.toggle_quick_options_bar();
                            ui.close();
                        }
                        ui.separator();
                        ui.checkbox(&mut self.show_left, self.language.translate("Toolbox"));
                        ui.checkbox(
                            &mut self.show_right,
                            self.language.translate("Navigator & Properties"),
                        );
                        ui.separator();
                        ui.menu_button(self.language.translate("Palettes"), |ui| {
                            for palette in [
                                "Info Box",
                                "Status Bar",
                                "Quick Options",
                                "Organizer",
                                "Navigator Preview",
                                "Drawing Manager",
                                "Model Check Report",
                                "Issue Manager",
                                "Change Manager",
                                "Design Options",
                                "Renovation",
                                "Trace & Reference",
                                "Favorites",
                                "Attributes",
                                "Profile Manager",
                                "Element Information",
                                "Selections",
                                "Coordinates",
                                "Control Box",
                                "Quick Layers",
                                "Teamwork Palette",
                            ] {
                                let selected = match palette {
                                    "Info Box" => self.info_box_visible,
                                    "Quick Options" => self.quick_options_palette_visible,
                                    _ => false,
                                };
                                if ui
                                    .selectable_label(selected, self.language.translate(palette))
                                    .clicked()
                                {
                                    if palette == "Info Box" {
                                        self.toggle_info_box_visibility();
                                        ui.close();
                                    } else if palette == "Trace & Reference" {
                                        self.open_trace_palette();
                                    } else if palette == "Quick Options" {
                                        self.toggle_quick_options_palette();
                                    } else if palette == "Design Options" {
                                        self.open_plan_option_dialog(PlanOptionKind::DesignOptions);
                                    } else if palette == "Renovation" {
                                        self.open_plan_option_dialog(PlanOptionKind::Renovation);
                                    } else {
                                        self.editor.status = format!("Palette · {palette}");
                                    }
                                }
                            }
                        });
                        ui.separator();
                        ui.menu_button(self.language.translate("Toolbars"), |ui| {
                            for toolbar in [
                                "Standard",
                                "Standard for Low-Res Screens",
                                "Tools - Architectural",
                                "Arrange Elements",
                                "Attributes",
                                "Drafting Aids",
                                "Drag Elements",
                                "Edit Elements",
                                "3D Visualization",
                                "Classic 3D Navigation",
                                "MEP Toolbar",
                                "Model Check",
                                "Structural Engineering",
                                "Teamwork",
                            ] {
                                if ui
                                    .selectable_label(false, self.language.translate(toolbar))
                                    .clicked()
                                {
                                    self.editor.status = format!("Toolbar · {toolbar}");
                                }
                            }
                        });
                        ui.separator();
                        if ui
                            .button(self.language.translate("Reset workspace layout"))
                            .clicked()
                        {
                            self.show_left = true;
                            self.show_right = true;
                            self.quick_options_visible = true;
                            self.quick_options_palette_visible = false;
                            self.info_box_visible = true;
                            self.view = View::Plan;
                            self.split = 0.53;
                            ui.close();
                        }
                    });
                    ui.menu_button(Self::t(self.language, "Help"), |ui| {
                        if ui
                            .button(self.language.translate("About OpenArchi"))
                            .clicked()
                        {
                            self.help = true;
                            ui.close();
                        }
                        ui.label(
                            RichText::new(format!(
                                "{} · wgpu / {}",
                                self.language.translate("Native Rust"),
                                crate::platform::RENDERER_BACKEND
                            ))
                            .small()
                            .color(MUTED),
                        );
                    });
                    ui.separator();
                    if ui
                        .add_enabled(
                            !self.editor.undo.is_empty(),
                            Button::new(self.language.translate("Undo")),
                        )
                        .on_hover_text(Self::t(self.language, "Undo · ⌘Z"))
                        .clicked()
                    {
                        self.editor.undo();
                    }
                    if ui
                        .add_enabled(
                            !self.editor.redo.is_empty(),
                            Button::new(self.language.translate("Redo")),
                        )
                        .on_hover_text(Self::t(self.language, "Redo · ⇧⌘Z"))
                        .clicked()
                    {
                        self.editor.redo();
                    }
                    ui.separator();
                    if Self::vector_labeled_button(
                        ui,
                        "Fit",
                        &Self::t(self.language, "Fit"),
                        &Self::t(self.language, "Fit model · F"),
                        false,
                    ) {
                        self.fit();
                    }
                    ui.add_space(12.);
                    ui.selectable_value(
                        &mut self.view,
                        View::Plan,
                        format!("{}  F2", Self::t(self.language, "2D Plan")),
                    );
                    ui.selectable_value(
                        &mut self.view,
                        View::Split,
                        Self::t(self.language, "Split view"),
                    );
                    ui.selectable_value(
                        &mut self.view,
                        View::Model,
                        format!("{}  F3", Self::t(self.language, "3D Perspective")),
                    );
                    if self.editor.marquee.is_some() {
                        ui.separator();
                        if ui
                            .checkbox(
                                &mut self.show_marquee_3d,
                                self.language.translate("3D Marquee"),
                            )
                            .changed()
                            && self.show_marquee_3d
                        {
                            if self.editor.marquee.is_some() {
                                self.fit_marquee();
                            } else {
                                self.fit_selection();
                            }
                        }
                        if self.show_marquee_3d {
                            ui.selectable_value(
                                &mut self.marquee_outside,
                                false,
                                self.language.translate("Inside"),
                            );
                            ui.selectable_value(
                                &mut self.marquee_outside,
                                true,
                                self.language.translate("Outside"),
                            );
                        }
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if Self::vector_labeled_button(
                            ui,
                            "Help",
                            &Self::t(self.language, "Help"),
                            &Self::t(self.language, "Open help and shortcuts"),
                            false,
                        ) {
                            self.help = true;
                        }
                        if ui.button(Self::t(self.language, "Save project")).clicked() {
                            self.file_action = Some(FileAction::Save);
                        }
                    });
                });
            });
        egui::Panel::top("standard_toolbar")
            .frame(
                Frame::new()
                    .fill(CHROME)
                    .inner_margin(Margin::symmetric(9, 3)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    for (icon, tip) in [
                        ("Home", "Home / fit project"),
                        ("Undo", "Undo"),
                        ("Redo", "Redo"),
                        ("Save", "Save project"),
                        ("Zoom In", "Zoom in"),
                        ("Zoom Out", "Zoom out"),
                    ] {
                        if Self::vector_icon_button(ui, icon, &Self::t(self.language, tip), false) {
                            match icon {
                                "Home" => self.fit(),
                                "Undo" => {
                                    self.editor.undo();
                                }
                                "Redo" => {
                                    self.editor.redo();
                                }
                                "Save" => self.file_action = Some(FileAction::Save),
                                "Zoom In" => {
                                    self.plan_scale = (self.plan_scale * 1.15).clamp(4., 400.)
                                }
                                "Zoom Out" => {
                                    self.plan_scale = (self.plan_scale / 1.15).clamp(4., 400.)
                                }
                                _ => {}
                            }
                        }
                    }
                    ui.separator();
                    ui.menu_button(Self::t(self.language, "Guides"), |ui| self.guides_menu(ui));
                    ui.menu_button(Self::t(self.language, "Grids"), |ui| self.grids_menu(ui));
                    if ui
                        .toggle_value(&mut self.trace.enabled, Self::t(self.language, "Trace"))
                        .changed()
                    {
                        self.trace_toggled();
                    }
                    ui.menu_button("▾", |ui| self.trace_menu(ui));
                    ui.small_button(Self::t(self.language, "Measure"))
                        .on_hover_text(if self.language == UiLanguage::SimplifiedChinese {
                            "测量工具预览"
                        } else {
                            "Measure tool preview"
                        });
                    ui.toggle_value(&mut self.groups_enabled, Self::t(self.language, "Groups"));
                    ui.toggle_value(
                        &mut self.cutaway_enabled,
                        Self::t(self.language, "3D Cutaway"),
                    );
                    ui.separator();
                    ui.selectable_value(
                        &mut self.view,
                        View::Plan,
                        Self::t(self.language, "Floor plan · F2"),
                    );
                    ui.selectable_value(
                        &mut self.view,
                        View::Model,
                        Self::t(self.language, "Perspective 3D · F3"),
                    );
                });
            });
        egui::Panel::top("edit_toolbar")
            .frame(
                Frame::new()
                    .fill(PANEL)
                    .inner_margin(Margin::symmetric(9, 2)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(self.language.translate("EDIT"))
                            .size(10.)
                            .strong()
                            .color(MUTED),
                    );
                    if Self::vector_labeled_button(
                        ui,
                        "Drag",
                        &Self::t(self.language, "Drag"),
                        &Self::t(self.language, "Move selection"),
                        false,
                    ) {
                        self.editor.status =
                            "Drag command · select an element and drag it in the canvas".into();
                    }
                    if Self::vector_labeled_button(
                        ui,
                        "Rotate",
                        &Self::t(self.language, "Rotate"),
                        &Self::t(self.language, "Rotate selection 90°"),
                        false,
                    ) {
                        self.editor.status = "Rotate command · choose a pivot in the canvas".into();
                    }
                    if Self::vector_labeled_button(
                        ui,
                        "Mirror",
                        &Self::t(self.language, "Mirror"),
                        &Self::t(self.language, "Mirror selection"),
                        false,
                    ) {
                        self.editor.status = "Mirror command · choose an axis in the canvas".into();
                    }
                    if Self::vector_labeled_button(
                        ui,
                        "Multiply",
                        &Self::t(self.language, "Multiply"),
                        &Self::t(self.language, "Duplicate selection"),
                        false,
                    ) {
                        self.editor.status = "Multiply command · choose copy settings".into();
                    }
                    ui.separator();
                    ui.label(
                        RichText::new(
                            self.language
                                .translate("Workspace controls · viewport context"),
                        )
                        .small()
                        .color(MUTED),
                    );
                });
            });
        if self.info_box_visible {
            self.sync_info_box_wall();
            self.sync_info_box_wall_tool();
            self.sync_info_box_opening();
            self.sync_info_box_opening_tool();
            egui::Panel::top("info_box")
            .frame(
                Frame::new()
                    .fill(PANEL_ALT)
                    .inner_margin(Margin::symmetric(12, 6)),
            )
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    Self::label(ui, &Self::t(self.language, "INFO BOX"));
                    ui.separator();
                    if let Some(name) = self.preview_tool.as_deref() {
                        ui.strong(Self::t(self.language, name));
                        ui.separator();
                        ComboBox::from_id_salt("preview_favorite").selected_text(self.language.translate("Favorites")).show_ui(ui, |ui| {
                            for favorite in ["Default", "Architectural", "Detailed", "Existing"] { let _ = ui.selectable_label(favorite == "Default", favorite); }
                        });
                        ui.label(self.language.translate("ID"));
                        ui.add_sized([78., 24.], TextEdit::singleline(&mut self.preview_element_id));
                        ComboBox::from_id_salt("preview_home_story").selected_text(Self::t(self.language, "Home Story")).show_ui(ui, |ui| {
                            for story in self.editor.project.stories.clone() { let _ = ui.selectable_label(story.id == self.editor.story, format!("{} · {}", story.id, story.name)); }
                        });
                        ComboBox::from_id_salt("preview_layer").selected_text(&self.preview_layer).width(155.).show_ui(ui, |ui| {
                            for layer in ["01 Site", "02 Drafting", "03 Plans - Preliminary", "04 Plans - Approval", "05 Plans - Detailed", "06 Plans - Mechanical", "07 Plans - Structural", "08 Rendering", "09 Layouting"] {
                                ui.selectable_value(&mut self.preview_layer, layer.to_owned(), layer);
                            }
                        });
                        ui.label(Self::t(self.language, "Offset"));
                        ui.add(DragValue::new(&mut self.preview_offset).speed(0.05).fixed_decimals(2).suffix(" m"));
                        ui.label(Self::t(self.language, "Reference"));
                        Self::inline_vector_icon(ui, "Reference", ACCENT);
                        if ui.button(Self::t(self.language, "Settings…")).clicked() { self.open_command_preview(format!("{name} Settings")); }
                    } else {
                        let tool_name = match self.editor.tool {
                            Tool::Select => "Arrow",
                            Tool::Marquee => "Marquee",
                            Tool::Wall => "Wall",
                            Tool::Slab => "Slab",
                            Tool::Stair => "Stair",
                            Tool::Polyline => "Polyline",
                            Tool::Door => "Door",
                            Tool::Window => "Window",
                        };
                        ui.strong(Self::t(self.language, tool_name));
                    }
                    if self.preview_tool.is_some() {
                        // Generic tool metadata fields above are UI-only until the tool backend is implemented.
                    } else if self.info_box_opening.is_some() {
                        self.info_box_opening_properties(ui);
                    } else if self.info_box_wall.is_some() {
                        self.info_box_wall_properties(ui);
                    } else if matches!(self.editor.tool, Tool::Door | Tool::Window) {
                        self.info_box_opening_tool_properties(ui);
                    } else if self.editor.tool == Tool::Wall {
                        self.info_box_wall_tool_properties(ui);
                        ui.separator();
                        ui.label(Self::t(self.language, "Material"));
                        ComboBox::from_id_salt("wall_tool_material")
                            .selected_text(self.editor.material.name())
                            .width(118.)
                            .show_ui(ui, |ui| {
                                for m in Material::ALL {
                                    ui.selectable_value(&mut self.editor.material, m, m.name());
                                }
                            });
                        ui.checkbox(&mut self.editor.snap, self.language.translate("Snap"))
                            .on_hover_text(Self::t(
                                self.language,
                                "Endpoints / midpoints / 100 mm grid",
                            ));
                        ui.checkbox(
                            &mut self.editor.orthogonal,
                            self.language.translate("Ortho"),
                        )
                        .on_hover_text(Self::t(
                            self.language,
                            "Constrain to X/Y; Shift temporarily constrains",
                        ));
                    } else if self.editor.tool == Tool::Marquee {
                        ComboBox::from_id_salt("marquee_geometry")
                            .selected_text(match self.marquee_geometry {
                                MarqueeGeometry::Rectangle => "Rectangle",
                                MarqueeGeometry::RotatedRectangle => "Rotated rectangle",
                                MarqueeGeometry::Polygon => "Polygon",
                            })
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut self.marquee_geometry, MarqueeGeometry::Rectangle, if self.language == UiLanguage::SimplifiedChinese { "矩形 · 拖动" } else { "Rectangle · drag" });
                                ui.selectable_value(&mut self.marquee_geometry, MarqueeGeometry::RotatedRectangle, if self.language == UiLanguage::SimplifiedChinese { "旋转矩形 · 三点" } else { "Rotated rectangle · 3 points" });
                                ui.selectable_value(&mut self.marquee_geometry, MarqueeGeometry::Polygon, if self.language == UiLanguage::SimplifiedChinese { "多边形 · 点击顶点，右键闭合" } else { "Polygon · click vertices, right-click to close" });
                            });
                        ui.selectable_value(&mut self.marquee_scope, MarqueeScope::CurrentStory, Self::t(self.language, "Single story"));
                        ui.selectable_value(&mut self.marquee_scope, MarqueeScope::AllStories, Self::t(self.language, "All stories"));
                        ui.label(Self::t(self.language, "3D depth"));
                        ui.add(DragValue::new(&mut self.marquee_height).speed(0.05).range(0.01..=100.).fixed_decimals(2).suffix(" m"));
                        if self.editor.marquee.is_some() {
                            if ui.button(Self::t(self.language, "Select all in Marquee · ⌘A")).clicked() {
                                let count = self.editor.select_all_in_marquee();
                                self.editor.status = format!("Selected {count} element(s) in Marquee");
                            }
                            if ui.button(Self::t(self.language, "Remove Marquee · Esc")).clicked() {
                                self.editor.remove_marquee();
                                self.editor.set_selection(Vec::new());
                            }
                            ui.label(self.language.translate("ΔX"));
                            ui.add(DragValue::new(&mut self.marquee_dx).speed(0.01).fixed_decimals(3).suffix(" m"));
                            ui.label(self.language.translate("ΔY"));
                            ui.add(DragValue::new(&mut self.marquee_dy).speed(0.01).fixed_decimals(3).suffix(" m"));
                            if ui.button(Self::t(self.language, "Move region")).clicked() {
                                self.editor.move_marquee(Point::new(self.marquee_dx, self.marquee_dy));
                            }
                            if ui.button(Self::t(self.language, "Stretch enclosed nodes")).clicked() {
                                match self.editor.stretch_nodes_in_marquee(Point::new(self.marquee_dx, self.marquee_dy)) {
                                    Ok(0) => self.editor.status = "No editable nodes inside Marquee".into(),
                                    Ok(_) => {},
                                    Err(error) => self.editor.status = error,
                                }
                            }
                            if !self.editor.selection_ids().is_empty() {
                                ui.separator();
                                ui.label(Self::t(self.language, "Selection operations"));
                                if ui.button(self.language.translate("Cut · ⌘X")).clicked() { self.editor.cut_selection(); }
                                if ui.button(self.language.translate("Copy · ⌘C")).clicked() { self.editor.copy_selection(); }
                                if ui.button(self.language.translate("Mirror X")).clicked() { self.editor.mirror_selection(true); }
                                if ui.button(self.language.translate("Mirror Y")).clicked() { self.editor.mirror_selection(false); }
                                if ui.button(self.language.translate("Rotate 90°")).clicked() { self.editor.rotate_selection_quarter_turn(false); }
                                if ui.button(self.language.translate("Move to story above")).clicked() { self.editor.move_selection_to_story(1); }
                                if ui.button(self.language.translate("Move to story below")).clicked() { self.editor.move_selection_to_story(-1); }
                                if ui.button(self.language.translate("Move selected elements")).clicked() {
                                    self.editor.move_selection(Point::new(self.marquee_dx, self.marquee_dy));
                                }
                                if ui.button(self.language.translate("Duplicate selected · ⌘D")).clicked() {
                                    self.editor.duplicate();
                                }
                                if ui.button(self.language.translate("Delete selected · Delete")).clicked() {
                                    self.editor.delete();
                                }
                                ui.label(self.selected_count_label(self.editor.selection_ids().len()));
                            }
                        }
                    } else if self.editor.tool == Tool::Select {
                        ui.separator();
                        ui.label(RichText::new(Self::t(self.language, "Selection Settings")).strong());
                        ui.checkbox(&mut self.select_sub_elements, Self::t(self.language, "Sub-element selection"))
                            .on_hover_text(Self::t(self.language, "Toggle element / sub-element selection mode"));
                        ui.checkbox(&mut self.select_locked_elements, Self::t(self.language, "Include locked elements"));
                        ui.separator();
                        ui.label(Self::t(self.language, "Selection methods"));
                        let _ = Self::vector_labeled_button(ui, "Element", &Self::t(self.language, "Element"), &Self::t(self.language, "Select whole elements"), true);
                        let _ = Self::vector_labeled_button(ui, "Area Selection", &Self::t(self.language, "Area"), &Self::t(self.language, "Select by area"), false);
                        let _ = Self::vector_labeled_button(ui, "Quick Selection", &Self::t(self.language, "Quick"), &Self::t(self.language, "Quick selection"), false);
                        ui.label(RichText::new(self.selected_count_label(self.editor.selection_ids().len())).color(MUTED));
                    } else {
                        ui.separator();
                        if self.editor.tool == Tool::Polyline {
                            ui.label(self.language.translate("2D drafting · left-click nodes · right-click to finish · click start to close"));
                        } else if self.editor.tool == Tool::Stair {
                            ui.label(format!("{} risers · {:.2} m wide · {:.2} m rise", self.editor.stair_spec.risers, self.editor.stair_spec.width, self.editor.stair_spec.total_rise));
                            if ui.button(self.language.translate("Stair Settings…")).clicked() {
                                self.stair_settings_draft = self.editor.stair_spec.clone();
                                self.stair_settings_error.clear();
                                self.stair_settings_open = true;
                            }
                        } else {
                            ui.label(Self::t(self.language, "Thickness"));
                            ui.add(
                                DragValue::new(&mut self.editor.thickness)
                                    .speed(0.01)
                                    .range(0.01..=5.)
                                    .fixed_decimals(3)
                                    .suffix(" m"),
                            );
                        }
                        if self.editor.tool != Tool::Slab && self.editor.tool != Tool::Stair && self.editor.tool != Tool::Polyline {
                            ui.label(Self::t(self.language, "Height"));
                            ui.add(
                                DragValue::new(&mut self.editor.height)
                                    .speed(0.05)
                                    .range(0.01..=100.)
                                    .fixed_decimals(2)
                                    .suffix(" m"),
                            );
                        }
                        ComboBox::from_id_salt("default_material")
                            .selected_text(self.editor.material.name())
                            .width(118.)
                            .show_ui(ui, |ui| {
                                for m in Material::ALL {
                                    ui.selectable_value(&mut self.editor.material, m, m.name());
                                }
                            });
                        ui.separator();
                        ui.checkbox(&mut self.editor.snap, self.language.translate("Snap"))
                            .on_hover_text(Self::t(self.language, "Endpoints / midpoints / 100 mm grid"));
                        ui.checkbox(&mut self.editor.orthogonal, self.language.translate("Ortho"))
                            .on_hover_text(Self::t(self.language, "Constrain to X/Y; Shift temporarily constrains"));
                    }
                });
            });
        }
    }
    pub(super) fn toolbox(&mut self, ui: &mut Ui) {
        egui::Panel::left("toolbox")
            .default_size(188.)
            .size_range(160.0..=240.)
            .frame(
                Frame::new()
                    .fill(PANEL)
                    .inner_margin(Margin::symmetric(8, 8)),
            )
            .show(ui, |ui| {
                ScrollArea::vertical().id_salt("toolbox_scroll").show(ui, |ui| {
                ui.horizontal(|ui| {
                    Self::label(ui, &Self::t(self.language, "TOOLBOX"));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.small_button("⋯").on_hover_text(Self::t(self.language, "Toolbox options"));
                    });
                });
                ui.add_space(7.);
                Self::label(ui, &Self::t(self.language, "Selection & Region"));
                for (t, label, key) in [
                    (Tool::Select, "Arrow", "V"),
                    (Tool::Marquee, "Marquee", "M"),
                ] {
                    let active = self.preview_tool.is_none() && self.editor.tool == t;
                    if Self::toolbox_tool_row(ui, label, &Self::t(self.language, label), active, Some(key)) {
                        self.editor.set_tool(t);
                        self.preview_tool = None;
                        self.tracker_locked = false;
                    }
                }
                ui.add_space(8.);
                ui.separator();
                ui.add_space(4.);
                CollapsingHeader::new(Self::t(self.language, "Design")).default_open(true).show(ui, |ui| {
                for (t, label, key) in [
                    (Tool::Wall, "Wall", "W"),
                    (Tool::Slab, "Slab", "L"),
                ] {
                    let active = self.preview_tool.is_none() && self.editor.tool == t;
                    if Self::toolbox_tool_row(ui, label, &Self::t(self.language, label), active, Some(key)) {
                        self.editor.set_tool(t);
                        self.preview_tool = None;
                        self.tracker_locked = false;
                    }
                }
                if Self::toolbox_tool_row(ui, "Stair", self.language.translate("Stair"), self.preview_tool.is_none() && self.editor.tool == Tool::Stair, Some("S")) {
                    self.activate_stair_tool();
                }
                for (t, label) in [(Tool::Door, "Door"), (Tool::Window, "Window")] {
                    let active = self.preview_tool.is_none() && self.editor.tool == t;
                    if Self::toolbox_tool_row(ui, label, &Self::t(self.language, label), active, None) {
                        self.preview_tool = None;
                        self.editor.set_tool(t);
                        self.tracker_locked = false;
                    }
                }
                for name in ["Column", "Beam", "Object", "Lamp", "Roof", "Shell", "Railing", "Curtain Wall", "Skylight", "Corner-Window", "Opening", "Wall End", "Mesh", "Morph", "Zone"] {
                    if Self::toolbox_tool_row(ui, name, &Self::t(self.language, name), self.preview_tool.as_deref() == Some(name), None) {
                        self.preview_tool = Some(name.to_owned());
                        self.editor.set_tool(Tool::Select);
                        self.editor.status = format!("{name} tool selected · model interaction is not available in this demo");
                    }
                }
                });
                ui.add_space(10.);
                ui.separator();
                ui.add_space(4.);
                for (group, names) in [
                    ("Viewpoint", &(["Section", "Elevation", "Interior Elevation", "Detail", "Worksheet", "3D Document"][..])),
                    ("Document", &(["Dimension", "Level Dimension", "Radial Dimension", "Angle Dimension", "Text", "Label", "Fill", "Line", "Arc / Circle", "Polyline", "Spline", "Hotspot", "Figure", "Drawing", "Grid Element", "Camera"][..])),
                    ("Structural Engineering", &(["Support", "Link", "Point Load", "Line Load", "Surface Load"][..])),
                    ("MEP Engineering", &(["Duct Route", "Pipe Route", "Cable Route", "Branch", "Fitting", "Terminal", "Accessory", "Equipment"][..])),
                ] {
                    CollapsingHeader::new(Self::t(self.language, group)).default_open(false).show(ui, |ui| {
                        for name in names {
                            let polyline = *name == "Polyline";
                            let active = if polyline {
                                self.preview_tool.is_none() && self.editor.tool == Tool::Polyline
                            } else {
                                self.preview_tool.as_deref() == Some(*name)
                            };
                            if Self::toolbox_tool_row(ui, name, &Self::t(self.language, name), active, polyline.then_some("P")) {
                                if polyline {
                                    self.activate_polyline_tool();
                                    continue;
                                }
                                self.preview_tool = Some((*name).to_owned());
                                self.editor.set_tool(Tool::Select);
                                self.editor.status = format!("{name} tool selected · interface preview");
                            }
                        }
                    });
                }
                ui.with_layout(Layout::bottom_up(Align::LEFT), |ui| {
                    ui.label(
                        RichText::new(format!(
                            "OPENARCHI\n{}\n{}",
                            Self::t(self.language, "Metres · Z up"),
                            crate::platform::RENDERER_LABEL
                        ))
                            .small()
                            .color(MUTED),
                    );
                    ui.separator();
                });
                });
            });
    }
}
