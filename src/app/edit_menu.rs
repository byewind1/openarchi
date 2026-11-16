use super::*;
use crate::editing::EditCommand;

// One catalog feeds both native macOS and egui menus. Unsupported commands
// stay disabled; they never open a configuration-preview success dialog.
pub(super) const EDIT_GROUPS: &[(&str, &[&str])] = &[
    (
        "",
        &[
            "Undo",
            "Redo",
            "Repeat Last Command",
            "Cut",
            "Copy",
            "Paste",
            "Delete",
            "Select All",
            "Find & Select…",
            "Search & Replace Text…",
        ],
    ),
    (
        "Groups",
        &["Group", "Ungroup", "Suspend Groups", "Autogroup"],
    ),
    (
        "Display Order",
        &[
            "Bring to Front",
            "Bring Forward",
            "Send Backward",
            "Send to Back",
            "Reset Default Order",
        ],
    ),
    ("Locking", &["Lock", "Unlock", "Unlock All"]),
    (
        "Move",
        &[
            "Drag",
            "Rotate",
            "Quick Rotate Clockwise",
            "Quick Rotate Counter-Clockwise",
            "Mirror",
            "Elevate…",
            "Drag a Copy",
            "Rotate a Copy",
            "Mirror a Copy",
            "Drag Multiple Copies",
            "Rotate Multiple Copies",
            "Multiply…",
            "Edit Elements by Stories…",
        ],
    ),
    (
        "Align",
        &[
            "Horizontal Left",
            "Horizontal Center",
            "Horizontal Right",
            "Vertical Top",
            "Vertical Center",
            "Vertical Bottom",
            "Special Align",
        ],
    ),
    (
        "Distribute",
        &["Along X", "Along Y", "Along XY", "Special Distribute"],
    ),
    (
        "Reshape",
        &[
            "Trim",
            "Magic Wand",
            "Trim Elements to Roof/Shell",
            "Stretch",
            "Resize…",
            "Split",
            "Fillet/Chamfer…",
            "Intersect",
            "Adjust",
            "Offset",
            "Explode into Current View",
            "Unify",
            "Linework Consolidation…",
            "Fill Consolidation…",
            "Split into Single-plane Roofs",
            "Adjust Elements to Slabs…",
            "Merge Beams",
            "Merge Columns",
            "Merge Walls",
        ],
    ),
    (
        "Element Settings",
        &[
            "Tool Settings",
            "Edit Selection Set…",
            "Pick Up Parameters",
            "Inject Parameters",
            "Element Transfer Settings…",
            "Relink Home Story…",
        ],
    ),
    (
        "Reference Line and Plane",
        &[
            "Modify Wall Reference Line…",
            "Invert Wall Direction",
            "Modify Slab Reference Plane…",
        ],
    ),
];
pub(super) fn implemented(name: &str) -> bool {
    !matches!(
        name,
        "Search & Replace Text…"
            | "Special Align"
            | "Special Distribute"
            | "Trim Elements to Roof/Shell"
            | "Stretch"
            | "Fill Consolidation…"
            | "Split into Single-plane Roofs"
            | "Adjust Elements to Slabs…"
            | "Merge Beams"
            | "Merge Columns"
            | "Modify Slab Reference Plane…"
            | "Edit Selection Set…"
    )
}
#[derive(Clone, Debug, PartialEq)]
pub(super) struct EditDialog {
    name: String,
    ids: Vec<u64>,
    revision: u64,
    x: f64,
    y: f64,
    origin: Point,
    angle: f64,
    scale: f64,
    copies: usize,
    story: u32,
    query: String,
    kind: String,
    error: String,
    preview: Vec<Vec<Point>>,
    node: usize,
    chamfer: bool,
}
impl EditDialog {
    fn command(&self) -> EditCommand {
        match self.name.as_str() {
            "Fillet/Chamfer…" => EditCommand::Corner {
                node: self.node,
                radius: self.x,
                chamfer: self.chamfer,
            },
            "Elevate…" => EditCommand::Elevate(self.x),
            "Offset" => EditCommand::Offset(self.x),
            "Edit Elements by Stories…" => EditCommand::Story {
                target: self.story,
                preserve_elevation: false,
            },
            "Relink Home Story…" => EditCommand::Story {
                target: self.story,
                preserve_elevation: true,
            },
            "Modify Wall Reference Line…" => EditCommand::WallReference(self.x),
            "Split" => EditCommand::Split {
                a: self.origin,
                b: Point::new(self.x, self.y),
            },
            "Adjust" => EditCommand::Adjust {
                a: self.origin,
                b: Point::new(self.x, self.y),
            },
            _ => EditCommand::Transform {
                offset: Point::new(self.x, self.y),
                origin: self.origin,
                angle: self.angle,
                scale: self.scale,
                mirror: self.name.starts_with("Mirror"),
                copies: self.copies,
            },
        }
    }
}
impl AtelierApp {
    pub(super) fn edit_command_enabled(&self, name: &str) -> bool {
        if self.navigation.mode != viewport_navigation::Mode::Edit {
            return false;
        }
        if matches!(self.view, View::Figure(_)) && !matches!(name, "Undo" | "Redo") {
            return false;
        }
        if name == "Delete" && self.drawings_ui.canvas_selected && self.drawings_ui.mode.is_none() {
            return true;
        }
        if super::reshape_ui::COMMANDS.contains(&name) {
            return self.reshape_enabled(name);
        }
        if self.reshape_session.is_some() {
            return false;
        }
        if !implemented(name) || self.edit_dialog.is_some() || self.pointer_drag.is_some() {
            return false;
        }
        match name {
            "Undo" => !self.host.document.editor.undo.is_empty(),
            "Redo" => !self.host.document.editor.redo.is_empty(),
            "Repeat Last Command" => {
                self.host.document.editor.last_edit.is_some()
                    && !self.host.document.editor.selection_ids().is_empty()
            }
            "Paste" => !self.host.document.editor.clipboard.is_empty(),
            "Select All"
            | "Find & Select…"
            | "Suspend Groups"
            | "Autogroup"
            | "Unlock All"
            | "Tool Settings"
            | "Pick Up Parameters"
            | "Inject Parameters"
            | "Element Transfer Settings…" => true,
            _ => !self.host.document.editor.selection_ids().is_empty(),
        }
    }
    pub(super) fn edit_menu(&mut self, ui: &mut Ui) {
        for &(group, items) in EDIT_GROUPS {
            if group.is_empty() {
                self.edit_menu_items(ui, items);
            } else {
                ui.menu_button(self.language.translate(group), |ui| {
                    self.edit_menu_items(ui, items)
                });
            }
        }
    }
    fn edit_menu_items(&mut self, ui: &mut Ui, items: &[&str]) {
        for &name in items {
            let enabled = self.edit_command_enabled(name);
            let checked = match name {
                "Suspend Groups" => self.host.document.editor.project.edit_state.suspend_groups,
                "Autogroup" => self.host.document.editor.project.edit_state.autogroup,
                _ => false,
            };
            let response = ui.add_enabled(
                enabled,
                Button::new(self.edit_menu_label(name)).selected(checked),
            );
            let response = if !implemented(name) {
                response.on_disabled_hover_text(
                    self.language
                        .translate("Unavailable for the current model types"),
                )
            } else {
                response
            };
            if response.clicked() {
                self.dispatch_edit(name);
                ui.close();
            }
        }
    }
    pub(super) fn dispatch_edit(&mut self, name: &str) {
        if !self.edit_command_enabled(name) {
            return;
        }
        if super::reshape_ui::COMMANDS.contains(&name) {
            self.start_reshape(name);
            return;
        }
        if matches!(name, "Drag" | "Drag a Copy" | "Drag Multiple Copies") {
            self.host.document.editor.set_tool(Tool::Select);
            self.wall_edit = None;

            self.drag = None;
            self.host.document.editor.status = "Enter Drag Reference Point".into();
            self.pointer_drag = Some(PointerDrag {
                origin: None,
                model_plane: None,
                shift_suppressed: false,
                reference_button_down: false,
                tracker: super::drag_tracker::DragTracker::default(),
                placed_ids: Vec::new(),
                last_placement: None,
                ids: self.host.document.editor.selection_ids(),
                input_scope: self.drag_input_scope(),
                revision: self.host.document.editor.revision,
                mode: if name == "Drag Multiple Copies" {
                    2
                } else if name == "Drag a Copy" {
                    1
                } else {
                    0
                },
                modifiers: if name == "Drag" {
                    Modifiers::COMMAND
                } else {
                    Modifiers::COMMAND | Modifiers::ALT
                },
            });
            return;
        }
        let command = match name {
            "Undo" => {
                self.dispatch_history(false);
                return;
            }
            "Redo" => {
                self.dispatch_history(true);
                return;
            }
            "Cut" => {
                self.host.document.editor.cut_selection();
                self.clipboard_write_pending = !self.host.document.editor.clipboard.is_empty();
                return;
            }
            "Copy" => {
                self.host.document.editor.copy_selection();
                self.clipboard_write_pending = !self.host.document.editor.clipboard.is_empty();
                return;
            }
            "Paste" => {
                self.host.document.editor.paste(Point::new(0.5, 0.5));
                return;
            }
            "Delete" => {
                if self.drawings_ui.canvas_selected {
                    self.delete_selected_drawing();
                    return;
                }
                self.host.document.editor.delete();
                return;
            }
            "Select All" => {
                if self.host.document.editor.tool == Tool::Marquee
                    && self.host.document.editor.marquee.is_some()
                {
                    let count = self.host.document.editor.select_all_in_marquee();
                    self.host.document.editor.status =
                        format!("Selected {count} element(s) in Marquee");
                    return;
                }
                let ids = self
                    .host
                    .document
                    .editor
                    .project
                    .elements
                    .iter()
                    .filter(|e| {
                        e.story == self.host.document.editor.story
                            && self
                                .host
                                .document
                                .editor
                                .project
                                .plan_options
                                .element_visible(e, &self.host.document.editor.project.layers)
                    })
                    .map(|e| e.id)
                    .collect();
                self.host.document.editor.set_selection(ids);
                return;
            }
            "Tool Settings" => {
                self.open_element_settings();
                return;
            }
            "Pick Up Parameters" => {
                self.pick_up_command();
                return;
            }
            "Inject Parameters" => {
                self.inject_command();
                return;
            }
            "Element Transfer Settings…" => {
                self.open_transfer_settings();
                return;
            }
            "Repeat Last Command" => self.host.document.editor.last_edit.clone(),
            "Intersect" => Some(EditCommand::Intersect),
            "Merge Walls" => Some(EditCommand::MergeWalls),
            "Group" => Some(EditCommand::Group),
            "Ungroup" => Some(EditCommand::Ungroup),
            "Suspend Groups" => Some(EditCommand::SuspendGroups),
            "Autogroup" => Some(EditCommand::Autogroup),
            "Lock" => Some(EditCommand::Lock),
            "Unlock" => Some(EditCommand::Unlock),
            "Unlock All" => Some(EditCommand::UnlockAll),
            "Bring to Front" => Some(EditCommand::Order(2)),
            "Bring Forward" => Some(EditCommand::Order(1)),
            "Send Backward" => Some(EditCommand::Order(-1)),
            "Send to Back" => Some(EditCommand::Order(-2)),
            "Reset Default Order" => Some(EditCommand::Order(0)),
            "Horizontal Left" => Some(EditCommand::Align { axis: 0, edge: -1 }),
            "Horizontal Center" => Some(EditCommand::Align { axis: 0, edge: 0 }),
            "Horizontal Right" => Some(EditCommand::Align { axis: 0, edge: 1 }),
            "Vertical Top" => Some(EditCommand::Align { axis: 1, edge: 1 }),
            "Vertical Center" => Some(EditCommand::Align { axis: 1, edge: 0 }),
            "Vertical Bottom" => Some(EditCommand::Align { axis: 1, edge: -1 }),
            "Along X" => Some(EditCommand::Distribute { x: true, y: false }),
            "Along Y" => Some(EditCommand::Distribute { x: false, y: true }),
            "Along XY" => Some(EditCommand::Distribute { x: true, y: true }),
            "Invert Wall Direction" => Some(EditCommand::InvertWall),
            "Explode into Current View" => Some(EditCommand::Explode),
            "Unify" => Some(EditCommand::Unify),
            "Linework Consolidation…" => Some(EditCommand::Consolidate),
            _ => None,
        };
        if let Some(command) = command {
            if let Err(error) = self.host.document.editor.edit(command) {
                self.host.document.editor.status = error;
            }
            return;
        }
        let ids = self.host.document.editor.selection_ids();
        let points: Vec<_> = self
            .host
            .document
            .editor
            .project
            .elements
            .iter()
            .filter(|e| ids.contains(&e.id))
            .flat_map(|e| geometry::element_footprint(&self.host.document.editor.project, e))
            .collect();
        let origin = if points.is_empty() {
            Point::default()
        } else {
            let lo = points
                .iter()
                .fold(Point::new(f64::INFINITY, f64::INFINITY), |a, p| {
                    Point::new(a.x.min(p.x), a.y.min(p.y))
                });
            let hi = points
                .iter()
                .fold(Point::new(f64::NEG_INFINITY, f64::NEG_INFINITY), |a, p| {
                    Point::new(a.x.max(p.x), a.y.max(p.y))
                });
            Point::new((lo.x + hi.x) / 2., (lo.y + hi.y) / 2.)
        };
        let mut d = EditDialog {
            name: name.into(),
            ids,
            revision: self.host.document.editor.revision,
            x: 0.,
            y: 0.,
            origin,
            angle: 0.,
            scale: 1.,
            copies: 0,
            story: self.host.document.editor.story,
            query: String::new(),
            kind: "All".into(),
            error: String::new(),
            preview: Vec::new(),
            node: 1,
            chamfer: false,
        };
        if name.contains("Copy") {
            d.copies = 1;
        }
        if name.contains("Copies") || name == "Multiply…" {
            d.copies = 3;
        }
        if name.starts_with("Drag") || name == "Multiply…" {
            d.x = 1.;
        }
        if name.starts_with("Rotate") {
            d.angle = 90.;
        }
        if name == "Fillet/Chamfer…" {
            d.x = 0.25;
        }
        if name == "Resize…" {
            d.scale = 2.;
        }
        if name == "Split" || name == "Adjust" {
            d.x = origin.x;
            d.y = origin.y + 1.;
        }
        if name.starts_with("Quick Rotate") {
            let mut command = d.command();
            if let EditCommand::Transform { angle, .. } = &mut command {
                *angle = if name == "Quick Rotate Clockwise" {
                    -90.
                } else {
                    90.
                };
            }
            if let Err(error) = self.host.document.editor.edit(command) {
                self.host.document.editor.status = error;
            }
            return;
        }
        self.edit_dialog = Some(d);
    }
    pub(super) fn edit_dialog_ui(&mut self, ctx: &Context) {
        let Some(mut d) = self.edit_dialog.take() else {
            return;
        };
        let lang = self.language;
        let mut apply = false;
        let mut cancel = false;
        let mut preview = false;
        let previous_command = d.command();
        let find = d.name == "Find & Select…";
        let modal = Modal::new(Id::new("edit_operation")).show(ctx, |ui| {
            ui.set_width(460.);
            ui.heading(lang.translate(&d.name));
            ui.separator();
            if find {
                ui.label(lang.translate("Name contains"));
                ui.text_edit_singleline(&mut d.query);
                ComboBox::from_id_salt("find_kind")
                    .selected_text(lang.translate(&d.kind))
                    .show_ui(ui, |ui| {
                        for kind in ["All", "Wall", "Slab", "Stair", "Polyline", "Door", "Window"] {
                            ui.selectable_value(&mut d.kind, kind.into(), lang.translate(kind));
                        }
                    });
                ui.label(lang.translate("Search visible elements on the current story"));
            } else {
                Grid::new("edit_parameters").num_columns(2).show(ui, |ui| {
                    if d.name == "Fillet/Chamfer…" {
                        ui.label(lang.translate("Interior node index"));
                        ui.add(DragValue::new(&mut d.node).range(1..=10000));
                        ui.end_row();
                        ui.label(lang.translate("Radius / chamfer distance"));
                        ui.add(DragValue::new(&mut d.x).speed(0.05).suffix(" m"));
                        ui.end_row();
                        ui.checkbox(&mut d.chamfer, lang.translate("Chamfer instead of fillet"));
                        ui.end_row();
                    }
                    let translate = d.name.starts_with("Drag") || d.name == "Multiply…";
                    let rotate = d.name.starts_with("Rotate");
                    let mirror = d.name.starts_with("Mirror");
                    let resize = d.name == "Resize…";
                    let line = d.name == "Split" || d.name == "Adjust";
                    if translate || line {
                        ui.label(lang.translate(if line { "End X" } else { "Displacement X" }));
                        ui.add(DragValue::new(&mut d.x).speed(0.1).suffix(" m"));
                        ui.end_row();
                        ui.label(lang.translate(if line { "End Y" } else { "Displacement Y" }));
                        ui.add(DragValue::new(&mut d.y).speed(0.1).suffix(" m"));
                        ui.end_row();
                    }
                    if rotate || mirror || resize || line {
                        ui.label(lang.translate("Origin X"));
                        ui.add(DragValue::new(&mut d.origin.x).speed(0.1).suffix(" m"));
                        ui.end_row();
                        ui.label(lang.translate("Origin Y"));
                        ui.add(DragValue::new(&mut d.origin.y).speed(0.1).suffix(" m"));
                        ui.end_row();
                    }
                    if rotate {
                        ui.label(lang.translate("Angle"));
                        ui.add(DragValue::new(&mut d.angle).speed(1.).suffix("°"));
                        ui.end_row();
                    }
                    if mirror {
                        ui.label(lang.translate("Mirror about the vertical line through Origin X"));
                        ui.end_row();
                    }
                    if resize {
                        ui.label(lang.translate("Scale factor"));
                        ui.add(
                            DragValue::new(&mut d.scale)
                                .speed(0.05)
                                .range(0.001..=1000.),
                        );
                        ui.end_row();
                    }
                    if d.name.contains("Copies") || d.name == "Multiply…" {
                        ui.label(lang.translate("Number of copies"));
                        ui.add(DragValue::new(&mut d.copies).range(1..=1000));
                        ui.end_row();
                    }
                    if matches!(
                        d.name.as_str(),
                        "Elevate…" | "Offset" | "Modify Wall Reference Line…"
                    ) {
                        ui.label(lang.translate("Distance"));
                        ui.add(DragValue::new(&mut d.x).speed(0.1).suffix(" m"));
                        ui.end_row();
                    }
                    if d.name == "Edit Elements by Stories…" || d.name == "Relink Home Story…" {
                        ui.label(lang.translate("Home Story"));
                        ComboBox::from_id_salt("edit_story")
                            .selected_text(
                                self.host
                                    .document
                                    .editor
                                    .project
                                    .stories
                                    .iter()
                                    .find(|s| s.id == d.story)
                                    .map(|s| s.name.as_str())
                                    .unwrap_or(""),
                            )
                            .show_ui(ui, |ui| {
                                for s in &self.host.document.editor.project.stories {
                                    ui.selectable_value(&mut d.story, s.id, &s.name);
                                }
                            });
                        ui.end_row();
                    }
                });
                ui.label(lang.translate("One operation · Undo restores all affected elements"));
                if self.host.document.editor.revision != d.revision {
                    d.error = "Project changed; reopen this command".into();
                }
            }
            if d.command() != previous_command {
                d.preview.clear();
                d.error.clear();
            }
            if !find {
                preview = ui.button(lang.translate("Preview")).clicked();
                if !d.preview.is_empty() {
                    let (rect, _) = ui.allocate_exact_size(vec2(440., 160.), Sense::hover());
                    let points: Vec<_> = d.preview.iter().flatten().copied().collect();
                    let lo = points
                        .iter()
                        .fold(Point::new(f64::INFINITY, f64::INFINITY), |a, p| {
                            Point::new(a.x.min(p.x), a.y.min(p.y))
                        });
                    let hi = points
                        .iter()
                        .fold(Point::new(f64::NEG_INFINITY, f64::NEG_INFINITY), |a, p| {
                            Point::new(a.x.max(p.x), a.y.max(p.y))
                        });
                    let scale = (420. / (hi.x - lo.x).max(0.1)).min(140. / (hi.y - lo.y).max(0.1));
                    let center = Point::new((lo.x + hi.x) / 2., (lo.y + hi.y) / 2.);
                    ui.painter().rect_filled(rect, 3., PANEL_ALT);
                    for path in &d.preview {
                        for pair in path.windows(2) {
                            let to_screen = |p: Point| {
                                rect.center()
                                    + vec2((p.x - center.x) * scale, -(p.y - center.y) * scale)
                            };
                            ui.painter().line_segment(
                                [to_screen(pair[0]), to_screen(pair[1])],
                                Stroke::new(1.5, ACCENT),
                            );
                        }
                    }
                }
            }
            if !d.error.is_empty() {
                ui.colored_label(Color32::LIGHT_RED, lang.translate(&d.error));
            }
            ui.separator();
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                apply = ui
                    .add_enabled(
                        find || self.host.document.editor.revision == d.revision,
                        Button::new(lang.translate(if find { "Select" } else { "OK" })),
                    )
                    .clicked();
                cancel = ui.button(lang.translate("Cancel")).clicked();
            });
        });
        if cancel || modal.should_close() {
            return;
        }
        if preview {
            let mut draft = Editor::new(self.host.document.editor.project.clone());
            draft.set_selection(d.ids.clone());
            match draft.edit(d.command()) {
                Ok(_) => {
                    let ids = draft.selection_ids();
                    d.preview = draft
                        .project
                        .elements
                        .iter()
                        .filter(|e| ids.contains(&e.id))
                        .take(256)
                        .flat_map(|e| {
                            geometry::element_plan_paths_on_story(
                                &draft.project,
                                e,
                                self.host.document.editor.story,
                            )
                        })
                        .collect();
                    d.error.clear();
                }
                Err(error) => {
                    d.error = error;
                    d.preview.clear();
                }
            }
        }
        if apply {
            if find {
                let query = d.query.to_lowercase();
                let ids = self
                    .host
                    .document
                    .editor
                    .project
                    .elements
                    .iter()
                    .filter(|e| {
                        e.story == self.host.document.editor.story
                            && (d.kind == "All" || e.kind() == d.kind)
                            && e.name.to_lowercase().contains(&query)
                            && self
                                .host
                                .document
                                .editor
                                .project
                                .plan_options
                                .element_visible(e, &self.host.document.editor.project.layers)
                    })
                    .map(|e| e.id)
                    .collect();
                self.host.document.editor.set_selection(ids);
                return;
            }
            let original = self.host.document.editor.selection_ids();
            self.host.document.editor.set_selection(d.ids.clone());
            match self.host.document.editor.edit(d.command()) {
                Ok(_) => return,
                Err(error) => {
                    self.host.document.editor.set_selection(original);
                    d.error = error;
                }
            }
        }
        self.edit_dialog = Some(d);
    }
}

