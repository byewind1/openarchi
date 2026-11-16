//! Personal keys use the existing command dispatcher and configuration draft.
use super::*;
use crate::workbench::{
    CommandRegistry, WorkbenchCommand,
    environment::Profile,
    shortcuts::{self, Binding, Chord},
};

impl AtelierApp {
    pub(super) fn localized_shortcut_error(&self, message: &str) -> String {
        if let Some((prefix, detail)) = message.split_once(": ") {
            if matches!(prefix, "Shortcut is reserved" | "Shortcut conflicts with") {
                let (key, owner) = detail.split_once(" · ").unwrap_or((detail, ""));
                let owner = owner
                    .split(" ↔ ")
                    .map(|id| {
                        self.language.translate(
                            CommandRegistry::resolve(id)
                                .map(|c| c.label())
                                .unwrap_or(id),
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(" ↔ ");
                return format!(
                    "{}: {} · {}",
                    self.language.translate(prefix),
                    key,
                    self.language.translate(&owner)
                );
            }
        }
        self.language.translate(message).into()
    }

    pub(super) fn shortcut_overrides(&self) -> &[Binding] {
        self.host
            .environment
            .as_ref()
            .and_then(|s| {
                s.configuration
                    .profiles
                    .iter()
                    .find(|p| p.id == self.workbench.active_profile)
            })
            .and_then(|p| p.shortcuts.as_deref())
            .unwrap_or(&[])
    }
    pub(super) fn shortcut_overridden(&self, id: &str) -> bool {
        shortcuts::overridden(self.shortcut_overrides(), id)
    }
    pub(super) fn personal_command_label(&self, id: &str, label: &str, default: &str) -> String {
        if !self.shortcut_overridden(id) {
            return default.into();
        }
        let hint = shortcuts::hint(self.shortcut_overrides(), id);
        let label = self.language.translate(label);
        if hint.is_empty() {
            label.into()
        } else {
            format!("{label}  {hint}")
        }
    }
    pub(super) fn personal_shortcuts(&mut self, ctx: &Context) -> bool {
        let bindings = self.shortcut_overrides().to_vec();
        if bindings.is_empty() {
            return false;
        }
        let mut matched = false;
        let mut command = None;
        ctx.input_mut(|input| {
            let mut modifiers = input.modifiers;
            input.events.retain(|event| {
                if let Event::ModifiersChanged(m) = event {
                    modifiers = *m;
                }
                // winit emits semantic clipboard events for some modified C/X/V
                // chords. Match the original modifiers, just like legacy editing.
                let stroke = match event {
                    Event::Key {
                        key,
                        modifiers,
                        pressed: true,
                        repeat,
                        ..
                    } => Some((*key, *modifiers, *repeat)),
                    Event::Copy => Some((Key::C, modifiers, false)),
                    Event::Cut => Some((Key::X, modifiers, false)),
                    Event::Paste(_) => Some((Key::V, modifiers, false)),
                    _ => None,
                };
                if let Some((key, modifiers, repeat)) = stroke {
                    if let Some(binding) = bindings
                        .iter()
                        .filter(|binding| {
                            !CommandRegistry::resolve(&binding.command_id)
                                .is_some_and(WorkbenchCommand::is_coordinate_input)
                        })
                        .find(|b| b.keys.iter().any(|k| k.matches(key, modifiers)))
                    {
                        matched = true;
                        if !repeat && command.is_none() {
                            command = Some(binding.command_id.clone());
                        }
                        return false;
                    }
                }
                true
            });
        });
        if let Some(id) = command {
            if self.workbench_pointer_active()
                || self.drag.is_some()
                || self.pending_design_parameters()
            {
                self.host.document.editor.status =
                    "Finish or cancel the current edit before using a personal shortcut".into();
            } else if let Some(command) = CommandRegistry::resolve(&id) {
                self.dispatch_workbench(command);
            } else {
                self.host.document.editor.status =
                    "Command provider is unavailable; saved ID retained".into();
            }
        }
        matched
    }
    fn record_personal_shortcut(&mut self, ctx: &Context, profile: &mut Profile) {
        if !ctx.input(|input| input.focused) {
            self.workbench.shortcut_recording = None;
            return;
        }
        let Some((id, append)) = self.workbench.shortcut_recording.clone() else {
            return;
        };
        let mut capture = None;
        ctx.input_mut(|input| {
            let mut mods = input.modifiers;
            input.events.retain(|event| {
                if let Event::ModifiersChanged(m) = event {
                    mods = *m;
                }
                let key = match event {
                    Event::Key {
                        key,
                        modifiers,
                        pressed: true,
                        repeat: false,
                        ..
                    } => Some((*key, *modifiers)),
                    Event::Copy => Some((Key::C, mods)),
                    Event::Cut => Some((Key::X, mods)),
                    Event::Paste(_) => Some((Key::V, mods)),
                    _ => None,
                };
                if let Some((key, mods)) = key {
                    if capture.is_none() {
                        capture = Some(Chord::from_event(key, mods));
                    }
                    return false;
                }
                !matches!(event, Event::Text(_))
            });
        });
        if let Some(chord) = capture {
            self.workbench.shortcut_recording = None;
            let result = if let Some(chord) = chord {
                let mut keys = if append {
                    shortcuts::keys(&profile.shortcuts, &id)
                } else {
                    vec![]
                };
                keys.push(chord);
                shortcuts::assign(&mut profile.shortcuts, &id, Some(keys))
            } else {
                Err("Use a letter, digit or F1–F12; navigation and text keys are reserved".into())
            };
            self.workbench.configuration_error = result.err().unwrap_or_default();
        }
    }
    pub(super) fn personal_shortcuts_ui(&mut self, ui: &mut Ui, profile: &mut Profile) {
        self.record_personal_shortcut(ui.ctx(), profile);
        let language = self.language;
        let expand = std::mem::take(&mut self.workbench.shortcuts_expand);
        egui::CollapsingHeader::new(language.translate("Keyboard shortcuts"))
            .id_salt("personal_shortcuts").open(expand.then_some(true)).show(ui,|ui| {
                ui.label(language.translate("Click Record, then press a key combination. Escape cancels recording. Changes apply when the mode is saved."));
                if let Some((id,_))=&self.workbench.shortcut_recording {
                    ui.label(format!("{}: {}",language.translate("Press shortcut"),CommandRegistry::resolve(id).map(|c|language.translate(c.label())).unwrap_or(id)));
                    if ui.button(language.translate("Cancel recording")).clicked() {self.workbench.shortcut_recording=None;}
                }
                ui.horizontal(|ui| {
                    let label=ui.label(language.translate("Filter shortcut commands"));
                    ui.add(TextEdit::singleline(&mut self.workbench.shortcut_filter).id_salt("shortcut_filter").hint_text(language.translate("Command name or ID"))).labelled_by(label.id);
                });
                egui::ScrollArea::vertical().id_salt("shortcut_bindings").max_height(300.).show(ui,|ui| {
                    let ids=WorkbenchCommand::ALL.iter().map(|c|c.id().to_string()).chain(profile.shortcuts.iter().filter(|b|CommandRegistry::resolve(&b.command_id).is_none()).map(|b|b.command_id.clone())).collect::<Vec<_>>();
                    for id in ids {
                        let filter=self.workbench.shortcut_filter.trim().to_lowercase();
                        if !filter.is_empty() && !id.to_lowercase().contains(&filter) && !CommandRegistry::resolve(&id).is_some_and(|c|language.translate(c.label()).to_lowercase().contains(&filter)) {continue;}
 ui.push_id(&id,|ui| {
                        let command=CommandRegistry::resolve(&id);
                        let label=command.map(|c|language.translate(c.label())).unwrap_or(&id);
                        ui.horizontal_wrapped(|ui| {
                            ui.label(label).on_hover_text(&id);
                            let hint=shortcuts::hint(&profile.shortcuts,&id);
                            ui.monospace(if hint.is_empty() {language.translate("Unassigned")} else {&hint});
                            if command.is_none() {ui.label(language.translate("Command provider is unavailable; saved ID retained"));}
                        });
                        ui.horizontal_wrapped(|ui| {
                            for (key,append) in [("Record shortcut",false),("Add shortcut",true)] {
                                let response=ui.button(format!("{} · {}",language.translate(key),label));
                                if response.clicked() { response.request_focus(); self.workbench.shortcut_recording=Some((id.clone(),append)); self.workbench.configuration_error.clear(); }
                            }
                            for (key,keys) in [("Unbind shortcut",Some(vec![])),("Default shortcut",None)] {
                                if ui.button(language.translate(key)).clicked() {
                                    self.workbench.shortcut_recording=None;
                                    self.workbench.configuration_error=shortcuts::assign(&mut profile.shortcuts,&id,keys).err().unwrap_or_default();
                                }
                            }
                        });
                        ui.separator();
                    }); }
                });
            });
    }
}
