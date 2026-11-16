use super::*;
use crate::diagrams::{
    self,
    report::{Edit, Section, Template},
};

#[derive(Clone, PartialEq, Eq)]
pub(super) struct Draft {
    pub original: Option<(u64, String)>,
    pub name: String,
    pub sections: Vec<Section>,
}
impl AtelierApp {
    pub(super) fn clear_report_draft_error(&mut self) {
        if self.drawings_ui.report_error
            == "Confirm or cancel the report template draft before saving or replacing the project"
        {
            self.drawings_ui.report_error.clear();
        }
    }
    pub(super) fn open_report_workspace(&mut self) -> Result<(), String> {
        if self.workbench_pointer_active() || self.pending_design_parameters() {
            return Err(
                "Finish or cancel the current model interaction before applying settings".into(),
            );
        }
        self.drawings_ui.report_open = true;
        if self.drawings_ui.report_draft.is_some() {
            self.drawings_ui.manager = true;
            return Ok(());
        }
        let reports = &self.host.document.editor.project.drawings.reports;
        let report = reports
            .iter()
            .find(|r| Some(r.id) == self.drawings_ui.report_selected)
            .or_else(|| reports.last());
        if let Some(report) = report {
            self.open_report_page(report.id, report.sections[0].figure)
        } else {
            self.drawings_ui.manager = true;
            Ok(())
        }
    }
    fn open_report_page(&mut self, report: u64, figure: u64) -> Result<(), String> {
        if self.menu_gated() || self.workbench_pointer_active() || self.pending_design_parameters()
        {
            return Err(
                "Finish or cancel the current model interaction before applying settings".into(),
            );
        }
        // A missing referenced figure is an inspectable page, never a skipped page.
        self.drawings_ui.report_selected = Some(report);
        self.drawings_ui.report_navigation = Some((report, figure));
        self.drawings_ui.manager = false;
        self.process_ui.open = false;
        self.view = View::Figure(figure);
        Ok(())
    }
    pub(super) fn report_navigation_controls(&mut self, ui: &mut Ui, figure: u64) {
        let Some((id, source)) = self.drawings_ui.report_navigation else {
            return;
        };
        if source != figure {
            return;
        }
        let l = self.language;
        let Some(report) = self
            .host
            .document
            .editor
            .project
            .drawings
            .reports
            .iter()
            .find(|r| r.id == id)
            .cloned()
        else {
            ui.label(l.translate("Report template is missing"));
            return;
        };
        let Some(index) = report.sections.iter().position(|s| s.figure == figure) else {
            ui.label(l.translate("Report section was removed; review the template"));
            return;
        };
        let mut target = None;
        ui.horizontal_wrapped(|ui| {
            ui.strong(format!(
                "{} · {}/{} · {}",
                report.name,
                index + 1,
                report.sections.len(),
                report.sections[index].heading
            ));
            if ui
                .add_enabled(
                    index > 0,
                    Button::new(l.translate("Previous report chapter")),
                )
                .clicked()
            {
                target = Some(index - 1);
            }
            if ui
                .add_enabled(
                    index + 1 < report.sections.len(),
                    Button::new(l.translate("Next report chapter")),
                )
                .clicked()
            {
                target = Some(index + 1);
            }
        });
        if let Some(target) = target {
            if let Err(error) = self.open_report_page(id, report.sections[target].figure) {
                self.drawings_ui.report_error = error;
            }
        }
    }
    pub(super) fn proposed_report(&self, p: &Project) -> Option<Template> {
        p.drawings
            .reports
            .iter()
            .find(|r| {
                self.host
                    .document
                    .editor
                    .project
                    .drawings
                    .reports
                    .iter()
                    .find(|o| o.id == r.id)
                    != Some(*r)
            })
            .cloned()
    }
    pub(super) fn report_controls(&mut self, ui: &mut Ui) {
        let l = self.language;
        ui.checkbox(
            &mut self.drawings_ui.report_open,
            l.translate("Report templates"),
        );
        if !self.drawings_ui.report_open {
            return;
        }
        let reports = self.host.document.editor.project.drawings.reports.clone();
        let mut edit = None;
        let mut open_figure = None;
        let mut show_report = None;
        let scroll = ScrollArea::vertical().id_salt("report_controls").max_height(340.).show(ui,|ui| {
            ui.small(l.translate("Project-bound figure order and chapter titles. Source geometry stays in linked figures."));
            ui.horizontal_wrapped(|ui| {
                if ui.add_enabled(self.drawings_ui.report_draft.is_none(),Button::new(l.translate("New report template"))).clicked() {
                    let sections=if let View::Figure(id)=self.view {
                        self.host.document.editor.project.drawings.diagrams.iter().find(|f|f.id==id).map(|f|vec![Section {heading:f.name.clone(),figure:id}]).unwrap_or_default()
                    } else {vec![]};
                    self.drawings_ui.report_draft=Some(Draft {original:None,name:l.translate("Concept report").into(),sections});
                }
                for report in &reports {
                    if ui.add_enabled(self.drawings_ui.report_draft.is_none(),Button::new(format!("#{} · {}",report.id,report.name))).clicked() {
                        self.drawings_ui.report_selected=Some(report.id);
                        self.drawings_ui.report_show_pages=false;
                        if self.drawings_ui.report_export.trim().is_empty() {
                            self.drawings_ui.report_export=std::path::Path::new(&self.path).parent().unwrap_or(std::path::Path::new(".")).join(format!("report-{}-v{}.svg",report.id,report.generation)).to_string_lossy().into();
                        }
                    }
                }
            });
            if let Some(draft)=&mut self.drawings_ui.report_draft {
                let name=ui.label(l.translate("Report template name"));
                ui.add(TextEdit::singleline(&mut draft.name).desired_width(360.)).labelled_by(name.id);
                let mut change=None;
                for (index,section) in draft.sections.iter_mut().enumerate() {
                    ui.horizontal_wrapped(|ui| {
                        let label=ui.label(format!("{} · #{}",index+1,section.figure));
                        ui.add(TextEdit::singleline(&mut section.heading).desired_width(250.)).labelled_by(label.id);
                        if ui.add_enabled(index>0,Button::new(l.translate("Move up"))).clicked(){change=Some((index,-1));}
                        if ui.button(l.translate("Move down")).clicked(){change=Some((index,1));}
                        if ui.button(l.translate("Remove section")).clicked(){change=Some((index,0));}
                    });
                }
                if let Some((index,direction))=change {
                    match direction {
                        -1=>draft.sections.swap(index,index-1),
                        1 if index+1<draft.sections.len()=>draft.sections.swap(index,index+1),
                        0=>{draft.sections.remove(index);},
                        _=>{}
                    }
                }
                ui.label(l.translate("Add an existing linked figure"));
                for figure in &self.host.document.editor.project.drawings.diagrams {
                    if !draft.sections.iter().any(|s|s.figure==figure.id) && ui.add_enabled(draft.sections.len()<16,Button::new(format!("+ #{} · {}",figure.id,figure.name))).clicked() {
                        draft.sections.push(Section {heading:figure.name.clone(),figure:figure.id});
                    }
                }
                if let Some((id,expected))=&draft.original {
                    if reports.iter().find(|r|r.id==*id).is_none_or(|r|r.expected()!=*expected) {
                        ui.colored_label(Color32::from_rgb(224,125,105),l.translate("Report template changed; review the original draft"));
                    }
                }
                if ui.button(l.translate("Preview report template")).clicked() {
                    edit=Some(match &draft.original {
                        Some((id,expected))=>Edit::Update {id:*id,expected_sha256:expected.clone(),name:draft.name.clone(),sections:draft.sections.clone()},
                        None=>Edit::Create {name:draft.name.clone(),sections:draft.sections.clone()},
                    });
                }
                if ui.button(l.translate("Cancel report edits")).clicked() {self.drawings_ui.report_draft=None;self.clear_report_draft_error();}
            } else if let Some(report)=reports.iter().find(|r|Some(r.id)==self.drawings_ui.report_selected) {
                ui.strong(&report.name);
                let p=&self.host.document.editor.project;
                ui.label(l.translate(report.status(p).label()));
                for (index,section) in report.sections.iter().enumerate() {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(format!("{}. {}",index+1,section.heading));
                        let figure=report.figure(p,index);
                        ui.label(l.translate(figure.map(|f|f.status(p).label()).unwrap_or("Figure is missing")));
                        if ui.add_enabled(figure.is_some(),Button::new(l.translate("Open source figure"))).clicked() {open_figure=Some(section.figure);}
                    });
                }
                ui.horizontal_wrapped(|ui| {
                    if ui.button(l.translate("Edit report template")).clicked() {
                        self.drawings_ui.report_draft=Some(Draft {original:Some((report.id,report.expected())),name:report.name.clone(),sections:report.sections.clone()});
                    }
                    if ui.button(l.translate("Preview delete report template")).clicked(){edit=Some(Edit::Delete{id:report.id,expected_sha256:report.expected()});}
                    ui.checkbox(&mut self.drawings_ui.report_show_pages,l.translate("Preview ordered report pages"));
                    if ui.button(l.translate("Show report on canvas")).clicked(){show_report=Some((report.id,report.sections[0].figure));}
                });
                let label=ui.label(l.translate("New report SVG path"));
                ui.add(TextEdit::singleline(&mut self.drawings_ui.report_export).desired_width(440.)).labelled_by(label.id);
                if ui.button(l.translate("Export ordered report SVG")).clicked() {
                    match report.svg(p).and_then(|svg|super::diagram_ui::export_new_svg(std::path::Path::new(self.drawings_ui.report_export.trim()),svg.as_bytes())) {
                        Ok(())=>{self.drawings_ui.report_status="SVG saved to new file".into();self.drawings_ui.report_error.clear();},
                        Err(error)=>{self.drawings_ui.report_error=error;self.drawings_ui.report_status.clear();},
                    }
                }
                if self.drawings_ui.report_show_pages {Self::report_pages(ui,p,report,l);}
            }
            if !self.drawings_ui.report_error.is_empty(){ui.colored_label(Color32::from_rgb(224,125,105),l.translate(&self.drawings_ui.report_error));}
            if !self.drawings_ui.report_status.is_empty(){ui.label(l.translate(&self.drawings_ui.report_status));}
        });
        #[cfg(test)]
        self.drawings_ui
            .rects
            .insert("report.scroll".into(), scroll.inner_rect);
        #[cfg(not(test))]
        let _ = scroll;
        if let Some(edit) = edit {
            self.preview_figure(diagrams::Edit::Report { edit });
        }
        if let Some(id) = open_figure {
            self.open_figure(id);
        }
        if let Some((id, figure)) = show_report {
            if let Err(error) = self.open_report_page(id, figure) {
                self.drawings_ui.report_error = error;
            }
        }
        ui.separator();
    }
    pub(super) fn report_pages(ui: &mut Ui, p: &Project, report: &Template, l: UiLanguage) {
        for (index, section) in report.sections.iter().enumerate() {
            ui.strong(format!("{}. {}", index + 1, section.heading));
            if let Some(figure) = report.figure(p, index) {
                ui.label(l.translate(figure.status(p).label()));
                Self::paint_figure(ui, figure, l, 460.);
            } else {
                ui.label(l.translate("Figure is missing"));
            }
        }
    }
}
