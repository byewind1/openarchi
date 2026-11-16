//! External sources and their independent placements use the shared document history.
use super::drawings_ui::Pick;
use super::*;
use crate::drawings::{self, DrawingPlacement, Worksheet};

pub(super) const EXTERNAL_COMMANDS: &[&str] = &[
    "Place Hotlink…",
    "Break Hotlink (Embed Elements)",
    "Save Hotlink as File…",
    "Save Selection as Module…",
    "Hotlink Module Manager…",
    "Place External Drawing…",
    "Place External Drawing from BIMcloud…",
    "Drawing Manager",
    "Attach Xref…",
    "Xref Manager…",
];
impl AtelierApp {
    pub(super) fn drawing_input_selector(&mut self, ui: &mut Ui) {
        let lang = self.language;
        ui.strong(lang.translate("Drawing input"));
        ui.label(lang.translate("Choose an imported image or search project images. The active canvas remains the output target."));
        ui.text_edit_singleline(&mut self.drawings_ui.search);
        let pages: Vec<_> = self
            .host
            .document
            .editor
            .project
            .drawings
            .search_sources(&self.drawings_ui.search)
            .into_iter()
            .map(|p| (p.id, p.name.clone()))
            .collect();
        ScrollArea::vertical().max_height(110.).show(ui, |ui| {
            for (id, name) in pages {
                if ui
                    .selectable_label(self.drawings_ui.source == Some(id), format!("#{id} {name}"))
                    .clicked()
                {
                    self.select_drawing(id);
                }
            }
        });
        if ui
            .button(lang.translate("Drawing settings / AI…"))
            .clicked()
        {
            self.drawings_ui.settings = true;
        }
        if ui.button(lang.translate("Drawing Manager")).clicked() {
            self.drawings_ui.manager = true;
        }
        ui.separator();
    }
    pub(super) fn external_command_enabled(&self, command: &str) -> bool {
        match command {
            "Break Hotlink (Embed Elements)" | "Save Hotlink as File…" => {
                self.selected_reference().is_some_and(|id| {
                    self.host
                        .document
                        .editor
                        .project
                        .drawings
                        .reference_instances
                        .iter()
                        .any(|v| {
                            v.id == id
                                && self
                                    .host
                                    .document
                                    .editor
                                    .project
                                    .drawings
                                    .reference_sources
                                    .iter()
                                    .any(|s| s.id == v.source && s.kind == ReferenceKind::Hotlink)
                        })
                })
            }
            "Save Selection as Module…" => {
                !self.host.document.editor.selection_ids().is_empty()
                    && self
                        .host
                        .document
                        .editor
                        .project
                        .plan_options
                        .active_worksheet
                        .is_none()
            }
            "Place Hotlink…"
            | "Attach Xref…"
            | "Place External Drawing…"
            | "Place External Drawing from BIMcloud…" => self.view == View::Plan,
            _ => true,
        }
    }
    pub(super) fn external_content_menu(&mut self, ui: &mut Ui) {
        for (i, key) in EXTERNAL_COMMANDS.iter().enumerate() {
            if [3, 5, 8].contains(&i) {
                ui.separator();
            }
            if ui
                .add_enabled(
                    self.external_command_enabled(key),
                    Button::new(self.language.translate(key)),
                )
                .clicked()
            {
                self.external_content_command(key);
                ui.close();
            }
        }
    }

