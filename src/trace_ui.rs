// Native Trace menu, palette and reference overlay. Included from app.rs so the
// controls share the application state without exposing extra public fields.
use crate::trace::{TraceReference, TraceSelection};

impl AtelierApp {
    fn trace_toggled(&mut self) {
        if self.trace.enabled {
            self.view = View::Plan;
            if self
                .trace
                .selection(
                    &self.editor.project,
                    self.editor.story,
                    self.editor.previous_story,
                )
                .is_none()
                && let Some(story) = self.nearest_trace_story()
            {
                self.trace.reference = TraceReference::Story(story);
            }
            self.editor.status = match self.trace.selection(
                &self.editor.project,
                self.editor.story,
                self.editor.previous_story,
            ) {
                Some(_) => format!("Trace Reference on · {}", self.trace_reference_label()),
                None => "Trace Reference on · no reference is available for this story".into(),
            };
        } else {
            self.trace.cancel_pointer_modes();
            self.editor.status = "Trace Reference off".into();
        }
    }

    fn open_trace_palette(&mut self) {
        self.trace.palette_open = true;
        if !self.trace.enabled {
            self.trace.enabled = true;
            self.trace_toggled();
        }
        self.view = View::Plan;
        self.editor.status = "Trace & Reference palette".into();
    }

    fn nearest_trace_story(&self) -> Option<u32> {
        let stories = crate::trace::ordered_story_ids(&self.editor.project);
        let index = stories.iter().position(|id| *id == self.editor.story)?;
        stories
            .get(index + 1)
            .or_else(|| index.checked_sub(1).and_then(|index| stories.get(index)))
            .copied()
    }

    fn trace_reference_label(&self) -> String {
        match self.trace.reference {
            TraceReference::AboveCurrentStory => {
                self.language.translate("Above Current Story").to_owned()
            }
            TraceReference::BelowCurrentStory => {
                self.language.translate("Below Current Story").to_owned()
            }
            TraceReference::PreviousStory => self.language.translate("Previous Story").to_owned(),
            TraceReference::EntireModel => {
                self.language.translate("Entire Model Display").to_owned()
            }
            TraceReference::Story(id) => self
                .editor
                .project
                .stories
                .iter()
                .find(|story| story.id == id)
                .map_or_else(
                    || format!("Story {id:02}"),
                    |story| format!("{:02} · {}", story.id, story.name),
                ),
        }
    }

    fn choose_trace_reference(&mut self, reference: TraceReference) {
        self.trace.reference = reference;
        self.trace.enabled = true;
        self.trace.browse_open = false;
        self.trace.cancel_pointer_modes();
        self.view = View::Plan;
        self.editor.status = if self
            .trace
            .selection(
                &self.editor.project,
                self.editor.story,
                self.editor.previous_story,
            )
            .is_some()
        {
            format!("Trace Reference · {}", self.trace_reference_label())
        } else {
            format!(
                "Trace Reference · {} · no story is available",
                self.trace_reference_label()
            )
        };
    }

