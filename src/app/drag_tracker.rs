use super::*;
use crate::precision_input::{LengthUnit, NumericDraft, Quantity};
use crate::workbench::shortcuts::{self, Binding};

const FIELD_COMMANDS: [&str; 4] = [
    "input.distance",
    "input.angle",
    "input.coordinate.x",
    "input.coordinate.y",
];
const FIELD_LABELS: [&str; 4] = ["Distance", "Angle", "X coordinate", "Y coordinate"];

/// One adapter over the shared numeric protocol. Values are display-unit locks;
/// the authoritative model is only changed by the existing Transform command.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct DragTracker {
    values: [Option<f64>; 4],
    active: Option<usize>,
    draft: Option<NumericDraft>,
    pending_field: Option<usize>,
    ime_composing: bool,
    user_coordinates: bool,
    user_origin_set: bool,
    expanded: bool,
    reference: Option<Point>,
    last_offset: Option<Point>,
    panel: Option<Rect>,
    target_invalid: bool,
    coordinate_live: [f64; 2],
}
impl DragTracker {
    pub(super) fn use_user_coordinates(&mut self, enabled: bool) {
        self.user_coordinates = enabled;
    }
    pub(super) fn ime_owns_input(&mut self, events: &[Event], focused: bool) -> bool {
        if !focused {
            self.ime_composing = false;
            return false;
        }
        let mut blocked = self.ime_composing;
        for event in events {
            if let Event::Ime(event) = event {
                blocked = true;
                match event {
                    egui::ImeEvent::Preedit { text, .. } => self.ime_composing = !text.is_empty(),
                    egui::ImeEvent::Commit(_) => self.ime_composing = false,
                    _ => {}
                }
            }
        }
        blocked
    }
    pub(super) fn cancel_input(&mut self) -> bool {
        let active = self.has_input();
        if active {
            self.values = [None; 4];
            self.active = None;
            self.draft = None;
            self.pending_field = None;
            self.target_invalid = false;
        }
        active
    }
    pub(super) fn has_input(&self) -> bool {
        self.active.is_some() || self.values.iter().any(Option::is_some)
    }
    pub(super) fn request_field(&mut self, index: usize) {
        self.pending_field = Some(index);
    }
    pub(super) fn owns_pointer(&self, pointer: Pos2) -> bool {
        self.panel.is_some_and(|rect| rect.contains(pointer))
    }
    pub(super) fn pointer_offset(&self, pointer: Pos2) -> Option<Point> {
        self.owns_pointer(pointer)
            .then_some(self.last_offset)
            .flatten()
    }
    pub(super) fn unit(style: crate::view_settings::DimensionStyle) -> (f64, &'static str) {
        use crate::view_settings::DimensionStyle::*;
        match style {
            Din | PlainMillimeter => (1000., "mm"),
            UsArchitect | UsBuilder => (1. / 0.3048, "ft"),
            UsDetailing => (1. / 0.0254, "in"),
            PlainMeter => (1., "m"),
        }
    }
    fn live(offset: Point, scale: f64, edit: Point, reference: Point) -> [f64; 4] {
        [
            offset.distance(Point::default()) * scale,
            offset.y.atan2(offset.x).to_degrees().rem_euclid(360.),
            (offset.x + edit.x - reference.x) * scale,
            (offset.y + edit.y - reference.y) * scale,
        ]
    }
    fn quantity(index: usize, unit: &str) -> Quantity {
        if index == 1 {
            Quantity::Angle
        } else {
            Quantity::Length(LengthUnit::from_symbol(unit).unwrap_or(LengthUnit::Meter))
        }
    }
    fn resolved(&self, raw: Point, scale: f64, edit: Point, reference: Point) -> Point {
        if self.values.iter().all(Option::is_none) && self.active.is_none() {
            return raw;
        }
        let live = Self::live(raw, scale, edit, reference);
        let mut fields = std::array::from_fn::<_, 4, _>(|i| self.values[i].unwrap_or(live[i]));
        if let (Some(index), Some(draft)) = (self.active, &self.draft) {
            fields[index] = draft.preview();
        }
        if self.active.is_some_and(|i| i >= 2) || self.values[2..].iter().any(Option::is_some) {
            Point::new(
                fields[2] / scale + reference.x - edit.x,
                fields[3] / scale + reference.y - edit.y,
            )
        } else {
            let a = fields[1].rem_euclid(360.).to_radians();
            Point::new(fields[0] / scale * a.cos(), fields[0] / scale * a.sin())
        }
    }
    fn accept(&mut self, current: Point, scale: f64, edit: Point, reference: Point) -> bool {
        if let Some(index) = self.active {
            let Some(Ok(value)) = self.draft.as_ref().map(NumericDraft::confirm) else {
                return false;
            };
            self.values[index] = Some(value);
            if index == 0 && value < 0. {
                // Canonical locks preserve the world vector when switching to Angle.
                let live = Self::live(current, scale, edit, reference);
                self.values[0] = Some(live[0]);
                self.values[1] = Some(live[1]);
                if let Some(quantity) = self.draft.as_ref().map(NumericDraft::quantity) {
                    self.draft = NumericDraft::new(live[0], quantity).ok();
                }
            }
        }
        true
    }
    fn activate(
        &mut self,
        index: usize,
        raw: Point,
        scale: f64,
        unit: &str,
        edit: Point,
        reference: Point,
    ) -> bool {
        let current = self.resolved(raw, scale, edit, reference);
        if !self.accept(current, scale, edit, reference) {
            return false;
        }
        let live = Self::live(current, scale, edit, reference);
        let old_xy =
            self.active.is_some_and(|i| i >= 2) || self.values[2..].iter().any(Option::is_some);
        if (index >= 2) != old_xy
            && (self.active.is_some() || self.values.iter().any(Option::is_some))
        {
            self.values = if index >= 2 {
                [None, None, Some(live[2]), Some(live[3])]
            } else {
                [Some(live[0]), Some(live[1]), None, None]
            };
        }
        self.active = Some(index);
        self.expanded |= index >= 2;
        self.draft = NumericDraft::new(
            self.values[index].unwrap_or(live[index]),
            Self::quantity(index, unit),
        )
        .ok();
        true
    }
    pub(super) fn update(
        &mut self,
        events: &[Event],
        raw: Point,
        scale: f64,
        unit: &str,
        edit: Point,
        user: Option<Point>,
        allow_xy: bool,
        overrides: &[Binding],
    ) -> (Point, bool, bool) {
        let reference = if self.user_coordinates {
            user.unwrap_or_default()
        } else {
            edit
        };
        self.user_origin_set = user.is_some();
        if let Some(old) = self.reference.filter(|old| *old != reference) {
            let delta = [(old.x - reference.x) * scale, (old.y - reference.y) * scale];
            for i in 2..4 {
                if let Some(value) = &mut self.values[i] {
                    *value += delta[i - 2];
                }
            }
            if let (Some(i), Some(draft)) = (self.active.filter(|i| *i >= 2), self.draft.as_ref()) {
                self.draft =
                    NumericDraft::new(draft.preview() + delta[i - 2], Self::quantity(i, unit)).ok();
            }
        }
        self.reference = Some(reference);
        let count = if allow_xy { 4 } else { 2 };
        if let Some(index) = self.pending_field.take().filter(|i| *i < count) {
            self.activate(index, raw, scale, unit, edit, reference);
        }
        let mut confirm = false;
        let mut suppressed = Vec::<String>::new();
        for event in events {
            match event {
                Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    repeat,
                    ..
                } => {
                    if let Some(index) = (0..count).find(|i| {
                        shortcuts::keys(overrides, FIELD_COMMANDS[*i])
                            .iter()
                            .any(|chord| chord.matches(*key, *modifiers))
                    }) {
                        if !repeat {
                            self.activate(index, raw, scale, unit, edit, reference);
                        }
                        if key.name().len() == 1 {
                            suppressed.push(key.name().to_ascii_lowercase());
                        }
                    } else if modifiers.command && *key == Key::A {
                        if let Some(draft) = &mut self.draft {
                            draft.select_all();
                        }
                    } else if !modifiers.command && !modifiers.ctrl && !modifiers.alt {
                        match key {
                            Key::Tab => {
                                let index = self.active.map_or(0, |i| {
                                    if modifiers.shift {
                                        (i + count - 1) % count
                                    } else {
                                        (i + 1) % count
                                    }
                                });
                                self.activate(index, raw, scale, unit, edit, reference);
                            }
                            Key::Enter => {
                                let current = self.resolved(raw, scale, edit, reference);
                                confirm = self.accept(current, scale, edit, reference);
                            }
                            Key::Backspace => {
                                if let Some(draft) = &mut self.draft {
                                    draft.backspace(f64::is_finite);
                                }
                            }
                            Key::Delete => {
                                if let Some(draft) = &mut self.draft {
                                    draft.clear();
                                }
                            }
                            _ => {}
                        }
                    }
                }
                Event::Text(text) | Event::Paste(text) => {
                    if let Some(i) = suppressed.iter().position(|s| text.eq_ignore_ascii_case(s)) {
                        suppressed.remove(i);
                        continue;
                    }
                    if self.active.is_none()
                        && text
                            .chars()
                            .next()
                            .is_some_and(|c| c.is_ascii_digit() || matches!(c, '-' | '+' | '.'))
                    {
                        self.activate(0, raw, scale, unit, edit, reference);
                    }
                    if let Some(draft) = &mut self.draft {
                        draft.input(text, f64::is_finite);
                    }
                }
                _ => {}
            }
        }
        let mut offset = self.resolved(raw, scale, edit, reference);
        self.target_invalid = !crate::precision::GeometryContext::point_in_range(Point::new(
            edit.x + offset.x,
            edit.y + offset.y,
        ));
        let valid =
            !self.target_invalid && self.draft.as_ref().is_none_or(NumericDraft::is_confirmable);
        if valid {
            self.last_offset = Some(offset);
        } else if let Some(previous) = self.last_offset {
            offset = previous;
        }
        let live = Self::live(offset, scale, edit, reference);
        self.coordinate_live = [live[2], live[3]];
        (offset, confirm && valid, valid)
    }
    pub(super) fn paint(
        &mut self,
        ui: &mut Ui,
        canvas: Rect,
        pointer: Pos2,
        offset: Point,
        scale: f64,
        unit: &str,
        language: UiLanguage,
        mode: u8,
        allow_xy: bool,
        overrides: &[Binding],
    ) {
        let error = self.draft.as_ref().and_then(NumericDraft::error);
        let mut size = vec2(
            266.,
            if self.expanded && allow_xy { 148. } else { 82. }
                + if error.is_some() || self.target_invalid {
                    24.
                } else {
                    0.
                },
        );
        // A wrapped full-precision value can add rows. Reuse the measured
        // extent for positioning, then measure every visible control below.
        if let Some(old) = self.panel {
            size.x = size.x.max(old.width());
            size.y = size.y.max(old.height());
        }
        let follows = Rect::from_min_size(
            pos2(
                (pointer.x + 22.)
                    .min(canvas.right() - size.x)
                    .max(canvas.left()),
                (pointer.y + 24.)
                    .min(canvas.bottom() - size.y)
                    .max(canvas.top()),
            ),
            size,
        );
        let rect = if let Some(old) = self
            .panel
            .filter(|old| self.active.is_some() || old.expand(10.).contains(pointer))
        {
            Rect::from_min_size(
                pos2(
                    old.left().min(canvas.right() - size.x).max(canvas.left()),
                    old.top().min(canvas.bottom() - size.y).max(canvas.top()),
                ),
                size,
            )
        } else {
            follows
        };
        let painter = ui.painter_at(canvas);
        let background = painter.add(egui::Shape::Noop);
        // Coordinate readouts were last resolved with the edit/reference context.
        let live_polar = [
            offset.distance(Point::default()) * scale,
            offset.y.atan2(offset.x).to_degrees().rem_euclid(360.),
        ];
        let contents = ui.scope_builder(UiBuilder::new().max_rect(rect.shrink(7.)), |ui| {
            let count = if self.expanded && allow_xy { 4 } else { 2 };
            for index in 0..count {
                let value = if self.active == Some(index) {
                    self.draft
                        .as_ref()
                        .map(|d| d.text().to_owned())
                        .unwrap_or_default()
                } else if index < 2 {
                    format!("{:.2}", live_polar[index])
                } else {
                    format!("{:.3}", self.coordinate_live[index - 2])
                };
                let hint = shortcuts::keys(overrides, FIELD_COMMANDS[index])
                    .first()
                    .map(|key| key.hint())
                    .unwrap_or_default();
                let text = format!(
                    "{}{} ({})  {}{}",
                    language.translate(FIELD_LABELS[index]),
                    if hint.is_empty() {
                        String::new()
                    } else {
                        format!(" [{hint}]")
                    },
                    if index == 1 { "°" } else { unit },
                    value,
                    if self.active == Some(index) {
                        " ▏"
                    } else {
                        ""
                    }
                );
                let active = self.active == Some(index);
                let response = ui.selectable_label(
                    active,
                    RichText::new(text).monospace().size(13.).color(if active {
                        Color32::from_rgb(255, 220, 130)
                    } else {
                        Color32::WHITE
                    }),
                );
                if response.clicked() {
                    self.request_field(index);
                    ui.ctx().request_repaint();
                }
            }
            if allow_xy {
                ui.horizontal(|ui| {
                    if ui
                        .small_button(if self.expanded { "− XY" } else { "+ XY" })
                        .clicked()
                    {
                        self.expanded = !self.expanded;
                        ui.ctx().request_repaint();
                    }
                    if self.expanded
                        && ui
                            .small_button(language.translate(if self.user_coordinates {
                                if self.user_origin_set {
                                    "User-origin coordinates"
                                } else {
                                    "Project-origin coordinates"
                                }
                            } else {
                                "Edit-origin coordinates"
                            }))
                            .clicked()
                    {
                        self.use_user_coordinates(!self.user_coordinates);
                        ui.ctx().request_repaint();
                    }
                });
            }
            if let Some(error) = error {
                ui.label(
                    RichText::new(language.translate(error.label()))
                        .color(Color32::from_rgb(255, 170, 160)),
                );
            } else if self.target_invalid {
                ui.label(
                    RichText::new(language.translate("Invalid edit parameters"))
                        .color(Color32::from_rgb(255, 170, 160)),
                );
            }
        });
        let actual = Rect::from_min_max(
            rect.min,
            pos2(
                rect.right().max(contents.response.rect.right() + 7.),
                contents.response.rect.bottom() + 7.,
            ),
        );
        painter.set(
            background,
            egui::Shape::rect_filled(actual, 3., Color32::from_rgb(76, 79, 82)),
        );
        if self.panel != Some(actual) {
            ui.ctx().request_repaint();
        }
        self.panel = Some(actual);
        if mode != 0 {
            ui.painter_at(canvas).text(
                pointer + vec2(12., -10.),
                Align2::LEFT_BOTTOM,
                if mode == 1 { "+" } else { "++" },
                FontId::monospace(16.),
                ACCENT,
            );
        }
    }
}