    pub(super) fn external_content_command(&mut self, command: &str) -> bool {
        match command {
            "Place Hotlink…" | "Hotlink Module Manager…" | "Hotlink Manager…" => {
                self.reference_ui.kind = Some(ReferenceKind::Hotlink);
                self.reference_ui.open = true;
                true
            }
            "Attach Xref…" | "Xref Manager…" => {
                self.reference_ui.kind = Some(ReferenceKind::Xref);
                self.reference_ui.open = true;
                true
            }
            "Break Hotlink (Embed Elements)" => {
                let result = self
                    .selected_reference()
                    .ok_or_else(|| "Select a reference instance".to_string())
                    .and_then(|instance| self.reference_apply(ReferenceEdit::Break { instance }));
                self.run_reference_action(result);
                self.reference_ui.open = true;
                true
            }
            "Save Hotlink as File…" | "Save Selection as Module…" => {
                self.reference_ui.save = true;
                self.reference_ui.save_instance = if command == "Save Hotlink as File…" {
                    self.selected_reference()
                } else {
                    None
                };
                if command == "Save Hotlink as File…" && self.reference_ui.save_instance.is_none()
                {
                    self.reference_ui.save = false;
                    self.reference_ui.open = true;
                    self.reference_ui.error = "Select a reference instance".into();
                } else {
                    self.reference_ui.error.clear();
                }
                true
            }
            "Place External Drawing…" => {
                self.begin_external_drawing();
                true
            }
            "Place External Drawing from BIMcloud…" => {
                self.cloud_drawing.open = true;
                true
            }
            "Drawing Manager" | "Drawing Manager…" => {
                self.drawings_ui.manager = true;
                true
            }
            _ => false,
        }
    }
    pub(super) fn new_worksheet(&mut self) -> Result<(), String> {
        let mut doc = self.host.document.editor.project.drawings.clone();
        let id = doc.next_id()?;
        doc.worksheets.push(Worksheet {
            id,
            name: format!("{} {id}", self.language.translate("Worksheet")),
        });
        self.set_drawings(doc)?;
        self.open_worksheet(id);
        Ok(())
    }
    pub(super) fn drawing_placement_canvas(
        &mut self,
        ui: &mut Ui,
        rect: Rect,
        response: &Response,
    ) -> bool {
        let Some(mut page) = self.drawings_ui.pending.clone() else {
            self.drawings_ui.cancel();
            return false;
        };
        let Some(placement) = page.placement.as_mut() else {
            return false;
        };
        if placement.worksheet
            != self
                .host
                .document
                .editor
                .project
                .plan_options
                .active_worksheet
            || placement.story != self.host.document.editor.story
        {
            self.drawings_ui.cancel();
            return false;
        }
        response.clone().on_hover_cursor(CursorIcon::Crosshair);
        let origin = response
            .hover_pos()
            .map(|p| self.world(rect, p))
            .unwrap_or(placement.origin);
        placement.origin = origin;
        let bounds = Rect::from_two_pos(
            self.screen(rect, origin),
            self.screen(
                rect,
                placement.world([page.pixels[0] as f64, page.pixels[1] as f64], 0.01),
            ),
        );
        ui.painter_at(rect)
            .rect_stroke(bounds, 0., Stroke::new(1.5, ACCENT), StrokeKind::Inside);
        if response.clicked_by(PointerButton::Primary) {
            let mut doc = self.host.document.editor.project.drawings.clone();
            page.id = doc.next_id().unwrap_or(page.id);
            let id = page.id;
            let pixels = page.pixels;
            doc.pages.push(page);
            let result = doc
                .add_region(
                    id,
                    self.host.document.editor.story,
                    [0., 0., pixels[0] as f64, pixels[1] as f64],
                )
                .and_then(|region| {
                    self.set_drawings(doc)?;
                    Ok(region)
                });
            match result {
                Ok(region) => {
                    self.drawings_ui.source = Some(id);
                    self.drawings_ui.selected = Some(region);
                    self.drawings_ui.cancel();
                }
                Err(error) => self.drawings_ui.error = error,
            }
        }
        true
    }
    pub(super) fn paint_placed_drawings(&mut self, painter: &Painter, viewport: Rect) {
        self.paint_cad_underlays(painter, viewport);
        let scope = self
            .host
            .document
            .editor
            .project
            .plan_options
            .active_worksheet;
        let ids: Vec<_> = self
            .host
            .document
            .editor
            .project
            .drawings
            .pages
            .iter()
            .filter(|p| {
                p.placement.as_ref().is_some_and(|v| {
                    v.visible
                        && v.worksheet == scope
                        && (scope.is_some() || v.story == self.host.document.editor.story)
                })
            })
            .map(|p| p.id)
            .collect();
        for id in ids {
            self.paint_worksheet_source(painter, viewport, id);
        }
    }
    pub(super) fn select_drawing(&mut self, id: u64) {
        self.drawings_ui.cancel();
        self.drawings_ui.canvas_selected = false;
        self.drawings_ui.proposal = None;
        self.drawings_ui.source = Some(id);
        self.drawings_ui.selected = self
            .host
            .document
            .editor
            .project
            .drawings
            .regions
            .iter()
            .find(|r| r.page == id)
            .map(|r| r.id);
        self.drawings_ui.placement_draft = None;
    }
    pub(super) fn delete_selected_drawing(&mut self) {
        if !self.drawings_ui.canvas_selected {
            return;
        }
        if let Some(id) = self.selected_drawing_source() {
            let mut doc = self.host.document.editor.project.drawings.clone();
            if !doc
                .pages
                .iter()
                .any(|p| p.id == id && p.placement.is_some())
            {
                return;
            }
            doc.pages.retain(|p| p.id != id);
            doc.regions.retain(|r| r.page != id);
            match self.set_drawings(doc) {
                Ok(()) => {
                    self.drawings_ui.source = None;
                    self.drawings_ui.canvas_selected = false;
                }
                Err(e) => self.drawings_ui.error = e,
            }
        }
    }
    fn drawing_status(&self, id: u64) -> &'static str {
        let Some(p) = self
            .host
            .document
            .editor
            .project
            .drawings
            .pages
            .iter()
            .find(|p| p.id == id)
        else {
            return "Missing";
        };
        let Some(path) = &p.linked_source else {
            return "Embedded";
        };
        match drawings::source_bytes(path) {
            Ok(bytes) => {
                if drawings::digest(&bytes) == p.sha256 {
                    "Up to date"
                } else {
                    "Modified"
                }
            }
            Err(_) => "Missing",
        }
    }
    fn update_drawing_source(&mut self, id: u64) -> Result<(), String> {
        let mut doc = self.host.document.editor.project.drawings.clone();
        let page = doc
            .pages
            .iter_mut()
            .find(|p| p.id == id)
            .ok_or("Select a source page")?;
        let source = page
            .linked_source
            .as_ref()
            .ok_or("Embedded drawings have no update source")?;
        let imported = drawings::import(source, &self.drawing_cache()?, id)?;
        // Keep existing frame/regions rather than silently reinterpret coordinates.
        if imported.pixels != page.pixels {
            return Err(
                "Source dimensions changed; place the changed image as a new drawing".into(),
            );
        }
        page.path = imported.path;
        page.sha256 = imported.sha256;
        for r in doc.regions.iter_mut().filter(|r| r.page == id) {
            r.revision = r
                .revision
                .checked_add(1)
                .ok_or("Drawing revision exhausted")?;
            r.needs_review = true;
        }
        self.set_drawings(doc)?;
        self.drawings_ui.textures_clear();
        Ok(())
    }
    fn drawing_target_label(&self, p: &drawings::SourcePage) -> String {
        if let Some(v) = &p.placement {
            match v.worksheet {
                Some(id) => format!("{} #{id}", self.language.translate("Worksheet")),
                None => format!("{} {}", self.language.translate("Story"), v.story),
            }
        } else {
            format!("{} #{}", self.language.translate("Worksheet"), p.id)
        }
    }
    pub(super) fn drawing_manager(&mut self, ctx: &Context) {
        if !self.drawings_ui.manager {
            return;
        }
        let mut open = true;
        let lang = self.language;
        Window::new(lang.translate("Drawing Manager")).id(Id::new("drawing_manager")).open(&mut open).default_width(700.).vscroll(true).show(ctx,|ui| {
            self.cad_manager_controls(ui);
            self.figure_manager_controls(ui);
            ui.separator();
            ui.label(lang.translate("Search imported images by name, ID or source path"));
            ui.text_edit_singleline(&mut self.drawings_ui.search);
            let pages:Vec<_>=self.host.document.editor.project.drawings.search_sources(&self.drawings_ui.search).into_iter().cloned().collect();
            ScrollArea::vertical().max_height(240.).show(ui,|ui| {
                for page in pages {
                    let status=self.drawings_ui.statuses.get(&page.id).map_or("Needs checking",String::as_str);
                    let label=format!("#{}  {}  · {} · {}",page.id,page.name,self.drawing_target_label(&page),lang.translate(status));
                    if ui.selectable_label(self.drawings_ui.source==Some(page.id),label).clicked() {self.select_drawing(page.id);}
                }
            });
            ui.horizontal_wrapped(|ui| {
                if ui.button(lang.translate("Place External Drawing…")).clicked() {self.begin_external_drawing();}
                if ui.button(lang.translate("New worksheet")).clicked() && let Err(e)=self.new_worksheet(){self.drawings_ui.error=e;}
            });
            if let Some(id)=self.selected_drawing_source() {
                let page=self.host.document.editor.project.drawings.pages.iter().find(|p|p.id==id).cloned().unwrap();
                ui.label(format!("{}: {}",lang.translate("Source"),page.linked_source.as_ref().unwrap_or(&page.path).display()));
                ui.horizontal_wrapped(|ui| {
                    if ui.button(lang.translate("Show placed drawing")).clicked() {
                        if let Some(v)=page.placement {
                            self.host.document.editor.switch_worksheet(v.worksheet);self.host.document.editor.story=v.story;self.view=View::Plan;
                            self.plan_center=v.origin;
                            self.plan_scale=(500./(page.pixels[0].max(page.pixels[1]) as f64*self.worksheet_pixel_scale(id))).clamp(3.,self.plan_view_scale_max(250.));
                        } else {self.open_worksheet(id);}
                        self.select_drawing(id);
                    }
                    if ui.button(lang.translate("Drawing settings / AI…")).clicked() {self.drawings_ui.settings=true;}
                    if ui.button(lang.translate("Check status")).clicked() {let status=self.drawing_status(id).to_owned();self.host.document.editor.status=lang.translate(&status).to_owned();self.drawings_ui.statuses.insert(id,status);}
                    if ui.add_enabled(page.linked_source.is_some(),Button::new(lang.translate("Update drawing"))).clicked() && let Err(e)=self.update_drawing_source(id){self.drawings_ui.error=e;}
                    if ui.add_enabled(page.linked_source.is_some(),Button::new(lang.translate("Break drawing link"))).clicked(){
                        let mut doc=self.host.document.editor.project.drawings.clone();doc.pages.iter_mut().find(|p|p.id==id).unwrap().linked_source=None;
                        if let Err(e)=self.set_drawings(doc){self.drawings_ui.error=e;}
                    }
                });
                ui.horizontal_wrapped(|ui| {
                    if ui.button(lang.translate("Crop drawing to selected redraw area")).clicked(){
                        let mut doc=self.host.document.editor.project.drawings.clone();
                        let frame=doc.regions.iter().find(|r|Some(r.id)==self.drawings_ui.selected&&r.page==id).map(|r|r.bounds);
                        if let Some(p)=doc.pages.iter_mut().find(|p|p.id==id).and_then(|p|p.placement.as_mut()){p.frame=frame;}
                        if let Err(e)=self.set_drawings(doc){self.drawings_ui.error=e;}
                    }
                    if ui.button(lang.translate("Fit frame to source")).clicked(){let mut doc=self.host.document.editor.project.drawings.clone();if let Some(p)=doc.pages.iter_mut().find(|p|p.id==id).and_then(|p|p.placement.as_mut()){p.frame=None;}if let Err(e)=self.set_drawings(doc){self.drawings_ui.error=e;}}
                });
                ui.label(lang.translate("Link to image file"));ui.text_edit_singleline(&mut self.drawings_ui.path);
                if ui.button(lang.translate("Link drawing")).clicked() {
                    let result=(||{let path=std::path::Path::new(self.drawings_ui.path.trim()).canonicalize().map_err(|e|e.to_string())?;
                        let candidate=drawings::import(&path,&self.drawing_cache()?,id)?;
                        let mut doc=self.host.document.editor.project.drawings.clone();let page=doc.pages.iter_mut().find(|p|p.id==id).unwrap();
                        if page.pixels!=candidate.pixels {return Err("Source dimensions changed; place the changed image as a new drawing".to_string());}
                        page.linked_source=Some(path);page.path=candidate.path;page.sha256=candidate.sha256;
                        for region in doc.regions.iter_mut().filter(|r|r.page==id){region.revision=region.revision.checked_add(1).ok_or("Drawing revision exhausted")?;region.needs_review=true;}
                        self.set_drawings(doc)?;self.drawings_ui.textures_clear();Ok(())
                    })();if let Err(e)=result{self.drawings_ui.error=e;}
                }
                ui.checkbox(&mut self.drawings_ui.confirm_delete,lang.translate("Confirm drawing deletion"));
                if ui.add_enabled(self.drawings_ui.confirm_delete,Button::new(lang.translate("Delete drawing"))).clicked() {
                    let mut doc=self.host.document.editor.project.drawings.clone();
                    if doc.pages.iter().any(|p|p.id==id && p.placement.is_none()) && self.host.document.editor.project.elements.iter().any(|e|e.worksheet==Some(id)) {self.drawings_ui.error="Remove worksheet elements before deleting its legacy source".into();}
                    else {doc.pages.retain(|p|p.id!=id);doc.regions.retain(|r|r.page!=id);if let Err(e)=self.set_drawings(doc){self.drawings_ui.error=e;}else{self.drawings_ui.source=None;self.drawings_ui.confirm_delete=false;}}
                }
            }
            self.worksheet_error(ui);
        });
        self.drawings_ui.manager &= open;
    }
    pub(super) fn drawing_placement_settings(&mut self, ui: &mut Ui, id: u64) {
        let Some(placement) = self
            .host
            .document
            .editor
            .project
            .drawings
            .pages
            .iter()
            .find(|p| p.id == id)
            .and_then(|p| p.placement.clone())
        else {
            return;
        };
        if self
            .drawings_ui
            .placement_draft
            .as_ref()
            .is_none_or(|(old, _)| *old != id)
        {
            self.drawings_ui.placement_draft = Some((id, placement));
            self.drawings_ui.size_mm = self
                .host
                .document
                .editor
                .project
                .drawings
                .pages
                .iter()
                .find(|p| p.id == id)
                .unwrap()
                .pixels[0] as f64
                * self.worksheet_pixel_scale(id) as f64
                * 1000.;
        }
        let lang = self.language;
        let (_, draft) = self.drawings_ui.placement_draft.as_mut().unwrap();
        ui.horizontal(|ui| {
            ui.label("X (m)");
            ui.add(DragValue::new(&mut draft.origin.x));
            ui.label("Y (m)");
            ui.add(DragValue::new(&mut draft.origin.y));
        });
        ui.horizontal(|ui| {
            ui.label(lang.translate("Angle"));
            ui.add(DragValue::new(&mut draft.angle).suffix("°"));
        });
        ui.checkbox(&mut draft.visible, lang.translate("Visible"));
        ui.add(egui::Slider::new(&mut draft.opacity, 0..=255).text(lang.translate("Opacity")));
        ui.horizontal(|ui| {
            ui.label(lang.translate("Drawing width (mm)"));
            ui.add(DragValue::new(&mut self.drawings_ui.size_mm));
        });
        if ui
            .button(lang.translate("Apply drawing placement"))
            .clicked()
        {
            let result = (|| {
                let mut doc = self.host.document.editor.project.drawings.clone();
                let page = doc.pages.iter_mut().find(|p| p.id == id).unwrap();
                page.placement = Some(self.drawings_ui.placement_draft.as_ref().unwrap().1.clone());
                let w = page.pixels[0] as f64;
                let current = w * self.worksheet_pixel_scale(id) as f64 * 1000.;
                if (current - self.drawings_ui.size_mm).abs() > 0.000001 {
                    doc.regions
                        .iter_mut()
                        .find(|r| r.page == id)
                        .ok_or("Select a drawing region")?
                        .calibrate(drawings::Calibration {
                            points: [[0., 0.], [w, 0.]],
                            distance_mm: self.drawings_ui.size_mm,
                        })?;
                }
                self.set_drawings(doc)
            })();
            if let Err(e) = result {
                self.drawings_ui.error = e;
            }
        }
    }
}