    fn trace_menu(&mut self, ui: &mut Ui) {
        let plan_view = self.view == View::Plan;
        let has_reference = self
            .trace
            .selection(
                &self.editor.project,
                self.editor.story,
                self.editor.previous_story,
            )
            .is_some();
        let mut browse = false;
        let mut activate = false;
        let mut switch_reference = false;
        let mut drag = false;
        let mut rotate = false;
        let mut reset = false;
        let mut rebuild = false;
        let mut displace = false;
        let mut splitter = false;
        ui.add_enabled_ui(plan_view, |ui| {
            ui.menu_button(self.language.translate("Choose Reference"), |ui| {
                if ui
                    .button(self.language.translate("Browse for Trace Reference…"))
                    .clicked()
                {
                    browse = true;
                    ui.close();
                }
                for (reference, label) in [
                    (TraceReference::AboveCurrentStory, "Above Current Story"),
                    (TraceReference::BelowCurrentStory, "Below Current Story"),
                    (TraceReference::PreviousStory, "Previous Story"),
                    (TraceReference::EntireModel, "Entire Model Display"),
                ] {
                    if ui
                        .selectable_label(
                            self.trace.reference == reference,
                            self.language.translate(label),
                        )
                        .clicked()
                    {
                        self.choose_trace_reference(reference);
                        ui.close();
                    }
                }
            });
            activate = ui
                .add_enabled(
                    has_reference,
                    Button::new(self.language.translate("Activate Reference")),
                )
                .clicked();
            switch_reference = ui
                .add_enabled(
                    has_reference
                        && self
                            .trace
                            .selection(
                                &self.editor.project,
                                self.editor.story,
                                self.editor.previous_story,
                            )
                            .and_then(TraceSelection::story)
                            .is_some(),
                    Button::new(self.language.translate("Switch Reference with Active")),
                )
                .clicked();
            drag = ui
                .add_enabled(
                    has_reference,
                    Button::new(self.language.translate("Drag Reference")),
                )
                .clicked();
            rotate = ui
                .add_enabled(
                    has_reference,
                    Button::new(self.language.translate("Rotate Reference")),
                )
                .clicked();
            reset = ui
                .add_enabled(
                    has_reference,
                    Button::new(self.language.translate("Reset to Default Position")),
                )
                .clicked();
            rebuild = ui
                .add_enabled(
                    has_reference,
                    Button::new(self.language.translate("Rebuild Reference")),
                )
                .clicked();
            displace = ui
                .add_enabled(
                    has_reference,
                    Button::new(self.language.translate("Temporarily displace Reference")),
                )
                .clicked();
            splitter = ui
                .add_enabled(
                    has_reference,
                    Button::new(self.language.translate("Show/Hide Splitter")),
                )
                .clicked();
            ui.checkbox(
                &mut self.trace.on_top,
                self.language.translate("Reference on Top"),
            );
            ui.checkbox(
                &mut self.trace.transparent_fills,
                self.language.translate("Transparent Fills and Zones"),
            );
            if ui
                .button(self.language.translate("Turn off Trace everywhere"))
                .clicked()
            {
                self.trace.enabled = false;
                self.trace.cancel_pointer_modes();
                self.editor.status = "Trace turned off everywhere".into();
                ui.close();
            }
        });
        ui.separator();
        let open_palette = ui
            .button(self.language.translate("Trace & Reference"))
            .clicked();

        if browse {
            self.trace.browse_choice = self.trace.reference;
            self.trace.browse_open = true;
            self.trace.palette_open = true;
        }
        if activate {
            self.activate_trace_reference();
        }
        if switch_reference {
            self.switch_trace_reference();
        }
        if drag {
            self.toggle_trace_drag();
        }
        if rotate {
            self.toggle_trace_rotate();
        }
        if reset {
            self.trace.reset_position();
            self.editor.status = "Trace Reference reset to default position".into();
        }
        if rebuild {
            self.editor.status = "Trace Reference rebuilt from the live model".into();
        }
        if displace {
            self.toggle_trace_displace();
        }
        if splitter {
            self.toggle_trace_splitter();
        }
        if open_palette {
            self.open_trace_palette();
        }
    }

    fn activate_trace_reference(&mut self) {
        let Some(selection) = self.trace.selection(
            &self.editor.project,
            self.editor.story,
            self.editor.previous_story,
        ) else {
            self.editor.status = "No Trace Reference is available".into();
            return;
        };
        let TraceSelection::Story(story) = selection else {
            self.editor.status = "Entire Model Display cannot be activated as a story".into();
            return;
        };
        if self.trace.reference == TraceReference::Story(story) {
            self.trace.reference = TraceReference::PreviousStory;
        }
        self.editor.switch_story(story);
        self.trace.enabled = true;
        self.view = View::Plan;
        self.editor.status = format!("Activated Trace Reference · story {story:02}");
    }

    fn switch_trace_reference(&mut self) {
        let Some(selection) = self.trace.selection(
            &self.editor.project,
            self.editor.story,
            self.editor.previous_story,
        ) else {
            self.editor.status = "No Trace Reference is available".into();
            return;
        };
        let TraceSelection::Story(story) = selection else {
            self.editor.status = "Entire Model Display cannot be switched with the Active".into();
            return;
        };
        let active = self.editor.story;
        self.editor.switch_story(story);
        self.trace.reference = TraceReference::Story(active);
        self.trace.enabled = true;
        self.trace.cancel_pointer_modes();
        self.view = View::Plan;
        self.editor.status = format!(
            "Switched Reference with Active · active story {story:02}, reference story {active:02}"
        );
    }

    fn toggle_trace_drag(&mut self) {
        let active = !self.trace.drag_mode;
        self.trace.cancel_pointer_modes();
        self.trace.drag_mode = active;
        self.trace.enabled = true;
        self.view = View::Plan;
        self.editor.status = if active {
            "Drag Reference · drag in the plan · Esc exits".into()
        } else {
            "Drag Reference cancelled".into()
        };
    }

