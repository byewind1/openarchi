// Native Trace menu, palette and reference overlay. Included from app.rs so the
// controls share the application state without exposing extra public fields.
use crate::trace::{TraceReference, TraceSelection};

/// Widget-only state: two ID sets discard egui's deferred DragValue text after
/// Escape without clearing other controls or storing a numeric draft in Project.
#[derive(Clone, Default)]
struct TraceNumericUi {
    alternate: bool,
    editing: bool,
    ids: Vec<Id>,
}

impl AtelierApp {
    fn active_trace_view(&self) -> crate::trace_document::TraceView {
        self.host
            .document
            .editor
            .project
            .plan_options
            .active_worksheet
            .map(crate::trace_document::TraceView::Worksheet)
            .unwrap_or_default()
    }

    fn trace_can_navigate(&self) -> bool {
        if !self.trace.enabled {
            return false;
        }
        self.trace
            .selection(
                &self.host.document.editor.project,
                self.host.document.editor.story,
                self.host.document.editor.previous_story,
            )
            .is_some_and(|s| !matches!(s, TraceSelection::EntireModel))
    }

    fn trace_has_reference(&self) -> bool {
        self.trace.enabled
            && self
                .trace
                .selection(
                    &self.host.document.editor.project,
                    self.host.document.editor.story,
                    self.host.document.editor.previous_story,
                )
                .is_some()
    }
    fn trace_choice_enabled(&self, reference: TraceReference) -> bool {
        self.trace.enabled
            || matches!(
                reference,
                TraceReference::AboveCurrentStory
                    | TraceReference::BelowCurrentStory
                    | TraceReference::PreviousStory
            )
    }

    fn reset_trace_position(&mut self) {
        self.trace.cancel_pointer_modes();
        self.trace.reset_position();
        self.trace.pivot = Some(self.trace_current_pivot());
        self.persist_trace_state(false);
        self.host.document.editor.status = "Trace Reference reset to default position".into();
    }

    fn turn_off_trace_everywhere(&mut self) {
        self.trace.cancel_pointer_modes();
        self.trace.enabled = false;
        self.persist_trace_state(false);
        self.host.turn_off_all_trace();
        self.host.document.editor.status = "Trace turned off everywhere".into();
    }

    fn rebuild_trace_reference(&mut self) {
        // Geometry is derived from the current authoritative project on every
        // frame. Invalidate only display caches; external source updates retain
        // their existing reviewed import/relink transaction.
        self.drawings_ui.textures_clear();
        self.host.document.editor.status =
            match self.host.rebuild_trace_sources(self.trace.reference) {
                Ok(_) => "Trace refreshed · external source changes use Update CAD source".into(),
                Err(error) => error,
            };
    }

    /// Commit a validated projection through the host, never a second model or
    /// a geometry command. Dialogs, temporary displacement and gestures stay local.
    fn persist_trace_state(&mut self, remember: bool) -> bool {
        let settings = crate::trace_document::TraceSettings::capture(&self.trace);
        // Opening an unchanged old document must not invent a Trace record.
        if !remember
            && settings == Default::default()
            && self.host.document.editor.project.trace.is_empty()
        {
            return true;
        }
        let editor = &self.host.document.editor;
        if let Err(error) = self.host.set_trace_window(
            crate::trace_document::TraceContext {
                view: self.trace_view,
                story: editor.story,
                previous_story: editor.previous_story,
                plan_visible: self.view == View::Plan,
                viewport: crate::trace_document::TraceViewport {
                    center: self.plan_center,
                    scale: self.plan_scale,
                },
            },
            settings,
            remember,
        ) {
            let palette_open = self.trace.palette_open;
            self.trace = self
                .host
                .document
                .editor
                .project
                .trace
                .settings(self.trace_view)
                .map(|s| s.runtime())
                .unwrap_or_default();
            self.trace.palette_open = palette_open;
            self.host.document.editor.status = error;
            return false;
        }
        self.trace.source_identity = self
            .host
            .document
            .editor
            .project
            .trace
            .settings(self.trace_view)
            .and_then(|s| s.source_identity.clone());
        true
    }

    fn restore_trace_window(&mut self) {
        self.trace_view = self.active_trace_view();
        let palette_open = self.trace.palette_open;
        let project = &self.host.document.editor.project;
        self.trace = project
            .trace
            .settings(self.trace_view)
            .map(|s| s.runtime())
            .unwrap_or_default();
        // Legacy image sheets use their source image as the active workspace.
        if project.trace.settings(self.trace_view).is_none()
            && let crate::trace_document::TraceView::Worksheet(id) = self.trace_view
        {
            if project
                .drawings
                .pages
                .iter()
                .any(|p| p.id == id && p.placement.is_none())
            {
                self.trace.reference = TraceReference::Worksheet(id);
                self.trace.enabled = true;
            } else {
                self.trace.reference = TraceReference::Story(self.host.document.editor.story);
            }
        }
        if self.trace.source_identity.is_none() {
            self.trace.source_identity = crate::trace_document::TraceSourceIdentity::for_reference(
                project,
                self.trace.reference,
            );
        }
        self.trace.palette_open = palette_open;
        if self.trace.enabled
            && self
                .trace
                .selection(
                    project,
                    self.host.document.editor.story,
                    self.host.document.editor.previous_story,
                )
                .is_none()
        {
            self.trace.enabled = false;
            self.host.document.editor.status = "Saved Trace Reference is unavailable".into();
        }
    }

    fn fit_opened_project(&mut self) {
        if self.host.document.editor.project.trace.plan_visible
            && let Some(viewport) = self.host.document.editor.project.trace.viewport
        {
            self.plan_center = viewport.center;
            self.plan_scale = viewport.scale;
            self.drawings_ui.fit_pending = None;
        } else {
            self.fit();
        }
    }

    fn restore_trace_document(&mut self) {
        self.trace = Default::default();
        let project = &self.host.document.editor.project;
        let invalid_window = matches!(project.trace.active_view, crate::trace_document::TraceView::Worksheet(id) if !project.drawings.worksheet_exists(id));
        if !project.trace.is_empty() {
            self.view = if project.trace.plan_visible {
                View::Plan
            } else {
                View::Model
            };
            let scope = match project.trace.active_view {
                crate::trace_document::TraceView::Worksheet(id)
                    if project.drawings.worksheet_exists(id) =>
                {
                    Some(id)
                }
                _ => None,
            };
            self.host.document.editor.switch_worksheet(scope);
        }
        self.restore_trace_window();
        if invalid_window {
            self.trace.enabled = false;
            self.host.document.editor.status = "Saved Trace window is unavailable".into();
        }
        if let Some(viewport) = self.host.document.editor.project.trace.viewport {
            self.plan_center = viewport.center;
            self.plan_scale = viewport.scale;
        }
    }

    fn sync_trace_context(&mut self) {
        if self.trace_view != self.active_trace_view() {
            self.persist_trace_state(false);
            self.restore_trace_window();
        }
    }

