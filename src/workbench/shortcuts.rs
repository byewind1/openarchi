//! Profile shortcut data and shared reservation policy; no model ownership.
use super::{CommandRegistry, WorkbenchCommand};
use eframe::egui::{Key, Modifiers};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy)]
pub struct EditShortcut {
    pub command: &'static str,
    pub key: Key,
    // 1 = platform command, 2 = option/alt, 4 = shift.
    pub modifiers: u8,
    pub hint: &'static str,
}
macro_rules! binding {
    ($name:literal, $key:ident, $mods:expr, $hint:literal) => {
        EditShortcut {
            command: $name,
            key: Key::$key,
            modifiers: $mods,
            hint: $hint,
        }
    };
}
pub const EDIT_SHORTCUTS: &[EditShortcut] = &[
    binding!("Undo", Z, 1, "⌘Z"),
    binding!("Redo", Z, 3, "⌥⌘Z"),
    // Existing OpenArchi/macOS convention retained as an explicit alias.
    binding!("Redo", Z, 5, "⇧⌘Z"),
    binding!("Repeat Last Command", B, 1, "⌘B"),
    binding!("Cut", X, 1, "⌘X"),
    binding!("Copy", C, 1, "⌘C"),
    binding!("Paste", V, 1, "⌘V"),
    binding!("Select All", A, 1, "⌘A"),
    binding!("Find & Select…", F, 1, "⌘F"),
    binding!("Group", G, 1, "⌘G"),
    binding!("Ungroup", G, 3, "⌥⌘G"),
    binding!("Suspend Groups", G, 2, "⌥G"),
    binding!("Drag", D, 1, "⌘D"),
    binding!("Rotate", E, 1, "⌘E"),
    binding!("Quick Rotate Clockwise", R, 5, "⇧⌘R"),
    binding!("Quick Rotate Counter-Clockwise", R, 1, "⌘R"),
    binding!("Mirror", M, 1, "⌘M"),
    binding!("Elevate…", Num9, 1, "⌘9"),
    binding!("Drag a Copy", D, 3, "⌥⌘D"),
    binding!("Rotate a Copy", E, 3, "⌥⌘E"),
    binding!("Rotate a Copy", R, 3, "⌥⌘R"),
    binding!("Mirror a Copy", M, 3, "⌥⌘M"),
    binding!("Multiply…", U, 1, "⌘U"),
    binding!("Resize…", K, 1, "⌘K"),
    binding!("Adjust", Minus, 1, "⌘−"),
    binding!("Explode into Current View", Equals, 1, "⌘="),
    binding!("Tool Settings", T, 1, "⌘T"),
    // Reserved even though the backend is currently unavailable. Must never
    // fall through into the less-specific Tool Settings binding.
    binding!("Edit Selection Set…", T, 3, "⌥⌘T"),
    binding!("Pick Up Parameters", C, 2, "⌥C"),
    binding!("Inject Parameters", C, 3, "⌥⌘C"),
];
impl EditShortcut {
    pub fn matches(self, key: Key, mods: Modifiers) -> bool {
        self.key == key
            && mods.command == (self.modifiers & 1 != 0)
            && mods.alt == (self.modifiers & 2 != 0)
            && mods.shift == (self.modifiers & 4 != 0)
            && !(mods.ctrl && mods.mac_cmd)
            && (mods.command || (!mods.ctrl && !mods.mac_cmd))
    }
}

