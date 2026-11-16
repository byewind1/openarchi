use super::*;
use crate::plugins::{
    self, Manifest,
    jobs::{Bundle, Terminal},
};
#[derive(Default)]
pub(super) struct PluginUi {
    pub open: bool,
    solar_draft: Option<super::solar_ui::Draft>,
    unresolved_job: Option<(String, String)>,
    pub error: String,
    path: String,
    draft: Option<Manifest>,
    executable_path: String,
    reviewed_executable: Option<plugins::provider::ExecutableGrant>,
    collapse_declaration: bool,
    collapse_history: bool,
    project: String,
    document_session: Option<u64>,
    ids: Vec<String>,
    selected: Option<String>,
    pub(super) record: Option<Bundle>,
    process: Option<plugins::provider::Capture>,
    execution: Option<plugins::worker::Observation>,
    record_revision: u64,
    pub(super) original_request: String,
    pub(super) export_path: String,
    import_path: String,
    import_status: String,
    pub(super) import_worker:
        Option<std::thread::JoinHandle<Result<plugins::jobs::Identity, String>>>,
    collapse_import: bool,
}
fn reviewed_object_buttons(
    ui: &mut Ui,
    language: UiLanguage,
    record: &Bundle,
    current: bool,
    ready: bool,
) -> Option<u64> {
    let mut selected = None;
    ui.collapsing(language.translate("Reviewed objects"), |ui| {
        for element in &record.intent.input.elements {
            let label = format!(
                "{} · {} #{}",
                language.translate("Inspect reviewed object in 3D"),
                language.translate(element.kind()),
                element.id
            );
            let response = ui.add_enabled(current && ready, Button::new(label));
            if response.clicked() {
                selected = Some(element.id);
            }
            if !current {
                response.on_hover_text(
                    language.translate("Job result is stale for the current candidate"),
                );
            } else if !ready {
                response.on_hover_text(language.translate(
                    "Finish or cancel the current model interaction before changing design tasks",
                ));
            }
        }
    });
    selected
}
impl AtelierApp {
    fn inspect_plugin_member(&mut self, request: &str, id: u64) {
        let result = (|| {
            if !self.can_review_object_discussions() || self.menu_gated() {
                return Err(
                    "Finish or cancel the current model interaction before changing design tasks"
                        .into(),
                );
            }
            let record = self
                .plugins_ui
                .record
                .as_ref()
                .filter(|b| b.intent.identity.request_id == request)
                .ok_or("Original job selection changed")?;
            record.reviewed_member(&self.host.document.editor, id)?;
            self.host.document.editor.set_selection(vec![id]);
            self.sync_process_space();
            self.show_right = true;
            self.tab = Tab::Properties;
            self.fit_selection();
            self.plugins_ui.open = false;
            Ok::<_, String>(())
        })();
        self.plugins_ui.error = result.err().unwrap_or_default();
    }
    pub(super) fn run_plugin_command(&mut self, command: &str) {
        if command == plugins::solar::COMMAND {
            if let Some(b) = self
                .plugins_ui
                .record
                .as_ref()
                .filter(|b| b.result.is_none())
            {
                self.plugins_ui.unresolved_job = Some((
                    b.intent.identity.project_id.clone(),
                    b.intent.identity.request_id.clone(),
                ));
                self.plugins_ui.original_request = b.intent.identity.request_id.clone();
            }
            if self
                .plugins_ui
                .unresolved_job
                .as_ref()
                .is_some_and(|(p, _)| p == self.host.document.editor.project_identity())
            {
                self.plugins_ui.error="Original job remains unresolved; inspect or cancel it before starting another study".into();
                self.plugins_ui.open = true;
                return;
            }
            let project = self.host.document.editor.project_identity().to_owned();
            let session = self.host.document.editor.document_session();
            if self.plugins_ui.project != project
                || self.plugins_ui.document_session != Some(session)
            {
                self.plugins_ui.project = project;
                self.plugins_ui.document_session = Some(session);
                self.plugins_ui.selected = None;
                self.plugins_ui.record = None;
                self.plugins_ui.process = None;
                self.plugins_ui.ids.clear();
            }
            let input = self
                .plugins_ui
                .record
                .as_ref()
                .filter(|b| {
                    b.intent.identity.project_id == self.host.document.editor.project_identity()
                })
                .and_then(|b| b.intent.input.solar.as_ref());
            match super::solar_ui::Draft::from_input(input) {
                Ok(d) => {
                    self.plugins_ui.solar_draft = Some(d);
                    self.plugins_ui.error.clear();
                }
                Err(e) => self.plugins_ui.error = e,
            }
            self.plugins_ui.open = true;
            return;
        }
        self.plugins_ui.collapse_declaration = true;
        let id = storage::new_document_identity();
        self.plugins_ui.original_request = id.clone();
        match self.host.start_plugin_job(command, &id) {
            Ok(identity) => {
                self.plugins_ui.project = identity.project_id;
                self.select_plugin_job(id);
            }
            Err(e) => self.plugins_ui.error = format!("{e} · {id}"),
        }
        self.plugins_ui.open = true;
    }
    fn select_plugin_job(&mut self, id: String) {
        self.plugins_ui.collapse_history = true;
        if self.plugins_ui.selected.as_ref() != Some(&id) || self.plugins_ui.export_path.is_empty()
        {
            self.plugins_ui.export_path = format!("{}.{}.job.json", self.path, &id[..12]);
        }
        self.plugins_ui.selected = Some(id);
        self.plugins_ui.record = None;
        self.plugins_ui.process = None;
        self.plugins_ui.execution = None;
        self.plugins_ui.error.clear();
        self.reload_plugin_jobs();
    }
    fn reload_plugin_jobs(&mut self) {
        if let Some(jobs) = &self.host.jobs {
            match jobs.ids_with_archive(
                self.host.document.editor.project_identity(),
                &self.host.document.editor.project.job_archive,
            ) {
                Ok(ids) => {
                    self.plugins_ui.ids = ids;
                }
                Err(e) => self.plugins_ui.error = e,
            }
        }
        if let (Some(jobs), Some(id)) = (&self.host.jobs, &self.plugins_ui.selected) {
            match jobs.bundle_with_archive(
                self.host.document.editor.project_identity(),
                id,
                &self.host.document.editor.project.job_archive,
            ) {
                Ok(b) => {
                    self.plugins_ui.execution =
                        if b.result.is_none() && b.intent.execution.is_some() {
                            match jobs.execution_observation(&b.intent.identity) {
                                Ok(observed) => Some(observed),
                                Err(error) => {
                                    self.plugins_ui.error = error;
                                    None
                                }
                            }
                        } else {
                            None
                        };
                    match jobs.process_with_archive(
                        &b.intent.identity,
                        &self.host.document.editor.project.job_archive,
                    ) {
                        Ok(process) => self.plugins_ui.process = process,
                        Err(e) => {
                            self.plugins_ui.process = None;
                            self.plugins_ui.error = e;
                        }
                    }
                    if b.result.is_some()
                        && self
                            .plugins_ui
                            .unresolved_job
                            .as_ref()
                            .is_some_and(|(p, id)| {
                                p == &b.intent.identity.project_id
                                    && id == &b.intent.identity.request_id
                            })
                    {
                        self.plugins_ui.unresolved_job = None;
                    }
                    self.plugins_ui.record = Some(b);
                    self.plugins_ui.record_revision = self.host.document.editor.revision;
                }
                Err(e) => {
                    self.plugins_ui.record = None;
                    self.plugins_ui.error = e
                }
            }
        }
    }
    pub(super) fn plugins_window(&mut self, ctx: &Context) {
        if let Some(jobs) = &mut self.host.jobs {
            jobs.poll();
        }
        if self
            .plugins_ui
            .import_worker
            .as_ref()
            .is_some_and(|w| w.is_finished())
        {
            let result = self.plugins_ui.import_worker.take().unwrap().join();
            match result {
                Ok(Ok(identity)) => {
                    self.plugins_ui.collapse_import = true;
                    if identity.project_id == self.host.document.editor.project_identity() {
                        self.plugins_ui.project = identity.project_id;
                        self.select_plugin_job(identity.request_id);
                        self.plugins_ui.import_status =
                            "Original job record restored; no provider was enabled or run.".into();
                    } else {
                        self.plugins_ui.import_status = "Job imported for its original project; return to that project to inspect it.".into();
                    }
                }
                Ok(Err(e)) => self.plugins_ui.error = e,
                Err(_) => {
                    self.plugins_ui.error =
                        "Import outcome unknown; inspect the original job identity before retrying."
                            .into()
                }
            }
        }
        if self.plugins_ui.import_worker.is_some() {
            ctx.request_repaint_after(Duration::from_millis(50));
        }
        if !self.plugins_ui.open {
            return;
        }
        let project = self.host.document.editor.project_identity().to_owned();
        let session = self.host.document.editor.document_session();
        if project != self.plugins_ui.project || self.plugins_ui.document_session != Some(session) {
            self.plugins_ui.project = project;
            self.plugins_ui.document_session = Some(session);
            self.plugins_ui.selected = None;
            self.plugins_ui.solar_draft = None;
            self.plugins_ui.record = None;
            self.plugins_ui.ids.clear();
            self.reload_plugin_jobs();
            if let Some(jobs) = &self.host.jobs {
                match jobs.preferred_initial_job(
                    &self.plugins_ui.project,
                    &self.host.document.editor.project.job_archive,
                ) {
                    Ok(Some(id)) => self.select_plugin_job(id),
                    Ok(None) => {}
                    Err(error) => self.plugins_ui.error = error,
                }
            }
        }
        let running = self
            .plugins_ui
            .selected
            .as_ref()
            .and_then(|id| {
                self.host
                    .jobs
                    .as_ref()
                    .and_then(|j| j.progress(&self.plugins_ui.project, id))
            })
            .is_some();
        if self
            .plugins_ui
            .record
            .as_ref()
            .is_some_and(|b| b.result.is_none())
        {
            self.reload_plugin_jobs();
            // Includes original workers surviving a GUI restart. Unknown owners
            // remain unknown; repainting never dispatches or grants authority.
            ctx.request_repaint_after(Duration::from_secs(1));
        }
        if running {
            ctx.request_repaint_after(Duration::from_millis(50));
        }
        let mut open = true;
        let l = self.language;
        let entries = self.host.plugins.entries().to_vec();
        let mut run = None;
        let mut solar_action = None;
        let mut inspect_unresolved = false;
        let mut reuse_solar = false;
        let mut inspect_object = None;
        let inspection_ready = self.can_review_object_discussions() && !self.menu_gated();
        let mut toggle = None;
        let mut choose = false;
        let mut preview = false;
        let mut enable = false;
        let mut choose_executable = false;
        let mut review_executable = false;
        let mut selected = None;
        let mut cancel = false;
        let mut export = false;
        let mut refresh = false;
        let mut close = false;
        let mut choose_import = false;
        let mut import = false;
        egui::Window::new(l.translate("Plugins and jobs…")).id(Id::new("plugin_jobs")).open(&mut open).default_width(620.).default_height(760.).resizable(true).show(ctx,|ui|{
   ui.label(l.translate("Read-only review of the current candidate; pending edits are excluded."));
   ui.label(format!("{} · {}",self.host.document.editor.project.name,self.host.document.editor.project.plan_options.active_design_option.and_then(|id|self.host.document.editor.project.design_options.name(id)).unwrap_or(l.translate("Main model"))));
   if ui.button(l.translate("Close")).clicked(){close=true;}
   egui::ScrollArea::vertical().max_height(620.).auto_shrink([false,false]).show(ui,|ui|{
    if let Some((project,id))=&self.plugins_ui.unresolved_job {if project==self.host.document.editor.project_identity(){ui.label(l.translate("Original job remains unresolved; inspect or cancel it before starting another study"));ui.monospace(id);if ui.button(l.translate("Inspect original job outcome")).clicked(){inspect_unresolved=true;}}}
    if let Some(d)=&mut self.plugins_ui.solar_draft{solar_action=super::solar_ui::contents(ui,l,d,&self.host.document.editor.project);}
    for p in &entries{ui.horizontal_wrapped(|ui|{let mut enabled=p.enabled;if ui.checkbox(&mut enabled,l.translate(&p.manifest.label)).changed(){toggle=Some((p.manifest.id.clone(),enabled));}ui.label(format!("v{}",p.manifest.version));});
     if p.enabled{for c in &p.manifest.commands{if ui.button(l.translate(&c.label)).clicked(){run=Some(c.id.clone());}}}
    }
    CollapsingHeader::new(l.translate("Load a local plugin declaration")).id_salt("local_plugin_declaration").open(self.plugins_ui.collapse_declaration.then_some(false)).show(ui,|ui|{
     let label=ui.label(l.translate("Plugin declaration path"));ui.add(TextEdit::singleline(&mut self.plugins_ui.path).hint_text(l.translate("Choose a local JSON declaration")).desired_width(470.)).labelled_by(label.id);
     ui.horizontal(|ui|{if ui.button(l.translate("Browse…")).clicked(){choose=true;}if ui.button(l.translate("Read declaration")).clicked(){preview=true;}});
     if let Some(m)=&self.plugins_ui.draft{ui.label(format!("{} · {} · v{}",m.label,m.id,m.version));
      if m.provider.is_some(){
       ui.label(l.translate("External program permission: reads this candidate and declared requirements. Runs with your OS account; process separation is not a sandbox."));
       let label=ui.label(l.translate("Local provider executable"));
       if ui.add(TextEdit::singleline(&mut self.plugins_ui.executable_path).hint_text(l.translate("Choose a local executable")).desired_width(470.)).labelled_by(label.id).changed(){self.plugins_ui.reviewed_executable=None;}
       ui.horizontal(|ui|{if ui.button(l.translate("Choose executable…")).clicked(){choose_executable=true;}if ui.button(l.translate("Read executable identity")).clicked(){review_executable=true;}});
       if let Some(grant)=&self.plugins_ui.reviewed_executable{ui.monospace(grant.path.display().to_string());ui.monospace(&grant.sha256);if ui.button(l.translate("Authorize and enable reviewed executable")).clicked(){enable=true;}}
      }else{ui.label(l.translate(if m.permissions.contains(&plugins::Permission::ReadProgram){"Permission: read authored geometry and declared requirements; no model writes or executable installation."}else{"Permission: read authored geometry only; no model writes or executable installation."}));if ui.button(l.translate("Enable reviewed plugin")).clicked(){enable=true;}}
      for c in &m.commands{ui.label(format!("{} · {}",c.label,c.id));}
     }
    });
    CollapsingHeader::new(l.translate("Restore an exported job record")).id_salt("restore_plugin_record").open(self.plugins_ui.collapse_import.then_some(false)).show(ui,|ui|{
     let label=ui.label(l.translate("Exported job record path"));ui.add(TextEdit::singleline(&mut self.plugins_ui.import_path).hint_text(l.translate("Choose an exported job record")).desired_width(470.)).labelled_by(label.id);
     ui.label(l.translate("Restores evidence for this project only; does not install, enable or run a provider. Saved exports do not include process logs."));
     ui.add_enabled_ui(self.plugins_ui.import_worker.is_none(),|ui|{ui.horizontal(|ui|{if ui.button(l.translate("Browse job record…")).clicked(){choose_import=true;}if ui.button(l.translate("Import original job record")).clicked(){import=true;}});});
    });
    if self.plugins_ui.import_worker.is_some(){ui.label(l.translate("Reading and validating the original job record…"));}
    if !self.plugins_ui.import_status.is_empty(){ui.label(l.translate(&self.plugins_ui.import_status));}
    ui.separator();if ui.button(l.translate("Refresh jobs")).clicked(){refresh=true;}
    CollapsingHeader::new(l.translate("Job history")).id_salt("job_history").open(self.plugins_ui.collapse_history.then_some(false)).default_open(false).show(ui,|ui|{
    ui.horizontal_wrapped(|ui|{for id in &self.plugins_ui.ids{if ui.selectable_label(self.plugins_ui.selected.as_ref()==Some(id),self.plugins_ui.record.as_ref().filter(|b|b.intent.identity.request_id==*id).or_else(||self.host.document.editor.project.job_archive.records().iter().find(|r|r.bundle.intent.identity.request_id==*id).map(|r|&r.bundle)).map(|b|format!("{} · {} · {}",l.translate(&b.intent.declaration.panel_title),b.intent.input.option_name,&id[..12])).unwrap_or_else(||id[..12].to_owned())).on_hover_text(id).clicked(){selected=Some(id.clone());}}});
    });
    if let Some(b)=&self.plugins_ui.record{
     ui.heading(l.translate(&b.intent.declaration.panel_title));ui.label(format!("{} · {}",l.translate("Original job request"),b.intent.identity.request_id));
     ui.label(l.translate(if running{"Running"}else{b.phase()}));
     if let Some(execution)=&self.plugins_ui.execution {
      ui.label(l.translate(if execution.status=="Running"{"Original execution is still active; cancellation goes to that worker."}else{"Original execution owner is unknown. Do not replay or signal a saved PID."}));
     }
     let current = b.current(&self.host.document.editor);
     ui.label(l.translate(if current{"Job input matches current candidate"}else{"Job result is stale for the current candidate"}));
     if !self.host.plugins.entries().iter().any(|p|p.enabled&&p.manifest==b.intent.declaration&&p.executable.as_ref().map(|g|&g.sha256)==b.intent.execution.as_ref().map(|g|&g.sha256)){ui.label(l.translate("Provider unavailable; saved input and result retained."));}
     ui.label(format!("{} · {}",b.intent.input.option_name,b.intent.identity.input_sha256));
     if let Some(process)=&self.plugins_ui.process {ui.collapsing(l.translate("Provider process diagnostics"),|ui|{
      ui.label(format!("PID {} · {}: {:?} · {}: {} · {}: {}",process.pid,l.translate("Exit code"),process.exit_code,l.translate("Timed out"),process.timed_out,l.translate("Cancel requested"),process.cancelled));
      if let Some(e)=&process.protocol_error{ui.label(e);}
      for (key,log)in[("Provider stdout",&process.stdout),("Provider stderr",&process.stderr)]{ui.label(format!("{} · {}/{} bytes",l.translate(key),log.bytes.len(),log.total_bytes));if log.truncated(){ui.label(l.translate("Provider log truncated; full stream was not retained."));}ui.monospace(String::from_utf8_lossy(&log.bytes));}
     });}
     if let Some(result)=&b.result{match &result.terminal{Terminal::Completed{report}=>{if let Some(s)=&report.solar{super::solar_ui::result_contents(ui,l,s);if ui.button(l.translate("Reuse this study's inputs")).clicked(){reuse_solar=true;}}else{for (key,t)in[("Authored Zones",&report.zones),("Authored concept masses",&report.concept_masses)]{ui.label(format!("{}: {} · {:.4} m² · {:.4} m³",l.translate(key),t.element_ids.len(),t.area_m2,t.volume_m3));}ui.label(l.translate("Domains and overlaps are counted separately; not net/legal area or compliance."));if let Some(id) = reviewed_object_buttons(ui,l,b,current,inspection_ready) { inspect_object=Some((b.intent.identity.request_id.clone(),id)); }
      if let Some(review)=&report.program_review{ui.label(l.translate("Purpose-label candidates are unverified; no approved requirement mapping or compliance verdict."));if review.rows.is_empty(){ui.label(l.translate("No declared requirements; review remains unknown."));}else{ui.collapsing(l.translate("Declared requirement review"),|ui|{for row in &review.rows{let required=match &row.required_area{crate::program::Area::Known{m2}=>format!("{m2:.4} m²"),crate::program::Area::Unknown{reason}=>format!("{} · {}",l.translate("Unknown"),reason)};let delta=row.delta_m2.map(|v|format!("{v:.4} m²")).unwrap_or_else(||l.translate("Unknown").into());ui.label(format!("{} · {} · {} · {}: {} · {}: {:.4} m² · Δ {}",row.sheet_id,row.key,row.label,l.translate("Declared target area"),required,l.translate("Candidate Zone area"),row.candidate_area_m2,delta));ui.label(l.translate(&row.mapping_status));}ui.label(format!("{}: {:?}",l.translate("Zones without approved assignment"),review.unassigned_zone_ids));});}}
     }},Terminal::Failed{message}=>{ui.colored_label(Color32::LIGHT_RED,l.translate(message));},Terminal::Cancelled=>{}}}
     if b.result.is_none()&&!b.cancel_requested&&ui.button(l.translate("Cancel original job")).clicked(){cancel=true;}
     ui.collapsing(l.translate("Saved job input"),|ui|{ui.monospace(serde_json::to_string_pretty(&b.intent).unwrap_or_default());});
     let label=ui.label(l.translate("New job record path"));ui.add(TextEdit::singleline(&mut self.plugins_ui.export_path).desired_width(470.)).labelled_by(label.id);if ui.button(l.translate("Export job record")).clicked(){export=true;}
    }
    if !self.plugins_ui.error.is_empty(){let label=if self.plugins_ui.error.starts_with("Solar start failed: "){l.translate("Solar study could not start; inspect the original request and diagnostic details.")}else{l.translate(&self.plugins_ui.error)};ui.colored_label(Color32::LIGHT_RED,label).on_hover_text(&self.plugins_ui.error);}
    if let Some(j)=&self.host.jobs{for e in &j.diagnostics{ui.colored_label(Color32::LIGHT_RED,e);}}
    ui.label(l.translate("Saved projects carry original job records. Running processes and execution permissions are not restored."));
   });
  });
        self.plugins_ui.open = open && !close;
        self.plugins_ui.collapse_declaration = false;
        self.plugins_ui.collapse_history = false;
        self.plugins_ui.collapse_import = false;
        if choose_import {
            if let Some(p) = rfd::FileDialog::new()
                .add_filter("OpenArchi job JSON", &["json"])
                .pick_file()
            {
                self.plugins_ui.import_path = p.display().to_string();
            }
        }
        if import && self.plugins_ui.import_worker.is_none() {
            self.plugins_ui.import_status.clear();
            self.plugins_ui.error.clear();
            match self
                .host
                .import_plugin_job(std::path::PathBuf::from(&self.plugins_ui.import_path))
            {
                Ok(worker) => self.plugins_ui.import_worker = Some(worker),
                Err(e) => self.plugins_ui.error = e,
            }
        }
        if choose {
            if let Some(p) = rfd::FileDialog::new()
                .add_filter("JSON", &["json"])
                .pick_file()
            {
                self.plugins_ui.path = p.display().to_string();
                self.plugins_ui.draft = None;
                self.plugins_ui.reviewed_executable = None;
            }
        }
        if preview {
            self.plugins_ui.draft = None;
            self.plugins_ui.reviewed_executable = None;
            match plugins::read(std::path::Path::new(&self.plugins_ui.path))
                .and_then(|b| Manifest::parse(&b))
            {
                Ok(m) => {
                    self.plugins_ui.draft = Some(m);
                    self.plugins_ui.error.clear();
                }
                Err(e) => self.plugins_ui.error = e,
            }
        }
        if choose_executable {
            if let Some(p) = rfd::FileDialog::new().pick_file() {
                self.plugins_ui.executable_path = p.display().to_string();
                self.plugins_ui.reviewed_executable = None;
            }
        }
        if review_executable {
            self.plugins_ui.reviewed_executable = None;
            match plugins::provider::ExecutableGrant::review(std::path::Path::new(
                &self.plugins_ui.executable_path,
            )) {
                Ok(grant) => {
                    self.plugins_ui.reviewed_executable = Some(grant);
                    self.plugins_ui.error.clear();
                }
                Err(e) => self.plugins_ui.error = e,
            }
        }
        if enable {
            if let Some(m) = self.plugins_ui.draft.clone() {
                let result = if m.provider.is_some() {
                    self.plugins_ui
                        .reviewed_executable
                        .clone()
                        .ok_or_else(|| "Review the executable identity first".to_string())
                        .and_then(|g| self.host.plugins.enable_reviewed_executable(m, g))
                } else {
                    self.host.plugins.enable(m)
                };
                match result {
                    Ok(()) => {
                        self.plugins_ui.draft = None;
                        self.plugins_ui.reviewed_executable = None;
                        self.plugins_ui.collapse_declaration = true;
                        self.plugins_ui.error.clear();
                    }
                    Err(e) => self.plugins_ui.error = e,
                }
            }
        }
        if let Some((id, enabled)) = toggle {
            if let Err(e) = self.host.set_plugin_enabled(&id, enabled) {
                self.plugins_ui.error = e;
            }
            self.reload_plugin_jobs();
        }
        if let Some(id) = selected {
            self.select_plugin_job(id);
        }
        if let Some((request, id)) = inspect_object {
            self.inspect_plugin_member(&request, id);
        }
        if inspect_unresolved {
            if let Some((project, id)) = self.plugins_ui.unresolved_job.clone() {
                if project == self.host.document.editor.project_identity() {
                    self.plugins_ui.solar_draft = None;
                    self.select_plugin_job(id);
                }
            }
        }
        if reuse_solar {
            self.run_plugin_command(plugins::solar::COMMAND);
        }
        if let Some(action) = solar_action {
            match action {
                super::solar_ui::Action::Cancel => self.plugins_ui.solar_draft = None,
                super::solar_ui::Action::Run(parameters) => {
                    let id = storage::new_document_identity();
                    self.plugins_ui.original_request = id.clone();
                    match self.host.start_solar_job(&parameters, &id) {
                        Ok(identity) => {
                            self.plugins_ui.solar_draft = None;
                            self.plugins_ui.project = identity.project_id;
                            self.select_plugin_job(id);
                        }
                        Err(e) => {
                            let project = self.host.document.editor.project_identity().to_owned();
                            let unknown =
                                e.to_ascii_lowercase().contains("unknown")
                                    || self.host.jobs.as_ref().is_some_and(|j| {
                                        match j.ids(&project) {
                                            Ok(ids) => ids.contains(&id),
                                            Err(_) => true,
                                        }
                                    });
                            if unknown {
                                self.plugins_ui.unresolved_job = Some((project, id.clone()));
                                self.plugins_ui.solar_draft = None;
                                self.select_plugin_job(id.clone());
                            }
                            self.plugins_ui.error = format!("Solar start failed: {e} · {id}");
                        }
                    }
                }
            }
        }
        if let Some(command) = run {
            self.run_plugin_command(&command);
        }
        if refresh {
            self.reload_plugin_jobs();
        }
        if cancel {
            if let (Some(j), Some(id)) = (&mut self.host.jobs, &self.plugins_ui.selected) {
                if let Err(e) = j.cancel(&self.plugins_ui.project, id) {
                    self.plugins_ui.error = e;
                }
            }
            self.reload_plugin_jobs();
        }
        if export {
            if let Some(b) = &self.plugins_ui.record {
                match b.export(std::path::Path::new(&self.plugins_ui.export_path)) {
                    Ok(()) => self.plugins_ui.error.clear(),
                    Err(e) => self.plugins_ui.error = e,
                }
            }
        }
    }
}
