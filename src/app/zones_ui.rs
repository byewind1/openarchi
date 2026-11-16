use super::*;
impl AtelierApp {
    pub(super) fn mass_polygon_close_target(&self, point: Point) -> Option<Point> {
        let editor = &self.host.document.editor;
        (editor.tool == Tool::ConceptMass
            && editor.zone_polygon
            && editor.polyline_points.len() >= 3)
            .then(|| editor.polyline_points[0])
            .filter(|start| start.distance(point) <= 9. / self.plan_scale)
    }

    pub(super) fn finish_mass_polygon_input(&mut self) {
        match self.host.document.editor.finish_concept_mass() {
            Ok(_) => {
                self.tracker_locked = false;
                self.draft_id = None;
            }
            Err(error) => self.host.document.editor.status = error,
        }
    }

    pub(super) fn concept_mass_tool_ui(&mut self, ui: &mut Ui) {
        let language = self.language;
        ui.label(language.translate("Height"));
        ui.add(
            DragValue::new(&mut self.host.document.editor.concept_mass_height)
                .range(0.01..=100.)
                .speed(0.1)
                .suffix(" m"),
        );
        ui.label(language.translate("Bottom offset"));
        ui.add(
            DragValue::new(&mut self.host.document.editor.concept_mass_bottom_offset)
                .range(-100. ..=100.)
                .speed(0.1)
                .suffix(" m"),
        );
        ui.add_enabled_ui(!self.host.document.editor.model_input_active(), |ui| {
            ui.selectable_value(
                &mut self.host.document.editor.zone_polygon,
                false,
                language.translate("Rectangular"),
            );
            ui.selectable_value(
                &mut self.host.document.editor.zone_polygon,
                true,
                language.translate("Polygonal"),
            );
        });
        if self.host.document.editor.zone_polygon
            && ui
                .add_enabled(
                    self.host.document.editor.polyline_points.len() >= 3,
                    egui::Button::new(language.translate("Finish mass")),
                )
                .clicked()
        {
            self.finish_mass_polygon_input();
        }
        ui.label(language.translate("Concept envelope; not space or statutory floor area"));
    }

    pub(super) fn zone_tool_ui(&mut self, ui: &mut Ui) {
        let language = self.language;
        ui.label(language.translate("Space name"));
        ui.add_sized(
            [120., 24.],
            TextEdit::singleline(&mut self.host.document.editor.zone_name),
        );
        ui.label(language.translate("Space purpose"));
        ui.add_sized(
            [100., 24.],
            TextEdit::singleline(&mut self.host.document.editor.zone_purpose),
        );
        ui.label(language.translate("Clear height"));
        ui.add(
            DragValue::new(&mut self.host.document.editor.zone_height)
                .range(0.01..=100.)
                .speed(0.1)
                .suffix(" m"),
        );
        ui.add_enabled_ui(
            self.host.document.editor.anchor.is_none()
                && self.host.document.editor.polyline_points.is_empty(),
            |ui| {
                ui.selectable_value(
                    &mut self.host.document.editor.zone_polygon,
                    false,
                    language.translate("Rectangular"),
                );
                ui.selectable_value(
                    &mut self.host.document.editor.zone_polygon,
                    true,
                    language.translate("Polygonal"),
                );
            },
        );
        if self.host.document.editor.zone_polygon
            && ui
                .add_enabled(
                    self.host.document.editor.polyline_points.len() >= 3,
                    egui::Button::new(language.translate("Finish space")),
                )
                .clicked()
        {
            if let Err(e) = self.host.document.editor.finish_zone() {
                self.host.document.editor.status = e;
            }
        }
        if ui.button(language.translate("Analyze spaces")).clicked() {
            self.zone_analysis_open = true;
        }
    }
    pub(super) fn zone_analysis_ui(&mut self, ctx: &Context) {
        if !self.zone_analysis_open {
            return;
        }
        let language = self.language;
        let mut open = true;
        egui::Window::new(language.translate("Space analysis")).open(&mut open).default_width(570.).show(ctx,|ui| {
            ui.label(format!("{}: {}",language.translate("Home Story"),self.host.document.editor.story));
            ui.label(language.translate("Near boundaries do not prove a door or a passable route."));
            let scope=crate::view_settings::DesignOptionScope::active(self.host.document.editor.project.plan_options.active_design_option);
            let option_name=match self.host.document.editor.project.plan_options.active_design_option {
                None=>language.translate("Main model").to_owned(),
                Some(id)=>self.host.document.editor.project.design_options.name(id).map(str::to_owned).unwrap_or_else(||format!("{} #{id}",language.translate("Unknown design option"))),
            };
            ui.label(format!("{}: {option_name}",language.translate("Analysis design option")));
            let key=(self.host.document.editor.document_session(),self.host.document.editor.revision,self.host.document.editor.story,scope);
            if self.zone_analysis_cache.as_ref().is_none_or(|c|(c.0,c.1,c.2,c.3)!=key) {
                let report=crate::zones::analysis::analyze(&self.host.document.editor.project,&crate::zones::analysis::AnalysisOptions {story:Some(self.host.document.editor.story),design_scope:scope,..Default::default()});
                self.zone_analysis_cache=Some((key.0,key.1,key.2,key.3,report));
            }
            let report=self.zone_analysis_cache.as_ref().unwrap().4.clone();
            match report {
                Err(e)=>{ui.label(language.translate(&e));},
                Ok(report)=>{
                    if let Some(t)=report.by_story.get(&self.host.document.editor.story) {
                        ui.label(format!("{}: {}   {}: {:.2} m²   {}: {:.2} m³",language.translate("Spaces"),t.count,language.translate("Space area"),t.area_m2,language.translate("Space volume"),t.space_volume_m3));
                    }
                    egui::ScrollArea::vertical().max_height(450.).show(ui,|ui| {
                        egui::Grid::new("space_analysis_rows").striped(true).show(ui,|ui| {
                            for k in ["ID","Space name","Space purpose","Space area"] {ui.strong(language.translate(k));}ui.end_row();
                            for s in report.spaces {
                                if ui.button(s.number).clicked() {self.host.document.editor.set_selection(vec![s.id]);}
                                if s.wall_footprint_overlap_m2>1e-6 {ui.colored_label(Color32::from_rgb(210,100,75),format!("{}: {:.3} m²",language.translate("Wall footprint overlap"),s.wall_footprint_overlap_m2));} else {ui.label(s.name);}ui.label(s.purpose);ui.label(format!("{:.2} m²",s.area_m2));ui.end_row();
                            }
                        });
                        ui.separator();
                        for r in report.relations {
                            ui.label(if r.relation=="overlap" {format!("{}: Z{:03} / Z{:03} · {:.3} m²",language.translate("Space overlap"),r.a,r.b,r.overlap_m2)} else {format!("{}: Z{:03} / Z{:03} · {:.3} m",language.translate("Near boundary"),r.a,r.b,r.boundary_distance_m)});
                        }
                        if report.relations_truncated {ui.label(language.translate("More relations are available in the analysis count."));}
                        ui.label(language.translate("Space areas are explicit boundaries; they do not update from walls automatically."));
                    });
                }
            }
        });
        self.zone_analysis_open = open;
    }
}