/// A click-to-place command; previews never mutate the document or undo stack.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct PointerDrag {
    shift_suppressed: bool,
    model_plane: Option<bool>,
    input_scope: (View, u32, Option<u64>, crate::view_settings::DimensionStyle),
    origin: Option<Point>,
    // Consume the release belonging to the reference-point press.
    reference_button_down: bool,
    tracker: super::drag_tracker::DragTracker,
    last_placement: Option<Pos2>,
    placed_ids: Vec<u64>,
    ids: Vec<u64>,
    revision: u64,
    mode: u8,
    modifiers: Modifiers,
}
impl AtelierApp {
    pub(super) fn start_direct_pointer_drag(&mut self, origin: Point) {
        self.wall_edit = None;
        self.wall_edit_palette_rect = None;

        self.drag = None;
        self.pointer_drag = Some(PointerDrag {
            origin: Some(origin),
            model_plane: None,
            shift_suppressed: false,
            reference_button_down: false,
            tracker: super::drag_tracker::DragTracker::default(),
            last_placement: None,
            placed_ids: Vec::new(),
            ids: self.host.document.editor.selection_ids(),
            input_scope: self.drag_input_scope(),
            revision: self.host.document.editor.revision,
            mode: 0,
            modifiers: Modifiers::NONE,
        });
    }