#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct Chord {
    pub key: String,
    pub primary: bool,
    pub alt: bool,
    pub shift: bool,
}
impl Chord {
    pub fn new(key: &str, primary: bool, alt: bool, shift: bool) -> Self {
        Self {
            key: key.into(),
            primary,
            alt,
            shift,
        }
    }
    pub fn from_event(key: Key, mods: Modifiers) -> Option<Self> {
        if mods.ctrl && mods.mac_cmd || !mods.command && (mods.ctrl || mods.mac_cmd) {
            return None;
        }
        let chord = Self::new(key.name(), mods.command, mods.alt, mods.shift);
        chord.validate().ok().map(|_| chord)
    }
    pub fn validate(&self) -> Result<(), String> {
        let valid = self.key.len() == 1
            && self
                .key
                .bytes()
                .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
            || (1..=12).any(|n| self.key == format!("F{n}"));
        if !valid {
            return Err(
                "Use a letter, digit or F1–F12; navigation and text keys are reserved".into(),
            );
        }
        Ok(())
    }
    pub fn matches(&self, key: Key, mods: Modifiers) -> bool {
        Self::from_event(key, mods).as_ref() == Some(self)
    }
    pub fn hint(&self) -> String {
        let (primary, alt, shift) = if cfg!(target_os = "macos") {
            ("⌘", "⌥", "⇧")
        } else {
            ("Ctrl+", "Alt+", "Shift+")
        };
        format!(
            "{}{}{}{}",
            if self.alt { alt } else { "" },
            if self.shift { shift } else { "" },
            if self.primary { primary } else { "" },
            self.key
        )
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    pub command_id: String,
    pub keys: Vec<Chord>,
}

pub fn default_keys(id: &str) -> Vec<Chord> {
    if let Some(label) = CommandRegistry::resolve(id).and_then(WorkbenchCommand::edit_label) {
        return EDIT_SHORTCUTS
            .iter()
            .filter(|b| b.command == label)
            .map(|b| {
                Chord::new(
                    b.key.name(),
                    b.modifiers & 1 != 0,
                    b.modifiers & 2 != 0,
                    b.modifiers & 4 != 0,
                )
            })
            .collect();
    }
    let values: &[(&str, bool, bool, bool)] = match id {
        "tool.select" => &[("V", false, false, false)],
        "tool.wall" => &[("W", false, false, false)],
        "view.plan" => &[("F2", false, false, false), ("1", false, false, false)],
        "view.model" => &[("F3", false, false, false), ("3", false, false, false)],
        "file.save" => &[("S", true, false, false)],
        "model.undo" => &[("Z", true, false, false)],
        "model.redo" => &[("Z", true, true, false), ("Z", true, false, true)],
        "input.coordinate.x" => &[("X", false, false, false)],
        "input.coordinate.y" => &[("Y", false, false, false)],
        "input.distance" => &[("D", false, false, false), ("R", false, false, false)],
        "input.angle" => &[("A", false, false, false)],
        _ => &[],
    };
    values
        .iter()
        .map(|(k, p, a, s)| Chord::new(k, *p, *a, *s))
        .collect()
}
pub fn keys(overrides: &[Binding], id: &str) -> Vec<Chord> {
    overrides
        .iter()
        .find(|b| b.command_id == id)
        .map(|b| b.keys.clone())
        .unwrap_or_else(|| default_keys(id))
}
pub fn hint(overrides: &[Binding], id: &str) -> String {
    keys(overrides, id)
        .iter()
        .map(Chord::hint)
        .collect::<Vec<_>>()
        .join(" / ")
}
pub fn overridden(overrides: &[Binding], id: &str) -> bool {
    overrides.iter().any(|b| b.command_id == id)
}
fn reserved(id: &str, chord: &Chord) -> Option<&'static str> {
    let coordinate_input =
        CommandRegistry::resolve(id).is_some_and(WorkbenchCommand::is_coordinate_input);
    // A field command may be renamed, while raw digits and shifted digit
    // operators must remain part of the fixed numeric-input protocol.
    if coordinate_input
        && !chord.primary
        && !chord.alt
        && chord.key.bytes().all(|b| b.is_ascii_digit())
    {
        return Some("Numeric input");
    }
    if let Some(binding) = EDIT_SHORTCUTS.iter().find(|b| {
        // Registered edit defaults participate in the effective assignment map
        // below. Unregistered edit bindings remain conservatively reserved.
        WorkbenchCommand::from_edit_label(b.command).is_none()
            && Chord::new(
                b.key.name(),
                b.modifiers & 1 != 0,
                b.modifiers & 2 != 0,
                b.modifiers & 4 != 0,
            ) == *chord
    }) {
        return Some(binding.command);
    }
    // Conservative product reservations: changing extra modifiers must not
    // turn a session/window shortcut into an apparently usable model binding.
    if chord.primary && matches!(chord.key.as_str(), "Q" | "W" | "H") {
        return Some(match chord.key.as_str() {
            "Q" => "Quit",
            "W" => "Close window",
            _ => "Hide",
        });
    }
    if chord.primary && chord.shift && matches!(chord.key.as_str(), "3" | "4" | "5" | "6")
        || chord.primary && chord.alt && chord.key.starts_with('F')
    {
        return Some("System window command");
    }
    if chord.primary && !chord.alt && !chord.shift {
        return match chord.key.as_str() {
            "O" => Some("Open…"),
            "L" => Some("Layers"),
            "Q" => Some("Quit"),
            "W" => Some("Close window"),
            "H" => Some("Hide"),
            _ => None,
        };
    }
    if chord.key == "F4" && chord.alt || chord.primary && chord.key == "H" {
        return Some("System window command");
    }
    if !chord.primary
        && !chord.alt
        && !coordinate_input
        && matches!(
            chord.key.as_str(),
            "M" | "L" | "P" | "S" | "F" | "Q" | "D" | "2" | "F5"
        )
    {
        return Some("Existing canvas command");
    }
    if !chord.primary && chord.key == "S" && chord.alt && !chord.shift {
        return Some("Grid Snap");
    }
    None
}
pub fn validate(overrides: &[Binding]) -> Result<(), String> {
    if overrides.len() > 128 {
        return Err("Too many shortcut overrides".into());
    }
    let mut ids = std::collections::BTreeSet::new();
    for binding in overrides {
        if !super::environment::identifier(&binding.command_id, 256)
            || !ids.insert(binding.command_id.as_str())
            || binding.keys.len() > 4
        {
            return Err("Invalid or duplicate shortcut command IDs".into());
        }
        for chord in &binding.keys {
            chord.validate()?;
            if let Some(owner) = reserved(&binding.command_id, chord) {
                return Err(format!(
                    "{}: {} · {}",
                    "Shortcut is reserved",
                    chord.hint(),
                    owner
                ));
            }
        }
    }
    let mut assigned = std::collections::BTreeMap::new();
    let all = WorkbenchCommand::ALL.iter().map(|c| c.id()).chain(
        overrides
            .iter()
            .filter(|b| CommandRegistry::resolve(&b.command_id).is_none())
            .map(|b| b.command_id.as_str()),
    );
    for id in all {
        let coordinate_input =
            CommandRegistry::resolve(id).is_some_and(WorkbenchCommand::is_coordinate_input);
        for chord in keys(overrides, id) {
            // Ordinary canvas commands are not dispatched during precision
            // input. Keep their bindings intact when adding field defaults.
            if let Some(owner) = assigned.insert((coordinate_input, chord.clone()), id) {
                return Err(format!(
                    "{}: {} · {} ↔ {}",
                    "Shortcut conflicts with",
                    chord.hint(),
                    owner,
                    id
                ));
            }
        }
    }
    Ok(())
}
/// Validate the whole effective map before mutating the draft.
pub fn assign(
    overrides: &mut Vec<Binding>,
    id: &str,
    next: Option<Vec<Chord>>,
) -> Result<(), String> {
    let mut draft = overrides.clone();
    draft.retain(|b| b.command_id != id);
    if let Some(keys) = next {
        draft.push(Binding {
            command_id: id.into(),
            keys,
        });
    }
    validate(&draft)?;
    *overrides = draft;
    Ok(())
}