#[cfg(test)]
mod precision_drag_protocol_tests {
    use super::*;
    fn key(key: Key) -> Event {
        Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        }
    }
    fn apply(t: &mut DragTracker, events: &[Event], raw: Point) -> (Point, bool, bool) {
        t.update(
            events,
            raw,
            1000.,
            "mm",
            Point::new(10., 20.),
            Some(Point::new(100., 200.)),
            true,
            &[],
        )
    }
    fn close(a: Point, b: Point) {
        assert!(a.distance(b) < 1e-10, "{a:?} != {b:?}");
    }
    #[test]
    fn precision_drag_postfix_reverse_and_invalid_preview_share_one_world_vector() {
        let mut t = DragTracker::default();
        let raw = Point::new(3., 4.);
        let (p, _, valid) = apply(&mut t, &[key(Key::R), Event::Text("2*3+".into())], raw);
        assert!(valid);
        close(p, Point::new(6.0018, 8.0024));
        apply(
            &mut t,
            &[Event::Key {
                key: Key::A,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::COMMAND,
            }],
            raw,
        );
        let (p, confirmed, valid) =
            apply(&mut t, &[Event::Text("0/".into()), key(Key::Enter)], raw);
        assert!(!confirmed && !valid);
        close(p, Point::new(6.0018, 8.0024));
        assert!(t.cancel_input());
        let (p, confirmed, valid) = apply(
            &mut t,
            &[key(Key::R), Event::Text("-2m".into()), key(Key::Enter)],
            raw,
        );
        assert!(confirmed && valid);
        close(p, Point::new(-1.2, -1.6));
        let (p, _, _) = apply(&mut t, &[key(Key::Tab)], raw);
        close(p, Point::new(-1.2, -1.6));
    }
    #[test]
    fn precision_drag_coordinate_reference_changes_preserve_world_preview_and_locks() {
        let mut t = DragTracker::default();
        let raw = Point::new(3., 4.);
        let (p, _, valid) = apply(
            &mut t,
            &[
                key(Key::X),
                Event::Text("-2m".into()),
                key(Key::Tab),
                Event::Text("7m".into()),
            ],
            raw,
        );
        assert!(valid);
        close(p, Point::new(-2., 7.));
        t.use_user_coordinates(true);
        let (p, _, valid) = apply(&mut t, &[], raw);
        assert!(valid);
        close(p, Point::new(-2., 7.));
        assert_eq!(t.coordinate_live, [-92000., -173000.]);
        let (p, confirmed, valid) = t.update(
            &[key(Key::Enter)],
            raw,
            1000.,
            "mm",
            Point::new(10., 20.),
            Some(Point::new(200., 300.)),
            true,
            &[],
        );
        assert!(confirmed && valid);
        close(p, Point::new(-2., 7.));
        assert_eq!(t.coordinate_live, [-192000., -273000.]);
    }
    #[test]
    fn precision_drag_switching_coordinate_families_does_not_jump() {
        let mut t = DragTracker::default();
        let raw = Point::new(3., 4.);
        let (expected, _, _) = apply(
            &mut t,
            &[
                key(Key::D),
                Event::Text("5m".into()),
                key(Key::A),
                Event::Text("90".into()),
            ],
            raw,
        );
        close(expected, Point::new(0., 5.));
        for field in [Key::X, Key::Y, Key::A, Key::D] {
            let (p, _, valid) = apply(&mut t, &[key(field)], raw);
            assert!(valid);
            close(p, expected);
        }
    }
    #[test]
    fn precision_drag_field_commands_follow_profile_bindings_and_unassignment() {
        let mut t = DragTracker::default();
        let raw = Point::new(3., 4.);
        let profile = vec![
            Binding {
                command_id: "input.distance".into(),
                keys: vec![shortcuts::Chord::new("K", false, false, false)],
            },
            Binding {
                command_id: "input.coordinate.x".into(),
                keys: vec![],
            },
        ];
        let update = |t: &mut DragTracker, events: &[Event]| {
            t.update(events, raw, 1., "m", Point::default(), None, true, &profile)
        };
        update(
            &mut t,
            &[
                key(Key::D),
                Event::Text("d".into()),
                key(Key::X),
                Event::Text("x".into()),
            ],
        );
        assert!(t.active.is_none());
        let (p, confirmed, valid) = update(
            &mut t,
            &[
                key(Key::K),
                Event::Text("k".into()),
                Event::Text("2*".into()),
                key(Key::Enter),
            ],
        );
        assert!(valid && confirmed);
        close(p, Point::new(6., 8.));
    }
}