    pub(super) fn pointer_drag_canvas(
        &mut self,
        ui: &mut Ui,
        r: Rect,
        response: &Response,
        model: bool,
    ) {
        // Text widgets, modal windows and background input retain their keys.
        // Keep the transient drag intact until the canvas owns input again.
        if self.pointer_drag_ime_owns_input(ui.ctx()) || self.canvas_keyboard_blocked(ui) {
            return;
        }
        let pointer = ui
            .input(|i| i.pointer.hover_pos())
            .filter(|p| r.contains(*p));
        let project_pointer = |p| {
            if model {
                Self::screen_to_ground(
                    &self.camera,
                    r,
                    p,
                    self.host
                        .document
                        .editor
                        .project
                        .elevation(self.host.document.editor.story),
                )
            } else {
                Some(self.world(r, p))
            }
        };
        let point = pointer.and_then(project_pointer);
        let reference_point = ui
            .input(|i| i.pointer.press_origin())
            .filter(|p| r.contains(*p))
            .and_then(project_pointer);
        let Some(mut drag) = self.pointer_drag.take() else {
            return;
        };
        if drag.input_scope != self.drag_input_scope() {
            self.complete_pointer_drag(drag);
            self.host.document.editor.status = "Edit cancelled after view or unit change".into();
            return;
        }
        if drag.revision != self.host.document.editor.revision {
            return;
        }
        let Some(point) = point else {
            self.pointer_drag = Some(drag);
            return;
        };
        if response.clicked_by(PointerButton::Secondary) {
            self.complete_pointer_drag(drag);
            return;
        }
        let Some(origin) = drag.origin else {
            ui.ctx().set_cursor_icon(CursorIcon::Crosshair);
            if response.is_pointer_button_down_on()
                && ui.input(|i| i.pointer.button_pressed(PointerButton::Primary))
            {
                let reference = reference_point.unwrap_or(point);
                drag.origin = Some(if !model && self.host.document.editor.snap {
                    self.input_origin_node(reference).unwrap_or(reference)
                } else {
                    reference
                });
                drag.model_plane = Some(model);
                drag.reference_button_down = true;
                drag.modifiers = ui.input(|i| i.modifiers);
                self.host.document.editor.status = "Enter Drag-to Point".into();
            }
            self.pointer_drag = Some(drag);
            return;
        };
        if drag.model_plane.is_some_and(|plane| plane != model) {
            self.complete_pointer_drag(drag);
            self.host.document.editor.status = "Edit cancelled after view or unit change".into();
            return;
        }
        drag.model_plane = Some(model);
        let pointer_on_tracker = pointer.is_some_and(|pos| drag.tracker.owns_pointer(pos));
        let point = pointer
            .and_then(|pos| drag.tracker.pointer_offset(pos))
            .map_or(point, |offset| {
                Point::new(origin.x + offset.x, origin.y + offset.y)
            });
        // The reference press owns its release, including tiny motions that egui
        // still classifies as a click. It must never also place the element.
        let reference_press = drag.reference_button_down;
        drag.reference_button_down &= ui.input(|i| i.pointer.button_down(PointerButton::Primary));
        let modifiers = ui.input(|i| i.modifiers);
        if !modifiers.shift {
            drag.shift_suppressed = false;
        }
        let constrain_shift = modifiers.shift && !drag.shift_suppressed;
        // Read transitions as well as final state: a quick tap can begin and end in one frame.
        let changes = ui.input(|i| {
            i.events
                .iter()
                .filter_map(|event| match event {
                    Event::ModifiersChanged(value) => Some(*value),
                    _ => None,
                })
                .collect::<Vec<_>>()
        });
        for value in changes.into_iter().chain(std::iter::once(modifiers)) {
            let multiple_pressed =
                value.alt && value.command && !(drag.modifiers.alt && drag.modifiers.command);
            if multiple_pressed {
                drag.mode = if drag.mode == 2 { 0 } else { 2 };
            } else if value.alt && !drag.modifiers.alt {
                drag.mode = if drag.mode == 0 { 1 } else { 0 };
            }
            drag.modifiers = value;
        }
        self.host.document.editor.status = match drag.mode {
            1 => "Enter Drag-to Point · Drag a Copy",
            2 => "Enter Drag-to Point · Drag Multiple Copies",
            _ => "Enter Drag-to Point",
        }
        .into();
        if modifiers.shift && drag.shift_suppressed {
            self.host
                .document
                .editor
                .status
                .push_str(" · Direction constraint released");
        }
        let mut offset = if model {
            Point::new(point.x - origin.x, point.y - origin.y)
        } else {
            self.drag_guide_feedback(ui, r, point, origin);
            let source_ids = drag
                .ids
                .iter()
                .chain(&drag.placed_ids)
                .copied()
                .collect::<Vec<_>>();
            let (target, _) =
                self.precise_drag_target_for_sources(point, origin, constrain_shift, &source_ids);
            Point::new(target.x - origin.x, target.y - origin.y)
        };
        if model && constrain_shift {
            if offset.x.abs() >= offset.y.abs() {
                offset.y = 0.;
            } else {
                offset.x = 0.;
            }
        }
        let (unit_scale, unit) = super::drag_tracker::DragTracker::unit(
            self.host
                .document
                .editor
                .project
                .plan_options
                .dimension_style,
        );
        let events = ui.input(|i| i.events.clone());
        let overrides = self.shortcut_overrides().to_vec();
        let (offset, tracker_confirm, tracker_valid) = drag.tracker.update(
            &events,
            offset,
            unit_scale,
            unit,
            origin,
            self.host.document.editor.project.plan_options.user_origin,
            !model,
            &overrides,
        );
        let destination = Point::new(origin.x + offset.x, origin.y + offset.y);
        let command = EditCommand::Transform {
            offset,
            origin: Point::default(),
            angle: 0.,
            scale: 1.,
            mirror: false,
            copies: usize::from(drag.mode != 0),
        };
        let mut draft = Editor::new(self.host.document.editor.project.clone());
        draft.set_selection(drag.ids.clone());
        let preview = draft.edit(command.clone());
        if let Err(error) = &preview {
            self.host.document.editor.status = error.clone();
        }
        let painter = ui.painter_at(r);
        let matrix = self.camera.matrix(r.aspect_ratio().into());
        let elevation = self
            .host
            .document
            .editor
            .project
            .elevation(self.host.document.editor.story);
        let screen = |point: Point| {
            if model {
                let q = matrix.project_point3(glam::DVec3::new(point.x, point.y, elevation));
                pos2(
                    f64::from(r.left()) + (q.x + 1.) * f64::from(r.width()) * 0.5,
                    f64::from(r.top()) + (1. - q.y) * f64::from(r.height()) * 0.5,
                )
            } else {
                self.screen(r, point)
            }
        };
        painter.line_segment(
            [screen(origin), screen(destination)],
            Stroke::new(1., ACCENT),
        );
        if preview.is_ok() {
            let ids = draft.selection_ids();
            for e in draft
                .project
                .elements
                .iter()
                .filter(|e| ids.contains(&e.id))
            {
                if !model
                    && let Some(plan) = crate::column_plan::for_element(
                        &draft.project,
                        e,
                        self.host.document.editor.story,
                    )
                {
                    if let Ok(plan) = plan {
                        Self::paint_column_plan_geometry_mapped(
                            &painter,
                            &plan,
                            screen,
                            Color32::TRANSPARENT,
                            Stroke::new(2., ACCENT),
                        );
                    }
                    continue;
                }
                for path in geometry::element_plan_paths(&draft.project, e) {
                    if path.len() > 1 {
                        painter.add(egui::Shape::line(
                            path.into_iter().map(screen).collect(),
                            Stroke::new(2., ACCENT),
                        ));
                    }
                }
            }
        }
        ui.ctx().set_cursor_icon(CursorIcon::Crosshair);
        if let Some(pos) = pointer {
            drag.tracker.paint(
                ui,
                r,
                pos,
                offset,
                unit_scale,
                unit,
                self.language,
                drag.mode,
                !model,
                &overrides,
            );
        }
        if !reference_press
            && !pointer_on_tracker
            && response.double_clicked()
            && drag.mode == 2
            && drag
                .last_placement
                .zip(pointer)
                .is_some_and(|(a, b)| a.distance(b) < 6.)
        {
            self.complete_pointer_drag(drag);
            return;
        }
        if !reference_press
            && tracker_valid
            && ((!pointer_on_tracker && response.clicked()) || tracker_confirm)
            && offset.distance(Point::default()) > 1e-6
        {
            self.host.document.editor.set_selection(drag.ids.clone());
            match self.host.document.editor.edit(command) {
                Ok(_) if drag.mode != 2 => {
                    self.draft_id = None;
                    return;
                }
                Ok(_) => {
                    drag.tracker = super::drag_tracker::DragTracker::default();
                    drag.revision = self.host.document.editor.revision;
                    drag.last_placement = pointer;
                    drag.placed_ids
                        .extend(self.host.document.editor.selection_ids());
                    self.host.document.editor.set_selection(drag.ids.clone());
                    self.draft_id = None;
                }
                Err(error) => self.host.document.editor.status = error,
            }
        }
        self.pointer_drag = Some(drag);
    }
}

