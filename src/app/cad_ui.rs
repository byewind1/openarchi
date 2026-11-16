use super::*;
use crate::cad_reference::{self as cad, Source, Transform, Underlay};
use std::{
    collections::BTreeSet,
    sync::mpsc::{self, Receiver, TryRecvError},
};
#[derive(Default)]
pub(super) struct CadUi {
    pub open: bool,
    pub path: String,
    pub converter: String,
    pub error: String,
    pending: Option<(
        crate::application_host::CommandContext,
        Option<u64>,
        Receiver<Result<(Source, image::RgbaImage), String>>,
    )>,
    preview: Option<(crate::application_host::CommandContext, Option<u64>, Source)>,
    thumbnail: Option<TextureHandle>,
    transform: Transform,
    pub unit_confirmed: bool,
    partial: bool,
    selected: Option<u64>,
    draft: Option<Underlay>,
    settings_base: Option<Underlay>,
    settings: bool,
    update_source: Option<u64>,
    alignment_reviewed: bool,
    #[cfg(test)]
    pub rects: std::collections::HashMap<String, Rect>,
}
/// Screen-only projection of the same f64 source transform, factored once per draw.
struct CadPlanProjection {
    anchor: [f64; 2],
    scale: f64,
    sin: f64,
    cos: f64,
    origin: [f64; 2],
}
impl CadPlanProjection {
    fn new(t: &Transform, center: Point, scale: f64, orientation: f64, viewport: Rect) -> Self {
        let (s, c) = orientation.to_radians().sin_cos();
        let (sin, cos) = (orientation + t.angle).to_radians().sin_cos();
        let x = t.xy.x - center.x;
        let y = t.xy.y - center.y;
        Self {
            anchor: t.anchor,
            scale: scale * t.metres_per_unit,
            sin,
            cos,
            origin: [
                viewport.center().x as f64 + (x * c - y * s) * scale,
                viewport.center().y as f64 - (x * s + y * c) * scale,
            ],
        }
    }
    fn screen(&self, p: [f64; 2]) -> Pos2 {
        let x = (p[0] - self.anchor[0]) * self.scale;
        let y = (p[1] - self.anchor[1]) * self.scale;
        pos2(
            self.origin[0] + x * self.cos - y * self.sin,
            self.origin[1] - x * self.sin - y * self.cos,
        )
    }
}
impl AtelierApp {
    pub(super) fn cad_configured_converter(&self) -> String {
        if !self.source_ui.converter_path.trim().is_empty() {
            return self.source_ui.converter_path.clone();
        }
        if let Some(repo) = &self.source_ui.repository {
            if let Ok(Some(r)) = repo.latest_conversion() {
                return r.tool.executable;
            }
        }
        if let Some(p) = crate::platform::config_dir().map(|p| p.join("dwg2dxf-path")) {
            if let Ok(v) = std::fs::read_to_string(p) {
                return v.trim().into();
            }
        }
        if let Some(v) = std::env::var_os("OPENARCHI_DWG2DXF") {
            return v.to_string_lossy().into_owned();
        }
        for dir in std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()) {
            let p = dir.join(if cfg!(windows) {
                "dwg2dxf.exe"
            } else {
                "dwg2dxf"
            });
            if p.is_file() {
                return p.display().to_string();
            }
        }
        String::new()
    }
    pub(super) fn cad_start(&mut self) -> Result<(), String> {
        let path = PathBuf::from(self.cad_ui.path.trim());
        let exe = PathBuf::from(self.cad_ui.converter.trim());
        let cache = self.drawing_cache()?.join("cad-v1");
        let base = self.host.context();
        let worksheet = self
            .host
            .document
            .editor
            .project
            .plan_options
            .active_worksheet;
        let (tx, rx) = mpsc::channel();
        self.cad_ui.preview = None;
        self.cad_ui.thumbnail = None;
        self.cad_ui.partial = false;
        self.cad_ui.error.clear();
        self.cad_ui.pending = Some((base, worksheet, rx));
        std::thread::spawn(move || {
            let result = cad::prepare(
                &path,
                if exe.as_os_str().is_empty() {
                    None
                } else {
                    Some(&exe)
                },
                &cache,
            );
            let result = result.and_then(|source| {
                let data = source.data.as_ref().ok_or("CAD preview unavailable")?;
                let image = cad::thumbnail(data, 620, 180);
                Ok((source, image))
            });
            let _ = tx.send(result);
        });
        Ok(())
    }
    pub(super) fn cad_confirm(&mut self) -> Result<(), String> {
        let (base, worksheet, source) = self
            .cad_ui
            .preview
            .clone()
            .ok_or("CAD preview unavailable")?;
        if !self.cad_ui.unit_confirmed || !self.cad_ui.partial {
            return Err("Confirm drawing units and partial import coverage".into());
        }
        self.cad_ui.transform.validate()?;
        if base != self.host.context()
            || (self.cad_ui.update_source.is_none()
                && worksheet
                    != self
                        .host
                        .document
                        .editor
                        .project
                        .plan_options
                        .active_worksheet)
        {
            return Err("Drawing preview is stale; preview the file again".into());
        }
        let (doc, worksheet, id) = if let Some(source_id) = self.cad_ui.update_source {
            if !self.cad_ui.alignment_reviewed {
                return Err("Review source alignment changes before updating".into());
            }
            let current = self
                .host
                .document
                .editor
                .project
                .drawings
                .cad_underlays
                .iter()
                .find(|u| u.source == source_id)
                .ok_or("CAD reference no longer exists")?;
            (
                cad::replace_source(
                    &self.host.document.editor.project.drawings,
                    source_id,
                    source,
                    true,
                )?,
                current.worksheet,
                current.id,
            )
        } else {
            cad::attach(
                &self.host.document.editor.project.drawings,
                source,
                worksheet,
                self.cad_ui.transform.clone(),
                self.cad_ui.partial,
            )?
        };
        self.host.dispatch(
            base,
            crate::application_host::HostCommand::Apply {
                request_id: storage::new_document_identity(),
                commands: vec![crate::command::ModelCommand::SetDrawings { document: doc }],
            },
        )?;
        self.cad_ui.selected = Some(id);
        self.cad_ui.settings = true;
        self.cad_ui.open = false;
        self.drawings_ui.open = false;
        self.cad_ui.preview = None;
        self.cad_ui.update_source = None;
        self.cad_ui.draft = None;
        if !self.cad_ui.converter.is_empty() {
            self.source_ui.converter_path = self.cad_ui.converter.clone();
            if let Some(root) = crate::platform::config_dir() {
                if let Err(e) = std::fs::create_dir_all(&root)
                    .and_then(|_| std::fs::write(root.join("dwg2dxf-path"), &self.cad_ui.converter))
                {
                    self.cad_ui.error = format!("Converter preference could not be saved: {e}");
                }
            }
        }
        self.open_worksheet(worksheet);
        Ok(())
    }
    pub(super) fn begin_external_drawing(&mut self) {
        self.drawings_ui.cancel();
        self.cad_ui.pending = None;
        self.cad_ui.preview = None;
        self.cad_ui.open = false;
        self.cad_ui.update_source = None;
        self.cad_ui.alignment_reviewed = false;
        self.cad_ui.unit_confirmed = false;
        self.cad_ui.partial = false;
        self.cad_ui.error.clear();
        self.cad_ui.settings = false;
        self.cad_ui.converter = self.cad_configured_converter();
        self.drawings_ui.open = true;
    }
    pub(super) fn preview_external_drawing(&mut self) -> Result<(), String> {
        let path = PathBuf::from(self.drawings_ui.path.trim());
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        match ext.as_str() {
            "dwg" | "dxf" => {
                self.cad_ui.path = self.drawings_ui.path.clone();
                self.cad_start()
            }
            "png" | "jpg" | "jpeg" => self.import_drawing(),
            _ => Err("Choose PNG, JPEG, DWG or ASCII DXF".into()),
        }
    }
    pub(super) fn cad_manager_controls(&mut self, ui: &mut Ui) {
        let lang = self.language;
        let entries: Vec<_> = self
            .host
            .document
            .editor
            .project
            .drawings
            .cad_underlays
            .iter()
            .filter_map(|u| {
                self.host
                    .document
                    .editor
                    .project
                    .drawings
                    .cad_sources
                    .iter()
                    .find(|s| s.id == u.source)
                    .map(|s| (u.id, u.worksheet, s.name.clone(), s.load_error.clone()))
            })
            .collect();
        for (id, worksheet, name, error) in entries {
            ui.horizontal_wrapped(|ui| {
                ui.label(format!("#{id} {name}"));
                if ui.button(lang.translate("Open worksheet")).clicked() {
                    self.open_worksheet(worksheet);
                }
                if ui
                    .button(lang.translate("CAD reference settings"))
                    .clicked()
                {
                    self.cad_ui.selected = Some(id);
                    self.cad_ui.draft = None;
                    self.cad_ui.settings = true;
                }
            });
            if let Some(e) = error {
                ui.colored_label(Color32::RED, e);
            }
        }
    }
    fn cad_transform_ui(ui: &mut Ui, language: UiLanguage, t: &mut Transform, enabled: bool) {
        ui.add_enabled_ui(enabled, |ui| {
            egui::Grid::new("cad_transform")
                .num_columns(2)
                .show(ui, |ui| {
                    let [anchor_x, anchor_y] = &mut t.anchor;
                    for (label, value) in [
                        ("Native anchor X", anchor_x),
                        ("Native anchor Y", anchor_y),
                        ("Metres per drawing unit", &mut t.metres_per_unit),
                        ("Model X (m)", &mut t.xy.x),
                        ("Model Y (m)", &mut t.xy.y),
                        ("Rotation (°)", &mut t.angle),
                        ("Elevation (m)", &mut t.elevation),
                    ] {
                        let label = ui.label(language.translate(label));
                        ui.add(DragValue::new(value).speed(0.01).max_decimals(9))
                            .labelled_by(label.id);
                        ui.end_row();
                    }
                });
        });
    }
    pub(super) fn cad_window(&mut self, ctx: &Context) {
        if let Some((base, worksheet, rx)) = &self.cad_ui.pending {
            match rx.try_recv() {
                Ok(result) => {
                    let (base, worksheet) = (*base, *worksheet);
                    self.cad_ui.pending = None;
                    if base != self.host.context()
                        || (self.cad_ui.update_source.is_none()
                            && worksheet
                                != self
                                    .host
                                    .document
                                    .editor
                                    .project
                                    .plan_options
                                    .active_worksheet)
                    {
                        self.cad_ui.preview = None;
                        self.cad_ui.thumbnail = None;
                        self.cad_ui.error =
                            "Drawing preview is stale; preview the file again".into();
                        return;
                    }
                    match result {
                        Ok((source, image)) => {
                            self.cad_ui.thumbnail = Some(ctx.load_texture(
                                format!("cad-preview-{}", source.parsed.sha256),
                                ColorImage::from_rgba_unmultiplied(
                                    [image.width() as usize, image.height() as usize],
                                    image.as_raw(),
                                ),
                                TextureOptions::LINEAR,
                            ));
                            self.cad_ui.transform = Transform {
                                metres_per_unit: cad::unit_scale(source.report.units).unwrap_or(1.),
                                ..Default::default()
                            };
                            self.cad_ui.unit_confirmed = false;
                            self.cad_ui.preview = Some((base, worksheet, source));
                        }
                        Err(e) => self.cad_ui.error = e,
                    }
                }
                Err(TryRecvError::Empty) => {
                    ctx.request_repaint_after(std::time::Duration::from_millis(50))
                }
                Err(TryRecvError::Disconnected) => {
                    self.cad_ui.pending = None;
                    self.cad_ui.error = "CAD worker stopped".into();
                }
            }
        }
        let lang = self.language;
        if self.cad_ui.open {
            // Source replacement rejoins the same external-drawing window.
            self.cad_ui.open = false;
            self.drawings_ui.open = true;
            self.drawings_ui.path = self.cad_ui.path.clone();
        }
        self.external_drawing_import_window(ctx);
        if self.cad_ui.settings
            && let Some(id) = self.cad_ui.selected
        {
            let current = self
                .host
                .document
                .editor
                .project
                .drawings
                .cad_underlays
                .iter()
                .find(|u| u.id == id)
                .cloned();
            if let Some(current) = current {
                if self.cad_ui.draft.as_ref().is_none_or(|d| d.id != id) {
                    self.cad_ui.draft = Some(current.clone());
                    self.cad_ui.settings_base = Some(current.clone());
                }
                let mut open = true;
                Window::new(lang.translate("CAD reference settings")).id(Id::new("cad_settings")).open(&mut open).default_width(470.).collapsible(false).show(ctx,|ui|{
                    let Some(source)=self.host.document.editor.project.drawings.cad_sources.iter().find(|s|s.id==current.source).cloned()else{return};
                    ui.set_width(470.);
                    ScrollArea::vertical().id_salt("cad_settings_body").max_height((ctx.content_rect().height()-230.).clamp(150.,520.)).show(ui,|ui|{
                        let d=self.cad_ui.draft.as_mut().unwrap();ui.strong(&source.name);
                        ui.checkbox(&mut d.visible,lang.translate("Visible"));
                        ui.checkbox(&mut d.show_3d,lang.translate("Show reference in 3D"));
                        ui.checkbox(&mut d.show_plan,lang.translate("Show reference on model plan"));
                        ui.checkbox(&mut d.xray,lang.translate("Show through model (X-ray)"));
                        ui.checkbox(&mut d.locked,lang.translate("Lock placement"));
                        ui.add(Slider::new(&mut d.fade,0..=220).text(lang.translate("Color fade")));
                        Self::cad_transform_ui(ui,lang,&mut d.transform,!d.locked);
                        ui.label(lang.translate("3D preview: depth-tested vector paths. Text and hatch fill in 3D are pending."));
                        if let Some(data)=&source.data {let layers:BTreeSet<_>=data.primitives.iter().map(|p|p.layer.clone()).collect();ui.collapsing(lang.translate("Layers"),|ui|{for layer in layers{let mut visible=!d.hidden_layers.contains(&layer);if ui.checkbox(&mut visible,&layer).changed(){if visible{d.hidden_layers.remove(&layer);}else{d.hidden_layers.insert(layer);}}}});}
                        Self::cad_report(ui,lang,&source);
                        if ui.button(lang.translate("Update CAD source…")).clicked(){self.cad_ui.path=source.linked_path.display().to_string();self.cad_ui.update_source=Some(source.id);self.cad_ui.alignment_reviewed=false;self.cad_ui.preview=None;self.cad_ui.open=true;self.cad_ui.settings=false;self.cad_ui.converter=self.cad_configured_converter();}
                    });
                    if !self.cad_ui.error.is_empty(){ui.colored_label(Color32::RED,&self.cad_ui.error);}
                    ui.separator();
                    ui.horizontal_wrapped(|ui|{
                        if ui.button(lang.translate("Apply CAD settings")).clicked(){
                            if self.cad_ui.settings_base.as_ref()!=Some(&current){self.cad_ui.error="CAD placement changed; cancel and reopen settings".into();return}
                            let mut doc=self.host.document.editor.project.drawings.clone();
                            *doc.cad_underlays.iter_mut().find(|u|u.id==id).unwrap()=self.cad_ui.draft.clone().unwrap();
                            if let Err(e)=self.set_drawings(doc){self.cad_ui.error=e;}else{self.cad_ui.settings_base=self.cad_ui.draft.clone();}
                        }
                        if ui.button(lang.translate("Open worksheet")).clicked(){self.open_worksheet(current.worksheet);}
                        if ui.button(lang.translate("View in 3D")).clicked(){self.host.document.editor.switch_worksheet(None);self.view=View::Model;self.fit();}
                        if ui.button(lang.translate("Cancel")).clicked(){self.cad_ui.draft=None;self.cad_ui.settings=false;}
                    });
                });
                self.cad_ui.settings &= open;
            }
        }
    }
    fn external_drawing_import_window(&mut self, ctx: &Context) {
        if !self.drawings_ui.open {
            return;
        }
        let lang = self.language;
        let mut open = true;
        let width = (ctx.content_rect().width() - 80.).clamp(300., 660.);
        Window::new(lang.translate("Place External Drawing…")).id(Id::new("drawing_import"))
            .open(&mut open).default_width(width).resizable(false).collapsible(false)
            .show(ctx,|ui| {
                ui.set_width(width);
                ui.label(lang.translate("Choose an image or DWG/DXF. Images are placed on the active 2D canvas; CAD requires preview confirmation."));
                ui.horizontal(|ui| {
                    let label=ui.label(lang.translate("Drawing file"));
                    if ui.text_edit_singleline(&mut self.drawings_ui.path).labelled_by(label.id).changed() {
                        self.cad_ui.preview=None;self.cad_ui.thumbnail=None;self.cad_ui.pending=None;self.cad_ui.unit_confirmed=false;self.cad_ui.partial=false;
                    }
                    if ui.button(lang.translate("Browse…")).clicked() {
                        if let Some(path)=rfd::FileDialog::new().add_filter("PNG / JPEG / DWG / DXF", &["png","jpg","jpeg","dwg","dxf"]).pick_file() {
                            self.drawings_ui.path=path.display().to_string();self.cad_ui.preview=None;self.cad_ui.thumbnail=None;self.cad_ui.pending=None;
                            self.cad_ui.unit_confirmed=false;self.cad_ui.partial=false;
                        }
                    }
                });
                let is_dwg=std::path::Path::new(self.drawings_ui.path.trim()).extension().is_some_and(|e|e.eq_ignore_ascii_case("dwg"));
                if is_dwg {
                    if self.cad_ui.converter.is_empty(){ui.label(lang.translate("DWG converter missing; choose a local dwg2dxf converter."));}
                    ui.collapsing(lang.translate("DWG converter"),|ui| {
                        let label=ui.label(lang.translate("Local dwg2dxf executable"));
                        ui.text_edit_singleline(&mut self.cad_ui.converter).labelled_by(label.id);
                        if ui.button(lang.translate("Choose converter")).clicked()&&let Some(p)=rfd::FileDialog::new().pick_file(){self.cad_ui.converter=p.display().to_string();}
                    });
                }
                let preview=self.cad_ui.preview.as_ref().map(|(_,_,s)|s.clone());
                if let Some(source)=preview {
                    ScrollArea::vertical().id_salt("external_drawing_preview").max_height((ctx.content_rect().height()-390.).clamp(100.,460.)).show(ui,|ui| {
                        ui.label(format!("{} · {} {}",source.name,source.report.rendered_entities,lang.translate("reference entities")));
                        ui.label(format!("{}: {:?} · INSUNITS {:?}",lang.translate("Native bounds"),source.report.bounds,source.report.units));
                        if let Some(texture)=&self.cad_ui.thumbnail {ui.image((texture.id(),vec2((width-20.).max(200.),180.)));}
                        if let Some(id)=self.cad_ui.update_source {
                            if let Some(old)=self.host.document.editor.project.drawings.cad_sources.iter().find(|s|s.id==id){ui.label(format!("{}: {:?} / {:?} → {:?} / {:?}",lang.translate("Source units / bounds changed"),old.report.units,old.report.bounds,source.report.units,source.report.bounds));if old.report.text_encoding!=source.report.text_encoding {ui.label(lang.translate("Source text decoding changed; review labels and hidden layers"));}}
                        }else {
                            if ui.button(lang.translate("Align drawing center to model origin")).clicked()&&let Some(b)=source.report.bounds {self.cad_ui.transform.anchor=[(b[0]+b[2])*0.5,(b[1]+b[3])*0.5];self.cad_ui.transform.xy=Point::default();}
                            Self::cad_transform_ui(ui,lang,&mut self.cad_ui.transform,true);
                        }
                        Self::cad_report(ui,lang,&source);
                    });
                    ui.separator();
                    if self.cad_ui.update_source.is_some(){ui.checkbox(&mut self.cad_ui.alignment_reviewed,lang.translate("I reviewed alignment changes; preserve all current placements"));}
                    ui.checkbox(&mut self.cad_ui.unit_confirmed,lang.translate("I verified the drawing units and scale"));
                    ui.checkbox(&mut self.cad_ui.partial,lang.translate("Import supported geometry with the reported losses"));
                }
                if self.cad_ui.pending.is_some(){ui.horizontal(|ui|{ui.spinner();ui.label(lang.translate("Converting / parsing in background…"));});}
                if !self.cad_ui.error.is_empty(){ui.colored_label(Color32::RED,lang.translate(&self.cad_ui.error));}
                ui.separator();
                ui.horizontal(|ui| {
                    if self.cad_ui.preview.is_some() {
                        let ready=self.cad_ui.unit_confirmed&&self.cad_ui.partial&&(self.cad_ui.update_source.is_none()||self.cad_ui.alignment_reviewed);
                        let response=ui.add_enabled(ready,Button::new(lang.translate("Confirm CAD import")));
                        #[cfg(test)] self.cad_ui.rects.insert("Confirm CAD import".into(),response.rect);
                        if response.clicked()&&let Err(e)=self.cad_confirm(){self.cad_ui.error=e;}
                    }else {
                        let place=ui.add_enabled(self.cad_ui.pending.is_none(),Button::new(lang.translate("Place drawing")));
                        #[cfg(test)] self.drawings_ui.rects.insert("Place drawing".into(),place.rect);
                        if place.clicked()&&let Err(e)=self.preview_external_drawing(){self.cad_ui.error=e;}
                    }
                    let cancel=ui.button(lang.translate("Cancel"));
                    #[cfg(test)] self.cad_ui.rects.insert("Cancel".into(),cancel.rect);
                    if cancel.clicked(){self.close_external_drawing();}
                });
            });
        if !open {
            self.close_external_drawing();
        }
    }
    fn close_external_drawing(&mut self) {
        self.drawings_ui.open = false;
        self.cad_ui.open = false;
        self.cad_ui.preview = None;
        self.cad_ui.thumbnail = None;
        self.cad_ui.pending = None;
        self.cad_ui.unit_confirmed = false;
        self.cad_ui.partial = false;
        self.cad_ui.error.clear();
        self.cad_ui.update_source = None;
    }
    fn cad_report(ui: &mut Ui, lang: UiLanguage, source: &Source) {
        if let Some(e) = &source.load_error {
            ui.colored_label(Color32::RED, e);
        }
        ui.collapsing(lang.translate("CAD import quality report"), |ui| {
            ui.label(lang.translate("Reference display is monochrome; original colors and line patterns are not reproduced"));
            for w in &source.report.warnings {
                ui.label(lang.translate(w));
            }
            for (kind, n) in &source.report.omitted {
                ui.label(format!("{kind}: {n} {}", lang.translate("omitted")));
            }
            ui.label(format!(
                "parse {:.1} ms · conversion/cache {:.1} ms",
                source.report.parse_ms, source.report.conversion_ms
            ));
            ui.collapsing(lang.translate("Conversion diagnostics"), |ui| {
                ui.label(&source.report.diagnostics);
            });
        });
    }
    pub(super) fn paint_cad_underlays(&self, painter: &Painter, viewport: Rect) {
        self.paint_cad_underlays_for(painter, viewport, None);
    }
    pub(super) fn paint_cad_underlays_for(
        &self,
        painter: &Painter,
        viewport: Rect,
        reference: Option<u64>,
    ) {
        let worksheet = reference.or(self
            .host
            .document
            .editor
            .project
            .plan_options
            .active_worksheet);
        let pivot = reference.map(|id| self.trace_worksheet_pivot(id));
        let corners = [
            viewport.left_top(),
            viewport.right_top(),
            viewport.left_bottom(),
            viewport.right_bottom(),
        ]
        .map(|p| self.world(viewport, p));
        for u in &self.host.document.editor.project.drawings.cad_underlays {
            if !u.visible_in_plan(worksheet) {
                continue;
            }
            let Some(data) = self
                .host
                .document
                .editor
                .project
                .drawings
                .cad_sources
                .iter()
                .find(|s| s.id == u.source)
                .and_then(|s| s.data.as_ref())
            else {
                continue;
            };
            let transform = pivot.map_or_else(
                || u.transform.clone(),
                |p| self.trace.cad_transform(&u.transform, p),
            );
            let native = corners.map(|p| transform.native(p));
            let b = cad::bounds(&native).unwrap();
            let color = if reference.is_some() {
                let rgb = self.trace.reference_color.unwrap_or([u.grey(); 3]);
                Color32::from_rgba_unmultiplied(
                    rgb[0],
                    rgb[1],
                    rgb[2],
                    self.trace.reference_opacity,
                )
            } else {
                Color32::from_gray(u.grey())
            };
            let projection = CadPlanProjection::new(
                &transform,
                self.plan_center,
                self.plan_scale,
                self.host
                    .document
                    .editor
                    .project
                    .plan_options
                    .normalized_orientation(),
                viewport,
            );
            let max_error = 0.35 / (self.plan_scale * transform.metres_per_unit);
            for prim in data.visible(b) {
                if u.hidden_layers.contains(&prim.layer) {
                    continue;
                }
                if let Some(text) = &prim.text {
                    let pos = projection.screen(prim.points[0]);
                    let size = (text.height * transform.metres_per_unit * self.plan_scale) as f32;
                    if size < 3. {
                        continue;
                    }
                    let galley = painter.layout_no_wrap(
                        text.value.clone(),
                        FontId::proportional(size.clamp(3., 256.)),
                        color,
                    );
                    let mut shape = egui::epaint::TextShape::new(pos, galley, color);
                    shape.angle = -(text.angle
                        + transform.angle
                        + self
                            .host
                            .document
                            .editor
                            .project
                            .plan_options
                            .normalized_orientation()) as f32
                        * std::f32::consts::PI
                        / 180.;
                    painter.add(shape);
                } else {
                    let points: Vec<_> = prim
                        .display_points(max_error)
                        .map(|p| projection.screen(p))
                        .collect();
                    if points.len() > 1 {
                        painter.add(egui::Shape::line(points, Stroke::new(0.8, color)));
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn frame(ctx: &Context, app: &mut AtelierApp, events: Vec<Event>) -> egui::FullOutput {
        let mut output = ctx.run_ui(
            RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1500., 1200.))),
                events,
                ..Default::default()
            },
            |ui| app.workspace(ui),
        );
        output.textures_delta.clear();
        output
    }
    fn text(shapes: &[egui::epaint::ClippedShape], label: &str) -> Option<Pos2> {
        fn at(s: &egui::Shape, label: &str) -> Option<Pos2> {
            match s {
                egui::Shape::Text(t) if t.galley.job.text == label => {
                    Some(t.pos + t.galley.rect.center().to_vec2())
                }
                egui::Shape::Vec(v) => v.iter().find_map(|s| at(s, label)),
                _ => None,
            }
        }
        shapes.iter().find_map(|s| at(&s.shape, label))
    }
    fn click(ctx: &Context, app: &mut AtelierApp, label: &str) {
        let mut out = frame(ctx, app, vec![]);
        for _ in 0..3 {
            if text(&out.shapes, label).is_some() {
                break;
            }
            out = frame(ctx, app, vec![]);
        }
        let pos = text(&out.shapes, label).unwrap_or_else(|| {
            let labels: Vec<_> = out
                .shapes
                .iter()
                .filter_map(|s| {
                    if let egui::Shape::Text(t) = &s.shape {
                        Some(t.galley.job.text.clone())
                    } else {
                        None
                    }
                })
                .collect();
            panic!(
                "missing button {label}; settings={} selected={:?}; labels={labels:?}",
                app.cad_ui.settings, app.cad_ui.selected
            )
        });
        for pressed in [true, false] {
            frame(
                ctx,
                app,
                vec![
                    Event::PointerMoved(pos),
                    Event::PointerButton {
                        pos,
                        button: PointerButton::Primary,
                        pressed,
                        modifiers: Modifiers::NONE,
                    },
                ],
            );
        }
    }
    #[test]
    fn external_drawing_common_menu_cad_preview_cancel_confirm_and_no_worksheet_toolbar_bilingual()
    {
        for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
            let ctx = Context::default();
            AtelierApp::style(&ctx);
            let mut app = AtelierApp::from_renderer(None, "CAD test".into());
            app.recovery_slot = None;
            app.host.document.editor = Editor::new(Project::default());
            app.language = language;
            app.in_window_menus = true;
            app.new_worksheet().unwrap();
            let initial = app.host.document.editor.project.clone();
            let history = app.host.document.editor.undo.len();
            let root = std::env::temp_dir()
                .join(format!("oa-cad-ui-{}", storage::new_document_identity()));
            std::fs::create_dir(&root).unwrap();
            app.drawings_ui.cache_override = Some(root.join("cache"));
            let path = root.join("underlay.dxf");
            std::fs::write(&path,"0\nSECTION\n2\nHEADER\n9\n$INSUNITS\n70\n6\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n0\nLINE\n5\nA\n8\nOutline\n10\n10\n20\n20\n11\n15\n21\n20\n0\nENDSEC\n0\nEOF\n").unwrap();
            for _ in 0..3 {
                frame(&ctx, &mut app, vec![]);
            }
            let output = frame(&ctx, &mut app, vec![]);
            for absent in [
                "Import DWG / DXF…",
                "Manual redraw",
                "Adjust source scale",
                "Adjust redraw area",
                "Drawing settings / AI…",
                "Return to model",
            ] {
                assert!(
                    text(&output.shapes, language.translate(absent)).is_none(),
                    "worksheet toolbar must be absent: {absent}"
                );
            }
            click(&ctx, &mut app, language.translate("File"));
            click(&ctx, &mut app, language.translate("External Content"));
            click(
                &ctx,
                &mut app,
                language.translate("Place External Drawing…"),
            );
            assert!(app.drawings_ui.open);
            app.drawings_ui.path = path.display().to_string();
            click(&ctx, &mut app, language.translate("Place drawing"));
            for _ in 0..200 {
                frame(&ctx, &mut app, vec![]);
                if app.cad_ui.preview.is_some() {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            assert!(app.cad_ui.preview.is_some(), "{}", app.cad_ui.error);
            assert_eq!(app.host.document.editor.project, initial);
            assert_eq!(app.host.document.editor.undo.len(), history);
            click(&ctx, &mut app, language.translate("Cancel"));
            assert!(app.cad_ui.preview.is_none());
            assert_eq!(app.host.document.editor.project, initial);
            assert!(app.external_content_command("Place External Drawing…"));
            click(&ctx, &mut app, language.translate("Place drawing"));
            for _ in 0..200 {
                frame(&ctx, &mut app, vec![]);
                if app.cad_ui.preview.is_some() {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            click(
                &ctx,
                &mut app,
                language.translate("I verified the drawing units and scale"),
            );
            click(
                &ctx,
                &mut app,
                language.translate("Import supported geometry with the reported losses"),
            );
            click(&ctx, &mut app, language.translate("Confirm CAD import"));
            assert_eq!(
                app.host
                    .document
                    .editor
                    .project
                    .drawings
                    .cad_underlays
                    .len(),
                1,
                "{}",
                app.cad_ui.error
            );
            assert_eq!(app.host.document.editor.project.elements, initial.elements);
            click(&ctx, &mut app, language.translate("Show reference in 3D"));
            click(&ctx, &mut app, language.translate("Apply CAD settings"));
            assert!(app.host.document.editor.project.drawings.cad_underlays[0].show_3d);
            app.host.document.editor.undo();
            assert!(!app.host.document.editor.project.drawings.cad_underlays[0].show_3d);
            app.host.document.editor.redo();
            assert!(app.host.document.editor.project.drawings.cad_underlays[0].show_3d);
            assert!(
                cad::snap(
                    &app.host.document.editor.project.drawings,
                    None,
                    Point::new(10., 20.),
                    0.01
                )
                .is_none(),
                "3D visibility alone must not attract plan input"
            );
            click(
                &ctx,
                &mut app,
                language.translate("Show reference on model plan"),
            );
            click(&ctx, &mut app, language.translate("Apply CAD settings"));
            app.host.document.editor.switch_worksheet(None);
            app.plan_center = Point::new(12.5, 20.);
            app.plan_scale = 50.;
            let output = frame(&ctx, &mut app, vec![]);
            assert!(output.shapes.iter().any(|s|matches!(&s.shape,egui::Shape::Path(p) if (p.stroke.width-0.8).abs()<0.001&&p.stroke.color==egui::epaint::ColorMode::Solid(Color32::from_gray(100)))),"enabled reference must be drawn on model plan");
            let (point, kind) = geometry::snap(
                &app.host.document.editor.project,
                0,
                Point::new(10.001, 20.),
                0.01,
                true,
            );
            assert_eq!(point, Point::new(10., 20.));
            assert_eq!(kind, geometry::SnapKind::Endpoint);
            let project = root.join("project.json");
            storage::save(&app.host.document.editor.project, &project).unwrap();
            let reopened = storage::load(&project).unwrap();
            assert!(reopened.drawings.cad_sources[0].data.is_some());
            assert!(reopened.drawings.cad_underlays[0].show_3d);
            assert_eq!(reopened.elements, initial.elements);
            std::fs::remove_dir_all(root).unwrap();
        }
    }
    #[test]
    fn external_drawing_pending_cancel_and_stale_targets_never_commit() {
        let ctx = Context::default();
        AtelierApp::style(&ctx);
        let mut app = AtelierApp::from_renderer(None, "late preview".into());
        app.recovery_slot = None;
        app.language = UiLanguage::English;
        app.host.document.editor = Editor::new(Project::default());
        let root =
            std::env::temp_dir().join(format!("oa-cad-late-{}", storage::new_document_identity()));
        std::fs::create_dir(&root).unwrap();
        let path = root.join("line.dxf");
        std::fs::write(&path,"0\nSECTION\n2\nENTITIES\n0\nLINE\n5\nA\n8\nOutline\n10\n0\n20\n0\n11\n10\n21\n0\n0\nENDSEC\n0\nEOF\n").unwrap();
        let source = cad::prepare(&path, None, &root.join("cache")).unwrap();
        let image = cad::thumbnail(source.data.as_ref().unwrap(), 100, 100);
        app.begin_external_drawing();
        let (tx, rx) = mpsc::channel();
        app.cad_ui.pending = Some((app.host.context(), None, rx));
        let before = app.host.document.editor.project.clone();
        click(&ctx, &mut app, "Cancel");
        assert!(tx.send(Ok((source.clone(), image.clone()))).is_err());
        frame(&ctx, &mut app, vec![]);
        assert_eq!(app.host.document.editor.project, before);
        assert!(app.cad_ui.preview.is_none());
        app.begin_external_drawing();
        let (tx, rx) = mpsc::channel();
        app.cad_ui.pending = Some((app.host.context(), None, rx));
        app.new_worksheet().unwrap();
        let after = app.host.document.editor.project.clone();
        tx.send(Ok((source.clone(), image.clone()))).unwrap();
        frame(&ctx, &mut app, vec![]);
        assert!(app.cad_ui.preview.is_none());
        assert_eq!(app.host.document.editor.project, after);
        // Switching between already existing worksheets does not change model revision.
        app.new_worksheet().unwrap();
        let worksheet = app
            .host
            .document
            .editor
            .project
            .plan_options
            .active_worksheet;
        app.cad_ui.preview = Some((app.host.context(), worksheet, source));
        app.cad_ui.unit_confirmed = true;
        app.cad_ui.partial = true;
        let revision = app.host.context().revision;
        app.host.document.editor.switch_worksheet(None);
        assert_eq!(app.host.context().revision, revision);
        let protected = app.host.document.editor.project.clone();
        assert!(app.cad_confirm().is_err());
        assert_eq!(app.host.document.editor.project, protected);
        assert!(
            app.host
                .document
                .editor
                .project
                .drawings
                .cad_sources
                .is_empty()
        );
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    #[ignore = "explicit real CAD project path required; records CPU frame timings, not GPU/native acceptance"]
    fn external_drawing_recovery_navigation_measurement() {
        let path = PathBuf::from(std::env::var("OPENARCHI_CAD_FRAME_PROJECT").unwrap());
        let output = PathBuf::from(std::env::var("OPENARCHI_CAD_FRAME_OUTPUT").unwrap());
        let root = output.parent().unwrap().join(format!(
            "recovery-measure-{}",
            storage::new_document_identity()
        ));
        let mut rows = vec![];
        for view in [View::Plan, View::Model] {
            for recovery in [false, true] {
                let ctx = Context::default();
                AtelierApp::style(&ctx);
                crate::fonts::install_cjk(&ctx);
                let mut app = AtelierApp::from_renderer(None, "Recovery measurement".into());
                app.host.document.editor = Editor::from_stored_document(&path).unwrap();
                app.reset_worksheet_session();
                app.view = view;
                app.host
                    .document
                    .editor
                    .project
                    .name
                    .push_str(" · unsaved measurement");
                if recovery {
                    app.initialize_recovery(&root.join(storage::new_document_identity()));
                }
                for _ in 0..3 {
                    frame(&ctx, &mut app, vec![]);
                }
                app.finish_recovery_write(true);
                let start = app.workbench.panel_rects["view.canvas"].center();
                frame(
                    &ctx,
                    &mut app,
                    vec![
                        Event::PointerMoved(start),
                        Event::PointerButton {
                            pos: start,
                            button: PointerButton::Secondary,
                            pressed: true,
                            modifiers: Modifiers::NONE,
                        },
                    ],
                );
                let mut elapsed = vec![];
                let mut writes = 0;
                let mut previous = app.last_recovery_save;
                for n in 1..=15 {
                    let t = std::time::Instant::now();
                    let o = frame(
                        &ctx,
                        &mut app,
                        vec![Event::PointerMoved(
                            start + vec2(n as f32 * 3., n as f32 * 2.),
                        )],
                    );
                    let _ = ctx.tessellate(o.shapes, o.pixels_per_point);
                    elapsed.push(t.elapsed().as_secs_f64() * 1000.);
                    if app.last_recovery_save != previous {
                        writes += 1;
                        previous = app.last_recovery_save;
                    }
                }
                let mut release_writes = 0;
                for events in [
                    vec![Event::PointerButton {
                        pos: start + vec2(45., 30.),
                        button: PointerButton::Secondary,
                        pressed: false,
                        modifiers: Modifiers::NONE,
                    }],
                    vec![],
                    vec![],
                ] {
                    frame(&ctx, &mut app, events);
                    if app.last_recovery_save != previous {
                        release_writes += 1;
                        previous = app.last_recovery_save;
                    }
                }
                app.finish_recovery_write(true);
                assert!(app.recovery_error.is_empty(), "{}", app.recovery_error);
                if recovery {
                    let recovered = app.recovery_slot.as_ref().unwrap().read().unwrap().unwrap();
                    assert_eq!(
                        recovered.elements,
                        app.host.document.editor.project.elements
                    );
                    assert_eq!(recovered.trace.viewport.unwrap().center, app.plan_center);
                }
                elapsed.sort_by(f64::total_cmp);
                rows.push(serde_json::json!({"view":format!("{view:?}"),"recovery_enabled":recovery,"drag_frame_median_ms":elapsed[7],"drag_frame_max_ms":elapsed[14],"drag_frames":15,"recovery_write_attempts_during_drag":writes,"recovery_write_attempts_after_release":release_writes,"scope":"UI workspace input + CPU tessellation; recovery enabled in background and independently read after completion; no GPU timing"}));
            }
        }
        std::fs::write(&output, serde_json::to_vec_pretty(&rows).unwrap()).unwrap();
        println!("{}", serde_json::to_string_pretty(&rows).unwrap());
    }

    #[test]
    #[ignore = "explicit real CAD project path required; records CPU frame timings, not GPU/native acceptance"]
    fn external_drawing_real_project_frame_measurement() {
        let path = std::env::var("OPENARCHI_CAD_FRAME_PROJECT")
            .expect("set an explicit private test project");
        let output =
            std::env::var("OPENARCHI_CAD_FRAME_OUTPUT").expect("set explicit measurement output");
        let ctx = Context::default();
        AtelierApp::style(&ctx);
        crate::fonts::install_cjk(&ctx);
        let mut app = AtelierApp::from_renderer(None, "CAD frame measurement".into());
        app.recovery_slot = None;
        app.language = UiLanguage::English;
        app.host.document.editor =
            Editor::from_stored_document(PathBuf::from(&path).as_path()).unwrap();
        let saved_viewport = app.host.document.editor.project.trace.viewport.clone();
        let ids: Vec<_> = app
            .host
            .document
            .editor
            .project
            .drawings
            .cad_underlays
            .iter()
            .map(|u| u.worksheet)
            .collect();
        let mut rows = vec![];
        for worksheet in ids {
            app.open_worksheet(worksheet);
            app.cad_ui.settings = false;
            if std::env::var_os("OPENARCHI_CAD_FRAME_TRACE").is_some() {
                app.host.document.editor.switch_worksheet(None);
                for u in &mut app.host.document.editor.project.drawings.cad_underlays {
                    u.show_plan = false;
                }
                app.choose_trace_reference(crate::trace::TraceReference::Worksheet(worksheet));
            }
            if std::env::var_os("OPENARCHI_CAD_FRAME_SAVED_VIEW").is_some()
                && let Some(viewport) = &saved_viewport
            {
                app.plan_center = viewport.center;
                app.plan_scale = viewport.scale;
            }
            let start = std::time::Instant::now();
            let output = frame(&ctx, &mut app, vec![]);
            let first_frame_ms = start.elapsed().as_secs_f64() * 1000.;
            let _ = ctx.tessellate(output.shapes, output.pixels_per_point);
            for _ in 0..3 {
                let o = frame(&ctx, &mut app, vec![]);
                let _ = ctx.tessellate(o.shapes, o.pixels_per_point);
            }
            let mut elapsed = vec![];
            let mut meshes = vec![];
            for _ in 0..15 {
                app.plan_center.x += 2. / app.plan_scale;
                app.plan_scale *= 1.002;
                let start = std::time::Instant::now();
                let o = frame(&ctx, &mut app, vec![]);
                let clipped = ctx.tessellate(o.shapes, o.pixels_per_point);
                elapsed.push(start.elapsed().as_secs_f64() * 1000.);
                meshes.push(
                    clipped
                        .iter()
                        .map(|p| {
                            if let egui::epaint::Primitive::Mesh(m) = &p.primitive {
                                m.vertices.len()
                            } else {
                                0
                            }
                        })
                        .sum::<usize>(),
                );
            }
            let mut rotated = vec![];
            for i in 0..15 {
                app.host
                    .document
                    .editor
                    .project
                    .plan_options
                    .orientation_degrees = i as f64 * 0.2;
                let start = std::time::Instant::now();
                let o = frame(&ctx, &mut app, vec![]);
                let _ = ctx.tessellate(o.shapes, o.pixels_per_point);
                rotated.push(start.elapsed().as_secs_f64() * 1000.);
            }
            app.host
                .document
                .editor
                .project
                .plan_options
                .orientation_degrees = 0.;
            rotated.sort_by(f64::total_cmp);
            elapsed.sort_by(f64::total_cmp);
            rows.push(serde_json::json!({"worksheet":worksheet,"first_frame_assembly_ms":first_frame_ms,"navigation_cpu_p95_ms":elapsed[14],"rotation_cpu_p95_ms":rotated[14],"navigation_cpu_median_ms":elapsed[7],"max_tessellated_vertices":meshes.into_iter().max(),"sample_count":15,"scope":"headless CPU layout + tessellation; GPU/native timing not covered"}));
        }
        std::fs::write(output, serde_json::to_vec_pretty(&rows).unwrap()).unwrap();
        println!("{}", serde_json::to_string_pretty(&rows).unwrap());
    }
    #[test]
    fn cad_projection_matches_shared_world_transform_for_rotated_survey_coordinates() {
        let ctx = Context::default();
        let mut app = AtelierApp::from_renderer(None, "projection".into());
        app.recovery_slot = None;
        let viewport = Rect::from_min_size(pos2(20., 30.), vec2(1200., 700.));
        for angle in [-180., -37., 17., 190.] {
            for orientation in [-45., 0., 31.] {
                let t = Transform {
                    anchor: [531223., 4082454.],
                    metres_per_unit: 0.001,
                    xy: Point::new(123.2, -77.9),
                    angle,
                    elevation: 2.4,
                };
                app.plan_center = Point::new(120., -80.);
                app.plan_scale = 87.;
                app.host
                    .document
                    .editor
                    .project
                    .plan_options
                    .orientation_degrees = orientation;
                let projection = CadPlanProjection::new(
                    &t,
                    app.plan_center,
                    app.plan_scale,
                    orientation,
                    viewport,
                );
                for point in [[531223.341, 4082454.055], [531224., 4082456.]] {
                    let expected = app.screen(viewport, t.world(point));
                    assert!(expected.distance(projection.screen(point)) < 0.001);
                }
            }
        }
    }
    fn assert_trace_model_unchanged(actual: &Project, expected: &Project) {
        let mut actual = actual.clone();
        actual.trace = expected.trace.clone();
        assert!(
            actual == *expected,
            "Trace must preserve model geometry, assets, options and placements"
        );
    }
    fn trace_fixture(language: UiLanguage) -> (Context, AtelierApp, PathBuf, u64) {
        let ctx = Context::default();
        AtelierApp::style(&ctx);
        let mut app = AtelierApp::from_renderer(None, "Trace DWG".into());
        app.recovery_slot = None;
        app.host.document.editor = Editor::new(Project::default());
        app.language = language;
        app.in_window_menus = true;
        let root =
            std::env::temp_dir().join(format!("oa-trace-dwg-{}", storage::new_document_identity()));
        std::fs::create_dir(&root).unwrap();
        let path = root.join("survey.dxf");
        std::fs::write(&path,"0\nSECTION\n2\nENTITIES\n0\nLINE\n5\nA\n8\nSurvey\n10\n50000\n20\n70000\n11\n53000\n21\n71000\n0\nTEXT\n5\nB\n8\nLabels\n10\n51000\n20\n70000\n40\n300\n1\nSurvey\n0\nENDSEC\n0\nEOF\n").unwrap();
        let source = cad::prepare(&path, None, &root.join("cache")).unwrap();
        let (mut document, worksheet, _) = cad::attach(
            &crate::drawings::DrawingDocument::default(),
            source,
            None,
            Transform {
                anchor: [50000., 70000.],
                metres_per_unit: 0.001,
                xy: Point::new(5., 4.),
                angle: 21.,
                elevation: 2.4,
            },
            true,
        )
        .unwrap();
        document.worksheets[0].name = "Survey drawing".into();
        document.cad_underlays[0].locked = true;
        app.set_drawings(document).unwrap();
        app.host.document.editor.mark_saved();
        app.plan_center = Point::new(6., 5.);
        app.plan_scale = 60.;
        (ctx, app, root, worksheet)
    }
    #[test]
    fn trace_navigation_reference_and_active_paint_the_same_input_frame() {
        for on_top in [false, true] {
            for orientation in [0., 31.] {
                for worksheet_active in [false, true] {
                    for button in [
                        PointerButton::Secondary,
                        PointerButton::Middle,
                        PointerButton::Primary,
                    ] {
                        let (ctx, mut app, root, worksheet) = trace_fixture(UiLanguage::English);
                        app.host.document.editor.project.drawings.cad_underlays[0]
                            .transform
                            .xy = Point::new(1_234_567., 4_567_890.);
                        app.choose_trace_reference(crate::trace::TraceReference::Worksheet(
                            worksheet,
                        ));
                        app.trace.on_top = on_top;
                        app.trace.offset = Point::new(1.7, -0.9);
                        app.trace.rotation_deg = 37.;
                        app.trace.reference_color = Some([19, 79, 173]);
                        app.trace.reference_opacity = 137;
                        app.trace.active_opacity = 255;
                        app.host
                            .document
                            .editor
                            .project
                            .plan_options
                            .orientation_degrees = orientation;
                        let doc = &app.host.document.editor.project.drawings;
                        let u = &doc.cad_underlays[0];
                        let points = doc.cad_sources[0].data.as_ref().unwrap().primitives[0]
                            .points
                            .iter()
                            .map(|p| {
                                app.trace.transform(
                                    u.transform.world(*p),
                                    app.trace_worksheet_pivot(worksheet),
                                )
                            })
                            .collect::<Vec<_>>();
                        let active_sheet = worksheet_active.then_some(900);
                        if worksheet_active {
                            app.host.document.editor.project.drawings.worksheets.push(
                                crate::drawings::Worksheet {
                                    id: 900,
                                    name: "Active drawing".into(),
                                },
                            );
                        }
                        app.host.document.editor.switch_worksheet(active_sheet);
                        app.plan_center = Point::new(
                            (points[0].x + points[1].x) / 2.,
                            (points[0].y + points[1].y) / 2.,
                        );
                        app.plan_scale = 60.;
                        let id = app.host.document.editor.project.add(
                            0,
                            Material::Concrete,
                            Shape::Polyline {
                                points,
                                arcs: vec![],
                            },
                        );
                        app.host
                            .document
                            .editor
                            .project
                            .elements
                            .iter_mut()
                            .find(|e| e.id == id)
                            .unwrap()
                            .worksheet = active_sheet;
                        app.persist_trace_state(false);
                        app.host.document.editor.mark_saved();
                        let project = app.host.document.editor.project.clone();
                        let history = app.host.document.editor.undo.len();
                        let frame = |app: &mut AtelierApp, events| {
                            let mut output = ctx.run_ui(
                                RawInput {
                                    screen_rect: Some(Rect::from_min_size(
                                        Pos2::ZERO,
                                        vec2(1000., 700.),
                                    )),
                                    focused: true,
                                    events,
                                    ..Default::default()
                                },
                                |ui| app.plan(ui),
                            );
                            output.textures_delta.clear();
                            output
                        };
                        let reference_color = Color32::from_rgba_unmultiplied(19, 79, 173, 137);
                        let assert_aligned = |out: &egui::FullOutput| {
                            let ps = paths(&out.shapes);
                            let reference = ps
                                .iter()
                                .find(|p| {
                                    p.points.len() == 2
                                        && p.stroke.color
                                            == egui::epaint::ColorMode::Solid(reference_color)
                                })
                                .expect("CAD reference line");
                            let active = ps
                                .iter()
                                .find(|p| {
                                    p.points.len() == 2
                                        && p.stroke.color
                                            != egui::epaint::ColorMode::Solid(reference_color)
                                })
                                .expect("active polyline");
                            for (a, b) in active.points.iter().zip(&reference.points) {
                                assert!(
                                    a.distance(*b) < 0.001,
                                    "same-frame alignment: active={a:?}, reference={b:?}, top={on_top}, orientation={orientation}, worksheet={worksheet_active}, button={button:?}"
                                );
                            }
                            reference.points[0]
                        };
                        let start = pos2(700., 500.);
                        let mut previous =
                            assert_aligned(&frame(&mut app, vec![Event::PointerMoved(start)]));
                        let space = Event::Key {
                            key: Key::Space,
                            physical_key: Some(Key::Space),
                            pressed: true,
                            repeat: false,
                            modifiers: Modifiers::NONE,
                        };
                        let mut press = vec![Event::PointerButton {
                            pos: start,
                            button,
                            pressed: true,
                            modifiers: Modifiers::NONE,
                        }];
                        if button == PointerButton::Primary {
                            press.insert(0, space);
                        }
                        frame(&mut app, press);
                        for delta in [vec2(40., 20.), vec2(75., -15.), vec2(100., 5.)] {
                            let out = frame(&mut app, vec![Event::PointerMoved(start + delta)]);
                            let point = assert_aligned(&out);
                            assert!(
                                point.distance(previous) > 1.,
                                "pan must actually move the view"
                            );
                            previous = point;
                        }
                        assert_aligned(&frame(
                            &mut app,
                            vec![
                                Event::PointerButton {
                                    pos: start + vec2(100., 5.),
                                    button,
                                    pressed: false,
                                    modifiers: Modifiers::NONE,
                                },
                                Event::Key {
                                    key: Key::Space,
                                    physical_key: Some(Key::Space),
                                    pressed: false,
                                    repeat: false,
                                    modifiers: Modifiers::NONE,
                                },
                            ],
                        ));
                        let before_zoom = app.plan_scale;
                        assert_aligned(&frame(
                            &mut app,
                            vec![Event::MouseWheel {
                                unit: egui::MouseWheelUnit::Point,
                                phase: egui::TouchPhase::Move,
                                delta: vec2(0., 60.),
                                modifiers: Modifiers::NONE,
                            }],
                        ));
                        assert!(app.plan_scale > before_zoom);
                        for _ in 0..4 {
                            assert_aligned(&frame(&mut app, vec![]));
                        }
                        assert_eq!(app.host.document.editor.project, project);
                        assert_eq!(app.host.document.editor.undo.len(), history);
                        assert!(app.host.document.editor.selected.is_none());
                        std::fs::remove_dir_all(root).unwrap();
                    }
                }
            }
        }
    }
    fn trace_render_with_textures(ctx: &Context, app: &mut AtelierApp) -> egui::FullOutput {
        ctx.run_ui(
            RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1000., 700.))),
                ..Default::default()
            },
            |ui| {
                app.paint_trace_reference(
                    ui.painter(),
                    Rect::from_min_size(Pos2::ZERO, vec2(1000., 700.)),
                    None,
                )
            },
        )
    }
    fn trace_render(ctx: &Context, app: &mut AtelierApp) -> egui::FullOutput {
        let mut output = trace_render_with_textures(ctx, app);
        output.textures_delta.clear();
        output
    }
    fn paths(shapes: &[egui::epaint::ClippedShape]) -> Vec<&egui::epaint::PathShape> {
        shapes
            .iter()
            .filter_map(|s| {
                if let egui::Shape::Path(p) = &s.shape {
                    Some(p)
                } else {
                    None
                }
            })
            .collect()
    }
    fn trace_viewport_settle(ctx: &Context, app: &mut AtelierApp, center: Point, scale: f64) {
        // In particular, exercise both delayed fit frames after worksheet layout.
        for _ in 0..5 {
            frame(ctx, app, vec![]);
            assert!(
                app.plan_center.distance(center) < 1e-8,
                "center {:?}, expected {center:?}",
                app.plan_center
            );
            assert_eq!(app.plan_scale, scale);
            assert!(app.drawings_ui.fit_pending.is_none());
        }
    }
    #[test]
    fn trace_viewport_activate_menu_and_palette_keep_zoom_and_map_center_bilingual() {
        for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
            for palette in [false, true] {
                for source in 0..3 {
                    let (ctx, mut app, root, worksheet) = trace_fixture(language);
                    let target = match source {
                        0 => crate::trace::TraceReference::Worksheet(worksheet),
                        1 => crate::trace::TraceReference::Story(1),
                        _ => {
                            let path = root.join("legacy.png");
                            image::RgbaImage::from_pixel(8, 8, image::Rgba([0, 0, 0, 255]))
                                .save(&path)
                                .unwrap();
                            let id = app.host.document.editor.project.drawings.next_id().unwrap();
                            let page =
                                crate::drawings::import(&path, &root.join("images"), id).unwrap();
                            app.host.document.editor.project.drawings.pages.push(page);
                            crate::trace::TraceReference::Worksheet(id)
                        }
                    };
                    if source == 1 {
                        app.enter_worksheet(worksheet);
                    }
                    app.host
                        .document
                        .editor
                        .project
                        .plan_options
                        .orientation_degrees = 31.;
                    app.host.document.editor.mark_saved();
                    app.choose_trace_reference(target);
                    app.trace.offset = Point::new(7., -11.);
                    app.trace.rotation_deg = 37.;
                    app.plan_center = Point::new(42_000_000.125, -3_000_000.875);
                    app.plan_scale = 237.5;
                    let center = app
                        .trace
                        .inverse_transform(app.plan_center, app.trace_current_pivot());
                    let mut expected = app.host.document.editor.project.clone();
                    expected.plan_options.active_worksheet = match target {
                        crate::trace::TraceReference::Worksheet(id) => Some(id),
                        _ => None,
                    };
                    let history = (
                        app.host.document.editor.undo.len(),
                        app.host.document.editor.redo.len(),
                    );
                    if palette {
                        app.trace.palette_open = true;
                        click(&ctx, &mut app, language.translate("Activate"));
                    } else {
                        click(&ctx, &mut app, language.translate("View"));
                        click(&ctx, &mut app, language.translate("Trace Options"));
                        click(&ctx, &mut app, language.translate("Activate Reference"));
                    }
                    trace_viewport_settle(&ctx, &mut app, center, 237.5);
                    assert_trace_model_unchanged(&app.host.document.editor.project, &expected);
                    assert_eq!(
                        (
                            app.host.document.editor.undo.len(),
                            app.host.document.editor.redo.len()
                        ),
                        history
                    );
                    assert!(app.host.document.editor.dirty());
                    if source == 1 {
                        assert_eq!(app.host.document.editor.story, 1);
                    }
                    std::fs::remove_dir_all(root).unwrap();
                }
            }
        }
    }
    #[test]
    fn trace_viewport_switch_roundtrip_keeps_zoom_between_worksheets_and_stories() {
        for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
            for worksheet_active in [false, true] {
                for transformed in [false, true] {
                    let (ctx, mut app, root, worksheet) = trace_fixture(language);
                    if worksheet_active {
                        let id = app.host.document.editor.project.drawings.next_id().unwrap();
                        app.host.document.editor.project.drawings.worksheets.push(
                            crate::drawings::Worksheet {
                                id,
                                name: "Another worksheet".into(),
                            },
                        );
                        app.enter_worksheet(id);
                    }
                    app.host
                        .document
                        .editor
                        .project
                        .plan_options
                        .orientation_degrees = -28.;
                    app.host.document.editor.mark_saved();
                    app.choose_trace_reference(crate::trace::TraceReference::Worksheet(worksheet));
                    if transformed {
                        app.trace.offset = Point::new(19., -5.);
                        app.trace.rotation_deg = -63.;
                    }
                    let center = Point::new(15.25, 37.125);
                    app.plan_center = center;
                    app.plan_scale = 317.;
                    let expected = app
                        .trace
                        .inverse_transform(center, app.trace_current_pivot());
                    let project = app.host.document.editor.project.clone();
                    let history = (
                        app.host.document.editor.undo.len(),
                        app.host.document.editor.redo.len(),
                    );
                    app.trace.palette_open = true;
                    click(&ctx, &mut app, language.translate("Switch with Active"));
                    trace_viewport_settle(&ctx, &mut app, expected, 317.);
                    assert_eq!(
                        app.host
                            .document
                            .editor
                            .project
                            .plan_options
                            .active_worksheet,
                        Some(worksheet)
                    );
                    click(&ctx, &mut app, language.translate("Switch with Active"));
                    trace_viewport_settle(&ctx, &mut app, center, 317.);
                    assert_trace_model_unchanged(&app.host.document.editor.project, &project);
                    assert_eq!(
                        (
                            app.host.document.editor.undo.len(),
                            app.host.document.editor.redo.len()
                        ),
                        history
                    );
                    assert!(app.host.document.editor.dirty());
                    std::fs::remove_dir_all(root).unwrap();
                }
            }
        }
    }
    #[test]
    fn trace_viewport_unavailable_targets_and_temporary_displacement_are_safe() {
        let (ctx, mut app, root, worksheet) = trace_fixture(UiLanguage::English);
        let center = Point::new(25., 31.);
        app.plan_center = center;
        app.plan_scale = 273.;
        for reference in [
            crate::trace::TraceReference::Worksheet(u64::MAX),
            crate::trace::TraceReference::Story(u32::MAX),
            crate::trace::TraceReference::EntireModel,
        ] {
            app.trace.reference = reference;
            let project = app.host.document.editor.project.clone();
            app.activate_trace_reference();
            app.switch_trace_reference();
            trace_viewport_settle(&ctx, &mut app, center, 273.);
            assert_trace_model_unchanged(&app.host.document.editor.project, &project);
        }
        app.choose_trace_reference(crate::trace::TraceReference::Worksheet(worksheet));
        app.trace.offset = Point::new(7., 9.);
        app.trace.rotation_deg = 22.;
        let expected = app
            .trace
            .inverse_transform(center, app.trace_current_pivot());
        app.trace.displace_mode = true;
        app.trace.displace_origin = Some(app.trace.offset);
        app.trace.offset = Point::new(700., 900.);
        app.switch_trace_reference();
        trace_viewport_settle(&ctx, &mut app, expected, 273.);
        app.switch_trace_reference();
        trace_viewport_settle(&ctx, &mut app, center, 273.);
        assert!(app.trace.offset.distance(Point::new(7., 9.)) < 1e-10);
        assert!(!app.trace.pointer_mode_active());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn trace_viewport_other_menu_commands_never_fit_or_pan_bilingual() {
        for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
            let (ctx, mut app, root, worksheet) = trace_fixture(language);
            app.choose_trace_reference(crate::trace::TraceReference::Worksheet(worksheet));
            app.plan_center = Point::new(15.25, 37.125);
            app.plan_scale = 317.;
            let original = app.host.document.editor.project.clone();
            for command in [
                "Drag Reference",
                "Rotate Reference",
                "Reset to Default Position",
                "Rebuild Reference",
                "Temporarily displace Reference",
                "Show/Hide Splitter",
                "Reference on Top",
                "Transparent Fills and Zones",
                "Turn off Trace everywhere",
                "Trace & Reference",
            ] {
                click(&ctx, &mut app, language.translate("View"));
                click(&ctx, &mut app, language.translate("Trace Options"));
                click(&ctx, &mut app, language.translate(command));
                frame(
                    &ctx,
                    &mut app,
                    vec![Event::Key {
                        key: Key::Escape,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers: Modifiers::NONE,
                    }],
                );
                trace_viewport_settle(&ctx, &mut app, Point::new(15.25, 37.125), 317.);
                assert_trace_model_unchanged(&app.host.document.editor.project, &original);
            }
            std::fs::remove_dir_all(root).unwrap();
        }
    }
    #[test]
    fn trace_dwg_normal_menu_browse_cancel_palette_switch_bilingual() {
        for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
            let (ctx, mut app, root, worksheet) = trace_fixture(language);
            let original = app.host.document.editor.project.clone();
            let history = app.host.document.editor.undo.len();
            for _ in 0..3 {
                frame(&ctx, &mut app, vec![]);
            }
            click(&ctx, &mut app, language.translate("Trace"));
            assert!(
                app.trace.enabled,
                "enable Trace before choosing a worksheet, as in AC29"
            );
            click(&ctx, &mut app, language.translate("View"));
            click(&ctx, &mut app, language.translate("Trace Options"));
            click(&ctx, &mut app, language.translate("Choose Reference"));
            let label = format!("{} · Survey drawing", language.translate("Worksheet"));
            click(&ctx, &mut app, &label);
            assert_eq!(
                app.trace.reference,
                crate::trace::TraceReference::Worksheet(worksheet)
            );
            assert!(app.trace.enabled);
            assert_eq!(app.trace_reference_label(), "Survey drawing");
            app.trace.browse_choice = crate::trace::TraceReference::Story(2);
            app.trace.browse_open = true;
            click(&ctx, &mut app, language.translate("Cancel"));
            assert_eq!(
                app.trace.reference,
                crate::trace::TraceReference::Worksheet(worksheet)
            );
            app.trace.palette_open = true;
            app.trace.drag_mode = true;
            let offset_before = app.trace.offset;
            click(&ctx, &mut app, language.translate("Original colors"));
            assert!(app.trace.reference_color.is_none());
            click(&ctx, &mut app, language.translate("Original colors"));
            assert!(app.trace.reference_color.is_some());
            assert_eq!(
                app.trace.offset, offset_before,
                "palette controls must not drag the underlying reference"
            );
            app.trace.drag_mode = false;

            click(&ctx, &mut app, language.translate("Switch with Active"));
            assert_eq!(
                app.host
                    .document
                    .editor
                    .project
                    .plan_options
                    .active_worksheet,
                Some(worksheet)
            );
            assert_eq!(app.trace.reference, crate::trace::TraceReference::Story(0));
            click(&ctx, &mut app, language.translate("Switch with Active"));
            assert_eq!(
                app.host
                    .document
                    .editor
                    .project
                    .plan_options
                    .active_worksheet,
                None
            );
            assert_eq!(
                app.trace.reference,
                crate::trace::TraceReference::Worksheet(worksheet)
            );
            assert_trace_model_unchanged(&app.host.document.editor.project, &original);
            assert_eq!(app.host.document.editor.undo.len(), history);
            assert!(app.host.document.editor.dirty());
            std::fs::remove_dir_all(root).unwrap();
        }
    }
    #[test]
    fn trace_dwg_painted_points_colors_text_fade_and_locked_snap_share_transform() {
        let (ctx, mut app, root, worksheet) = trace_fixture(UiLanguage::English);
        app.choose_trace_reference(crate::trace::TraceReference::Worksheet(worksheet));
        app.trace.offset = Point::new(1.7, -0.9);
        app.trace.rotation_deg = 37.;
        app.trace.reference_color = Some([19, 79, 173]);
        app.trace.reference_opacity = 137;
        app.host
            .document
            .editor
            .project
            .plan_options
            .orientation_degrees = 17.;
        let original = app.host.document.editor.project.clone();
        let history = app.host.document.editor.undo.len();
        let u = &original.drawings.cad_underlays[0];
        let data = original.drawings.cad_sources[0].data.as_ref().unwrap();
        let native = data.primitives[0].endpoints[1];
        let pivot = app.trace_worksheet_pivot(worksheet);
        let world = u.transform.world(native);
        let (rad_s, rad_c) = 37f64.to_radians().sin_cos();
        let expected = Point::new(
            pivot.x + (world.x - pivot.x) * rad_c - (world.y - pivot.y) * rad_s + 1.7,
            pivot.y + (world.x - pivot.x) * rad_s + (world.y - pivot.y) * rad_c - 0.9,
        );
        let out = trace_render(&ctx, &mut app);
        let ps = paths(&out.shapes);
        assert!(!ps.is_empty());
        let screen = app.screen(Rect::from_min_size(Pos2::ZERO, vec2(1000., 700.)), expected);
        assert!(
            ps.iter()
                .any(|p| p.points.iter().any(|q| q.distance(screen) < 0.001))
        );
        let expected_color = Color32::from_rgba_unmultiplied(19, 79, 173, 137);
        assert!(
            ps.iter()
                .all(|p| p.stroke.color == egui::epaint::ColorMode::Solid(expected_color))
        );
        assert!(out.shapes.iter().any(|s|matches!(&s.shape,egui::Shape::Text(t) if t.galley.job.text=="Survey"&&t.fallback_color==expected_color)));
        let near = Point::new(expected.x + 0.02, expected.y - 0.01);
        let (q, kind) = app.base_target(near, false);
        assert_eq!(kind, SnapKind::Endpoint);
        assert!(q.distance(expected) < 1e-10);
        app.trace.reference_color = None;
        let out = trace_render(&ctx, &mut app);
        assert!(paths(&out.shapes).iter().all(|p| p.stroke.color
            == egui::epaint::ColorMode::Solid(Color32::from_rgba_unmultiplied(
                u.grey(),
                u.grey(),
                u.grey(),
                137
            ))));
        app.trace.reference_opacity = 0;
        assert!(paths(&trace_render(&ctx, &mut app).shapes).is_empty());
        assert!(app.trace_cad_snap(near, 0.1).is_none());
        app.trace.reference_opacity = 255;
        app.trace.visibility.drawings = false;
        assert!(paths(&trace_render(&ctx, &mut app).shapes).is_empty());
        assert!(app.trace_cad_snap(near, 0.1).is_none());
        app.trace.visibility.drawings = true;
        app.host.document.editor.project.drawings.cad_underlays[0]
            .hidden_layers
            .insert("Survey".into());
        assert!(app.trace_cad_snap(near, 0.1).is_none());
        app.host.document.editor.project.drawings = original.drawings.clone();
        assert_trace_model_unchanged(&app.host.document.editor.project, &original);
        assert_eq!(app.host.document.editor.undo.len(), history);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn trace_dwg_switch_roundtrip_inverts_alignment_without_changing_saved_placement() {
        let (_ctx, mut app, root, worksheet) = trace_fixture(UiLanguage::English);
        app.choose_trace_reference(crate::trace::TraceReference::Worksheet(worksheet));
        app.trace.offset = Point::new(2., -3.);
        app.trace.rotation_deg = 37.;
        let old = app.trace.clone();
        let pivot = app.trace_worksheet_pivot(worksheet);
        let original = app.host.document.editor.project.clone();
        app.switch_trace_reference();
        assert_eq!(
            app.host
                .document
                .editor
                .project
                .plan_options
                .active_worksheet,
            Some(worksheet)
        );
        let p = Point::new(4., 5.);
        let shifted = old.transform(p, pivot);
        let restored = app.trace.transform(shifted, app.trace_current_pivot());
        assert!(restored.distance(p) < 1e-10);
        app.switch_trace_reference();
        assert_trace_model_unchanged(&app.host.document.editor.project, &original);
        assert_eq!(app.trace.reference, old.reference);
        assert!((app.trace.rotation_deg - old.rotation_deg).abs() < 1e-10);
        assert!(app.trace.offset.distance(old.offset) < 1e-10);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn trace_dwg_mixed_image_uses_cached_ink_tint_and_keeps_original_pixels() {
        let (ctx, mut app, root, worksheet) = trace_fixture(UiLanguage::English);
        let path = root.join("ink.png");
        let mut image = image::RgbaImage::new(2, 2);
        image.put_pixel(0, 0, image::Rgba([0, 0, 0, 255]));
        image.put_pixel(1, 0, image::Rgba([255, 255, 255, 255]));
        image.put_pixel(0, 1, image::Rgba([255, 0, 0, 255]));
        image.put_pixel(1, 1, image::Rgba([0, 0, 0, 0]));
        image.save(&path).unwrap();
        let mut page = crate::drawings::import(
            &path,
            &root.join("images"),
            app.host.document.editor.project.drawings.next_id().unwrap(),
        )
        .unwrap();
        page.placement = Some(crate::drawings::DrawingPlacement {
            worksheet: Some(worksheet),
            story: 0,
            origin: Point::new(3., 4.),
            angle: 13.,
            visible: true,
            opacity: 255,
            frame: None,
        });
        let id = page.id;
        app.host
            .document
            .editor
            .project
            .drawings
            .pages
            .push(page.clone());
        app.choose_trace_reference(crate::trace::TraceReference::Worksheet(worksheet));
        app.trace.reference_color = Some([23, 145, 89]);
        app.trace.reference_opacity = 117;
        app.trace.rotation_deg = 33.;
        app.trace.offset = Point::new(1., 2.);
        let original = app.host.document.editor.project.clone();
        let mut output = trace_render_with_textures(&ctx, &mut app);
        let mesh = output
            .shapes
            .iter()
            .find_map(|s| {
                if let egui::Shape::Mesh(m) = &s.shape {
                    Some(m)
                } else {
                    None
                }
            })
            .unwrap();
        let mask_id = mesh.texture_id;
        assert!(
            mesh.vertices
                .iter()
                .all(|v| v.color == Color32::from_rgba_unmultiplied(23, 145, 89, 117))
        );
        let ink = &output.textures_delta.set[&mask_id][0];
        let egui::ImageData::Color(pixels) = &ink.image;
        assert_eq!(pixels.pixels[0], Color32::WHITE);
        assert_eq!(pixels.pixels[1], Color32::TRANSPARENT);
        assert_eq!(pixels.pixels[3], Color32::TRANSPARENT);
        let expected = app.screen(
            Rect::from_min_size(Pos2::ZERO, vec2(1000., 700.)),
            app.trace
                .transform(Point::new(3., 4.), app.trace_worksheet_pivot(worksheet)),
        );
        assert!(mesh.vertices[0].pos.distance(expected) < 0.001);
        output.textures_delta.clear();
        app.trace.reference_color = Some([220, 30, 90]);
        app.trace.reference_opacity = 50;
        let mut output = trace_render_with_textures(&ctx, &mut app);
        assert!(
            !output
                .textures_delta
                .set
                .iter()
                .any(|(id, _)| *id == mask_id),
            "changing hue/intensity reuses the ink texture"
        );
        output.textures_delta.clear();
        let mut output = ctx.run_ui(
            RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1000., 700.))),
                ..Default::default()
            },
            |ui| {
                app.paint_drawing_source(
                    ui.painter(),
                    Rect::from_min_size(Pos2::ZERO, vec2(1000., 700.)),
                    id,
                    false,
                )
            },
        );
        let mesh = output
            .shapes
            .iter()
            .find_map(|s| {
                if let egui::Shape::Mesh(m) = &s.shape {
                    Some(m)
                } else {
                    None
                }
            })
            .unwrap();
        assert_ne!(mesh.texture_id, mask_id);
        let delta = &output.textures_delta.set[&mesh.texture_id][0];
        let egui::ImageData::Color(pixels) = &delta.image;
        assert_eq!(pixels.pixels[0], Color32::BLACK);
        assert_eq!(pixels.pixels[1], Color32::WHITE);
        assert_eq!(pixels.pixels[2], Color32::RED);
        output.textures_delta.clear();
        assert_trace_model_unchanged(&app.host.document.editor.project, &original);
        assert_eq!(
            std::fs::read(&path).unwrap(),
            crate::drawings::verified_bytes(&page).unwrap()
        );
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn trace_dwg_rotated_plan_drag_and_temporary_displacement_follow_screen_direction() {
        let (ctx, mut app, root, worksheet) = trace_fixture(UiLanguage::English);
        app.choose_trace_reference(crate::trace::TraceReference::Worksheet(worksheet));
        app.host
            .document
            .editor
            .project
            .plan_options
            .orientation_degrees = 90.;
        app.plan_scale = 50.;
        let original = app.host.document.editor.project.clone();
        let history = app.host.document.editor.undo.len();
        fn plan_events(ctx: &Context, app: &mut AtelierApp, events: Vec<Event>) {
            let mut output = ctx.run_ui(
                RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1000., 700.))),
                    events,
                    ..Default::default()
                },
                |ui| app.plan(ui),
            );
            output.textures_delta.clear();
        }
        for temporary in [false, true] {
            app.trace.offset = Point::default();
            app.trace.drag_mode = !temporary;
            app.trace.displace_mode = temporary;
            plan_events(&ctx, &mut app, vec![Event::PointerMoved(pos2(400., 300.))]);
            plan_events(
                &ctx,
                &mut app,
                vec![Event::PointerButton {
                    pos: pos2(400., 300.),
                    button: PointerButton::Primary,
                    pressed: true,
                    modifiers: Modifiers::NONE,
                }],
            );
            plan_events(&ctx, &mut app, vec![Event::PointerMoved(pos2(450., 300.))]);
            assert!(
                app.trace.offset.distance(Point::new(0., -1.)) < 1e-9,
                "right drag on a 90-degree plan must follow the inverse projection: {:?}",
                app.trace.offset
            );
            plan_events(
                &ctx,
                &mut app,
                vec![Event::PointerButton {
                    pos: pos2(450., 300.),
                    button: PointerButton::Primary,
                    pressed: false,
                    modifiers: Modifiers::NONE,
                }],
            );
            assert!(
                app.trace.pointer_mode_active(),
                "release leaves the destination phase active"
            );
            for pressed in [true, false] {
                plan_events(
                    &ctx,
                    &mut app,
                    vec![Event::PointerButton {
                        pos: pos2(450., 300.),
                        button: PointerButton::Primary,
                        pressed,
                        modifiers: Modifiers::NONE,
                    }],
                );
            }
            if temporary {
                assert_eq!(app.trace.offset, Point::default());
            } else {
                assert!(app.trace.offset.distance(Point::new(0., -1.)) < 1e-9);
            }
        }
        assert_trace_model_unchanged(&app.host.document.editor.project, &original);
        assert_eq!(app.host.document.editor.undo.len(), history);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn trace_persistence_cad_and_image_assets_survive_save_as_and_reopen() {
        use crate::trace_document::{TraceSettings, TraceView};
        for image_source in [false, true] {
            let (ctx, mut app, root, worksheet) = trace_fixture(UiLanguage::English);
            let reference = if image_source {
                let path = root.join("original.png");
                image::RgbaImage::from_pixel(8, 12, image::Rgba([11, 29, 57, 255]))
                    .save(&path)
                    .unwrap();
                let id = app.host.document.editor.project.drawings.next_id().unwrap();
                let page = crate::drawings::import(&path, &root.join("images"), id).unwrap();
                app.host.document.editor.project.drawings.pages.push(page);
                id
            } else {
                worksheet
            };
            app.trace.browse_open = true;
            app.trace.browse_choice = crate::trace::TraceReference::Worksheet(reference);
            click(&ctx, &mut app, "OK");
            assert_eq!(
                app.trace.reference,
                crate::trace::TraceReference::Worksheet(reference)
            );
            app.trace.offset = Point::new(21., -31.);
            app.trace.rotation_deg = 43.;
            app.trace.reference_opacity = 87;
            app.trace.active_opacity = 191;
            app.trace.reference_color = Some([19, 79, 173]);
            app.trace.visibility.beams = false;
            let expected = TraceSettings::capture(&app.trace);
            app.save_project_to(root.join("first.json").to_str().unwrap())
                .unwrap();
            app.save_project_to(root.join("second.json").to_str().unwrap())
                .unwrap();
            for filename in ["first.json", "second.json"] {
                let project = storage::load(&root.join(filename)).unwrap();
                assert_eq!(
                    project.trace.settings(TraceView::FloorPlan),
                    Some(&expected)
                );
                if image_source {
                    let page = project
                        .drawings
                        .pages
                        .iter()
                        .find(|p| p.id == reference)
                        .unwrap();
                    assert!(!crate::drawings::verified_bytes(page).unwrap().is_empty());
                } else {
                    assert!(project.drawings.cad_sources[0].data.is_some());
                    assert_eq!(project.drawings.cad_underlays[0].worksheet, reference);
                }
                let mut reopened = AtelierApp::from_renderer(None, "New session".into());
                reopened.host.document.editor = Editor::new(project);
                reopened.reset_worksheet_session();
                assert_eq!(TraceSettings::capture(&reopened.trace), expected);
                assert!(!reopened.host.document.editor.dirty());
                let pivot = reopened.trace_current_pivot();
                reopened.plan_center = reopened.trace.transform(pivot, pivot);
                assert!(
                    !trace_render(&ctx, &mut reopened).shapes.is_empty(),
                    "reopened source must actually paint"
                );
            }
            std::fs::remove_dir_all(root).unwrap();
        }
    }

    #[test]
    fn trace_rebuild_reloads_frozen_assets_and_recovers_missing_cache_without_model_edits() {
        let (_ctx, mut app, root, worksheet) = trace_fixture(UiLanguage::English);
        app.choose_trace_reference(crate::trace::TraceReference::Worksheet(worksheet));
        app.host.document.editor.mark_saved();
        let source = app.host.document.editor.project.drawings.cad_sources[0].clone();
        let bytes = source.original.bytes().unwrap();
        let points = source.data.as_ref().unwrap().primitives[0].points.clone();
        let history = app.host.document.editor.undo.len();
        // A linked file changing is distinct from an accepted source changing.
        std::fs::write(&source.linked_path, "changed external file").unwrap();
        app.rebuild_trace_reference();
        assert_eq!(
            app.host.document.editor.project.drawings.cad_sources[0]
                .data
                .as_ref()
                .unwrap()
                .primitives[0]
                .points,
            points
        );
        assert!(!app.host.document.editor.dirty());
        std::fs::remove_file(&source.original.path).unwrap();
        app.rebuild_trace_reference();
        assert!(
            app.host.document.editor.project.drawings.cad_sources[0]
                .data
                .is_none()
        );
        assert!(app.host.document.editor.status.contains("unavailable"));
        assert!(!app.host.document.editor.dirty());
        std::fs::write(&source.original.path, bytes).unwrap();
        app.rebuild_trace_reference();
        assert!(
            app.host.document.editor.project.drawings.cad_sources[0]
                .data
                .is_some()
        );
        assert_eq!(app.host.document.editor.undo.len(), history);
        assert!(!app.host.document.editor.dirty());
        std::fs::remove_dir_all(root).unwrap();
    }
}