    fn toggle_trace_rotate(&mut self) {
        let active = !self.trace.rotate_mode;
        self.trace.cancel_pointer_modes();
        self.trace.rotate_mode = active;
        self.trace.enabled = true;
        self.view = View::Plan;
        self.editor.status = if active {
            "Rotate Reference · drag horizontally in the plan · Esc exits".into()
        } else {
            "Rotate Reference cancelled".into()
        };
    }

    fn toggle_trace_displace(&mut self) {
        let active = !self.trace.displace_mode;
        self.trace.cancel_pointer_modes();
        self.trace.displace_mode = active;
        self.trace.enabled = true;
        self.view = View::Plan;
        self.editor.status = if active {
            "Temporarily displace Reference · drag to inspect · release returns it".into()
        } else {
            "Temporary Reference displacement ended".into()
        };
    }

    fn toggle_trace_splitter(&mut self) {
        self.trace.splitter = self.trace.splitter.is_none().then_some(0.5);
        self.trace.enabled = true;
        self.view = View::Plan;
        self.editor.status = if self.trace.splitter.is_some() {
            "Trace splitter shown · drag the vertical bar".into()
        } else {
            "Trace splitter hidden".into()
        };
    }

    fn trace_split_regions(&self, r: Rect) -> Option<(Rect, Rect)> {
        if !self.trace.enabled {
            return None;
        }
        let ratio = self.trace.splitter?.clamp(0.1, 0.9);
        let x = r.left() + r.width() * ratio;
        Some((
            Rect::from_min_max(r.left_top(), pos2(x, r.bottom())),
            Rect::from_min_max(pos2(x, r.top()), r.right_bottom()),
        ))
    }

    fn trace_pointer_in_plan(&mut self, ui: &Ui, rect: Rect) {
        let (delta, primary_down, primary_released, press_origin) = ui.input(|i| {
            (
                i.pointer.delta(),
                i.pointer.button_down(PointerButton::Primary),
                i.pointer.primary_released(),
                i.pointer.press_origin(),
            )
        });
        let dragging_reference = press_origin.is_some_and(|pos| rect.contains(pos));
        if primary_down && dragging_reference {
            if self.trace.displace_mode && self.trace.displace_origin.is_none() {
                self.trace.displace_origin = Some(self.trace.offset);
            }
            if delta != Vec2::ZERO {
                if self.trace.rotate_mode {
                    self.trace.rotation_deg =
                        (self.trace.rotation_deg + delta.x * 0.5).rem_euclid(360.);
                } else {
                    self.trace.offset.x += delta.x / self.plan_scale;
                    self.trace.offset.y -= delta.y / self.plan_scale;
                }
                ui.ctx().request_repaint();
            }
        }
        if primary_released && self.trace.displace_mode {
            if let Some(origin) = self.trace.displace_origin.take() {
                self.trace.offset = origin;
            }
            self.trace.displace_mode = false;
            self.editor.status = "Temporary Reference displacement ended".into();
        }
    }

    fn trace_cursor(&self, ui: &Ui) {
        let down = ui.input(|i| i.pointer.button_down(PointerButton::Primary));
        ui.ctx()
            .set_cursor_icon(match (self.trace.rotate_mode, down) {
                (true, true) => CursorIcon::ResizeHorizontal,
                (true, false) => CursorIcon::Crosshair,
                (false, true) => CursorIcon::Grabbing,
                (false, false) => CursorIcon::Grab,
            });
    }

    fn trace_palette(&mut self, ctx: &Context) {
        if !self.trace.palette_open {
            return;
        }
        let mut open = self.trace.palette_open;
        Window::new(self.language.translate("Trace & Reference"))
            .open(&mut open)
            .default_width(310.)
            .resizable(false)
            .show(ctx, |ui| self.trace_palette_contents(ui));
        self.trace.palette_open = open;
    }

