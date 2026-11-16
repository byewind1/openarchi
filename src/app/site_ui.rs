use super::*;
use crate::{
    application_host::{CommandContext, HostCommand},
    command::ModelCommand,
    site::*,
};
#[derive(Clone)]
pub(super) struct SiteDialog {
    pub context: SiteContext,
    pub base: CommandContext,
    pub error: String,
}
impl AtelierApp {
    pub(super) fn open_site_reference(&mut self) {
        self.site_dialog = Some(SiteDialog {
            context: self.host.document.editor.project.site.clone(),
            base: self.host.context(),
            error: String::new(),
        });
    }
    pub(super) fn apply_site_reference(&mut self) -> Result<(), String> {
        let dialog = self
            .site_dialog
            .as_ref()
            .ok_or("Site reference dialog is closed")?;
        if dialog.base != self.host.context() {
            return Err("Site reference changed while editing; reopen the dialog".into());
        }
        dialog.context.validate()?;
        if dialog.context == self.host.document.editor.project.site {
            return Ok(());
        }
        let mut commands = Vec::new();
        if dialog.context.boundary.is_none()
            && self.host.document.editor.project.site.boundary.is_some()
        {
            commands.push(ModelCommand::DetachSiteBoundary);
        }
        if dialog.context.control_region.is_none()
            && self
                .host
                .document
                .editor
                .project
                .site
                .control_region
                .is_some()
        {
            commands.push(ModelCommand::DetachSiteControlRegion);
        }
        commands.push(ModelCommand::SetSiteContext {
            context: dialog.context.clone(),
        });
        self.host.dispatch(
            dialog.base,
            HostCommand::Apply {
                request_id: crate::storage::new_document_identity(),
                commands,
            },
        )?;
        Ok(())
    }
    pub(super) fn site_reference_ui(&mut self, ctx: &Context) {
        let Some(mut dialog) = self.site_dialog.take() else {
            return;
        };
        let l = self.language;
        let choices = self
            .source_ui
            .repository
            .as_ref()
            .filter(|r| r.manifest.document_id == self.host.document.editor.project_identity())
            .map(|r| {
                r.manifest
                    .sources
                    .iter()
                    .filter(|s| {
                        !matches!(
                            s.quality,
                            crate::project_repository::SourceQuality::Rejected { .. }
                        )
                    })
                    .map(|s| {
                        (
                            s.id.clone(),
                            format!(
                                "{} · {}",
                                s.names.join(" / "),
                                s.id.chars().take(8).collect::<String>()
                            ),
                        )
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let mut accept = false;
        let mut cancel = false;
        let modal=Modal::new(Id::new("site_reference")).show(ctx,|ui|{
   ui.set_width((ctx.content_rect().width()-48.).clamp(280.,620.));ui.heading(l.translate("Site reference"));
   ui.label(l.translate("Declarations are not survey approval. Grid north and true north are separate. Terrain and neighboring geometry remain unknown."));
   site_conditions_fields(ui,&mut dialog.context,l,&choices,ctx.content_rect().height());
   if !dialog.error.is_empty(){ui.colored_label(Color32::LIGHT_RED,l.translate(&dialog.error));}
   ui.horizontal(|ui|{if ui.button(l.translate("Apply")).clicked(){accept=true;}if ui.button(l.translate("Cancel")).clicked(){cancel=true;}});
  });
        if cancel || modal.should_close() {
            return;
        }
        self.site_dialog = Some(dialog);
        if accept {
            match self.apply_site_reference() {
                Ok(()) => self.site_dialog = None,
                Err(error) => self.site_dialog.as_mut().unwrap().error = error,
            }
        }
    }
}
fn site_text(ui: &mut Ui, l: UiLanguage, label: &str, value: &mut String) {
    ui.horizontal(|ui| {
        let label = ui.label(l.translate(label));
        ui.text_edit_singleline(value).labelled_by(label.id);
    });
}
fn site_number(ui: &mut Ui, l: UiLanguage, label: &str, value: &mut f64) {
    ui.horizontal(|ui| {
        let label = ui.label(l.translate(label));
        ui.add(egui::DragValue::new(value).speed(0.01).max_decimals(8))
            .labelled_by(label.id);
    });
}
fn site_pair(ui: &mut Ui, l: UiLanguage, label: &str, value: &mut [f64; 2]) {
    ui.horizontal(|ui| {
        let label = ui.label(l.translate(label));
        for v in value {
            ui.add(egui::DragValue::new(v).speed(0.01).max_decimals(8))
                .labelled_by(label.id);
        }
    });
}
fn evidence(
    ui: &mut Ui,
    value: &mut SiteEvidence,
    l: UiLanguage,
    choices: &[(String, String)],
    scope: &str,
) {
    ui.push_id(scope,|ui|{
        let selected=choices.iter().find(|(id,_)|id==&value.source_sha256).map(|(_,name)|name.as_str()).unwrap_or(l.translate(if value.source_sha256.is_empty(){"Choose reference source"}else{"Reference source is not linked"}));
        egui::ComboBox::from_label(l.translate("Reference source")).selected_text(selected).width(390.).show_ui(ui,|ui|{
            for (id,name) in choices {ui.selectable_value(&mut value.source_sha256,id.clone(),name);}
        });
        if choices.is_empty(){ui.label(l.translate("Open this document's project sources to select a file. Saved declarations remain unverified."));}
        ui.collapsing(l.translate("Source identity details"),|ui|{site_text(ui,l,"Source SHA256",&mut value.source_sha256);});
        site_text(ui, l, "Source location or entity", &mut value.locator);
        site_text(ui, l, "Reference note", &mut value.note);
    });
}

pub(super) fn site_conditions_fields(
    ui: &mut Ui,
    context: &mut SiteContext,
    l: UiLanguage,
    choices: &[(String, String)],
    height: f32,
) {
    if let Some(boundary) = &context.boundary {
        ui.label(format!(
            "{}: {} (#{} )",
            l.translate("Site boundary"),
            boundary.name,
            boundary.element_id
        ));
        if ui
            .button(l.translate("Detach boundary and keep its Polyline"))
            .clicked()
        {
            context.boundary = None;
        }
    }
    if let Some(region) = &context.control_region {
        ui.label(format!(
            "{}: {} (#{} )",
            l.translate("Declared control region"),
            region.name,
            region.element_id
        ));
        ui.label(l.translate("A declared control region is not an approved building line."));
        if ui
            .button(l.translate("Detach control region and keep its Polyline"))
            .clicked()
        {
            context.control_region = None;
        }
    }
    egui::ScrollArea::vertical().max_height((height-180.).max(180.)).show(ui,|ui|{
    let mut known=context.horizontal.is_some();if ui.checkbox(&mut known,l.translate("Declare projected reference")).changed(){context.horizontal=known.then(HorizontalReference::default);}
    if let Some(h)=&mut context.horizontal {
     site_text(ui,l,"Projected CRS (metres)",&mut h.crs);
     site_pair(ui,l,"Projected model origin E / N (m)",&mut h.origin_east_north_m);
     site_number(ui,l,"Model to grid rotation (CCW degrees)",&mut h.model_to_grid_ccw_deg);
     evidence(ui,&mut h.evidence,l,&choices,"horizontal");
    }
    ui.separator();let mut known=context.true_north.is_some();if ui.checkbox(&mut known,l.translate("Declare true north")).changed(){context.true_north=known.then(TrueNorth::default);}
    if let Some(n)=&mut context.true_north {site_number(ui,l,"True north clockwise from model +Y (degrees)",&mut n.clockwise_from_model_y_deg);evidence(ui,&mut n.evidence,l,&choices,"north");}
    ui.separator();let mut known=context.vertical.is_some();if ui.checkbox(&mut known,l.translate("Declare elevation datum")).changed(){context.vertical=known.then(VerticalReference::default);}
    if let Some(v)=&mut context.vertical {site_text(ui,l,"Elevation datum",&mut v.datum);site_number(ui,l,"Elevation at model zero (m)",&mut v.model_zero_elevation_m);evidence(ui,&mut v.evidence,l,&choices,"vertical");}
    ui.separator();ui.strong(l.translate("Coordinate controls"));
    let mut tolerance=context.control_tolerance_m.is_some();if ui.checkbox(&mut tolerance,l.translate("State control tolerance (m)")).changed(){context.control_tolerance_m=tolerance.then_some(0.01);}
    if let Some(t)=&mut context.control_tolerance_m {site_number(ui,l,"Control tolerance (m)",t);}
    let mut remove=None;
    for (index,control) in context.controls.iter_mut().enumerate(){ui.push_id(index,|ui|{
     ui.horizontal(|ui|{ui.label(format!("{} {}",l.translate("Control point"),index+1));if ui.button(l.translate("Remove control")).clicked(){remove=Some(index);}});
     site_pair(ui,l,"Model X / Y (m)",&mut control.model_xy_m);site_pair(ui,l,"Reference E / N (m)",&mut control.reference_east_north_m);evidence(ui,&mut control.evidence,l,&choices,"control");ui.separator();
    });}
    if let Some(index)=remove{context.controls.remove(index);}
    if ui.add_enabled(context.controls.len()<1000,Button::new(l.translate("Add control point"))).clicked(){context.controls.push(CoordinateControl::default());}
    match context.control_check(){
     Ok(ControlCheck::Insufficient{..})=>{ui.label(l.translate("Control check needs a projected reference, two distinct points and an explicit tolerance."));},
     Ok(ControlCheck::Measured{max_residual_m,consistent,..})=>{ui.label(format!("{}: {:.6} m · {}",l.translate("Maximum control residual"),max_residual_m,l.translate(if consistent{"Numerically consistent"}else{"Control mismatch"})));},
     Err(error)=>{ui.colored_label(Color32::LIGHT_RED,l.translate(&error));},
    }
    ui.label(l.translate("Control residuals compare declarations only; they do not certify the source, boundary or legal approval."));
   });
}