use crate::external::{self, ReferenceEdit, ReferenceKind, ReferenceSource};
#[derive(Default)]
pub(super) struct ReferenceUi {
    open: bool,
    kind: Option<ReferenceKind>,
    path: String,
    selected: Option<u64>,
    angle: f64,
    save: bool,
    save_instance: Option<u64>,
    replace: bool,
    error: String,
    pub pending: Option<ReferenceSource>,
}
impl AtelierApp {
    fn selected_reference(&self) -> Option<u64> {
        if self.reference_ui.open
            && let Some(id) = self.reference_ui.selected.filter(|id| {
                self.host
                    .document
                    .editor
                    .project
                    .drawings
                    .reference_instances
                    .iter()
                    .any(|v| v.id == *id)
            })
        {
            return Some(id);
        }
        self.host
            .document
            .editor
            .selection_ids()
            .iter()
            .find_map(|id| {
                self.host
                    .document
                    .editor
                    .project
                    .drawings
                    .reference_owner(*id)
            })
            .or(self.reference_ui.selected)
    }
    fn reference_window_instance(
        &self,
        kind: ReferenceKind,
    ) -> Option<crate::external::ReferenceInstance> {
        let id = self.selected_reference()?;
        self.host
            .document
            .editor
            .project
            .drawings
            .reference_instances
            .iter()
            .find(|v| {
                v.id == id
                    && self
                        .host
                        .document
                        .editor
                        .project
                        .drawings
                        .reference_sources
                        .iter()
                        .any(|s| s.id == v.source && s.kind == kind)
            })
            .cloned()
    }
    fn reference_apply(&mut self, edit: ReferenceEdit) -> Result<(), String> {
        let placed = matches!(
            edit,
            ReferenceEdit::Place { .. }
                | ReferenceEdit::Replace { .. }
                | ReferenceEdit::ReplaceSelection { .. }
        );
        self.host
            .document
            .editor
            .apply(crate::command::ModelCommand::External(edit))?;
        if placed
            && let Some(v) = self
                .host
                .document
                .editor
                .project
                .drawings
                .reference_instances
                .last()
                .cloned()
        {
            self.reference_ui.selected = Some(v.id);
            self.host.document.editor.set_selection(v.elements);
        } else {
            let ids = self.host.document.editor.selection_ids();
            self.host.document.editor.set_selection(ids);
        }
        Ok(())
    }
    fn run_reference_action(&mut self, result: Result<(), String>) {
        if let Err(e) = result {
            self.reference_ui.error = e;
        } else {
            self.reference_ui.error.clear();
        }
    }
    fn save_native_module(&mut self) -> Result<(), String> {
        let mut module = self.host.document.editor.project.clone();
        if let Some(instance) = self.reference_ui.save_instance {
            let v = module
                .drawings
                .reference_instances
                .iter()
                .find(|v| v.id == instance)
                .ok_or("Select a reference instance")?;
            let source = module
                .drawings
                .reference_sources
                .iter()
                .find(|s| s.id == v.source)
                .unwrap();
            module.elements = source.elements.clone();
            module.stories = source.stories.clone();
            module.layers = source.layers.clone();
        } else {
            let ids = self.host.document.editor.selection_ids();
            if ids.is_empty() {
                return Err("Select elements to create a module".into());
            }
            if ids
                .iter()
                .any(|id| self.host.document.editor.locked_element(*id))
            {
                return Err("Selection contains locked elements".into());
            }
            module.elements.retain(|e| {
                ids.contains(&e.id)
                    || matches!(e.shape,Shape::Opening{host_wall,..} if ids.contains(&host_wall))
            });
            if module.elements.iter().any(|e| e.worksheet.is_some()) {
                return Err("Native model modules require floor-plan elements".into());
            }
        }
        module.name = std::path::Path::new(&self.reference_ui.path)
            .file_stem()
            .and_then(|v| v.to_str())
            .unwrap_or("Module")
            .to_owned();
        module.drawings = Default::default();
        module.edit_state = Default::default();
        module.plan_options.active_worksheet = None;
        module.next_id = module
            .elements
            .iter()
            .map(|e| e.id)
            .max()
            .unwrap_or(0)
            .checked_add(1)
            .ok_or("Invalid element identity or story")?;
        let path = std::path::Path::new(self.reference_ui.path.trim());
        if path
            .extension()
            .is_none_or(|v| !v.eq_ignore_ascii_case("json"))
        {
            return Err("Native modules use .json, not MOD or PLN".into());
        }
        if path.exists() {
            return Err("Choose a new file name; existing files are preserved".into());
        }
        crate::storage::save(&module, path)?;
        if self.reference_ui.replace {
            let source = external::source(path, ReferenceKind::Hotlink)?;
            if let Some(instance) = self.reference_ui.save_instance {
                self.reference_apply(ReferenceEdit::Replace { instance, source })?;
            } else {
                // Selection replacement is a single typed transaction, never two history records.
                self.reference_apply(ReferenceEdit::ReplaceSelection {
                    ids: module.elements.iter().map(|e| e.id).collect(),
                    source,
                    story: self.host.document.editor.story,
                })?;
            }
        }
        Ok(())
    }
    pub(super) fn reference_placement_canvas(
        &mut self,
        ui: &mut Ui,
        rect: Rect,
        response: &Response,
    ) -> bool {
        let Some(source) = self.reference_ui.pending.clone() else {
            return false;
        };
        response.clone().on_hover_cursor(CursorIcon::Crosshair);
        if let Some(pos) = response.hover_pos() {
            let at = self.world(rect, pos);
            let placement = DrawingPlacement {
                worksheet: self
                    .host
                    .document
                    .editor
                    .project
                    .plan_options
                    .active_worksheet,
                story: self.host.document.editor.story,
                origin: at,
                angle: self.reference_ui.angle,
                visible: true,
                opacity: 255,
                frame: None,
            };
            let outline = source
                .elements
                .iter()
                .flat_map(|e| {
                    crate::geometry::element_plan_outline(&self.host.document.editor.project, e)
                })
                .map(|p| self.screen(rect, placement.world([p.x as f64, -p.y as f64], 1.)))
                .collect::<Vec<_>>();
            if !outline.is_empty() {
                let bounds = Rect::from_points(&outline);
                ui.painter_at(rect).rect_stroke(
                    bounds,
                    0.,
                    Stroke::new(1.5, ACCENT),
                    StrokeKind::Inside,
                );
            }
            if response.clicked_by(PointerButton::Primary) {
                let result = self.reference_apply(ReferenceEdit::Place { source, placement });
                if result.is_ok() {
                    self.reference_ui.pending = None;
                    self.drawings_ui.cancel();
                    self.reference_ui.selected = self
                        .host
                        .document
                        .editor
                        .project
                        .drawings
                        .reference_instances
                        .last()
                        .map(|v| v.id);
                }
                self.run_reference_action(result);
            }
        }
        true
    }
    pub(super) fn reference_windows(&mut self, ctx: &Context) {
        let lang = self.language;
        if self.reference_ui.save {
            let mut open = true;
            Window::new(lang.translate("Save native module"))
                .id(Id::new("save_native_module"))
                .open(&mut open)
                .show(ctx, |ui| {
                    ui.label(lang.translate(
                        "OpenArchi JSON modules · MOD/PLN compatibility is not available",
                    ));
                    ui.text_edit_singleline(&mut self.reference_ui.path);
                    if ui.button(lang.translate("Browse…")).clicked()
                        && let Some(p) = rfd::FileDialog::new()
                            .add_filter("OpenArchi JSON", &["json"])
                            .save_file()
                    {
                        self.reference_ui.path = p.display().to_string();
                    }
                    ui.checkbox(
                        &mut self.reference_ui.replace,
                        lang.translate("Replace with linked module"),
                    );
                    if ui.button(lang.translate("Save")).clicked() {
                        let result = self.save_native_module();
                        if result.is_ok() {
                            self.reference_ui.save = false;
                        }
                        self.run_reference_action(result);
                    }
                    if !self.reference_ui.error.is_empty() {
                        ui.colored_label(Color32::RED, lang.translate(&self.reference_ui.error));
                    }
                });
            if !open {
                self.reference_ui.save = false;
            }
        }
        if !self.reference_ui.open {
            return;
        }
        let kind = self.reference_ui.kind.unwrap_or(ReferenceKind::Hotlink);
        let title = if kind == ReferenceKind::Hotlink {
            "Hotlink Module Manager…"
        } else {
            "Xref Manager…"
        };
        let mut open = true;
        Window::new(lang.translate(title))
            .id(Id::new("reference_manager"))
            .open(&mut open)
            .default_width(620.)
            .show(ctx, |ui| {
                ui.label(lang.translate(if kind == ReferenceKind::Hotlink {
                    "OpenArchi JSON modules · MOD/PLN compatibility is not available"
                } else {
                    "Xref: OpenArchi JSON or planar ASCII DXF (LINE / straight LWPOLYLINE); no DWG"
                }));
                ui.text_edit_singleline(&mut self.reference_ui.path);
                if ui.button(lang.translate("Browse…")).clicked()
                    && let Some(p) = rfd::FileDialog::new().pick_file()
                {
                    self.reference_ui.path = p.display().to_string();
                }
                ui.horizontal(|ui| {
                    ui.label(lang.translate("Angle"));
                    ui.add(DragValue::new(&mut self.reference_ui.angle).suffix("°"));
                });
                if ui
                    .button(lang.translate("Choose source and place"))
                    .clicked()
                {
                    match external::source(
                        std::path::Path::new(self.reference_ui.path.trim()),
                        kind,
                    ) {
                        Ok(source) => {
                            self.drawings_ui.mode = Some(Pick::Place);
                            self.reference_ui.pending = Some(source);
                            self.reference_ui.open = false;
                            self.reference_ui.error.clear();
                        }
                        Err(e) => self.reference_ui.error = e,
                    }
                }
                let instances = self
                    .host
                    .document
                    .editor
                    .project
                    .drawings
                    .reference_instances
                    .clone();
                ScrollArea::vertical().max_height(200.).show(ui, |ui| {
                    for v in instances {
                        let Some(source) = self
                            .host
                            .document
                            .editor
                            .project
                            .drawings
                            .reference_sources
                            .iter()
                            .find(|s| s.id == v.source && s.kind == kind)
                        else {
                            continue;
                        };
                        let status = self
                            .drawings_ui
                            .statuses
                            .get(&source.id)
                            .map_or("Needs checking", String::as_str);
                        if ui
                            .selectable_label(
                                self.selected_reference() == Some(v.id),
                                format!(
                                    "#{} {} · {} · {}",
                                    v.id,
                                    source.name,
                                    lang.translate(status),
                                    lang.translate(if v.loaded { "Loaded" } else { "Unloaded" })
                                ),
                            )
                            .clicked()
                        {
                            self.reference_ui.selected = Some(v.id);
                        }
                    }
                });
                if let Some(instance) = self.reference_window_instance(kind) {
                    let source = self
                        .host
                        .document
                        .editor
                        .project
                        .drawings
                        .reference_sources
                        .iter()
                        .find(|v| v.id == instance.source)
                        .cloned()
                        .unwrap();
                    ui.label(source.path.display().to_string());
                    ui.horizontal_wrapped(|ui| {
                        if ui.button(lang.translate("Check status")).clicked() {
                            let status = match drawings::source_bytes(&source.path) {
                                Ok(b) => {
                                    if drawings::digest(&b) == source.sha256 {
                                        "Up to date"
                                    } else {
                                        "Modified"
                                    }
                                }
                                Err(_) => "Missing",
                            };
                            self.drawings_ui
                                .statuses
                                .insert(source.id, status.to_owned());
                        }
                        if ui
                            .button(lang.translate("Update source and all instances"))
                            .clicked()
                        {
                            let result =
                                external::source(&source.path, source.kind).and_then(|mut s| {
                                    s.id = source.id;
                                    self.reference_apply(ReferenceEdit::Update { source: s })
                                });
                            self.run_reference_action(result);
                        }
                        if ui.button(lang.translate("Relink source")).clicked() {
                            let result = external::source(
                                std::path::Path::new(self.reference_ui.path.trim()),
                                source.kind,
                            )
                            .and_then(|mut s| {
                                s.id = source.id;
                                self.reference_apply(ReferenceEdit::Update { source: s })
                            });
                            self.run_reference_action(result);
                        }
                        if ui
                            .button(lang.translate("Change selected module"))
                            .clicked()
                        {
                            let result = external::source(
                                std::path::Path::new(self.reference_ui.path.trim()),
                                source.kind,
                            )
                            .and_then(|s| {
                                self.reference_apply(ReferenceEdit::Replace {
                                    instance: instance.id,
                                    source: s,
                                })
                            });
                            self.run_reference_action(result);
                        }
                        if kind == ReferenceKind::Hotlink
                            && ui
                                .button(lang.translate("Break Hotlink (Embed Elements)"))
                                .clicked()
                        {
                            let result = self.reference_apply(ReferenceEdit::Break {
                                instance: instance.id,
                            });
                            self.run_reference_action(result);
                        }
                        if ui.button(lang.translate("Bind all instances")).clicked() {
                            let result =
                                self.reference_apply(ReferenceEdit::Bind { source: source.id });
                            self.run_reference_action(result);
                        }
                        if ui.button(lang.translate("Unload")).clicked() {
                            let result = self.reference_apply(ReferenceEdit::Loaded {
                                source: source.id,
                                loaded: false,
                            });
                            self.run_reference_action(result);
                        }
                        if ui.button(lang.translate("Reload")).clicked() {
                            let result =
                                external::source(&source.path, source.kind).and_then(|mut s| {
                                    s.id = source.id;
                                    self.reference_apply(ReferenceEdit::Reload { source: s })
                                });
                            self.run_reference_action(result);
                        }
                        if ui
                            .button(lang.translate("Delete selected instance"))
                            .clicked()
                        {
                            let result = self.reference_apply(ReferenceEdit::Delete {
                                instance: instance.id,
                            });
                            self.run_reference_action(result);
                        }
                    });
                }
                if !self.reference_ui.error.is_empty() {
                    ui.colored_label(Color32::RED, lang.translate(&self.reference_ui.error));
                }
            });
        if !open {
            self.reference_ui.open = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_module_save_replace_and_export_selected_instance_use_exact_history_and_files() {
        let mut app = AtelierApp::from_renderer(None, "external module test".into());
        app.recovery_slot = None;
        app.host.document.editor = Editor::new(Project::default());
        let id = app.host.document.editor.project.add(
            0,
            Material::Plaster,
            Shape::Polyline {
                points: vec![Point::new(1., 2.), Point::new(3., 2.)],
                arcs: vec![],
            },
        );
        app.host.document.editor.mark_saved();
        app.host.document.editor.set_selection(vec![id]);
        let root = std::env::temp_dir().join(format!("oa-module-save-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let file = root.join("selected.json");
        let _ = std::fs::remove_file(&file);
        app.reference_ui.path = file.display().to_string();
        app.reference_ui.replace = true;
        let before = app.host.document.editor.project.clone();
        app.save_native_module().unwrap();
        assert_eq!(app.host.document.editor.undo.len(), 1);
        assert_eq!(
            app.host
                .document
                .editor
                .project
                .drawings
                .reference_instances
                .len(),
            1
        );
        assert_eq!(
            crate::storage::load(&file).unwrap().elements,
            before.elements
        );
        let linked = app.host.document.editor.project.clone();
        #[cfg(target_os = "macos")]
        {
            app.host.document.editor.set_selection(vec![]);
            app.reference_ui.open = true;
            app.reference_ui.selected = None;
            let before = app.menu_signature();
            app.reference_ui.selected = Some(linked.drawings.reference_instances[0].id);
            assert_ne!(app.menu_signature(), before);
            app.reference_ui.open = false;
        }

        app.reference_ui.open = true;
        assert!(
            app.reference_window_instance(ReferenceKind::Hotlink)
                .is_some()
        );
        assert!(app.reference_window_instance(ReferenceKind::Xref).is_none());
        app.reference_ui.open = false;

        assert!(app.save_native_module().is_err());
        assert_eq!(app.host.document.editor.project, linked);
        app.host.document.editor.undo();
        assert_eq!(app.host.document.editor.project, before);
        app.host.document.editor.redo();
        assert_eq!(app.host.document.editor.project, linked);
        app.reference_ui.save_instance = Some(linked.drawings.reference_instances[0].id);
        app.reference_ui.replace = false;
        app.reference_ui.path = root.join("exported.json").display().to_string();
        app.save_native_module().unwrap();
        assert_eq!(app.host.document.editor.project, linked);
        assert_eq!(
            crate::storage::load(&root.join("exported.json"))
                .unwrap()
                .elements,
            before.elements
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}