impl AtelierApp {
    fn complete_pointer_drag(&mut self, drag: PointerDrag) {
        if !drag.placed_ids.is_empty() {
            let mut ids = drag.ids;
            ids.extend(drag.placed_ids);
            self.host.document.editor.set_selection(ids);
        }
        self.host.document.editor.status = "READY".into();
    }
    pub(super) fn cancel_pointer_drag_input(&mut self, shift_held: bool) -> bool {
        let Some(drag) = self.pointer_drag.as_mut() else {
            return false;
        };
        if drag.tracker.cancel_input() {
            return true;
        }
        if shift_held && !drag.shift_suppressed {
            drag.shift_suppressed = true;
            self.host.document.editor.status = "Direction constraint released".into();
            return true;
        }
        false
    }
    pub(super) fn pointer_drag_has_input(&self) -> bool {
        self.pointer_drag
            .as_ref()
            .is_some_and(|drag| drag.tracker.has_input())
    }
    pub(super) fn pointer_drag_origin(&self) -> Option<Point> {
        self.pointer_drag.as_ref().and_then(|drag| drag.origin)
    }
    pub(super) fn pointer_drag_origin_changed(&mut self) {
        if let Some(drag) = self.pointer_drag.as_mut() {
            // Only the coordinate reference changed; the source geometry is identical.
            drag.revision = self.host.document.editor.revision;
        }
    }
    pub(super) fn request_pointer_drag_field(&mut self, index: usize) -> Result<(), String> {
        let drag = self
            .pointer_drag
            .as_mut()
            .filter(|drag| drag.origin.is_some())
            .ok_or("Coordinate input requires an active precision input field")?;
        if index >= 2 && drag.model_plane == Some(true) {
            return Err("Coordinate input requires an active precision input field".into());
        }
        drag.tracker.request_field(index);
        Ok(())
    }
    fn drag_input_scope(&self) -> (View, u32, Option<u64>, crate::view_settings::DimensionStyle) {
        let editor = &self.host.document.editor;
        (
            self.view,
            editor.story,
            editor.project.plan_options.active_worksheet,
            editor.project.plan_options.dimension_style,
        )
    }
    pub(super) fn cancel_pointer_drag_outside_scope(&mut self) {
        if self
            .pointer_drag
            .as_ref()
            .is_some_and(|drag| drag.input_scope != self.drag_input_scope())
        {
            self.finish_pointer_drag();
            self.host.document.editor.status = "Edit cancelled after view or unit change".into();
        }
    }
    pub(super) fn pointer_drag_ime_owns_input(&mut self, ctx: &Context) -> bool {
        self.pointer_drag
            .as_mut()
            .is_some_and(|drag| ctx.input(|i| drag.tracker.ime_owns_input(&i.events, i.focused)))
    }
    pub(super) fn finish_pointer_drag(&mut self) {
        if let Some(drag) = self.pointer_drag.take() {
            self.complete_pointer_drag(drag);
        }
    }
}

