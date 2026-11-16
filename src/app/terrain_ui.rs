use super::project_sources_ui::SourceAction;
use super::*;
use crate::{
    application_host::{CommandContext, HostCommand},
    project_repository::terrain::TerrainPlacement,
    site::terrain::{self, Edit, Geometry},
};
#[derive(Clone)]
pub(super) struct TerrainPreview {
    pub proposal: terrain::Proposal,
    pub base: CommandContext,
    pub metrics: terrain::Metrics,
}
pub(super) struct VertexDraft {
    pub index: u32,
    pub expected: [f64; 3],
    pub position: [f64; 3],
    pub base: CommandContext,
}
#[derive(Default)]
pub(super) struct TerrainUi {
    pub open: bool,
    pub name: String,
    pub placement: Option<TerrainPlacement>,
    pub preview: Option<TerrainPreview>,
    pub draft: Option<VertexDraft>,
    pub vertex: u32,
    pub query_xy: [f64; 2],
    pub report: Option<(CommandContext, serde_json::Value)>,
    remove: bool,
    pub(super) metrics_cache: Option<(CommandContext, terrain::Metrics)>,
}
fn outline(ui: &mut Ui, g: &Geometry, marker: Option<[f64; 3]>) {
    if let Ok(edges) = g.edges() {
        let min = [
            g.vertices
                .iter()
                .map(|v| v[0])
                .fold(f64::INFINITY, f64::min),
            g.vertices
                .iter()
                .map(|v| v[1])
                .fold(f64::INFINITY, f64::min),
        ];
        let max = [
            g.vertices
                .iter()
                .map(|v| v[0])
                .fold(f64::NEG_INFINITY, f64::max),
            g.vertices
                .iter()
                .map(|v| v[1])
                .fold(f64::NEG_INFINITY, f64::max),
        ];
        let (rect, _) = ui.allocate_exact_size(vec2(350., 150.), Sense::hover());
        let scale = ((rect.width() - 12.) as f64 / (max[0] - min[0]).max(0.01))
            .min((rect.height() - 12.) as f64 / (max[1] - min[1]).max(0.01));
        let to_screen = |v: [f64; 3]| {
            pos2(
                rect.left() as f64 + 6. + (v[0] - min[0]) * scale,
                rect.bottom() as f64 - 6. - (v[1] - min[1]) * scale,
            )
        };
        if let Some(v) = marker.filter(|v| v.iter().all(|v| v.is_finite())) {
            ui.painter_at(rect)
                .circle_filled(to_screen(v), 4., Color32::from_rgb(214, 151, 63));
        }
        for [a, b] in edges {
            let to_screen = |v: [f64; 3]| {
                pos2(
                    rect.left() as f64 + 6. + (v[0] - min[0]) * scale,
                    rect.bottom() as f64 - 6. - (v[1] - min[1]) * scale,
                )
            };
            ui.painter()
                .line_segment([to_screen(a), to_screen(b)], Stroke::new(1., ACCENT));
        }
    }
}
impl AtelierApp {
    pub(super) fn apply_terrain_edit(&mut self, base: CommandContext, edit: Edit) {
        if self.menu_gated() || self.workbench_pointer_active() {
            self.source_ui.error = "Finish the active interaction before editing terrain".into();
            return;
        }
        match self.dispatch_project_preparation(
            base,
            HostCommand::Apply {
                request_id: storage::new_document_identity(),
                commands: vec![crate::command::ModelCommand::Terrain { edit }],
            },
        ) {
            Ok(_) => {
                self.terrain_ui.draft = None;
                self.terrain_ui.report = None;
                self.terrain_ui.metrics_cache = None;
                self.source_ui.error.clear();
            }
            Err(e) => self.source_ui.error = e,
        }
    }
    pub(super) fn terrain_window(&mut self, ctx: &Context) {
        if !self.terrain_ui.open || self.project_conditions_ui.open {
            return;
        }
        let mut open = true;
        let mut action = None;
        let mut edit = None;
        let language = self.language;
        if self.terrain_ui.name.is_empty() {
            self.terrain_ui.name = language.translate("Terrain surface").into();
        }
        egui::Window::new(language.translate("Terrain surface"))
            .id(egui::Id::new("terrain_surface"))
            .open(&mut open)
            .default_width(720.)
            .default_height(800.)
            .vscroll(true)
            .show(ctx, |ui| {
                self.terrain_content(ui, language, &mut action, &mut edit)
            });
        self.terrain_ui.open = open;
        if let Some((base, e)) = edit {
            self.apply_terrain_edit(base, e);
        }
        if let Some(a) = action {
            self.start_source_action(a);
        }
    }
    pub(super) fn terrain_content(
        &mut self,
        ui: &mut Ui,
        language: UiLanguage,
        action: &mut Option<SourceAction>,
        edit: &mut Option<(CommandContext, Edit)>,
    ) {
        ui.label(language.translate("Terrain is an unverified source declaration. Missing coverage is unknown; flat ground is not inferred."));
        ui.label(language.translate("Neighboring elevations and windows remain unverified."));
        if ui.button(language.translate("Project sources")).clicked() {
            self.project_conditions_ui.navigation = Some(ConditionsGroup::Sources);
        }
        if let Some(preview) = &self.terrain_ui.preview {
            ui.separator();
            ui.strong(format!(
                "{} · {}",
                language.translate("Terrain preview"),
                preview.proposal.terrain.name
            ));
            {
                let m = &preview.metrics;
                ui.label(format!(
                    "{}: {} · {}: {}",
                    language.translate("Vertices"),
                    m.vertices,
                    language.translate("Triangles"),
                    m.triangles
                ));
                ui.label(format!(
                    "{}: {:.3} m² · Z {:.3}..{:.3} m",
                    language.translate("Projected coverage"),
                    m.projected_area_m2,
                    m.bounds_m[0][2],
                    m.bounds_m[1][2]
                ));
            }
            outline(ui, &preview.proposal.terrain.geometry, None);
            if ui
                .button(language.translate("Use preview as terrain"))
                .clicked()
            {
                *action = Some(SourceAction::CommitTerrain(preview.clone()));
            }
            if ui
                .button(language.translate("Discard terrain preview"))
                .clicked()
            {
                self.terrain_ui.preview = None;
            }
        }
        let bound = self.source_ui.repository.as_ref().is_some_and(|r| {
            r.manifest.document_id == self.host.document.editor.project_identity()
        });
        ui.horizontal(|ui| {
            ui.label(language.translate("Terrain name"));
            ui.text_edit_singleline(&mut self.terrain_ui.name);
        });
        egui::ComboBox::from_id_salt("terrain_placement")
            .selected_text(language.translate(match self.terrain_ui.placement {
                None => "Choose terrain placement explicitly",
                Some(TerrainPlacement::ModelXyz) => "Drawing XYZ to model XYZ",
                Some(TerrainPlacement::SiteReference) => "Projected EN and datum elevation",
            }))
            .show_ui(ui, |ui| {
                ui.selectable_value(
                    &mut self.terrain_ui.placement,
                    Some(TerrainPlacement::ModelXyz),
                    language.translate("Drawing XYZ to model XYZ"),
                );
                ui.selectable_value(
                    &mut self.terrain_ui.placement,
                    Some(TerrainPlacement::SiteReference),
                    language.translate("Projected EN and datum elevation"),
                );
            });
        ui.label(language.translate("Confirm source units in Project sources. Only triangular OBJ geometry is accepted; no automatic triangulation."));
        if let Some(repo) = &self.source_ui.repository {
            for s in &repo.manifest.sources {
                if s.format == "obj" {
                    ui.horizontal(|ui| {
                        ui.label(s.names.join(", "));
                        ui.label(s.confirmed_unit.map_or(
                            language.translate("Units unconfirmed"),
                            crate::site::LengthUnit::symbol,
                        ));
                        if ui
                            .add_enabled(
                                bound
                                    && !matches!(
                                        s.quality,
                                        crate::project_repository::SourceQuality::Rejected { .. }
                                    ),
                                egui::Button::new(language.translate("Preview terrain")),
                            )
                            .clicked()
                        {
                            *action = Some(SourceAction::PreviewTerrain(s.id.clone()));
                        }
                    });
                }
            }
        }
        ui.separator();
        let terrain = self.host.document.editor.project.site.terrain.clone();
        if let Some(t) = terrain {
            ui.strong(&t.name);
            let base = self.host.context();
            if self
                .terrain_ui
                .metrics_cache
                .as_ref()
                .is_none_or(|(context, _)| *context != base)
            {
                self.terrain_ui.metrics_cache = t.geometry.metrics().ok().map(|m| (base, m));
            }
            if let Some((_, m)) = &self.terrain_ui.metrics_cache {
                ui.label(format!(
                    "{}: {} · {}: {} · {:.3} m²",
                    language.translate("Vertices"),
                    m.vertices,
                    language.translate("Triangles"),
                    m.triangles,
                    m.projected_area_m2
                ));
            }
            if t.user_modified() {
                ui.label(
                    language
                        .translate("Terrain contains user edits; automatic reimport is refused"),
                );
            }
            if !t.reference_current(&self.host.document.editor.project.site) {
                ui.colored_label(
                    Color32::YELLOW,
                    language.translate("Terrain reference changed; coordinates were not moved"),
                );
            }
            if ui.button(language.translate("Focus terrain")).clicked() {
                self.fit();
            }
            self.terrain_ui.vertex = self
                .terrain_ui
                .vertex
                .max(1)
                .min(t.geometry.vertices.len() as u32);
            ui.horizontal(|ui| {
                ui.label(language.translate("Source vertex number"));
                ui.add(
                    egui::DragValue::new(&mut self.terrain_ui.vertex)
                        .range(1..=t.geometry.vertices.len() as u32),
                );
                if ui
                    .button(language.translate("Edit terrain vertex"))
                    .clicked()
                {
                    let index = self.terrain_ui.vertex - 1;
                    let value = t.geometry.vertices[index as usize];
                    self.terrain_ui.draft = Some(VertexDraft {
                        index,
                        expected: value,
                        position: value,
                        base: self.host.context(),
                    });
                }
            });
            let index = (self.terrain_ui.vertex - 1) as usize;
            let mut shown = t.geometry.clone();
            let mut marker = shown.vertices[index];
            if let Some(draft) = &self.terrain_ui.draft {
                if let Some(v) = shown.vertices.get_mut(draft.index as usize) {
                    *v = draft.position;
                    marker = draft.position;
                }
            }
            outline(ui, &shown, Some(marker));

            if t.geometry.vertices[index] != t.imported.vertices[index]
                && ui
                    .button(language.translate("Restore imported vertex"))
                    .clicked()
            {
                *edit = Some((
                    self.host.context(),
                    Edit::UpdateVertex {
                        index: index as u32,
                        expected: t.geometry.vertices[index],
                        position: t.imported.vertices[index],
                    },
                ));
            }
            if ui.button(language.translate("Remove terrain…")).clicked() {
                self.terrain_ui.remove = true;
            }
            if self.terrain_ui.remove {
                ui.label(
                    language.translate("Remove this terrain surface? Original source is retained."),
                );
                if ui
                    .button(language.translate(if self.project_conditions_ui.open {
                        "Stage condition change"
                    } else {
                        "Confirm remove terrain"
                    }))
                    .clicked()
                {
                    *edit = Some((self.host.context(), Edit::Remove { expected: t }));
                    self.terrain_ui.remove = false;
                }
                if ui.button(language.translate("Cancel")).clicked() {
                    self.terrain_ui.remove = false;
                }
            }
        } else {
            ui.label(language.translate("No terrain surface; flat ground is not inferred"));
        }
        if let Some(draft) = &mut self.terrain_ui.draft {
            ui.separator();
            ui.strong(format!(
                "{} {}",
                language.translate("Edit terrain vertex"),
                draft.index + 1
            ));
            ui.horizontal(|ui| {
                for (i, label) in ["X (m)", "Y (m)", "Z (m)"].into_iter().enumerate() {
                    ui.label(label);
                    ui.add(egui::DragValue::new(&mut draft.position[i]).speed(0.01));
                }
            });
            if ui
                .button(language.translate(if self.project_conditions_ui.open {
                    "Stage condition change"
                } else {
                    "Apply terrain vertex"
                }))
                .clicked()
            {
                *edit = Some((
                    draft.base,
                    Edit::UpdateVertex {
                        index: draft.index,
                        expected: draft.expected,
                        position: draft.position,
                    },
                ));
            }
            if ui
                .button(language.translate("Cancel vertex edit"))
                .clicked()
            {
                self.terrain_ui.draft = None;
            }
        }
        ui.separator();
        ui.horizontal(|ui| {
            ui.label(language.translate("Query model XY elevation"));
            for v in &mut self.terrain_ui.query_xy {
                ui.add(egui::DragValue::new(v).speed(0.1));
            }
            if ui
                .button(language.translate("Query terrain height"))
                .clicked()
            {
                let service =
                    crate::automation::ReadService::new("native-terrain-inspection".into());
                let result = service.terrain_height(
                    &self.host.document.editor,
                    &crate::automation::TerrainRequest {
                        target: service.target(&self.host.document.editor),
                        expected_revision: self.host.document.editor.revision,
                        xy_m: self.terrain_ui.query_xy,
                    },
                );
                match result {
                    Ok(report) => self.terrain_ui.report = Some((self.host.context(), report)),
                    Err(e) => self.source_ui.error = e.code.into(),
                }
            }
        });
        if let Some((base, report)) = &self.terrain_ui.report {
            if *base != self.host.context()
                || report["xy_m"] != serde_json::json!(self.terrain_ui.query_xy)
            {
                ui.label(language.translate("Terrain sample is stale; query again"));
            } else {
                let s = &report["sample"];
                if let Some(height) = s["elevation_model_m"].as_f64() {
                    ui.strong(format!("Z = {height:.4} m"));
                    ui.label(language.translate("Declared height; not survey approval"));
                } else {
                    ui.label(
                        language
                            .translate(s["reason"].as_str().unwrap_or("Unknown terrain height")),
                    );
                }
            }
        }
        if self.source_ui.pending_for_ui() {
            ui.spinner();
        }
        if !self.source_ui.error.is_empty() {
            ui.colored_label(
                Color32::LIGHT_RED,
                language.translate(&self.source_ui.error),
            );
        }
    }
}
