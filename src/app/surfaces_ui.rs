//! Presentation controls beside the existing canvas; no independent model state.
use super::*;
impl AtelierApp {
    pub(super) fn preview_space_surfaces(&mut self, ids: Vec<u64>, seed: String) {
        if self.menu_gated() || self.workbench_pointer_active() || self.pending_design_parameters()
        {
            return;
        }
        let editor = &self.host.document.editor;
        let result =
            crate::zones::surfaces::random_commands(editor, &ids, &seed).and_then(|commands| {
                editor
                    .prepare_change(editor.revision, commands)
                    .map_err(|e| e.message)
            });
        if let Err(error) = &result {
            self.host.document.editor.status = error.clone();
        }
        self.process_set_preview(seed, "Assign space surfaces", result);
    }
    pub(super) fn apply_3d_style(&mut self, style: crate::view_settings::Model3dStyle) {
        if self.menu_gated() || self.workbench_pointer_active() || self.pending_design_parameters()
        {
            return;
        }
        let editor = &self.host.document.editor;
        if editor.project.plan_options.model_3d_style == style {
            return;
        }
        let result = editor
            .prepare_change(editor.revision, vec![style.command(&editor.project)])
            .map_err(|e| e.message)
            .and_then(|change| {
                self.host
                    .commit_prepared(
                        self.host.context(),
                        &storage::new_document_identity(),
                        &change,
                    )
                    .map(|_| ())
            });
        if let Err(error) = result {
            self.host.document.editor.status = error;
        }
    }
    pub(super) fn model_surfaces_ui(&mut self, ui: &mut Ui) {
        use crate::view_settings::Model3dStyle;
        let language = self.language;
        let gated = self.menu_gated()
            || self.workbench_pointer_active()
            || self.pending_design_parameters();
        let editor = &self.host.document.editor;
        let selected = editor.selection_ids();
        let filter = self.model_view_filter(&selected);
        let selected_zones: Vec<_> = editor
            .project
            .elements
            .iter()
            .filter(|e| selected.contains(&e.id) && matches!(e.shape, Shape::Zone { .. }))
            .map(|e| e.id)
            .collect();
        let (scope, ids) = if selected_zones.is_empty() {
            (
                "Visible spaces",
                editor
                    .project
                    .elements
                    .iter()
                    .filter(|e| {
                        matches!(e.shape, Shape::Zone { .. }) && filter.includes(&editor.project, e)
                    })
                    .map(|e| e.id)
                    .collect::<Vec<_>>(),
            )
        } else {
            ("Selected spaces", selected_zones)
        };
        let old = editor.project.plan_options.model_3d_style;
        let mut style = old;
        let mut random = false;
        ui.add_enabled_ui(!gated, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(language.translate("3D display"));
                for option in [
                    Model3dStyle::Wireframe,
                    Model3dStyle::Shaded,
                    Model3dStyle::ShadedWithEdges,
                ] {
                    ui.selectable_value(&mut style, option, language.translate(option.label()));
                }
                ui.separator();
                random = ui
                    .add_enabled(
                        !ids.is_empty(),
                        egui::Button::new(language.translate("Randomize space surfaces")),
                    )
                    .clicked();
                ui.label(format!("{}: {}", language.translate(scope), ids.len()));
            })
        });
        if old != style {
            self.apply_3d_style(style);
        }
        if random {
            self.preview_space_surfaces(ids, storage::new_document_identity());
        }
    }
}
