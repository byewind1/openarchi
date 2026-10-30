//! AC29 macOS Standard Shortcuts 29, global + non-text contexts.
//! Binding and menu hint come from one table. AppKit key equivalents are
//! deliberately not installed: egui text widgets own their editing keys.
use super::*;

#[derive(Clone, Copy)]
pub(super) struct EditShortcut {
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
pub(super) const EDIT_SHORTCUTS: &[EditShortcut] = &[
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
    fn matches(self, key: Key, mods: Modifiers) -> bool {
        self.key == key
            && mods.command == (self.modifiers & 1 != 0)
            && mods.alt == (self.modifiers & 2 != 0)
            && mods.shift == (self.modifiers & 4 != 0)
            && !(mods.ctrl && mods.mac_cmd)
            && (mods.command || (!mods.ctrl && !mods.mac_cmd))
    }
}
pub(super) fn shortcut_hint(command: &str) -> String {
    if command == "Delete" {
        return "⌫ / Delete".into();
    }
    EDIT_SHORTCUTS
        .iter()
        .filter(|b| b.command == command)
        .map(|b| {
            if cfg!(target_os = "macos") {
                b.hint.to_owned()
            } else {
                b.hint
                    .replace('⌥', "Alt+")
                    .replace('⇧', "Shift+")
                    .replace('⌘', "Ctrl+")
            }
        })
        .collect::<Vec<_>>()
        .join(" / ")
}
impl AtelierApp {
    pub(super) fn edit_menu_label(&self, command: &str) -> String {
        let hint = shortcut_hint(command);
        let label = self.language.translate(command);
        if hint.is_empty() {
            label.to_owned()
        } else {
            format!("{label}  {hint}")
        }
    }
    /// Consume a matched combination even when disabled, and dispatch once.
    /// Exact modifiers prevent Cmd+Alt+E from also invoking Cmd+E.
    pub(super) fn edit_shortcuts(&mut self, ctx: &Context) -> bool {
        let mut handled = false;
        let mut command = None;
        ctx.input_mut(|input| {
            let mut modifiers = input.modifiers;
            input.events.retain(|event| {
                if let Event::ModifiersChanged(value) = event {
                    modifiers = *value;
                }
                // egui-winit replaces C/X/V key-downs with clipboard events.
                // It also classifies Cmd+Alt+C as Copy, so recover the chord.
                let clipboard_key = match event {
                    Event::Copy => Some(Key::C),
                    Event::Cut => Some(Key::X),
                    Event::Paste(_) => Some(Key::V),
                    _ => None,
                };
                if let Some(key) = clipboard_key {
                    let binding = EDIT_SHORTCUTS.iter().find(|b| b.matches(key, modifiers));
                    let semantic = if modifiers.is_none() {
                        match event {
                            Event::Copy => Some("Copy"),
                            Event::Cut => Some("Cut"),
                            Event::Paste(_) => Some("Paste"),
                            _ => None,
                        }
                    } else {
                        None
                    };
                    if let Some(name) = binding.map(|b| b.command).or(semantic) {
                        handled = true;
                        if command.is_none() {
                            command = Some(name);
                        }
                        return false;
                    }
                }
                if let Event::Key {
                    key,
                    modifiers,
                    pressed: true,
                    repeat,
                    ..
                } = event
                {
                    if let Some(binding) =
                        EDIT_SHORTCUTS.iter().find(|b| b.matches(*key, *modifiers))
                    {
                        handled = true;
                        if !repeat && command.is_none() {
                            command = Some(binding.command);
                        }
                        return false;
                    }
                }
                true
            })
        });
        if let Some(command) = command {
            self.dispatch_edit(command);
        }
        handled
    }
}
