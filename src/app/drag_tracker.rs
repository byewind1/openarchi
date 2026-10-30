use super::*;

#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct DragTracker {
    values: [Option<f32>; 2],
    active: Option<usize>,
    text: String,
    replace: bool,
    invalid: bool,
}
impl DragTracker {
    pub(super) fn cancel_input(&mut self) -> bool {
        let active = self.active.is_some() || self.values.iter().any(Option::is_some);
        if active {
            *self = Self::default();
        }
        active
    }
    fn value(&self) -> Option<f32> {
        self.text
            .trim()
            .parse::<f32>()
            .ok()
            .filter(|v| v.is_finite() && (self.active != Some(0) || *v >= 0.))
    }
    fn accept(&mut self) -> bool {
        if let Some(index) = self.active {
            if let Some(value) = self.value() {
                self.values[index] = Some(value);
            } else {
                self.invalid = true;
                return false;
            }
        }
        self.invalid = false;
        true
    }
    fn activate(&mut self, index: usize, live: [f32; 2]) {
        self.active = Some(index);
        self.text = format!("{:.3}", self.values[index].unwrap_or(live[index]));
        self.replace = true;
        self.invalid = false;
    }
    pub(super) fn unit(style: crate::view_settings::DimensionStyle) -> (f32, &'static str) {
        use crate::view_settings::DimensionStyle::*;
        match style {
            Din | PlainMillimeter => (1000., "mm"),
            UsArchitect | UsBuilder => (1. / 0.3048, "ft"),
            UsDetailing => (1. / 0.0254, "in"),
            PlainMeter => (1., "m"),
        }
    }
    pub(super) fn update(
        &mut self,
        events: &[Event],
        raw: Point,
        scale: f32,
    ) -> (Point, bool, bool) {
        let live = [
            raw.distance(Point::default()) * scale,
            raw.y.atan2(raw.x).to_degrees().rem_euclid(360.),
        ];
        let mut confirm = false;
        for event in events {
            match event {
                Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } if !modifiers.command && !modifiers.ctrl && !modifiers.alt => {
                    let field = match key {
                        Key::D | Key::R => Some(0),
                        Key::A => Some(1),
                        Key::Tab => Some(self.active.map_or(0, |i| 1 - i)),
                        _ => None,
                    };
                    if let Some(field) = field {
                        if self.accept() {
                            self.activate(field, live);
                        }
                    } else if *key == Key::Enter {
                        confirm = self.accept();
                    } else if matches!(key, Key::Backspace | Key::Delete) && self.active.is_some() {
                        if self.replace || *key == Key::Delete {
                            self.text.clear();
                        } else {
                            self.text.pop();
                        }
                        self.replace = false;
                    }
                }
                Event::Key {
                    key: Key::A,
                    pressed: true,
                    modifiers,
                    ..
                } if modifiers.command => {
                    self.replace = true;
                }
                Event::Text(value) | Event::Paste(value) => {
                    // D/A generate both Key and Text on native platforms; consume the
                    // mnemonic only once. Numeric text never reaches global tool shortcuts.
                    if matches!(value.as_str(), "d" | "D" | "r" | "R" | "a" | "A") {
                        continue;
                    }
                    if self.active.is_none()
                        && value
                            .chars()
                            .next()
                            .is_some_and(|c| c.is_ascii_digit() || c == '-' || c == '.' || c == '+')
                    {
                        self.activate(0, live);
                    }
                    if self.active.is_some() {
                        if self.replace {
                            self.text.clear();
                            self.replace = false;
                        }
                        self.text.push_str(value);
                    }
                }
                _ => {}
            }
        }
        let mut values = [
            self.values[0].unwrap_or(live[0]),
            self.values[1].unwrap_or(live[1]),
        ];
        if let Some(index) = self.active {
            self.invalid = self.value().is_none();
            if let Some(value) = self.value() {
                values[index] = value;
            }
        }
        let angle = values[1].to_radians();
        let offset = if self.active.is_none() && self.values.iter().all(Option::is_none) {
            raw
        } else {
            Point::new(
                values[0] / scale * angle.cos(),
                values[0] / scale * angle.sin(),
            )
        };
        (
            offset,
            confirm && !self.invalid,
            !self.invalid && offset.x.is_finite() && offset.y.is_finite(),
        )
    }
    pub(super) fn paint(
        &self,
        ui: &mut Ui,
        canvas: Rect,
        pointer: Pos2,
        offset: Point,
        scale: f32,
        unit: &str,
        language: UiLanguage,
        mode: u8,
    ) {
        let size = vec2(238., if self.invalid { 83. } else { 60. });
        let top = pos2(
            (pointer.x + 22.)
                .min(canvas.right() - size.x)
                .max(canvas.left()),
            (pointer.y + 24.)
                .min(canvas.bottom() - size.y)
                .max(canvas.top()),
        );
        let rect = Rect::from_min_size(top, size);
        ui.painter_at(canvas)
            .rect_filled(rect, 3., Color32::from_rgb(76, 79, 82));
        let live = [
            offset.distance(Point::default()) * scale,
            offset.y.atan2(offset.x).to_degrees().rem_euclid(360.),
        ];
        ui.scope_builder(UiBuilder::new().max_rect(rect.shrink(7.)), |ui| {
            for (index, label) in ["Distance", "Angle"].iter().enumerate() {
                let active = self.active == Some(index);
                let number = if active {
                    self.text.clone()
                } else {
                    format!("{:.2}", live[index])
                };
                let value = format!(
                    "{} {}  {}{}{}",
                    language.translate(label),
                    if index == 0 { "[D]" } else { "[A]" },
                    number,
                    if index == 0 { unit } else { "°" },
                    if active { " ▏" } else { "" }
                );
                let text = RichText::new(value).monospace().size(13.).color(if active {
                    Color32::from_rgb(255, 220, 130)
                } else {
                    Color32::WHITE
                });
                ui.label(if active { text.strong() } else { text });
            }
            if self.invalid {
                ui.label(
                    RichText::new(language.translate("Enter a valid number"))
                        .color(Color32::from_rgb(255, 170, 160)),
                );
            }
        });
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