#[cfg(test)]
mod precision_drag_layout_tests {
    use super::*;
    fn basis(shape: &egui::Shape) -> Option<Rect> {
        match shape {
            egui::Shape::Text(text) if text.galley.text() == "Edit-origin coordinates" => {
                Some(Rect::from_min_size(text.pos, text.galley.size()))
            }
            egui::Shape::Vec(shapes) => shapes.iter().find_map(basis),
            _ => None,
        }
    }
    #[test]
    fn precision_drag_wrapped_coordinate_footer_is_inside_hit_region_and_canvas() {
        let ctx = Context::default();
        AtelierApp::style(&ctx);
        let canvas = Rect::from_min_size(Pos2::ZERO, vec2(800., 600.));
        let mut tracker = DragTracker::default();
        tracker.request_field(2);
        let (offset, _, _) = tracker.update(
            &[],
            Point::new(-1.3029325003662962, -0.01599023),
            1.,
            "m",
            Point::default(),
            None,
            true,
            &[],
        );
        for frame in 0..3 {
            let mut output = ctx.run_ui(
                RawInput {
                    screen_rect: Some(canvas),
                    ..Default::default()
                },
                |ui| {
                    ui.spacing_mut().item_spacing.y = 8.;
                    tracker.paint(
                        ui,
                        canvas,
                        pos2(740., 580.),
                        offset,
                        1.,
                        "m",
                        UiLanguage::English,
                        0,
                        true,
                        &[],
                    );
                },
            );
            output.textures_delta.clear();
            let footer = output.shapes.iter().find_map(|shape| basis(&shape.shape));
            if frame < 2 {
                continue;
            }
            let footer = footer.expect("basis footer visible after measured layout settles");
            assert!(
                tracker.owns_pointer(footer.center()),
                "footer must not commit a canvas click"
            );
            assert!(
                tracker.panel.unwrap().contains_rect(footer),
                "background must enclose wrapped controls"
            );
        }
        assert!(
            canvas.contains_rect(tracker.panel.unwrap()),
            "measured panel is repositioned inside the canvas"
        );
    }
}