    fn trace_palette_contents(&mut self, ui: &mut Ui) {
        if ui
            .checkbox(
                &mut self.trace.enabled,
                self.language.translate("Show Reference"),
            )
            .changed()
        {
            self.trace_toggled();
        }
        let mut choice = None;
        let stories = self.editor.project.stories.clone();
        ui.horizontal(|ui| {
            ui.label(self.language.translate("Reference:"));
            ComboBox::from_id_salt("trace_palette_reference")
                .selected_text(self.trace_reference_label())
                .width(190.)
                .show_ui(ui, |ui| {
                    for (reference, label) in [
                        (TraceReference::AboveCurrentStory, "Above Current Story"),
                        (TraceReference::BelowCurrentStory, "Below Current Story"),
                        (TraceReference::PreviousStory, "Previous Story"),
                        (TraceReference::EntireModel, "Entire Model Display"),
                    ] {
                        if ui
                            .selectable_label(
                                self.trace.reference == reference,
                                self.language.translate(label),
                            )
                            .clicked()
                        {
                            choice = Some(reference);
                        }
                    }
                    ui.separator();
                    for story in &stories {
                        let reference = TraceReference::Story(story.id);
                        if ui
                            .selectable_label(
                                self.trace.reference == reference,
                                format!("{:02} · {}", story.id, story.name),
                            )
                            .clicked()
                        {
                            choice = Some(reference);
                        }
                    }
                });
            if ui
                .button("…")
                .on_hover_text(self.language.translate("Browse for Trace Reference…"))
                .clicked()
            {
                self.trace.browse_choice = self.trace.reference;
                self.trace.browse_open = true;
            }
        });
        if let Some(reference) = choice {
            self.choose_trace_reference(reference);
        }
        ui.add_space(4.);
        ui.add(
            Slider::new(&mut self.trace.reference_opacity, 0..=255)
                .text(self.language.translate("Reference intensity")),
        );
        ui.add(
            Slider::new(&mut self.trace.active_opacity, 0..=255)
                .text(self.language.translate("Active intensity")),
        );
        ui.horizontal(|ui| {
            let mut original = self.trace.reference_color.is_none();
            if ui
                .checkbox(&mut original, self.language.translate("Original colors"))
                .changed()
            {
                self.trace.reference_color = (!original).then_some([116, 137, 145]);
            }
            if let Some(rgb) = &mut self.trace.reference_color {
                let mut color = Color32::from_rgb(rgb[0], rgb[1], rgb[2]);
                if ui.color_edit_button_srgba(&mut color).changed() {
                    *rgb = [color.r(), color.g(), color.b()];
                }
            }
        });
        ui.horizontal(|ui| {
            ui.label(self.language.translate("Rotation:"));
            ui.add(
                DragValue::new(&mut self.trace.rotation_deg)
                    .speed(0.5)
                    .fixed_decimals(1)
                    .suffix("°"),
            );
            if ui.button(self.language.translate("Reset")).clicked() {
                self.trace.reset_position();
                self.editor.status = "Trace Reference reset to default position".into();
            }
        });
        if let Some(ratio) = &mut self.trace.splitter {
            ui.add(
                Slider::new(ratio, 0.1..=0.9).text(self.language.translate("Splitter position")),
            );
        }
        ui.separator();
        let has_reference = self
            .trace
            .selection(
                &self.editor.project,
                self.editor.story,
                self.editor.previous_story,
            )
            .is_some();
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(
                    has_reference,
                    Button::new(self.language.translate("Activate")),
                )
                .clicked()
            {
                self.activate_trace_reference();
            }
            if ui
                .add_enabled(
                    has_reference,
                    Button::new(self.language.translate("Switch with Active")),
                )
                .clicked()
            {
                self.switch_trace_reference();
            }
            if ui
                .add_enabled(has_reference, Button::new(self.language.translate("Drag")))
                .clicked()
            {
                self.toggle_trace_drag();
            }
            if ui
                .add_enabled(
                    has_reference,
                    Button::new(self.language.translate("Rotate")),
                )
                .clicked()
            {
                self.toggle_trace_rotate();
            }
            if ui
                .add_enabled(
                    has_reference,
                    Button::new(self.language.translate("Rebuild")),
                )
                .clicked()
            {
                self.editor.status = "Trace Reference rebuilt from the live model".into();
            }
            if ui
                .add_enabled(
                    has_reference,
                    Button::new(self.language.translate("Displace")),
                )
                .clicked()
            {
                self.toggle_trace_displace();
            }
            if ui
                .add_enabled(
                    has_reference,
                    Button::new(self.language.translate("Splitter")),
                )
                .clicked()
            {
                self.toggle_trace_splitter();
            }
        });
        ui.checkbox(
            &mut self.trace.on_top,
            self.language.translate("Reference on Top"),
        );
        ui.checkbox(
            &mut self.trace.transparent_fills,
            self.language.translate("Transparent Fills and Zones"),
        );
        CollapsingHeader::new(self.language.translate("Visible Elements"))
            .default_open(false)
            .show(ui, |ui| {
                ui.checkbox(
                    &mut self.trace.visibility.walls,
                    self.language.translate("Walls"),
                );
                ui.checkbox(
                    &mut self.trace.visibility.slabs,
                    self.language.translate("Slabs"),
                );
                ui.checkbox(
                    &mut self.trace.visibility.stairs,
                    self.language.translate("Stairs"),
                );
                ui.checkbox(
                    &mut self.trace.visibility.polylines,
                    self.language.translate("Polylines"),
                );
            });
        ui.add_enabled(
            false,
            Button::new(self.language.translate("Apply Settings to All References")),
        )
        .on_disabled_hover_text(
            self.language
                .translate("OpenArchi currently has one floor-plan Reference"),
        );
        ui.label(
            RichText::new(self.language.translate(
                "Reference elements are visible but cannot be edited until Switch with Active.",
            ))
            .small()
            .color(MUTED),
        );
    }

    fn trace_browse_dialog(&mut self, ctx: &Context) {
        if !self.trace.browse_open {
            return;
        }
        let mut open = self.trace.browse_open;
        let mut choice = self.trace.browse_choice;
        let stories = self.editor.project.stories.clone();
        let mut accept = false;
        let mut cancel = false;
        Window::new(self.language.translate("Choose Trace Reference"))
            .open(&mut open)
            .default_width(390.)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label(
                    self.language
                        .translate("Only floor-plan stories and the entire model are available."),
                );
                ui.separator();
                ui.selectable_value(
                    &mut choice,
                    TraceReference::EntireModel,
                    self.language.translate("Entire Model Display"),
                );
                for story in &stories {
                    ui.selectable_value(
                        &mut choice,
                        TraceReference::Story(story.id),
                        format!(
                            "{:02} · {}     {:+.2} m",
                            story.id, story.name, story.elevation
                        ),
                    );
                }
                ui.separator();
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui
                        .add_sized(
                            [88., 30.],
                            Button::new(self.language.translate("OK")).fill(ACCENT),
                        )
                        .clicked()
                    {
                        accept = true;
                    }
                    if ui.button(self.language.translate("Cancel")).clicked() {
                        cancel = true;
                    }
                });
            });
        self.trace.browse_choice = choice;
        if accept {
            self.choose_trace_reference(choice);
            self.trace.browse_open = false;
            self.trace.palette_open = true;
        } else {
            self.trace.browse_open = open && !cancel;
        }
    }

    fn trace_active_fill(&self, color: Color32) -> Color32 {
        if self.trace.enabled && self.trace.transparent_fills {
            Color32::TRANSPARENT
        } else {
            color
        }
    }

    fn trace_reference_pivot(&self, selection: TraceSelection, elements: &[&Element]) -> Point {
        let mut min = Point::new(f32::INFINITY, f32::INFINITY);
        let mut max = Point::new(f32::NEG_INFINITY, f32::NEG_INFINITY);
        for story in selection.story_ids(&self.editor.project) {
            for element in elements.iter().filter(|element| element.story == story) {
                for point in geometry::footprint(element) {
                    min.x = min.x.min(point.x);
                    min.y = min.y.min(point.y);
                    max.x = max.x.max(point.x);
                    max.y = max.y.max(point.y);
                }
            }
        }
        if min.x.is_finite() && min.y.is_finite() && max.x.is_finite() && max.y.is_finite() {
            Point::new((min.x + max.x) * 0.5, (min.y + max.y) * 0.5)
        } else {
            Point::default()
        }
    }

    fn paint_trace_reference(&self, painter: &Painter, rect: Rect, clip: Option<Rect>) {
        if !self.trace.enabled || self.trace.reference_opacity == 0 {
            return;
        }
        let Some(selection) = self.trace.selection(
            &self.editor.project,
            self.editor.story,
            self.editor.previous_story,
        ) else {
            return;
        };
        let elements: Vec<_> = self
            .editor
            .project
            .elements
            .iter()
            .filter(|element| self.trace.visibility.shows(element))
            .collect();
        if elements.is_empty() {
            return;
        }
        let pivot = self.trace_reference_pivot(selection, &elements);
        let painter = clip.map_or_else(|| painter.clone(), |clip| painter.with_clip_rect(clip));
        let opacity = u16::from(self.trace.reference_opacity);
        let fill_alpha = ((opacity * 42) / 100).min(255) as u8;
        let stroke_alpha = self.trace.reference_opacity;
        let reference_color = |element: &Element, alpha: u8| {
            let [red, green, blue] = self.trace.color_for(element.material);
            Color32::from_rgba_unmultiplied(red, green, blue, alpha)
        };
        let transform = |point: Point| self.trace.transform(point, pivot);
        let screen = |point: Point| self.screen(rect, transform(point));
        for story in selection.story_ids(&self.editor.project) {
            let story_elements: Vec<_> = elements
                .iter()
                .copied()
                .filter(|element| element.story == story)
                .collect();
            for element in story_elements
                .iter()
                .copied()
                .filter(|element| !matches!(element.shape, Shape::Wall { .. }))
            {
                match &element.shape {
                    Shape::Polyline { points, arcs } => {
                        let path: Vec<Pos2> = geometry::polyline_path(points, arcs)
                            .into_iter()
                            .map(screen)
                            .collect();
                        if path.len() >= 2 {
                            painter.add(egui::Shape::line(
                                path,
                                Stroke::new(1.15, reference_color(element, stroke_alpha)),
                            ));
                        }
                    }
                    Shape::Slab { .. } | Shape::Stair { .. } => {
                        let points: Vec<Pos2> = geometry::footprint(element)
                            .into_iter()
                            .map(screen)
                            .collect();
                        let fill = if self.trace.transparent_fills {
                            Color32::TRANSPARENT
                        } else {
                            reference_color(element, fill_alpha)
                        };
                        painter.add(egui::Shape::convex_polygon(
                            points,
                            fill,
                            Stroke::new(1.05, reference_color(element, stroke_alpha)),
                        ));
                        if let Shape::Stair { ref spec } = element.shape
                            && let Ok(stair) = crate::stairs::generate(spec, 0.0)
                        {
                            for [start, end] in stair.tread_lines {
                                painter.line_segment(
                                    [screen(start), screen(end)],
                                    Stroke::new(
                                        0.75,
                                        reference_color(element, stroke_alpha.saturating_sub(30)),
                                    ),
                                );
                            }
                        }
                    }
                    Shape::Wall { .. } | Shape::Opening { .. } => {}
                }
            }
            let walls: Vec<_> = story_elements
                .iter()
                .copied()
                .filter(|element| matches!(element.shape, Shape::Wall { .. }))
                .collect();
            // Trace fills follow material colors, but wall junctions are
            // physical geometry shared by every material on the story.
            let footprints_by_id: std::collections::HashMap<u64, [Point; 4]> =
                geometry::plan_wall_footprints_by_id(&walls)
                    .into_iter()
                    .collect();
            for material in Material::ALL {
                let same_material: Vec<_> = walls
                    .iter()
                    .copied()
                    .filter(|wall| wall.material == material)
                    .collect();
                if same_material.is_empty() {
                    continue;
                }
                if !self.trace.transparent_fills {
                    for wall in &same_material {
                        let footprint = footprints_by_id[&wall.id];
                        let points: Vec<Pos2> = footprint.iter().copied().map(screen).collect();
                        painter.add(egui::Shape::convex_polygon(
                            points,
                            reference_color(wall, fill_alpha),
                            // Same-colour bleed seals sub-pixel gaps between
                            // separately editable wall footprints.
                            Stroke::new(0.75, reference_color(wall, fill_alpha)),
                        ));
                    }
                }
            }
            let story_footprints: Vec<_> = walls
                .iter()
                .map(|wall| footprints_by_id[&wall.id])
                .collect();
            // One exposed perimeter for the whole story keeps the outline
            // continuous across material boundaries.
            let outline_color = Color32::from_rgba_unmultiplied(36, 40, 42, stroke_alpha);
            for mut path in geometry::wall_outline_paths(&story_footprints) {
                let closed = path.len() > 2 && path[0].distance(*path.last().unwrap()) <= 1e-3;
                if closed {
                    path.pop();
                }
                let points: Vec<Pos2> = path.into_iter().map(screen).collect();
                if points.len() < 2 {
                    continue;
                }
                if closed {
                    painter.add(egui::Shape::closed_line(
                        points,
                        Stroke::new(1.1, outline_color),
                    ));
                } else {
                    painter.add(egui::Shape::line(points, Stroke::new(1.1, outline_color)));
                }
            }
        }
    }
}
