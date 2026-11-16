//! AC29 macOS Standard Shortcuts 29, global + non-text contexts.
//! Binding and menu hint come from one table. AppKit key equivalents are
//! deliberately not installed: egui text widgets own their editing keys.
use super::*;

pub(super) use crate::workbench::shortcuts::EDIT_SHORTCUTS;
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
        let id = crate::workbench::WorkbenchCommand::from_edit_label(command).map(|c| c.id());
        let hint = if let Some(id) = id.filter(|id| self.shortcut_overridden(id)) {
            crate::workbench::shortcuts::hint(self.shortcut_overrides(), id)
        } else {
            shortcut_hint(command)
        };
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
        let allowed = |b: &&crate::workbench::shortcuts::EditShortcut| {
            !crate::workbench::WorkbenchCommand::from_edit_label(b.command)
                .is_some_and(|c| self.shortcut_overridden(c.id()))
        };
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
                    let binding = EDIT_SHORTCUTS
                        .iter()
                        .filter(allowed)
                        .find(|b| b.matches(key, modifiers));
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
                    if let Some(binding) = EDIT_SHORTCUTS
                        .iter()
                        .filter(allowed)
                        .find(|b| b.matches(*key, *modifiers))
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