#[cfg(test)]
mod column_preview_event_tests {
    use super::*;

    fn frame(ctx: &Context, app: &mut AtelierApp, events: Vec<Event>) -> egui::FullOutput {
        let mut out = ctx.run_ui(
            RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1400., 900.))),
                events,
                ..Default::default()
            },
            |ui| app.workspace(ui),
        );
        out.textures_delta.clear();
        out
    }
    fn label(shapes: &[egui::epaint::ClippedShape], text: &str) -> Pos2 {
        fn find(shape: &egui::Shape, text: &str) -> Option<Pos2> {
            match shape {
                egui::Shape::Text(t) if t.galley.text() == text => {
                    Some(t.pos + t.galley.size() * 0.5)
                }
                egui::Shape::Vec(shapes) => shapes.iter().find_map(|s| find(s, text)),
                _ => None,
            }
        }
        shapes
            .iter()
            .find_map(|s| find(&s.shape, text))
            .expect(text)
    }
    fn click(ctx: &Context, app: &mut AtelierApp, text: &str) {
        let out = frame(ctx, app, vec![]);
        let pos = label(&out.shapes, text);
        frame(ctx, app, vec![Event::PointerMoved(pos)]);
        for pressed in [true, false] {
            frame(
                ctx,
                app,
                vec![Event::PointerButton {
                    pos,
                    button: PointerButton::Primary,
                    pressed,
                    modifiers: Modifiers::NONE,
                }],
            );
        }
    }

    #[test]
    fn column_numeric_preview_uses_view_story_and_cancel_keeps_real_redo_bilingual() {
        for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
            for circular in [false, true] {
                let ctx = Context::default();
                AtelierApp::style(&ctx);
                let mut app = AtelierApp::from_renderer(None, "Headless test".into());
                app.recovery_slot = None;
                app.in_window_menus = true;
                app.language = language;
                app.host.document.editor = Editor::new(Project::default());
                app.host.document.editor.column_plan.display =
                    crate::column_plan::ColumnPlanDisplay::ProjectedWithOverhead;
                let section = if circular {
                    crate::column::ColumnSection::Circular { diameter: 0.6 }
                } else {
                    crate::column::ColumnSection::Rectangular {
                        width: 0.4,
                        depth: 0.6,
                    }
                };
                let id = app
                    .host
                    .document
                    .editor
                    .place_column_with_inclination(
                        Point::default(),
                        section,
                        6.,
                        Some(crate::column_inclination::ColumnInclination {
                            slant_degrees: 60.,
                            direction_degrees: 30.,
                        }),
                    )
                    .unwrap();
                app.host.document.editor.set_selection(vec![id]);
                app.host.document.editor.move_selection(Point::new(2., 0.));
                let redo_target = app.host.document.editor.project.clone();
                app.host.document.editor.undo();
                app.host.document.editor.switch_story(1);
                app.host.document.editor.set_selection(vec![id]);
                let before = app.host.document.editor.project.clone();
                let history = (
                    app.host.document.editor.undo.len(),
                    app.host.document.editor.redo.len(),
                    app.host.document.editor.revision,
                );
                assert_eq!(history.1, 1);
                let plan = crate::column_plan::for_element(&before, &before.elements[0], 1)
                    .unwrap()
                    .unwrap();
                let mut expected = vec![plan.cut, plan.uncut, plan.overhead];
                expected.retain(|p| !p.is_empty());
                for path in &mut expected {
                    if path.first() != path.last() {
                        path.push(path[0]);
                    }
                }
                // Multiply's existing defaults create three copies at +1m intervals.
                let expected: Vec<Vec<Point>> = (1..=3)
                    .flat_map(|copy| {
                        expected.iter().map(move |path| {
                            path.iter()
                                .map(|p| Point::new(p.x + copy as f64, p.y))
                                .collect()
                        })
                    })
                    .collect();
                app.dispatch_edit("Multiply…");
                frame(&ctx, &mut app, vec![]);
                click(&ctx, &mut app, language.translate("Preview"));
                let dialog = app.edit_dialog.as_ref().unwrap();
                assert!(dialog.error.is_empty(), "{}", dialog.error);
                assert_eq!(dialog.preview.len(), expected.len());
                for (actual, expected) in dialog.preview.iter().zip(&expected) {
                    assert_eq!(actual.first(), actual.last());
                    // Translation can change redundant collinear hull vertices by f64
                    // rounding. Compare the two closed boundaries in both directions.
                    for (points, boundary) in [(actual, expected), (expected, actual)] {
                        for point in points {
                            assert!(
                                boundary.windows(2).any(|edge| geometry::segment_distance(
                                    *point, edge[0], edge[1]
                                ) < 0.00001)
                            );
                        }
                    }
                }
                assert_ne!(
                    dialog.preview,
                    geometry::element_plan_paths(&before, &before.elements[0])
                );
                assert_eq!(app.host.document.editor.project, before);
                assert_eq!(
                    (
                        app.host.document.editor.undo.len(),
                        app.host.document.editor.redo.len(),
                        app.host.document.editor.revision
                    ),
                    history
                );
                // Render the new thumbnail before locating the moved footer.
                frame(&ctx, &mut app, vec![]);
                click(&ctx, &mut app, language.translate("Cancel"));
                assert!(app.edit_dialog.is_none());
                assert_eq!(app.host.document.editor.project, before);
                assert_eq!(app.host.document.editor.selection_ids(), vec![id]);
                assert_eq!(
                    (
                        app.host.document.editor.undo.len(),
                        app.host.document.editor.redo.len(),
                        app.host.document.editor.revision
                    ),
                    history
                );
                app.host.document.editor.redo();
                assert_eq!(app.host.document.editor.project, redo_target);
            }
        }
    }
}