    fn trace_toolbar_menu(&mut self, ui: &mut Ui) {
        // Draw the chevron directly: a font-dependent glyph can appear as a
        // missing-character square, especially with the Chinese font stack.
        let (response, _) = egui::containers::menu::MenuButton::from_button(
            Button::new("").min_size(vec2(24.0, ui.spacing().interact_size.y)),
        )
        .ui(ui, |ui| self.trace_menu(ui));
        response.widget_info(|| {
            egui::WidgetInfo::labeled(
                egui::WidgetType::Button,
                response.enabled(),
                self.language.translate("Trace Options"),
            )
        });
        let center = response.rect.center();
        let stroke = ui.style().interact(&response).fg_stroke;
        ui.painter().add(egui::Shape::line(
            vec![
                center + vec2(-4.0, -2.0),
                center + vec2(0.0, 2.0),
                center + vec2(4.0, -2.0),
            ],
            Stroke::new(1.5, stroke.color),
        ));
        response.on_hover_text(self.language.translate("Trace Options"));
    }

    fn trace_toggled(&mut self) {
        if self.trace.enabled {
            self.view = View::Plan;
            if self
                .trace
                .selection(
                    &self.host.document.editor.project,
                    self.host.document.editor.story,
                    self.host.document.editor.previous_story,
                )
                .is_none()
                && self.trace.reference.is_relative()
                && let Some(story) = self.nearest_trace_story()
            {
                self.trace.reference = TraceReference::Story(story);
            }
            self.host.document.editor.status = match self.trace.selection(
                &self.host.document.editor.project,
                self.host.document.editor.story,
                self.host.document.editor.previous_story,
            ) {
                Some(_) => format!("Trace Reference on · {}", self.trace_reference_label()),
                None => "Trace Reference on · no reference is available for this story".into(),
            };
        } else {
            self.trace.cancel_pointer_modes();
            self.host.document.editor.status = "Trace Reference off".into();
        }
        if self.trace.enabled && self.trace.pivot.is_none() {
            self.trace.pivot = Some(self.trace_current_pivot());
        }
        self.persist_trace_state(false);
    }

    fn open_trace_palette(&mut self) {
        // Opening the palette does not change the project's Trace switch.
        self.trace.palette_open = true;
        self.host.document.editor.status = "Trace & Reference palette".into();
    }

    fn nearest_trace_story(&self) -> Option<u32> {
        let stories = crate::trace::ordered_story_ids(&self.host.document.editor.project);
        let index = stories
            .iter()
            .position(|id| *id == self.host.document.editor.story)?;
        stories
            .get(index + 1)
            .or_else(|| index.checked_sub(1).and_then(|index| stories.get(index)))
            .copied()
    }

    fn trace_reference_label(&self) -> String {
        self.trace_label(self.trace.reference)
    }

