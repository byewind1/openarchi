use super::*;

impl AtelierApp {
    pub(super) fn top(&mut self, ui: &mut Ui) {
        let plan_label = self.personal_command_label(
            "view.plan",
            "Floor Plan",
            &format!("{}  F2", Self::t(self.language, "Floor Plan")),
        );
        let plan2_label = self.personal_command_label(
            "view.plan",
            "2D Plan",
            &format!("{}  F2", Self::t(self.language, "2D Plan")),
        );
        let model_label = self.personal_command_label(
            "view.model",
            "3D Perspective",
            &format!("{}  F3", Self::t(self.language, "3D Perspective")),
        );
        let plan_hover = self.personal_command_label(
            "view.plan",
            "Floor Plan",
            &Self::t(self.language, "Floor plan · F2"),
        );
        let model_hover = self.personal_command_label(
            "view.model",
            "3D Perspective",
            &Self::t(self.language, "Perspective 3D · F3"),
        );
        // On macOS the menus live in the native system menu bar instead of
        // this in-window strip (see src/app/native_menu.rs).
        if self.in_window_menus {
            egui::Panel::top("commands")
                .frame(
                    Frame::new()
                        .fill(PANEL)
                        .inner_margin(Margin::symmetric(10, 4)),
                )
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.menu_button(self.language.translate("Project"), |ui| {
                            for id in self.current_environment_profile().project_menu {
                                if let Some(command) =
                                    crate::workbench::CommandRegistry::resolve(&id)
                                {
                                    let reason =
                                        command.disabled_reason(&self.host.document.editor);
                                    if ui
                                        .add_enabled(
                                            reason.is_none(),
                                            Button::new(self.personal_command_label(
                                                command.id(),
                                                command.label(),
                                                self.language.translate(command.label()),
                                            )),
                                        )
                                        .on_hover_text(
                                            reason
                                                .map(|r| self.language.translate(r))
                                                .unwrap_or(&id),
                                        )
                                        .clicked()
                                    {
                                        self.dispatch_workbench(command);
                                        ui.close();
                                    }
                                } else {
                                    ui.add_enabled(false, Button::new(&id)).on_hover_text(
                                        self.language.translate(
                                            "Command provider is unavailable; saved ID retained",
                                        ),
                                    );
                                }
                            }
                        });
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
                                        "Customize work environment…",
                                        "MCP Settings…",
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
                                .button(self.personal_command_label(
                                    "file.save",
                                    "Save JSON…",
                                    &format!("{}      ⌘S", Self::t(self.language, "Save JSON…")),
                                ))
                                .clicked()
                            {
                                self.request_project_save();
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
                                if ui.button(&self.host.document.editor.project.name).clicked() {
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
                                self.external_content_menu(ui);
                            });
                            ui.menu_button(
                                self.language.translate("Libraries and Objects"),
                                |ui| {
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
                                },
                            );
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
                        ui.menu_button(Self::t(self.language, "Edit"), |ui| self.edit_menu(ui));
                        ui.menu_button(Self::t(self.language, "View"), |ui| {
                            if ui
                                .button(self.language.translate("AI collaboration"))
                                .clicked()
                            {
                                self.set_ai_workspace(true);
                                ui.close();
                            }
                            if ui
                                .button(Self::t(self.language, "AI Operations…"))
                                .clicked()
                            {
                                self.open_command_preview("AI Operations…".into());
                                ui.close();
                            }
                            if ui
                                .selectable_value(&mut self.view, View::Plan, plan_label.clone())
                                .clicked()
                            {
                                self.ensure_canvas_selection_mode();
                            }
                            if ui
                                .selectable_value(
                                    &mut self.view,
                                    View::Split,
                                    Self::t(self.language, "Split view"),
                                )
                                .clicked()
                            {
                                self.ensure_canvas_selection_mode();
                            }
                            if ui
                                .selectable_label(self.view == View::Model, model_label.clone())
                                .clicked()
                            {
                                self.dispatch_workbench(crate::workbench::WorkbenchCommand::Model);
                            }
                            if ui
                                .button(
                                    self.language
                                        .translate("Show selection / Marquee in 3D  F5"),
                                )
                                .clicked()
                            {
                                if self.host.document.editor.marquee.is_some() {
                                    self.show_marquee_3d = true;
                                    self.marquee_outside = false;
                                    self.fit_marquee();
                                } else if !self.host.document.editor.selection_ids().is_empty() {
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
                                if self.host.document.editor.marquee.is_some() {
                                    self.fit_marquee();
                                } else {
                                    self.fit_selection();
                                }
                            }
                            if self.show_marquee_3d && self.host.document.editor.marquee.is_some() {
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
                            let before_guides = self.host.document.editor.project.clone();
                            ui.menu_button(self.language.translate("Guides"), |ui| {
                                self.guides_menu(ui)
                            });
                            if ui
                                .checkbox(
                                    &mut self
                                        .host
                                        .document
                                        .editor
                                        .project
                                        .guide_settings
                                        .guide_lines_enabled,
                                    self.language.translate("Guide Lines"),
                                )
                                .changed()
                            {
                                self.host.document.editor.commit(before_guides);
                            }
                            ui.checkbox(
                                &mut self
                                    .host
                                    .document
                                    .editor
                                    .project
                                    .grid_settings
                                    .show_construction_grid,
                                self.language.translate("Construction Grid Display"),
                            );
                            ui.checkbox(
                                &mut self.host.document.editor.snap,
                                self.language.translate("Element Snap"),
                            );
                            ui.menu_button(self.language.translate("Snap Options"), |ui| {
                                ui.checkbox(
                                    &mut self.host.document.editor.snap,
                                    self.language.translate("Element Snap"),
                                );
                                let grid_snap = {
                                    let enabled = &mut self
                                        .host
                                        .document
                                        .editor
                                        .project
                                        .grid_settings
                                        .grid_snap_enabled;
                                    ui.checkbox(enabled, self.language.translate("Grid Snap"))
                                        .changed()
                                        .then_some(*enabled)
                                };
                                if let Some(enabled) = grid_snap {
                                    self.set_grid_snap_enabled(enabled);
                                }
                                let before = self.host.document.editor.project.clone();
                                if ui
                                    .checkbox(
                                        &mut self
                                            .host
                                            .document
                                            .editor
                                            .project
                                            .guide_settings
                                            .snap_enabled,
                                        self.language.translate("Snap Guides and Points"),
                                    )
                                    .changed()
                                {
                                    self.host.document.editor.commit(before);
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
                                    if self.host.document.editor.marquee.is_some() {
                                        self.fit_marquee();
                                    } else {
                                        self.fit_selection();
                                    }
                                }
                                if ui
                                    .button(self.language.translate("Hide Selection"))
                                    .clicked()
                                {
                                    self.host.document.editor.status =
                                        "3D selection visibility · interface preview".into();
                                }
                                if ui.button(self.language.translate("Show All")).clicked() {
                                    self.host.document.editor.status =
                                        "All elements shown in 3D".into();
                                }
                            });
                            ui.menu_button(self.language.translate("3D View Options"), |ui| {
                                for style in [
                                    crate::view_settings::Model3dStyle::Wireframe,
                                    crate::view_settings::Model3dStyle::Shaded,
                                    crate::view_settings::Model3dStyle::ShadedWithEdges,
                                ] {
                                    if ui
                                        .selectable_label(
                                            self.host
                                                .document
                                                .editor
                                                .project
                                                .plan_options
                                                .model_3d_style
                                                == style,
                                            self.language.translate(style.label()),
                                        )
                                        .clicked()
                                    {
                                        self.apply_3d_style(style);
                                        self.dispatch_workbench(
                                            crate::workbench::WorkbenchCommand::Model,
                                        );
                                        ui.close();
                                    }
                                }
                                ui.separator();
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
                                ] {
                                    if ui.button(self.language.translate(option)).clicked() {
                                        self.host.document.editor.status =
                                            format!("{option} · viewport interface preview");
                                        ui.close();
                                    }
                                }
                                ui.separator();
                                for (label, mode) in [
                                    ("Orbit", viewport_navigation::Mode::Orbit),
                                    ("Explore Model", viewport_navigation::Mode::Explore),
                                ] {
                                    if ui.button(self.language.translate(label)).clicked() {
                                        self.navigation_begin(mode);
                                        ui.close();
                                    }
                                }
                            });
                            ui.menu_button(self.language.translate("Zoom and Rebuild"), |ui| {
                                if ui
                                    .button(self.language.translate("Fit in Window · F"))
                                    .clicked()
                                {
                                    self.navigation_fit();
                                    ui.close();
                                }
                                if ui.button(self.language.translate("Rebuild")).clicked() {
                                    self.host.document.editor.status = "View refreshed".into();
                                    ui.close();
                                }
                                if ui.button(self.language.translate("Refresh")).clicked() {
                                    self.host.document.editor.status = "View refreshed".into();
                                }
                                if ui
                                    .button(self.language.translate("Rebuild & Regenerate"))
                                    .clicked()
                                {
                                    self.host.document.editor.status =
                                        "Model refresh requested".into();
                                }
                            });
                            ui.separator();
                            ui.menu_button(Self::t(self.language, "Language"), |ui| {
                                let previous = self.language;
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
                                if self.language != previous {
                                    self.language.save_preference();
                                    self.host.document.editor.status = format!(
                                        "{} · {}",
                                        self.language.translate("Language"),
                                        self.language.label()
                                    );
                                }
                            });
                        });
                        ui.menu_button(Self::t(self.language, "Design"), |ui| {
                            ui.menu_button(self.language.translate("Analysis"), |ui| {
                                if ui
                                    .button(self.language.translate("Check current design"))
                                    .clicked()
                                {
                                    self.dispatch_workbench(
                                        crate::workbench::WorkbenchCommand::Checks,
                                    );
                                    ui.close();
                                }
                            });
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
                                        "Concept mass",
                                        "Zone",
                                        "Mesh",
                                        "Morph",
                                        "Object",
                                        "Lamp",
                                    ],
                                ) {
                                    self.activate_named_tool(&command);
                                }
                            });
                            ui.menu_button(
                                self.language.translate("Structural Engineering Tools"),
                                |ui| {
                                    if let Some(command) = Self::preview_menu_items(
                                        ui,
                                        self.language,
                                        &[
                                            "Support",
                                            "Link",
                                            "Point Load",
                                            "Line Load",
                                            "Surface Load",
                                        ],
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
                                    &["RoofMaker…", "TrussMaker…", "Grid System…", "Update Zones"]
                                        [..],
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
                            if ui
                                .button(self.language.translate("Floor Plan Cut Plane…"))
                                .clicked()
                            {
                                self.open_plan_option_dialog(PlanOptionKind::FloorPlanCut);
                                ui.close();
                            }

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
                                &mut self.host.document.editor.snap,
                                self.language.translate("Snap to geometry and grid"),
                            );
                            ui.checkbox(
                                &mut self.host.document.editor.orthogonal,
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
                                        "Customize work environment…",
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
                                            self.open_layer_settings();
                                        } else if command == "Pens & Color…" {
                                            self.open_plan_option_dialog(PlanOptionKind::PenSets);
                                        } else if command == "Dimensions…" {
                                            self.open_plan_option_dialog(
                                                PlanOptionKind::Dimensions,
                                            );
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
                                self.host.document.editor.status =
                                    "Add-On Manager · interface preview".into();
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
                                self.host.document.editor.status =
                                    "Teamwork Palette · standalone preview".into();
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
                                        .selectable_label(
                                            selected,
                                            self.language.translate(palette),
                                        )
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
                                            self.open_plan_option_dialog(
                                                PlanOptionKind::DesignOptions,
                                            );
                                        } else if palette == "Renovation" {
                                            self.open_plan_option_dialog(
                                                PlanOptionKind::Renovation,
                                            );
                                        } else {
                                            self.host.document.editor.status =
                                                format!("Palette · {palette}");
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
                                    if matches!(toolbar, "Standard" | "Edit Elements") {
                                        let standard = toolbar == "Standard";
                                        let visible = if standard {
                                            self.standard_toolbar_visible
                                        } else {
                                            self.edit_toolbar_visible
                                        };
                                        let label = if standard {
                                            "Standard toolbar"
                                        } else {
                                            "Edit toolbar"
                                        };
                                        if ui
                                            .selectable_label(
                                                visible,
                                                self.language.translate(label),
                                            )
                                            .clicked()
                                        {
                                            self.toggle_work_toolbar(standard);
                                            ui.close();
                                        }
                                        continue;
                                    }
                                    if ui
                                        .selectable_label(false, self.language.translate(toolbar))
                                        .clicked()
                                    {
                                        self.host.document.editor.status =
                                            format!("Toolbar · {toolbar}");
                                    }
                                }
                            });
                            ui.separator();
                            if ui
                                .button(self.language.translate("Save workspace layout"))
                                .clicked()
                            {
                                self.dispatch_workbench(
                                    crate::workbench::WorkbenchCommand::SaveWorkspace,
                                );
                                ui.close();
                            }
                            if ui
                                .button(self.language.translate("Reset workspace layout"))
                                .clicked()
                            {
                                self.dispatch_workbench(
                                    crate::workbench::WorkbenchCommand::ResetWorkspace,
                                );
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
                                !self.host.document.editor.undo.is_empty(),
                                Button::new(self.language.translate("Undo")),
                            )
                            .on_hover_text(self.personal_command_label(
                                "model.undo",
                                "Undo",
                                &Self::t(self.language, "Undo · ⌘Z"),
                            ))
                            .clicked()
                        {
                            self.dispatch_history(false);
                        }
                        if ui
                            .add_enabled(
                                !self.host.document.editor.redo.is_empty(),
                                Button::new(self.language.translate("Redo")),
                            )
                            .on_hover_text(self.personal_command_label(
                                "model.redo",
                                "Redo",
                                &Self::t(self.language, "Redo · ⌥⌘Z"),
                            ))
                            .clicked()
                        {
                            self.dispatch_history(true);
                        }
                        ui.separator();
                        if Self::vector_labeled_button(
                            ui,
                            "Fit",
                            &Self::t(self.language, "Fit"),
                            &Self::t(self.language, "Fit model · F"),
                            false,
                        ) {
                            self.navigation_fit();
                        }
                        ui.add_space(12.);
                        if ui
                            .selectable_label(self.view == View::Plan, plan2_label.clone())
                            .clicked()
                        {
                            self.dispatch_workbench(crate::workbench::WorkbenchCommand::Plan);
                        }
                        if ui
                            .selectable_value(
                                &mut self.view,
                                View::Split,
                                Self::t(self.language, "Split view"),
                            )
                            .clicked()
                        {
                            self.ensure_canvas_selection_mode();
                        }
                        if ui
                            .selectable_label(self.view == View::Model, model_label.clone())
                            .clicked()
                        {
                            self.dispatch_workbench(crate::workbench::WorkbenchCommand::Model);
                        }
                        if self.host.document.editor.marquee.is_some() {
                            ui.separator();
                            if ui
                                .checkbox(
                                    &mut self.show_marquee_3d,
                                    self.language.translate("3D Marquee"),
                                )
                                .changed()
                                && self.show_marquee_3d
                            {
                                if self.host.document.editor.marquee.is_some() {
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
                                self.request_project_save();
                            }
                        });
                    });
                });
        }
        if self.ai_workspace.enabled {
            return;
        }
        if self.standard_toolbar_visible {
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
                            if Self::vector_icon_button(
                                ui,
                                icon,
                                &Self::t(self.language, tip),
                                false,
                            ) {
                                match icon {
                                    "Home" => self.navigation_fit(),
                                    "Undo" => {
                                        self.dispatch_history(false);
                                    }
                                    "Redo" => {
                                        self.dispatch_history(true);
                                    }
                                    "Save" => self.request_project_save(),
                                    "Zoom In" => self.navigation_zoom(1.15),
                                    "Zoom Out" => self.navigation_zoom(1. / 1.15),
                                    _ => {}
                                }
                            }
                        }
                        ui.separator();
                        if Self::vector_icon_button(
                            ui,
                            "Pick Up",
                            &self.pick_up_tooltip(),
                            self.pick_armed,
                        ) {
                            self.pick_up_command();
                        }
                        if Self::vector_icon_button(
                            ui,
                            "Inject",
                            &self.inject_tooltip(),
                            self.inject_armed,
                        ) {
                            self.inject_command();
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
                        self.trace_toolbar_menu(ui);
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
                        self.reshape_toolbar(ui);
                        if ui
                            .button(self.language.translate("Drawing Manager"))
                            .clicked()
                        {
                            self.dispatch_workbench(crate::workbench::WorkbenchCommand::Drawings);
                        }
                        ui.separator();
                        if ui
                            .selectable_value(&mut self.view, View::Plan, plan_hover.clone())
                            .clicked()
                        {
                            self.ensure_canvas_selection_mode();
                        }
                        if ui
                            .selectable_label(self.view == View::Model, model_hover.clone())
                            .clicked()
                        {
                            self.dispatch_workbench(crate::workbench::WorkbenchCommand::Model);
                        }
                    });
                });
        }
        if self.edit_toolbar_visible {
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
                            self.host.document.editor.status =
                                "Drag command · select an element and drag it in the canvas".into();
                        }
                        if Self::vector_labeled_button(
                            ui,
                            "Rotate",
                            &Self::t(self.language, "Rotate"),
                            &Self::t(self.language, "Rotate selection 90°"),
                            false,
                        ) {
                            self.host.document.editor.status =
                                "Rotate command · choose a pivot in the canvas".into();
                        }
                        if Self::vector_labeled_button(
                            ui,
                            "Mirror",
                            &Self::t(self.language, "Mirror"),
                            &Self::t(self.language, "Mirror selection"),
                            false,
                        ) {
                            self.host.document.editor.status =
                                "Mirror command · choose an axis in the canvas".into();
                        }
                        if Self::vector_labeled_button(
                            ui,
                            "Multiply",
                            &Self::t(self.language, "Multiply"),
                            &Self::t(self.language, "Duplicate selection"),
                            false,
                        ) {
                            self.dispatch_edit("Multiply…");
                        }
                        ui.separator();
                        if ui
                            .button(self.language.translate("AI collaboration"))
                            .clicked()
                        {
                            self.set_ai_workspace(true);
                        }
                    });
                });
        }
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
                            for story in self.host.document.editor.project.stories.clone() { let _ = ui.selectable_label(story.id == self.host.document.editor.story, format!("{} · {}", story.id, story.name)); }
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
                        let tool_name = match self.host.document.editor.tool {
                            Tool::ConceptMass => "Concept mass",
                            Tool::Zone => "Zone",
                            Tool::Roof => "Roof",
                            Tool::Beam => "Beam",
                            Tool::Column => "Column",
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
                    } else if matches!(self.host.document.editor.tool, Tool::Door | Tool::Window) {
                        self.info_box_opening_tool_properties(ui);
                    } else if self.host.document.editor.tool == Tool::ConceptMass {
                        self.concept_mass_tool_ui(ui);
                    } else if self.host.document.editor.tool == Tool::Zone {
                        self.zone_tool_ui(ui);
                    } else if self.host.document.editor.tool == Tool::Roof {
                        use crate::roof::RoofGeometryMethod;
                        ui.add_enabled_ui(self.host.document.editor.roof_draft.is_none() && self.host.document.editor.roof_polygon_draft().is_none(), |ui| {
                            ui.selectable_value(&mut self.host.document.editor.roof_method, RoofGeometryMethod::Polygonal, self.language.translate("Polygonal"));
                            ui.selectable_value(&mut self.host.document.editor.roof_method, RoofGeometryMethod::Rectangular, self.language.translate("Rectangular"));
                            ui.selectable_value(&mut self.host.document.editor.roof_method, RoofGeometryMethod::RotatedRectangle, self.language.translate("Rotated Rectangle"));
                        }).response.on_hover_text(self.language.translate("Choose the roof creation method before placing points"));
                        ui.label(self.language.translate("Roof pitch"));
                        ui.add(DragValue::new(&mut self.host.document.editor.roof_size[0]).range(-80. ..=80.).speed(0.1).suffix("°"));
                        ui.label(self.language.translate("Vertical thickness"));
                        ui.add(DragValue::new(&mut self.host.document.editor.roof_size[1]).range(0.01..=5.).speed(0.01).suffix(" m"));
                        ui.label(self.language.translate("Pivot offset"));
                        ui.add(DragValue::new(&mut self.host.document.editor.roof_size[2]).range(-100. ..=100.).speed(0.05).suffix(" m"));
                        ui.label(self.language.translate("Single plane · Bottom pivot"));
                        ComboBox::from_id_salt("roof_material").selected_text(self.language.translate(self.host.document.editor.roof_material.name())).show_ui(ui,|ui|{for m in Material::ALL {ui.selectable_value(&mut self.host.document.editor.roof_material,m,self.language.translate(m.name()));}});
                        ComboBox::from_id_salt("roof_layer").selected_text(self.host.document.editor.project.layers.layers.iter().find(|l|l.id==self.host.document.editor.roof_layer.0).map_or("Layer",|l|l.name.as_str())).show_ui(ui,|ui|{for l in &self.host.document.editor.project.layers.layers {ui.selectable_value(&mut self.host.document.editor.roof_layer,crate::layers::ElementLayer(l.id),&l.name);}});
                    } else if self.host.document.editor.tool == Tool::Beam {
                        let curved = self.host.document.editor.beam_method == crate::beam_creation::BeamGeometryMethod::CenterRadius;
                        for method in [crate::beam_creation::BeamGeometryMethod::Single, crate::beam_creation::BeamGeometryMethod::CenterRadius] {
                            if ui.selectable_label(self.host.document.editor.beam_method == method, self.language.translate(method.label())).clicked() && self.host.document.editor.beam_method != method {
                                self.host.document.editor.set_beam_method(method);
                                self.tracker_locked = false;
                            }
                        }
                        ui.label(self.language.translate("Width"));
                        ui.add(DragValue::new(&mut self.host.document.editor.beam_size[0]).range(0.01..=100.).speed(0.01).suffix(" m"));
                        ui.label(self.language.translate("Height")).on_hover_text(self.language.translate("Section height perpendicular to the beam axis"));
                        ui.add(DragValue::new(&mut self.host.document.editor.beam_size[1]).range(0.01..=100.).speed(0.01).suffix(" m"));
                        ui.label(self.language.translate("Beam top offset")).on_hover_text(self.language.translate("At the start point, relative to the home story"));
                        ui.add(DragValue::new(&mut self.host.document.editor.beam_size[2]).range(-100. ..=100.).speed(0.05).suffix(" m"));
                        let angle_label = ui.label(self.language.translate("Slant angle"));
                        ui.add_enabled(!curved, DragValue::new(&mut self.host.document.editor.beam_slant_angle).range(-89. ..=89.).speed(0.5).fixed_decimals(1).suffix("°"))
                            .labelled_by(angle_label.id)
                            .on_hover_text(self.language.translate("Angle from the start point; positive rises toward the end"));
                        ui.label(self.language.translate(if curved || self.host.document.editor.beam_slant_angle == 0. { "Horizontal · Top-center axis" } else { "Slanted · Top-center axis" }));
                        ComboBox::from_id_salt("beam_material").selected_text(self.language.translate(self.host.document.editor.beam_material.name())).show_ui(ui, |ui| {
                            for material in Material::ALL { ui.selectable_value(&mut self.host.document.editor.beam_material, material, self.language.translate(material.name())); }
                        });
                        ComboBox::from_id_salt("beam_layer").selected_text(self.host.document.editor.project.layers.layers.iter().find(|l| l.id==self.host.document.editor.beam_layer.0).map_or("Layer", |l| l.name.as_str())).show_ui(ui, |ui| {
                            for layer in &self.host.document.editor.project.layers.layers {ui.selectable_value(&mut self.host.document.editor.beam_layer, crate::layers::ElementLayer(layer.id), &layer.name);}
                        });
                    } else if self.host.document.editor.tool == Tool::Column {
                        let circular = self.host.document.editor.column_section.is_circular();
                        for (value, label) in [(false, "Rectangular"), (true, "Circular")] {
                            if ui.selectable_label(circular == value, self.language.translate(label)).clicked() {
                                self.host.document.editor.column_section = self.host.document.editor.column_section.with_circular(value);
                            }
                        }
                        match &mut self.host.document.editor.column_section {
                            crate::column::ColumnSection::Rectangular { width, depth } => {
                                let label = ui.label(self.language.translate("Width"));
                                ui.add(DragValue::new(width).range(0.01..=100.).speed(0.01).suffix(" m")).labelled_by(label.id);
                                let label = ui.label(self.language.translate("Depth"));
                                ui.add(DragValue::new(depth).range(0.01..=100.).speed(0.01).suffix(" m")).labelled_by(label.id);
                            }
                            crate::column::ColumnSection::Circular { diameter } => {
                                let label = ui.label(self.language.translate("Diameter"));
                                ui.add(DragValue::new(diameter).range(0.01..=100.).speed(0.01).suffix(" m")).labelled_by(label.id);
                            }
                        }
                        let height_label = ui.label(self.language.translate("Height"));
                        ui.add(DragValue::new(&mut self.host.document.editor.column_height).range(0.01..=100.).speed(0.05).suffix(" m")).labelled_by(height_label.id);
                        let offset_label = ui.label(self.language.translate("Bottom offset to story"));
                        ui.add(DragValue::new(&mut self.host.document.editor.column_bottom_offset).speed(0.01).fixed_decimals(3).suffix(" m")).labelled_by(offset_label.id);
                        if !self.host.document.editor.column_section.is_circular() {
                        let angle_label = ui.label(self.language.translate("Axis rotation"));
                        ui.add(DragValue::new(&mut self.host.document.editor.column_axis_rotation).speed(1.).fixed_decimals(1).suffix("°")).labelled_by(angle_label.id);
                        }
                        self.column_positioning_menu(ui);
                        ComboBox::from_id_salt("column_material").selected_text(self.language.translate(self.host.document.editor.column_material.name())).show_ui(ui, |ui| {
                            for material in Material::ALL { ui.selectable_value(&mut self.host.document.editor.column_material, material, self.language.translate(material.name())); }
                        });
                        ComboBox::from_id_salt("column_layer").selected_text(self.host.document.editor.project.layers.layers.iter().find(|l| l.id==self.host.document.editor.column_layer.0).map_or("Layer", |l| l.name.as_str())).show_ui(ui, |ui| {
                            for layer in &self.host.document.editor.project.layers.layers {ui.selectable_value(&mut self.host.document.editor.column_layer, crate::layers::ElementLayer(layer.id), &layer.name);}
                        });
                    } else if self.host.document.editor.tool == Tool::Wall {
                        self.info_box_wall_tool_properties(ui);
                        ui.separator();
                        ui.label(Self::t(self.language, "Material"));
                        ComboBox::from_id_salt("wall_tool_material")
                            .selected_text(self.host.document.editor.material.name())
                            .width(118.)
                            .show_ui(ui, |ui| {
                                for m in Material::ALL {
                                    ui.selectable_value(&mut self.host.document.editor.material, m, m.name());
                                }
                            });
                        ui.checkbox(&mut self.host.document.editor.snap, self.language.translate("Snap"))
                            .on_hover_text(Self::t(
                                self.language,
                                "Endpoints / midpoints / 100 mm grid",
                            ));
                        ui.checkbox(
                            &mut self.host.document.editor.orthogonal,
                            self.language.translate("Ortho"),
                        )
                        .on_hover_text(Self::t(
                            self.language,
                            "Constrain to X/Y; Shift temporarily constrains",
                        ));
                    } else if self.host.document.editor.tool == Tool::Marquee {
                        ComboBox::from_id_salt("marquee_geometry")
                            .selected_text(self.language.translate(match self.marquee_geometry {
                                MarqueeGeometry::Rectangle => "Rectangle",
                                MarqueeGeometry::RotatedRectangle => "Rotated rectangle",
                                MarqueeGeometry::Polygon => "Polygon",
                            }))
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut self.marquee_geometry, MarqueeGeometry::Rectangle, if self.language == UiLanguage::SimplifiedChinese { "矩形 · 拖动" } else { "Rectangle · drag" });
                                ui.selectable_value(&mut self.marquee_geometry, MarqueeGeometry::RotatedRectangle, if self.language == UiLanguage::SimplifiedChinese { "旋转矩形 · 三点" } else { "Rotated rectangle · 3 points" });
                                ui.selectable_value(&mut self.marquee_geometry, MarqueeGeometry::Polygon, if self.language == UiLanguage::SimplifiedChinese { "多边形 · 点击顶点，右键闭合" } else { "Polygon · click vertices, right-click to close" });
                            });
                        ui.selectable_value(&mut self.marquee_scope, MarqueeScope::CurrentStory, Self::t(self.language, "Single story"));
                        ui.selectable_value(&mut self.marquee_scope, MarqueeScope::AllStories, Self::t(self.language, "All stories"));
                        ui.label(Self::t(self.language, "3D depth"));
                        ui.add(DragValue::new(&mut self.marquee_height).speed(0.05).range(0.01..=100.).fixed_decimals(2).suffix(" m"));
                        if self.host.document.editor.marquee.is_some() {
                            if ui.button(Self::t(self.language, "Select all in Marquee · ⌘A")).clicked() {
                                let count = self.host.document.editor.select_all_in_marquee();
                                self.host.document.editor.status = format!("Selected {count} element(s) in Marquee");
                            }
                            if ui.button(Self::t(self.language, "Remove Marquee · Esc")).clicked() {
                                self.host.document.editor.remove_marquee();
                                self.host.document.editor.set_selection(Vec::new());
                            }
                            ui.label(self.language.translate("ΔX"));
                            ui.add(DragValue::new(&mut self.marquee_dx).speed(0.01).fixed_decimals(3).suffix(" m"));
                            ui.label(self.language.translate("ΔY"));
                            ui.add(DragValue::new(&mut self.marquee_dy).speed(0.01).fixed_decimals(3).suffix(" m"));
                            if ui.button(Self::t(self.language, "Move region")).clicked() {
                                self.host.document.editor.move_marquee(Point::new(self.marquee_dx, self.marquee_dy));
                            }
                            if ui.button(Self::t(self.language, "Stretch enclosed nodes")).clicked() {
                                match self.host.document.editor.stretch_nodes_in_marquee(Point::new(self.marquee_dx, self.marquee_dy)) {
                                    Ok(0) => self.host.document.editor.status = "No editable nodes inside Marquee".into(),
                                    Ok(_) => {},
                                    Err(error) => self.host.document.editor.status = error,
                                }
                            }
                            if !self.host.document.editor.selection_ids().is_empty() {
                                ui.separator();
                                ui.label(Self::t(self.language, "Selection operations"));
                                if ui.button(self.language.translate("Cut · ⌘X")).clicked() { self.host.document.editor.cut_selection(); }
                                if ui.button(self.language.translate("Copy · ⌘C")).clicked() { self.host.document.editor.copy_selection(); }
                                if ui.button(self.language.translate("Mirror X")).clicked() { self.host.document.editor.mirror_selection(true); }
                                if ui.button(self.language.translate("Mirror Y")).clicked() { self.host.document.editor.mirror_selection(false); }
                                if ui.button(self.language.translate("Rotate 90°")).clicked() { self.host.document.editor.rotate_selection_quarter_turn(false); }
                                if ui.button(self.language.translate("Move to story above")).clicked() { self.host.document.editor.move_selection_to_story(1); }
                                if ui.button(self.language.translate("Move to story below")).clicked() { self.host.document.editor.move_selection_to_story(-1); }
                                if ui.button(self.language.translate("Move selected elements")).clicked() {
                                    self.host.document.editor.move_selection(Point::new(self.marquee_dx, self.marquee_dy));
                                }
                                if ui.button(self.language.translate("Duplicate selected")).clicked() {
                                    self.host.document.editor.duplicate();
                                }
                                if ui.button(self.language.translate("Delete selected · Delete")).clicked() {
                                    self.host.document.editor.delete();
                                }
                                ui.label(self.selected_count_label(self.host.document.editor.selection_ids().len()));
                            }
                        }
                    } else if self.host.document.editor.tool == Tool::Select {
                        ui.separator();
                        ui.label(RichText::new(Self::t(self.language, "Selection Settings")).strong());
                        ui.checkbox(&mut self.select_sub_elements, Self::t(self.language, "Sub-element selection"))
                            .on_hover_text(Self::t(self.language, "Toggle element / sub-element selection mode"));
                        ui.checkbox(&mut self.select_locked_elements, Self::t(self.language, "Include locked elements"));
                        ui.separator();
                        ui.label(Self::t(self.language, "Selection methods"));
                        let _ = Self::vector_labeled_button(ui, "Element", &Self::t(self.language, "Element"), &Self::t(self.language, "Select whole elements"), true);
                        let _ = Self::vector_labeled_button(ui, "Area Selection", &Self::t(self.language, "Area"), &Self::t(self.language, "Select by area"), false);
                        ui.checkbox(&mut self.canvas_pick.surface, self.language.translate("Surface quick selection"))
                            .on_hover_text(self.language.translate("Space-click temporarily reverses surface selection; Shift+Space-click in other tools"));
                        ui.label(RichText::new(self.selected_count_label(self.host.document.editor.selection_ids().len())).color(MUTED));
                    } else {
                        ui.separator();
                        if self.host.document.editor.tool == Tool::Slab {
                            use crate::slab_creation::SlabGeometryMethod;
                            let idle = self.host.document.editor.anchor.is_none() && self.host.document.editor.slab_draft.is_none() && self.host.document.editor.slab_polygon_draft().is_none();
                            let previous = self.host.document.editor.slab_method;
                            ui.add_enabled_ui(idle, |ui| {
                                ui.selectable_value(&mut self.host.document.editor.slab_method, SlabGeometryMethod::Rectangular, self.language.translate("Rectangular"));
                                ui.selectable_value(&mut self.host.document.editor.slab_method, SlabGeometryMethod::RotatedRectangle, self.language.translate("Rotated Rectangle"));
                                ui.selectable_value(&mut self.host.document.editor.slab_method, SlabGeometryMethod::Polygonal, self.language.translate("Polygonal"));
                            }).response.on_hover_text(self.language.translate("Choose the slab creation method before placing points"));
                            if previous != self.host.document.editor.slab_method {
                                self.host.document.editor.status = self.host.document.editor.slab_method.status_key().into();
                                self.tracker_locked = false;
                            }
                        }
                        if self.host.document.editor.tool == Tool::Polyline {
                            ui.label(self.language.translate("2D drafting · left-click nodes · right-click to finish · click start to close"));
                        } else if self.host.document.editor.tool == Tool::Stair {
                            ui.label(self.language.translate("{risers} risers · {width} m wide · {rise} m rise")
                                .replace("{risers}", &self.host.document.editor.stair_spec.risers.to_string())
                                .replace("{width}", &format!("{:.2}", self.host.document.editor.stair_spec.width))
                                .replace("{rise}", &format!("{:.2}", self.host.document.editor.stair_spec.total_rise)));
                            if ui.button(self.language.translate("Stair Settings…")).clicked() {
                                self.stair_settings_draft = self.host.document.editor.stair_spec.clone();
                                self.stair_settings_error.clear();
                                self.stair_settings_open = true;
                            }
                        } else {
                            ui.label(Self::t(self.language, "Thickness"));
                            ui.add(
                                DragValue::new(&mut self.host.document.editor.thickness)
                                    .speed(0.01)
                                    .range(0.01..=5.)
                                    .fixed_decimals(3)
                                    .suffix(" m"),
                            );
                        }
                        if self.host.document.editor.tool == Tool::Slab {
                            let elevation_label = ui.label(self.language.translate("Top offset to story"));
                            ui.add(DragValue::new(&mut self.host.document.editor.slab_elevation)
                                .speed(0.01).fixed_decimals(3).suffix(" m"))
                                .labelled_by(elevation_label.id)
                                .on_hover_text(self.language.translate("Slab top relative to the home story; thickness extends downward"));
                        }
                        if self.host.document.editor.tool != Tool::Slab && self.host.document.editor.tool != Tool::Stair && self.host.document.editor.tool != Tool::Polyline {
                            ui.label(Self::t(self.language, "Height"));
                            ui.add(
                                DragValue::new(&mut self.host.document.editor.height)
                                    .speed(0.05)
                                    .range(0.01..=100.)
                                    .fixed_decimals(2)
                                    .suffix(" m"),
                            );
                        }
                        ComboBox::from_id_salt("default_material")
                            .selected_text(self.language.translate(self.host.document.editor.material.name()))
                            .width(118.)
                            .show_ui(ui, |ui| {
                                for m in Material::ALL {
                                    ui.selectable_value(&mut self.host.document.editor.material, m, self.language.translate(m.name()));
                                }
                            });
                        if self.host.document.editor.tool == Tool::Slab {
                            ComboBox::from_id_salt("slab_layer").selected_text(self.host.document.editor.project.layers.layers.iter().find(|l| l.id == self.host.document.editor.slab_layer.0).map_or("Layer", |l| l.name.as_str())).show_ui(ui, |ui| {
                                for layer in &self.host.document.editor.project.layers.layers {
                                    ui.selectable_value(&mut self.host.document.editor.slab_layer, crate::layers::ElementLayer(layer.id), &layer.name);
                                }
                            });
                        }
                        ui.separator();
                        ui.checkbox(&mut self.host.document.editor.snap, self.language.translate("Snap"))
                            .on_hover_text(Self::t(self.language, "Endpoints / midpoints / 100 mm grid"));
                        ui.checkbox(&mut self.host.document.editor.orthogonal, self.language.translate("Ortho"))
                            .on_hover_text(Self::t(self.language, "Constrain to X/Y; Shift temporarily constrains"));
                    }
                });
                // Keep validation messages together below the wrapped controls.
                if self.preview_tool.is_none()
                    && self.info_box_opening.is_none()
                    && let Some(state) = &self.info_box_wall
                    && !state.error.is_empty()
                {
                    ui.colored_label(Color32::from_rgb(224, 125, 105), &state.error);
                }
            });
        }
    }
    /// Activate a named Architectural/Documenting tool entry. Shared by the
    /// egui Design menu and the native macOS menu bar.
    pub(super) fn activate_named_tool(&mut self, command: &str) {
        use crate::workbench::WorkbenchCommand;
        match command {
            "Arrow" => self.dispatch_workbench(WorkbenchCommand::Select),
            "Wall" => self.dispatch_workbench(WorkbenchCommand::Wall),
            "Concept mass" => self.dispatch_workbench(WorkbenchCommand::ConceptMass),
            "Zone" => self.dispatch_workbench(WorkbenchCommand::Zone),
            _ => self.activate_named_tool_legacy(command),
        }
    }
    pub(super) fn activate_named_tool_legacy(&mut self, command: &str) {
        if matches!(self.view, View::Figure(_)) {
            self.view = View::Plan;
        }
        match command {
            "Concept mass" => {
                self.preview_tool = None;
                self.host.document.editor.set_tool(Tool::ConceptMass);
            }
            "Zone" => {
                self.preview_tool = None;
                self.host.document.editor.set_tool(Tool::Zone);
            }
            "Wall" => {
                self.host.document.editor.set_tool(Tool::Wall);
                self.preview_tool = None;
            }
            "Slab" => {
                self.host.document.editor.set_tool(Tool::Slab);
                self.preview_tool = None;
            }
            "Roof" => {
                self.host.document.editor.set_tool(Tool::Roof);
                self.preview_tool = None;
                self.tracker_locked = false;
            }
            "Beam" => {
                self.host.document.editor.set_tool(Tool::Beam);
                self.preview_tool = None;
                self.tracker_locked = false;
            }
            "Column" => {
                self.host.document.editor.set_tool(Tool::Column);
                self.preview_tool = None;
                self.tracker_locked = false;
            }
            "Stair" => self.activate_stair_tool(),
            "Polyline" => self.activate_polyline_tool(),
            "Marquee" => {
                self.host.document.editor.set_tool(Tool::Marquee);
                self.preview_tool = None;
            }
            "Arrow" => {
                self.host.document.editor.set_tool(Tool::Select);
                self.preview_tool = None;
            }
            _ => {
                self.preview_tool = Some(command.to_owned());
                self.host.document.editor.set_tool(Tool::Select);
                self.host.document.editor.status =
                    format!("{command} tool selected · interface preview");
            }
        }
        self.tracker_locked = false;
    }
    pub(super) fn toolbox_contents(&mut self, ui: &mut Ui) {
        ScrollArea::vertical().id_salt("toolbox_scroll").show(ui, |ui| {
                self.workbench_project_navigation(ui);
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
                    let active = self.preview_tool.is_none() && self.host.document.editor.tool == t;
                    let id=match t {Tool::Select=>Some("tool.select"),Tool::Wall=>Some("tool.wall"),Tool::ConceptMass=>Some("tool.concept_mass"),Tool::Zone=>Some("tool.zone"),_=>None};
                    let hint=id.filter(|id|self.shortcut_overridden(id)).map(|id|crate::workbench::shortcuts::hint(self.shortcut_overrides(),id)).unwrap_or_else(||key.into());
                    if Self::toolbox_tool_row(ui, label, &Self::t(self.language, label), active, if hint.is_empty(){None}else{Some(&hint)}) {
                        self.host.document.editor.set_tool(t);
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
                    (Tool::ConceptMass,"Concept mass",""),
                    (Tool::Zone,"Zone",""),
                    (Tool::Slab, "Slab", "L"),
                    (Tool::Column, "Column", ""),
                    (Tool::Beam, "Beam", ""),
                    (Tool::Roof,"Roof",""),
                ] {
                    let active = self.preview_tool.is_none() && self.host.document.editor.tool == t;
                    let id=match t {Tool::Select=>Some("tool.select"),Tool::Wall=>Some("tool.wall"),Tool::ConceptMass=>Some("tool.concept_mass"),Tool::Zone=>Some("tool.zone"),_=>None};
                    let hint=id.filter(|id|self.shortcut_overridden(id)).map(|id|crate::workbench::shortcuts::hint(self.shortcut_overrides(),id)).unwrap_or_else(||key.into());
                    if Self::toolbox_tool_row(ui, label, &Self::t(self.language, label), active, if hint.is_empty(){None}else{Some(&hint)}) {
                        self.host.document.editor.set_tool(t);
                        self.preview_tool = None;
                        self.tracker_locked = false;
                    }
                }
                if Self::toolbox_tool_row(ui, "Stair", self.language.translate("Stair"), self.preview_tool.is_none() && self.host.document.editor.tool == Tool::Stair, Some("S")) {
                    self.activate_stair_tool();
                }
                for (t, label) in [(Tool::Door, "Door"), (Tool::Window, "Window")] {
                    let active = self.preview_tool.is_none() && self.host.document.editor.tool == t;
                    if Self::toolbox_tool_row(ui, label, &Self::t(self.language, label), active, None) {
                        self.preview_tool = None;
                        self.host.document.editor.set_tool(t);
                        self.tracker_locked = false;
                    }
                }
                for name in ["Object", "Lamp", "Shell", "Railing", "Curtain Wall", "Skylight", "Corner-Window", "Opening", "Wall End", "Mesh", "Morph"] {
                    if Self::toolbox_tool_row(ui, name, &Self::t(self.language, name), self.preview_tool.as_deref() == Some(name), None) {
                        self.preview_tool = Some(name.to_owned());
                        self.host.document.editor.set_tool(Tool::Select);
                        self.host.document.editor.status = format!("{name} tool selected · model interaction is not available in this demo");
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
                                self.preview_tool.is_none() && self.host.document.editor.tool == Tool::Polyline
                            } else {
                                self.preview_tool.as_deref() == Some(*name)
                            };
                            if Self::toolbox_tool_row(ui, name, &Self::t(self.language, name), active, polyline.then_some("P")) {
                                if polyline {
                                    self.activate_polyline_tool();
                                    continue;
                                }
                                self.preview_tool = Some((*name).to_owned());
                                self.host.document.editor.set_tool(Tool::Select);
                                self.host.document.editor.status = format!("{name} tool selected · interface preview");
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
    }
}