    fn trace_label(&self, reference: TraceReference) -> String {
        match reference {
            TraceReference::Worksheet(id) => self
                .host
                .document
                .editor
                .project
                .drawings
                .worksheet_name(id)
                .unwrap_or(self.language.translate("Worksheet"))
                .to_owned(),
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
                .host
                .document
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

    fn trace_worksheet_choices(&self) -> Vec<(TraceReference, String)> {
        let doc = &self.host.document.editor.project.drawings;
        doc.worksheets
            .iter()
            .map(|w| (w.id, w.name.as_str()))
            .chain(
                doc.pages
                    .iter()
                    .filter(|p| p.placement.is_none())
                    .map(|p| (p.id, p.name.as_str())),
            )
            .map(|(id, name)| {
                (
                    TraceReference::Worksheet(id),
                    format!("{} · {}", self.language.translate("Worksheet"), name),
                )
            })
            .collect()
    }

    fn trace_recent_choices(&self) -> Vec<(TraceReference, String)> {
        self.host
            .document
            .editor
            .project
            .trace
            .recent
            .iter()
            .copied()
            .filter(|r| matches!(r, TraceReference::Worksheet(_) | TraceReference::Story(_)))
            .filter(|r| {
                r.resolve(
                    &self.host.document.editor.project,
                    self.host.document.editor.story,
                    self.host.document.editor.previous_story,
                )
                .is_some()
            })
            .map(|r| (r, self.trace_label(r)))
            .collect()
    }
    /// One pivot for CAD, placed images and annotations in the same worksheet.
    pub(super) fn trace_worksheet_pivot(&self, id: u64) -> Point {
        let doc = &self.host.document.editor.project.drawings;
        // Legacy source worksheets retain their calibrated image pivot.
        if doc
            .pages
            .iter()
            .any(|p| p.id == id && p.placement.is_none())
        {
            return self.worksheet_pivot(id);
        }
        let mut points = Vec::new();
        for u in doc
            .cad_underlays
            .iter()
            .filter(|u| u.worksheet == id && u.visible)
        {
            if let Some(b) = doc
                .cad_sources
                .iter()
                .find(|s| s.id == u.source)
                .and_then(|s| s.report.bounds)
            {
                points.extend(
                    [[b[0], b[1]], [b[2], b[3]], [b[0], b[3]], [b[2], b[1]]]
                        .map(|p| u.transform.world(p)),
                );
            }
        }
        for page in &doc.pages {
            if let Some(v) = &page.placement
                && v.worksheet == Some(id)
                && v.visible
            {
                let b = v
                    .frame
                    .unwrap_or([0., 0., page.pixels[0] as f64, page.pixels[1] as f64]);
                let scale = self.worksheet_pixel_scale(page.id);
                points.extend(
                    [[b[0], b[1]], [b[2], b[3]], [b[0], b[3]], [b[2], b[1]]]
                        .map(|p| v.world(p, scale)),
                );
            }
        }
        for e in self
            .host
            .document
            .editor
            .project
            .elements
            .iter()
            .filter(|e| e.worksheet == Some(id))
        {
            points.extend(geometry::element_plan_outline(
                &self.host.document.editor.project,
                e,
            ));
        }
        let Some(first) = points.first().copied() else {
            return Point::default();
        };
        let (lo, hi) = points.iter().fold((first, first), |(lo, hi), p| {
            (
                Point::new(lo.x.min(p.x), lo.y.min(p.y)),
                Point::new(hi.x.max(p.x), hi.y.max(p.y)),
            )
        });
        Point::new((lo.x + hi.x) / 2., (lo.y + hi.y) / 2.)
    }
    pub(super) fn trace_cad_snap(&self, p: Point, tolerance: f64) -> Option<Point> {
        if !self.trace.enabled
            || self.trace.reference_opacity == 0
            || !self.trace.visibility.drawings
        {
            return None;
        }
        let TraceReference::Worksheet(id) = self.trace.reference else {
            return None;
        };
        if self
            .trace
            .selection(
                &self.host.document.editor.project,
                self.host.document.editor.story,
                self.host.document.editor.previous_story,
            )
            .is_none()
        {
            return None;
        }
        if self
            .host
            .document
            .editor
            .project
            .plan_options
            .active_worksheet
            == Some(id)
        {
            return None;
        }
        let doc = &self.host.document.editor.project.drawings;
        let pivot = self.trace_worksheet_pivot(id);
        let mut best = None;
        let mut distance = tolerance;
        for u in doc
            .cad_underlays
            .iter()
            .filter(|u| u.worksheet == id && u.visible)
        {
            let Some(data) = doc
                .cad_sources
                .iter()
                .find(|s| s.id == u.source)
                .and_then(|s| s.data.as_ref())
            else {
                continue;
            };
            let transform = self.trace.cad_transform(&u.transform, pivot);
            if let Some(q) = crate::cad_reference::snap_underlay(data, u, &transform, p, distance) {
                distance = p.distance(q);
                best = Some(q);
            }
        }
        best
    }

    fn trace_model_snap(&self, point: Point, tolerance: f64) -> Option<(Point, SnapKind)> {
        if !self.trace.enabled || self.trace.reference_opacity == 0 {
            return None;
        }
        let editor = &self.host.document.editor;
        let project = &editor.project;
        let selection = self
            .trace
            .selection(project, editor.story, editor.previous_story)?;
        let pivot = self.trace_current_pivot();
        let mut best = None;
        let mut distance = tolerance;
        for element in &project.elements {
            if !self.trace.visibility.shows(element)
                || !project
                    .plan_options
                    .element_visible(element, &project.layers)
            {
                continue;
            }
            let stories = match selection {
                TraceSelection::Worksheet(id) if element.worksheet == Some(id) => {
                    vec![element.story]
                }
                TraceSelection::Story(id) if element.worksheet.is_none() => vec![id],
                TraceSelection::EntireModel if element.worksheet.is_none() => {
                    crate::trace::ordered_story_ids(project)
                }
                _ => continue,
            };
            for story in stories {
                if element.worksheet.is_none()
                    && !geometry::element_on_plan_story(project, element, story)
                {
                    continue;
                }
                for (source, kind) in geometry::element_snap_points(project, element, story) {
                    let candidate = self.trace.transform(source, pivot);
                    let d = point.distance(candidate);
                    if d < distance {
                        best = Some((candidate, kind));
                        distance = d;
                    }
                }
            }
        }
        best
    }

    fn choose_trace_reference(&mut self, reference: TraceReference) {
        self.sync_trace_context();
        self.trace.cancel_pointer_modes();
        let identity = crate::trace_document::TraceSourceIdentity::for_reference(
            &self.host.document.editor.project,
            reference,
        );
        if self.trace.reference != reference || self.trace.source_identity != identity {
            self.trace.reset_position();
        }
        self.trace.reference = reference;
        self.trace.source_identity = identity;
        self.trace.enabled = true;
        self.trace.pivot = Some(self.trace_current_pivot());
        self.trace.browse_open = false;
        self.trace.cancel_pointer_modes();
        self.view = View::Plan;
        self.host.document.editor.status = if self
            .trace
            .selection(
                &self.host.document.editor.project,
                self.host.document.editor.story,
                self.host.document.editor.previous_story,
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
        self.persist_trace_state(true);
    }

    fn trace_menu(&mut self, ui: &mut Ui) {
        let before = crate::trace_document::TraceSettings::capture(&self.trace);
        let plan_view = self.view == View::Plan;
        let has_reference = self.trace_has_reference();
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
                    .add_enabled(
                        self.trace.enabled,
                        Button::new(self.language.translate("Browse for Trace Reference…")),
                    )
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
                        .add_enabled(
                            self.trace_choice_enabled(reference),
                            Button::selectable(
                                self.trace.reference == reference,
                                self.language.translate(label),
                            ),
                        )
                        .clicked()
                    {
                        self.choose_trace_reference(reference);
                        ui.close();
                    }
                }
                let recent = self.trace_recent_choices();
                if !recent.is_empty() {
                    ui.separator();
                    ui.label(self.language.translate("Recent References"));
                    for (reference, label) in recent {
                        if ui
                            .add_enabled(
                                self.trace.enabled,
                                Button::selectable(self.trace.reference == reference, label),
                            )
                            .clicked()
                        {
                            self.choose_trace_reference(reference);
                            ui.close();
                        }
                    }
                }
                ui.separator();
                for (reference, label) in self.trace_worksheet_choices() {
                    if ui
                        .add_enabled(
                            self.trace.enabled,
                            Button::selectable(self.trace.reference == reference, label),
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
                    self.trace_can_navigate(),
                    Button::new(self.language.translate("Activate Reference")),
                )
                .clicked();
            switch_reference = ui
                .add_enabled(
                    self.trace_can_navigate(),
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
            ui.add_enabled(
                self.trace.enabled,
                egui::Checkbox::new(
                    &mut self.trace.on_top,
                    self.language.translate("Reference on Top"),
                ),
            );
            ui.add_enabled(
                self.trace.enabled,
                egui::Checkbox::new(
                    &mut self.trace.transparent_fills,
                    self.language.translate("Transparent Fills and Zones"),
                ),
            );
            if ui
                .button(self.language.translate("Turn off Trace everywhere"))
                .clicked()
            {
                self.turn_off_trace_everywhere();
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
            self.reset_trace_position();
        }
        if rebuild {
            self.rebuild_trace_reference();
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
        if before != crate::trace_document::TraceSettings::capture(&self.trace) {
            self.persist_trace_state(false);
        }
    }

    fn activate_trace_reference(&mut self) {
        let Some(selection) = self.trace.selection(
            &self.host.document.editor.project,
            self.host.document.editor.story,
            self.host.document.editor.previous_story,
        ) else {
            self.host.document.editor.status = "No Trace Reference is available".into();
            return;
        };
        if matches!(selection, TraceSelection::EntireModel) {
            self.host.document.editor.status =
                "Entire Model Display cannot be activated as a story".into();
            return;
        }
        // Cancel temporary displacement before reading the committed alignment.
        self.trace.cancel_pointer_modes();
        let center = self
            .trace
            .inverse_transform(self.plan_center, self.trace_current_pivot());
        if let TraceSelection::Worksheet(id) = selection {
            self.enter_worksheet(id);
            self.plan_center = center;
            return;
        }
        let TraceSelection::Story(story) = selection else {
            unreachable!()
        };
        if self.trace.reference == TraceReference::Story(story) {
            self.trace.reference = TraceReference::PreviousStory;
        }
        self.persist_trace_state(false);
        self.host.document.editor.switch_worksheet(None);
        self.host.document.editor.switch_story(story);
        self.sync_trace_context();
        self.drawings_ui.fit_pending = None;
        self.plan_center = center;
        self.trace.enabled = true;
        self.view = View::Plan;
        self.host.document.editor.status = format!("Activated Trace Reference · story {story:02}");
        self.persist_trace_state(false);
    }

    fn trace_current_pivot(&self) -> Point {
        if let Some(pivot) = self.trace.pivot {
            return pivot;
        }
        if let TraceReference::Worksheet(id) = self.trace.reference {
            return self.trace_worksheet_pivot(id);
        }
        let Some(selection) = self.trace.selection(
            &self.host.document.editor.project,
            self.host.document.editor.story,
            self.host.document.editor.previous_story,
        ) else {
            return Point::default();
        };
        let elements: Vec<_> = self
            .host
            .document
            .editor
            .project
            .elements
            .iter()
            .filter(|e| e.worksheet.is_none() && self.trace.visibility.shows(e))
            .collect();
        Self::trace_batch_pivot(&self.trace_geometry_batches(selection, &elements))
    }
    fn switch_trace_reference(&mut self) {
        let Some(selection) = self.trace.selection(
            &self.host.document.editor.project,
            self.host.document.editor.story,
            self.host.document.editor.previous_story,
        ) else {
            self.host.document.editor.status = "No Trace Reference is available".into();
            return;
        };
        if matches!(selection, TraceSelection::EntireModel) {
            self.host.document.editor.status =
                "Entire Model Display cannot be switched with the Active".into();
            return;
        }
        let active = self
            .host
            .document
            .editor
            .project
            .plan_options
            .active_worksheet
            .map(TraceReference::Worksheet)
            .unwrap_or(TraceReference::Story(self.host.document.editor.story));
        let old_pivot = self.trace_current_pivot();
        self.trace.cancel_pointer_modes();
        let old = self.trace.clone();
        let center = old.inverse_transform(self.plan_center, old_pivot);
        match selection {
            TraceSelection::Worksheet(id) => {
                self.enter_worksheet(id);
            }
            TraceSelection::Story(story) => {
                self.host.document.editor.switch_worksheet(None);
                self.host.document.editor.switch_story(story);
            }
            TraceSelection::EntireModel => unreachable!(),
        }
        self.drawings_ui.fit_pending = None;
        self.plan_center = center;
        self.trace = old.clone();
        self.trace.reference = active;
        self.trace.source_identity = crate::trace_document::TraceSourceIdentity::for_reference(
            &self.host.document.editor.project,
            active,
        );
        self.trace.pivot = None;
        self.trace.enabled = true;
        self.trace.cancel_pointer_modes();
        let pivot = self.trace_current_pivot();
        let transformed = old.inverse_transform(pivot, old_pivot);
        self.trace.offset = Point::new(transformed.x - pivot.x, transformed.y - pivot.y);
        self.trace.rotation_deg = -old.rotation_deg;
        self.trace.pivot = Some(pivot);
        self.view = View::Plan;
        self.host.document.editor.status =
            format!("Trace Reference · {}", self.trace_reference_label());
        self.trace_view = self.active_trace_view();
        self.persist_trace_state(true);
    }

    fn toggle_trace_drag(&mut self) {
        let active = !self.trace.drag_mode;
        self.trace.cancel_pointer_modes();
        self.trace.drag_mode = active;
        self.trace.gesture_origin = active.then_some((self.trace.offset, self.trace.rotation_deg));
        self.trace.tool_on_entry = active.then_some(self.host.document.editor.tool);
        self.trace.enabled = true;
        self.view = View::Plan;
        self.host.document.editor.status = if active {
            "Drag Reference · choose start and destination · Esc cancels".into()
        } else {
            "Drag Reference cancelled".into()
        };
    }

    fn toggle_trace_rotate(&mut self) {
        let active = !self.trace.rotate_mode;
        self.trace.cancel_pointer_modes();
        self.trace.rotate_mode = active;
        self.trace.gesture_origin = active.then_some((self.trace.offset, self.trace.rotation_deg));
        self.trace.tool_on_entry = active.then_some(self.host.document.editor.tool);
        self.trace.enabled = true;
        self.view = View::Plan;
        self.host.document.editor.status = if active {
            "Rotate Reference · choose center, start and end · Esc cancels".into()
        } else {
            "Rotate Reference cancelled".into()
        };
    }

    fn toggle_trace_displace(&mut self) {
        let active = !self.trace.displace_mode;
        self.trace.cancel_pointer_modes();
        self.trace.displace_mode = active;
        self.trace.displace_origin = active.then_some(self.trace.offset);
        self.trace.tool_on_entry = active.then_some(self.host.document.editor.tool);
        self.trace.enabled = true;
        self.view = View::Plan;
        self.host.document.editor.status = if active {
            "Temporarily displace Reference · choose a point, move, click to restore".into()
        } else {
            "Temporary Reference displacement ended".into()
        };
    }

    fn toggle_trace_splitter(&mut self) {
        self.trace.splitter = self.trace.splitter.is_none().then_some(0.5);
        self.trace.enabled = true;
        self.view = View::Plan;
        self.host.document.editor.status = if self.trace.splitter.is_some() {
            "Trace splitter shown · drag any edge handle".into()
        } else {
            "Trace splitter hidden".into()
        };
    }

    fn trace_split_regions(&self, r: Rect) -> Option<(Rect, Rect)> {
        if !self.trace.enabled {
            return None;
        }
        let horizontal = self.trace.split_axis?;
        let ratio = self.trace.splitter?.clamp(0., 1.) as f32;
        let (a, b) = if horizontal {
            let y = r.top() + r.height() * ratio;
            (
                Rect::from_min_max(r.left_top(), pos2(r.right(), y)),
                Rect::from_min_max(pos2(r.left(), y), r.right_bottom()),
            )
        } else {
            let x = r.left() + r.width() * ratio;
            (
                Rect::from_min_max(r.left_top(), pos2(x, r.bottom())),
                Rect::from_min_max(pos2(x, r.top()), r.right_bottom()),
            )
        };
        Some(if self.trace.split_from_end {
            (b, a)
        } else {
            (a, b)
        })
    }

    fn trace_splitter_handles(r: Rect) -> [(Rect, bool, bool); 4] {
        [
            (
                Rect::from_center_size(pos2(r.left() + 8., r.center().y), vec2(16., 36.)),
                false,
                false,
            ),
            (
                Rect::from_center_size(pos2(r.right() - 8., r.center().y), vec2(16., 36.)),
                false,
                true,
            ),
            (
                Rect::from_center_size(pos2(r.center().x, r.top() + 8.), vec2(36., 16.)),
                true,
                false,
            ),
            (
                Rect::from_center_size(pos2(r.center().x, r.bottom() - 8.), vec2(36., 16.)),
                true,
                true,
            ),
        ]
    }

    fn trace_splitter_input(&mut self, ui: &mut Ui, r: Rect) {
        if !self.trace.enabled || self.trace.splitter.is_none() || self.trace.pointer_mode_active()
        {
            return;
        }
        for (index, (handle, horizontal, from_end)) in
            Self::trace_splitter_handles(r).into_iter().enumerate()
        {
            let response = ui.interact(handle, Id::new(("trace_splitter", index)), Sense::drag());
            if response.dragged() {
                self.trace.split_axis = Some(horizontal);
                self.trace.split_from_end = from_end;
                if let Some(p) = response.interact_pointer_pos() {
                    let ratio = if horizontal {
                        (p.y - r.top()) / r.height()
                    } else {
                        (p.x - r.left()) / r.width()
                    };
                    self.trace.splitter = Some(f64::from(ratio.clamp(0., 1.)));
                }
            }
            if response.drag_stopped() {
                self.trace.split_axis = None;
                self.trace.splitter = Some(0.5);
            }
            response.on_hover_cursor(if horizontal {
                CursorIcon::ResizeVertical
            } else {
                CursorIcon::ResizeHorizontal
            });
        }
    }

    fn paint_trace_splitter(&self, ui: &Ui, r: Rect, regions: Option<(Rect, Rect)>) {
        if !self.trace.enabled || self.trace.splitter.is_none() {
            return;
        }
        if let Some((reference, active)) = regions {
            let ratio = self.trace.splitter.unwrap_or(0.5) as f32;
            let line = if self.trace.split_axis == Some(true) {
                let y = r.top() + r.height() * ratio;
                [pos2(r.left(), y), pos2(r.right(), y)]
            } else {
                let x = r.left() + r.width() * ratio;
                [pos2(x, r.top()), pos2(x, r.bottom())]
            };
            ui.painter().line_segment(line, Stroke::new(1.5, LINE));
            for (area, label) in [(reference, "Reference"), (active, "Active")] {
                ui.painter().text(
                    area.left_top() + vec2(8., 8.),
                    Align2::LEFT_TOP,
                    self.language.translate(label),
                    FontId::proportional(10.),
                    MUTED,
                );
            }
        } else {
            for (handle, horizontal, _) in Self::trace_splitter_handles(r) {
                ui.painter()
                    .rect_filled(handle, 3., Color32::from_black_alpha(35));
                let center = handle.center();
                let delta = if horizontal {
                    vec2(5., 0.)
                } else {
                    vec2(0., 5.)
                };
                ui.painter()
                    .line_segment([center - delta, center + delta], Stroke::new(1.5, MUTED));
            }
        }
    }

    fn finish_trace_gesture(&mut self) {
        let mut candidate = self.trace.clone();
        candidate.gesture_origin = None;
        candidate.displace_origin = None;
        if let Err(error) = crate::trace_document::TraceSettings::capture(&candidate).validate() {
            self.host.document.editor.status = error;
            return;
        }
        self.trace.gesture_origin = None;
        self.trace.cancel_pointer_modes();
        self.persist_trace_state(false);
        self.host.document.editor.status = "Trace placement confirmed".into();
    }

    fn trace_rotation_preview(&mut self, delta: f64) {
        let (Some(center), Some((offset, angle))) =
            (self.trace.pointer_anchor, self.trace.gesture_origin)
        else {
            return;
        };
        let pivot = self.trace_current_pivot();
        let old_center = Point::new(pivot.x + offset.x, pivot.y + offset.y);
        let (sin, cos) = delta.to_radians().sin_cos();
        let x = old_center.x - center.x;
        let y = old_center.y - center.y;
        self.trace.offset = Point::new(
            center.x + x * cos - y * sin - pivot.x,
            center.y + x * sin + y * cos - pivot.y,
        );
        self.trace.rotation_deg = angle + delta;
    }

    fn trace_pointer_in_plan(&mut self, ui: &Ui, rect: Rect) {
        if !ui.input(|i| i.focused) {
            self.trace.cancel_pointer_modes();
            return;
        }
        let (pressed, released, clicked, position) = ui.input(|i| {
            (
                i.pointer.primary_pressed(),
                i.pointer.primary_released(),
                i.pointer.primary_clicked(),
                i.pointer.interact_pos(),
            )
        });
        let Some(position) = position
            .filter(|p| rect.contains(*p) && ui.ctx().layer_id_at(*p) == Some(ui.layer_id()))
        else {
            return;
        };
        let point = self.world(rect, position);
        if pressed {
            if self.trace.pointer_anchor.is_none() {
                self.trace.pivot = Some(self.trace_current_pivot());
                self.trace.pointer_anchor = Some(point);
                self.trace.pointer_just_started = true;
                if self.trace.displace_mode {
                    self.trace.displace_origin.get_or_insert(self.trace.offset);
                } else {
                    self.trace
                        .gesture_origin
                        .get_or_insert((self.trace.offset, self.trace.rotation_deg));
                }
                self.host.document.editor.status = if self.trace.rotate_mode {
                    "Rotate Reference · choose start ray"
                } else {
                    "Trace Reference · move to destination"
                }
                .into();
            } else if self.trace.rotate_mode && self.trace.rotation_arm.is_none() {
                if point.distance(self.trace.pointer_anchor.unwrap()) > 1e-9 {
                    self.trace.rotation_arm = Some(point);
                    self.trace.pointer_just_started = true;
                    self.host.document.editor.status = "Rotate Reference · choose end ray".into();
                }
            }
        }
        if let Some(anchor) = self.trace.pointer_anchor {
            if self.trace.rotate_mode {
                if let Some(arm) = self.trace.rotation_arm {
                    let start = (arm.y - anchor.y).atan2(arm.x - anchor.x);
                    let end = (point.y - anchor.y).atan2(point.x - anchor.x);
                    self.trace_rotation_preview((end - start).to_degrees());
                }
            } else {
                let origin = self
                    .trace
                    .gesture_origin
                    .map(|v| v.0)
                    .or(self.trace.displace_origin)
                    .unwrap_or(self.trace.offset);
                self.trace.offset =
                    Point::new(origin.x + point.x - anchor.x, origin.y + point.y - anchor.y);
            }
            if released {
                if self.trace.pointer_just_started {
                    self.trace.pointer_just_started = false;
                } else if clicked && (!self.trace.rotate_mode || self.trace.rotation_arm.is_some())
                {
                    if self.trace.displace_mode {
                        self.trace.cancel_pointer_modes();
                        self.host.document.editor.status =
                            "Temporary Reference displacement ended".into();
                    } else {
                        self.finish_trace_gesture();
                    }
                }
            }
            ui.ctx().request_repaint();
        }
    }

    fn trace_cursor(&self, ui: &Ui) {
        ui.ctx().set_cursor_icon(if self.trace.rotate_mode {
            CursorIcon::Crosshair
        } else if self.trace.pointer_anchor.is_some() {
            CursorIcon::Grabbing
        } else {
            CursorIcon::Grab
        });
    }

    fn trace_palette(&mut self, ctx: &Context) {
        if !self.trace.palette_open {
            return;
        }
        let mut open = self.trace.palette_open;
        Window::new(self.language.translate("Trace & Reference"))
            .id(Id::new("trace_palette"))
            .open(&mut open)
            .default_width(310.)
            .default_height(720.)
            .min_height(240.)
            .resizable([false, true])
            .vscroll(true)
            .max_height((ctx.content_rect().height() - 80.).max(200.))
            .show(ctx, |ui| self.trace_palette_contents(ui));
        self.trace.palette_open = open;
    }

    fn trace_palette_contents(&mut self, ui: &mut Ui) {
        let before = crate::trace_document::TraceSettings::capture(&self.trace);
        if ui
            .checkbox(
                &mut self.trace.enabled,
                self.language.translate("Show Reference"),
            )
            .changed()
        {
            self.trace_toggled();
        }
        ui.add_enabled_ui(self.trace.enabled, |ui| {
        let mut choice = None;
        let stories = self.host.document.editor.project.stories.clone();
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
                    for (reference, label) in self.trace_recent_choices() {
                        if ui
                            .selectable_label(self.trace.reference == reference, label)
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
                    ui.separator();
                    for (reference, label) in self.trace_worksheet_choices() {
                        if ui
                            .selectable_label(self.trace.reference == reference, label)
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
        if self
            .trace
            .selection(
                &self.host.document.editor.project,
                self.host.document.editor.story,
                self.host.document.editor.previous_story,
            )
            .is_none()
        {
            ui.colored_label(
                Color32::RED,
                self.language
                    .translate("Saved Trace Reference is unavailable"),
            );
        }
        if let TraceReference::Worksheet(id) = self.trace.reference {
            let doc = &self.host.document.editor.project.drawings;
            for u in doc.cad_underlays.iter().filter(|u| u.worksheet == id) {
                if let Some(source) = doc.cad_sources.iter().find(|s| s.id == u.source)
                    && source.data.is_none()
                {
                    ui.colored_label(
                        Color32::RED,
                        self.language
                            .translate("Source unavailable · relink the matching original"),
                    );
                }
            }
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
        if self.trace.reference_color.is_none()
            && let TraceReference::Worksheet(id) = self.trace.reference
            && self
                .host
                .document
                .editor
                .project
                .drawings
                .cad_underlays
                .iter()
                .any(|u| u.worksheet == id)
        {
            ui.label(RichText::new(self.language.translate("Reference display is monochrome; original colors and line patterns are not reproduced")).small().color(MUTED));
        }
        ui.horizontal(|ui| {
            let mut original = self.trace.active_color.is_none();
            if ui
                .checkbox(
                    &mut original,
                    self.language.translate("Active original colors"),
                )
                .changed()
            {
                self.trace.active_color = (!original).then_some([47, 103, 180]);
            }
            if let Some(rgb) = &mut self.trace.active_color {
                let mut color = Color32::from_rgb(rgb[0], rgb[1], rgb[2]);
                if ui.color_edit_button_srgba(&mut color).changed() {
                    *rgb = [color.r(), color.g(), color.b()];
                }
            }
        });
        let numeric_key = ui.make_persistent_id(("trace_numeric_ui",
            self.host.document.editor.document_session(),
            format!("{:?}|{:?}|{:?}|{}|{}", self.trace_view, self.trace.reference, self.trace.source_identity,
                self.host.document.editor.story, self.host.document.editor.previous_story)));
        let mut numeric_ui = ui.data_mut(|d| d.get_temp::<TraceNumericUi>(numeric_key).unwrap_or_default());
        let escape = ui.input(|i| i.raw.events.iter().any(|event| matches!(event, Event::Key { key: Key::Escape, pressed: true, .. })));
        if escape && (numeric_ui.editing || numeric_ui.ids.iter().any(|id| ui.memory(|m| m.has_focus(*id)))) {
            for id in &numeric_ui.ids { ui.memory_mut(|m| m.surrender_focus(*id)); }
            numeric_ui.alternate = !numeric_ui.alternate;
        }
        numeric_ui.ids.clear();
        numeric_ui.editing = false;
        ui.push_id((numeric_key, numeric_ui.alternate), |ui| {
        ui.horizontal(|ui| {
            ui.label(self.language.translate("Rotation:"));
            let response = ui.add(
                    DragValue::new(&mut self.trace.rotation_deg)
                        .update_while_editing(false)
                        .speed(0.5)
                        .fixed_decimals(1)
                        .suffix("°"),
                );
            numeric_ui.ids.push(response.id);
            numeric_ui.editing |= response.has_focus();
            let changed = response.changed();
            if changed && self.trace.pivot.is_none() {
                self.trace.pivot = Some(self.trace_current_pivot());
            }
            if changed
                && self.trace.rotate_mode
                && let Some((_, angle)) = self.trace.gesture_origin
            {
                self.trace_rotation_preview(self.trace.rotation_deg - angle);
            }
            if ui.button(self.language.translate("Reset")).clicked() {
                self.reset_trace_position();
            }
        });
        ui.horizontal(|ui| {
            ui.label(self.language.translate("Offset X:"));
            let response = ui.add(
                DragValue::new(&mut self.trace.offset.x)
                    .update_while_editing(false)
                    .speed(0.01)
                    .suffix(" m"),
            );
            numeric_ui.ids.push(response.id);
            numeric_ui.editing |= response.has_focus();
            ui.label(self.language.translate("Offset Y:"));
            let response = ui.add(
                DragValue::new(&mut self.trace.offset.y)
                    .update_while_editing(false)
                    .speed(0.01)
                    .suffix(" m"),
            );
            numeric_ui.ids.push(response.id);
            numeric_ui.editing |= response.has_focus();
        });
        });
        ui.data_mut(|d| d.insert_temp(numeric_key, numeric_ui));
        if self.trace.pointer_anchor.is_some() {
            ui.horizontal(|ui| {
                if ui.button(self.language.translate("Confirm")).clicked() {
                    if self.trace.displace_mode {
                        self.trace.cancel_pointer_modes();
                    } else {
                        self.finish_trace_gesture();
                    }
                }
                if ui.button(self.language.translate("Cancel")).clicked() {
                    self.trace.cancel_pointer_modes();
                }
            });
        }
        ui.separator();
        let has_reference = self
            .trace
            .selection(
                &self.host.document.editor.project,
                self.host.document.editor.story,
                self.host.document.editor.previous_story,
            )
            .is_some();
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(
                    self.trace_can_navigate(),
                    Button::new(self.language.translate("Activate")),
                )
                .clicked()
            {
                self.activate_trace_reference();
            }
            if ui
                .add_enabled(
                    self.trace_can_navigate(),
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
                self.rebuild_trace_reference();
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
                egui::Grid::new("trace_visible_types")
                    .num_columns(2)
                    .show(ui, |ui| {
                        ui.checkbox(
                            &mut self.trace.visibility.roofs,
                            self.language.translate("Roofs"),
                        );
                        ui.checkbox(
                            &mut self.trace.visibility.beams,
                            self.language.translate("Beams"),
                        );
                        ui.end_row();
                        ui.checkbox(
                            &mut self.trace.visibility.columns,
                            self.language.translate("Columns"),
                        );
                        ui.checkbox(
                            &mut self.trace.visibility.walls,
                            self.language.translate("Walls"),
                        );
                        ui.end_row();
                        ui.checkbox(
                            &mut self.trace.visibility.slabs,
                            self.language.translate("Slabs"),
                        );
                        ui.checkbox(
                            &mut self.trace.visibility.stairs,
                            self.language.translate("Stairs"),
                        );
                        ui.end_row();
                        ui.checkbox(
                            &mut self.trace.visibility.polylines,
                            self.language.translate("Polylines"),
                        );
                        ui.checkbox(
                            &mut self.trace.visibility.drawings,
                            self.language.translate("Drawings"),
                        );
                        ui.end_row();
                        ui.checkbox(
                            &mut self.trace.visibility.masses,
                            self.language.translate("Concept masses"),
                        );
                        ui.checkbox(
                            &mut self.trace.visibility.zones,
                            self.language.translate("Zones"),
                        );
                        ui.end_row();
                    });
            });
        if ui
            .button(self.language.translate("Apply Settings to All References"))
            .clicked()
        {
            self.persist_trace_state(false);
            self.host
                .apply_trace_visibility_to_all(self.trace.visibility);
        }
        });
        if ui
            .button(self.language.translate("Turn off Trace everywhere"))
            .clicked()
        {
            self.turn_off_trace_everywhere();
        }
        ui.label(
            RichText::new(self.language.translate(
                "Reference elements are visible but cannot be edited until Switch with Active.",
            ))
            .small()
            .color(MUTED),
        );
        if before != crate::trace_document::TraceSettings::capture(&self.trace) {
            self.persist_trace_state(false);
        }
    }

    fn trace_browse_dialog(&mut self, ctx: &Context) {
        if !self.trace.browse_open {
            return;
        }
        let mut open = self.trace.browse_open;
        let mut choice = self.trace.browse_choice;
        let stories = self.host.document.editor.project.stories.clone();
        let mut accept = false;
        let mut cancel = false;
        Window::new(self.language.translate("Choose Trace Reference"))
            .open(&mut open)
            .default_width(390.)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label(
                    self.language
                        .translate("Choose a model story or imported worksheet."),
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
                for (reference, label) in self.trace_worksheet_choices() {
                    ui.selectable_value(&mut choice, reference, label);
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

    fn tint_trace_active(&self, painter: &Painter, start: egui::layers::ShapeIdx) {
        let Some(rgb) = self.trace.active_color.filter(|_| self.trace.enabled) else {
            return;
        };
        painter.ctx().graphics_mut(|graphics| {
            let list = graphics.entry(painter.layer_id());
            let end = list.next_idx();
            for index in start.0 + 1..end.0 {
                list.mutate_shape(egui::layers::ShapeIdx(index), |entry| {
                    egui::epaint::shape_transform::adjust_colors(&mut entry.shape, move |color| {
                        if *color != Color32::PLACEHOLDER && color.a() != 0 {
                            *color =
                                Color32::from_rgba_unmultiplied(rgb[0], rgb[1], rgb[2], color.a());
                        }
                    });
                });
            }
        });
    }

    fn trace_geometry_batches(
        &self,
        selection: TraceSelection,
        elements: &[&Element],
    ) -> Vec<(u32, crate::derivation_cache::GeometryBatch)> {
        let requests = selection
            .story_ids(&self.host.document.editor.project)
            .into_iter()
            .map(|story| {
                let ids = elements
                    .iter()
                    .filter(|element| {
                        geometry::element_on_plan_story(
                            &self.host.document.editor.project,
                            element,
                            story,
                        )
                    })
                    .map(|e| e.id)
                    .collect::<Vec<_>>();
                (story, ids)
            })
            .collect::<Vec<_>>();
        self.host
            .document
            .editor
            .with_geometry_read(|read| {
                requests
                    .iter()
                    .map(|(story, ids)| (*story, read.batch(ids, Some(*story))))
                    .collect()
            })
            .unwrap_or_else(|error| {
                requests
                    .iter()
                    .map(|(story, ids)| {
                        (
                            *story,
                            crate::derivation_cache::GeometryBatch::failed(
                                ids,
                                self.host.document.editor.revision,
                                self.host.document.editor.document_session(),
                                error.clone(),
                            ),
                        )
                    })
                    .collect()
            })
    }
    fn trace_batch_pivot(batches: &[(u32, crate::derivation_cache::GeometryBatch)]) -> Point {
        let mut min = Point::new(f64::INFINITY, f64::INFINITY);
        let mut max = Point::new(f64::NEG_INFINITY, f64::NEG_INFINITY);
        for (_, batch) in batches {
            for id in batch.ids() {
                if let Ok(outline) = batch.plan_outline(id) {
                    for point in outline {
                        min.x = min.x.min(point.x);
                        min.y = min.y.min(point.y);
                        max.x = max.x.max(point.x);
                        max.y = max.y.max(point.y);
                    }
                }
            }
        }
        if min.finite() && max.finite() {
            Point::new((min.x + max.x) * 0.5, (min.y + max.y) * 0.5)
        } else {
            Point::default()
        }
    }
    #[cfg(test)]
    fn trace_reference_pivot(&self, selection: TraceSelection, elements: &[&Element]) -> Point {
        if let TraceSelection::Worksheet(id) = selection {
            return self.trace_worksheet_pivot(id);
        }
        Self::trace_batch_pivot(&self.trace_geometry_batches(selection, elements))
    }

    fn paint_trace_reference(&mut self, painter: &Painter, rect: Rect, clip: Option<Rect>) {
        if !self.trace.enabled || self.trace.reference_opacity == 0 {
            return;
        }
        if let TraceReference::Worksheet(id) = self.trace.reference {
            if self
                .host
                .document
                .editor
                .project
                .plan_options
                .active_worksheet
                == Some(id)
                && self
                    .host
                    .document
                    .editor
                    .project
                    .drawings
                    .worksheets
                    .iter()
                    .any(|w| w.id == id)
            {
                return;
            }
            let painter = clip.map_or_else(|| painter.clone(), |clip| painter.with_clip_rect(clip));
            if self.trace.visibility.drawings
                && self
                    .host
                    .document
                    .editor
                    .project
                    .drawings
                    .pages
                    .iter()
                    .any(|p| p.id == id && p.placement.is_none())
            {
                self.paint_drawing_source(&painter, rect, id, true);
            }
            let sources: Vec<_> = self
                .host
                .document
                .editor
                .project
                .drawings
                .pages
                .iter()
                .filter(|p| {
                    self.trace.visibility.drawings
                        && p.placement
                            .as_ref()
                            .is_some_and(|v| v.worksheet == Some(id) && v.visible)
                })
                .map(|p| p.id)
                .collect();
            for source in sources {
                self.paint_drawing_source(&painter, rect, source, true);
            }
            if self.trace.visibility.drawings {
                self.paint_cad_underlays_for(&painter, rect, Some(id));
            }
            let pivot = self.trace_worksheet_pivot(id);
            for element in self
                .host
                .document
                .editor
                .project
                .elements
                .iter()
                .filter(|e| e.worksheet == Some(id) && self.trace.visibility.shows(e))
            {
                let points =
                    geometry::element_plan_outline(&self.host.document.editor.project, element)
                        .iter()
                        .map(|p| self.screen(rect, self.trace.transform(*p, pivot)))
                        .collect::<Vec<_>>();
                let rgb = self.trace.color_for(element.material);
                painter.add(egui::Shape::line(
                    points,
                    Stroke::new(
                        1.,
                        Color32::from_rgba_unmultiplied(
                            rgb[0],
                            rgb[1],
                            rgb[2],
                            self.trace.reference_opacity,
                        ),
                    ),
                ));
            }
            return;
        }
        let Some(selection) = self.trace.selection(
            &self.host.document.editor.project,
            self.host.document.editor.story,
            self.host.document.editor.previous_story,
        ) else {
            return;
        };
        let elements: Vec<_> = self
            .host
            .document
            .editor
            .project
            .elements
            .iter()
            .filter(|element| self.trace.visibility.shows(element))
            .filter(|element| element.worksheet.is_none())
            .collect();
        if elements.is_empty() {
            return;
        }
        let geometry_batches = self.trace_geometry_batches(selection, &elements);
        let pivot = Self::trace_batch_pivot(&geometry_batches);
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
        for (story, geometry_batch) in &geometry_batches {
            let story = *story;
            let wall_queries = geometry_batch;
            let story_elements: Vec<_> = elements
                .iter()
                .copied()
                .filter(|element| {
                    geometry::element_on_plan_story(
                        &self.host.document.editor.project,
                        element,
                        story,
                    )
                })
                .collect();
            for element in story_elements
                .iter()
                .copied()
                .filter(|element| !matches!(element.shape, Shape::Wall { .. }))
            {
                let source = match geometry_batch.source(element.id) {
                    Ok(handle) => handle.source.as_ref(),
                    Err(d) => {
                        Self::paint_geometry_error(&painter, self.language, &d);
                        continue;
                    }
                };
                match &element.shape {
                    Shape::Polyline { .. } => {
                        let crate::derivation_cache::GeometrySource::Polyline(g) = source else {
                            continue;
                        };
                        let path: Vec<Pos2> =
                            g.geometry.plan_path.iter().copied().map(screen).collect();
                        if path.len() >= 2 {
                            painter.add(egui::Shape::line(
                                path,
                                Stroke::new(1.15, reference_color(element, stroke_alpha)),
                            ));
                        }
                    }
                    Shape::ConceptMass { .. }
                    | Shape::Zone { .. }
                    | Shape::Roof { .. }
                    | Shape::Beam { .. }
                    | Shape::Column { .. }
                    | Shape::Slab { .. }
                    | Shape::Stair { .. } => {
                        let fill = if self.trace.transparent_fills {
                            Color32::TRANSPARENT
                        } else {
                            reference_color(element, fill_alpha)
                        };
                        let stroke = Stroke::new(1.05, reference_color(element, stroke_alpha));
                        if matches!(element.shape, Shape::Column { .. }) {
                            match geometry_batch.column_plan(element.id) {
                                Ok(plan) => match &plan.view {
                                    crate::column_geometry::ColumnPlanView::Projected {
                                        geometry,
                                        ..
                                    } => Self::paint_column_plan_geometry_mapped(
                                        &painter, geometry, screen, fill, stroke,
                                    ),
                                    crate::column_geometry::ColumnPlanView::SymbolicFootprint(
                                        outline,
                                    ) => Self::paint_wall_polygon(
                                        &painter, outline, screen, fill, stroke,
                                    ),
                                    crate::column_geometry::ColumnPlanView::NotOnStory => {}
                                },
                                Err(d) => Self::paint_geometry_error(&painter, self.language, &d),
                            }
                        } else if matches!(element.shape, Shape::Roof { .. }) {
                            if let crate::derivation_cache::GeometrySource::Roof(g) = source {
                                Self::paint_roof_snapshot(
                                    &painter,
                                    &g.geometry,
                                    screen,
                                    fill,
                                    stroke,
                                );
                            }
                        } else if matches!(element.shape, Shape::Beam { .. }) {
                            if let crate::derivation_cache::GeometrySource::Beam(g) = source {
                                Self::paint_beam_snapshot(
                                    &painter,
                                    &g.geometry,
                                    screen,
                                    fill,
                                    stroke,
                                );
                            }
                        } else if matches!(
                            element.shape,
                            Shape::ConceptMass { .. } | Shape::Zone { .. }
                        ) {
                            if let crate::derivation_cache::GeometrySource::ConceptMass(g)
                            | crate::derivation_cache::GeometrySource::Zone(g) = source
                            {
                                Self::paint_slab_snapshot(&painter, g, screen, fill, stroke);
                            }
                        } else if matches!(element.shape, Shape::Slab { .. }) {
                            if let crate::derivation_cache::GeometrySource::Slab(g) = source {
                                Self::paint_slab_snapshot(&painter, g, screen, fill, stroke);
                            }
                        } else if matches!(element.shape, Shape::Stair { .. }) {
                            match source {
                                crate::derivation_cache::GeometrySource::Stair(g) => {
                                    Self::paint_stair_snapshot(
                                        &painter,
                                        &g.geometry,
                                        screen,
                                        fill,
                                        stroke,
                                        Some(Stroke::new(
                                            0.75,
                                            reference_color(
                                                element,
                                                stroke_alpha.saturating_sub(30),
                                            ),
                                        )),
                                    );
                                }
                                _ => {}
                            }
                        } else {
                            let outline = geometry::element_plan_outline(
                                &self.host.document.editor.project,
                                element,
                            );
                            Self::paint_wall_polygon(&painter, &outline, screen, fill, stroke);
                        }
                    }
                    Shape::Wall { .. } | Shape::Opening { .. } => {}
                }
            }
            for wall in story_elements
                .iter()
                .copied()
                .filter(|e| matches!(e.shape, Shape::Wall { .. }) && e.wall_contour.is_some())
            {
                match wall_queries.wall(wall.id) {
                    Ok(g) => {
                        let fill = if self.trace.transparent_fills {
                            Color32::TRANSPARENT
                        } else {
                            reference_color(wall, fill_alpha)
                        };
                        Self::paint_wall_snapshot(
                            &painter,
                            &g.geometry,
                            screen,
                            fill,
                            Stroke::new(1.1, reference_color(wall, stroke_alpha)),
                        );
                    }
                    Err(d) => Self::paint_geometry_error(&painter, self.language, &d),
                }
            }
            let walls: Vec<_> = story_elements
                .iter()
                .copied()
                .filter(|element| {
                    matches!(element.shape, Shape::Wall { .. }) && element.wall_contour.is_none()
                })
                .collect();
            // Trace fills follow material colors, but wall junctions are
            // physical geometry shared by every material on the story.
            let footprints_by_id = wall_queries
                .context(story)
                .map(|c| c.joined_footprints.clone())
                .unwrap_or_default();
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
                        match wall_queries.wall(wall.id) {
                            Ok(g) => {
                                Self::paint_wall_snapshot(
                                    &painter,
                                    &g.geometry,
                                    screen,
                                    reference_color(wall, fill_alpha),
                                    Stroke::new(0.75, reference_color(wall, fill_alpha)),
                                );
                            }
                            Err(d) => Self::paint_geometry_error(&painter, self.language, &d),
                        }
                    }
                }
            }
            let story_footprints: Vec<_> = walls
                .iter()
                .map(|wall| footprints_by_id.get(&wall.id).cloned().unwrap_or_default())
                .collect();
            // One exposed perimeter for the whole story keeps the outline
            // continuous across material boundaries.
            let outline_color = Color32::from_rgba_unmultiplied(36, 40, 42, stroke_alpha);
            for mut path in geometry::wall_outline_paths_from_segments(
                geometry::wall_polygon_outline_segments(&story_footprints),
            ) {
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
