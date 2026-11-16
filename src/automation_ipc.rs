//! Opt-in Unix IPC. Worker threads never access the application's Editor.
#[cfg(unix)]
mod unix {
    use crate::{
        automation::{Query, ReadService, Target},
        editor::Editor,
    };
    use serde::Deserialize;
    use serde_json::{Value, json};
    use sha2::{Digest, Sha256};
    use std::{
        fs,
        io::{Read, Write},
        os::unix::{
            fs::{DirBuilderExt, MetadataExt, PermissionsExt},
            net::UnixListener,
        },
        path::PathBuf,
        sync::{
            Arc, Mutex,
            atomic::{AtomicBool, Ordering},
            mpsc::{self, Receiver, SyncSender},
        },
        thread,
        time::Duration,
    };
    #[derive(Deserialize)]
    #[serde(tag = "method", content = "params", deny_unknown_fields)]
    enum Request {
        // Host-owned continuation only. No wire client can supply an admission.
        #[serde(skip)]
        CitedReady {
            original: Box<Request>,
            admission:
                Result<crate::project_repository::constraints::ConstraintCommitGuard, String>,
            context: String,
            sources: (PathBuf, String),
        },
        #[serde(rename = "inspect_work_environment")]
        EnvironmentInspect(EnvironmentInspect),
        #[serde(rename = "propose_work_environment")]
        EnvironmentPropose(EnvironmentPropose),
        #[serde(rename = "get_environment_proposal")]
        EnvironmentProposal(EnvironmentOperation),
        #[serde(rename = "cancel_environment_proposal")]
        EnvironmentCancel(EnvironmentOperation),
        #[serde(rename = "list_plugin_commands")]
        PluginCommands(crate::plugins::query::CatalogRequest),
        #[serde(rename = "list_plugin_jobs")]
        PluginJobs(crate::plugins::query::ListRequest),
        #[serde(rename = "read_plugin_job")]
        PluginJob(crate::plugins::query::ReadRequest),
        #[serde(rename = "prepare_spatial_intent")]
        PrepareSpatialIntent(crate::automation::intent::Prepare),
        #[serde(rename = "prepare_task_update")]
        PrepareTaskUpdate(crate::automation::intent::PrepareUpdate),
        #[serde(rename = "query_task_updates")]
        QueryTaskUpdates(TaskUpdatesQuery),
        #[serde(rename = "query_object_task_operations")]
        ObjectTaskOperations(ObjectTaskOperationsQuery),
        #[serde(rename = "query_model_figures")]
        QueryModelFigures(TaskUpdatesQuery),
        #[serde(rename = "query_mass_spaces")]
        QueryMassSpaces(MassSpacesQuery),
        #[serde(rename = "prepare_mass_intent")]
        PrepareMassIntent(crate::automation::intent::PrepareMass),
        #[serde(rename = "prepare_parametric_proposal")]
        PrepareParametric(crate::automation::intent::PrepareParametric),
        #[serde(rename = "query_project_sources")]
        ProjectSources(crate::automation::sources::Search),
        #[serde(rename = "read_project_source")]
        ReadProjectSource(crate::automation::sources::Read),
        #[serde(rename = "analyze_spaces")]
        AnalyzeSpaces(crate::automation::SpaceRequest),
        #[serde(rename = "query_terrain_height")]
        TerrainHeight(crate::automation::TerrainRequest),
        #[serde(rename = "query_project_rules")]
        ProjectRules(crate::automation::RulesRequest),
        #[serde(rename = "query_source_evidence")]
        SourceEvidence(crate::automation::EvidenceRequest),
        #[serde(rename = "prepare_discovered_change")]
        PrepareDiscovered(crate::automation_change::PrepareDiscovered),
        #[serde(rename = "validate_model")]
        Validate(crate::automation::Validate),
        #[serde(rename = "recover_operation")]
        Recover(crate::automation_change::Recover),
        #[serde(rename = "recover_document")]
        RecoverDocument(crate::automation_change::Recover),
        #[serde(rename = "describe_command")]
        Describe(Description),
        #[serde(rename = "search_commands")]
        Search(CommandQuery),
        #[serde(rename = "get_native_schema")]
        NativeSchema,
        #[serde(rename = "list_instances")]
        Instances,
        #[serde(rename = "prepare_native_change")]
        PrepareNative(crate::automation_change::PrepareNative),
        #[serde(rename = "save_project")]
        Save(FileParams),
        #[serde(rename = "reopen_project")]
        Reopen(FileParams),
        #[serde(rename = "prepare_change")]
        Prepare(crate::automation_change::Prepare),
        #[serde(rename = "commit_change")]
        Commit(crate::automation_change::Commit),
        #[serde(rename = "get_operation")]
        Operation(crate::automation_change::Operation),
        #[serde(rename = "undo_operation")]
        Undo(crate::automation_change::Operation),
        #[serde(rename = "cancel_operation")]
        Cancel(crate::automation_change::Cancel),
        #[serde(rename = "get_context")]
        Context(Target),
        #[serde(rename = "query_model")]
        Query(Query),
        #[serde(rename = "invoke_workbench_command")]
        InvokeWorkbench(WorkbenchInvoke),
        #[serde(rename = "get_workbench_operation")]
        WorkbenchOperation(WorkbenchOperation),
        #[serde(rename = "list_workbench_commands")]
        WorkbenchCommands(Target),
        #[serde(rename = "get_target")]
        Target,
    }
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct MassSpacesQuery {
        target: Target,
        expected_revision: u64,
        mass: Option<u64>,
        zone: Option<u64>,
        offset: usize,
        limit: usize,
    }
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct TaskUpdatesQuery {
        target: Target,
        expected_revision: u64,
        offset: usize,
        limit: usize,
    }
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Description {
        instance_id: String,
        project_id: String,
        document_session_id: u64,
        command: Option<String>,
    }
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct CommandQuery {
        target: Target,
        #[serde(default)]
        query: String,
        limit: usize,
        cursor: Option<String>,
    }
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct ObjectTaskOperationsQuery {
        target: Target,
        expected_revision: u64,
        request_id: String,
    }
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct FileParams {
        pub target: Target,
        pub expected_revision: u64,
        pub filename: String,
    }
    pub struct FilePending {
        pub params: FileParams,
        pub reopen: bool,
        pub reply: SyncSender<Value>,
    }
    struct Pending {
        request: Request,
        reply: SyncSender<Value>,
    }
    type Wake = Arc<dyn Fn() + Send + Sync>;
    include!("automation_ipc/workbench.rs");
    include!("automation_ipc/environment.rs");
    include!("automation_ipc/parametric.rs");
    include!("automation_ipc/object_task.rs");
    pub struct Endpoint {
        build_identity: Value,
        receiver: Receiver<Pending>,
        source_return: SyncSender<Pending>,
        pub files: Vec<FilePending>,
        pub workbench_pending: Vec<WorkbenchPending>,
        workbench_receipts: std::collections::HashMap<String, WorkbenchReceipt>,
        pub documents: Vec<(crate::automation_change::Recover, SyncSender<Value>)>,
        service: ReadService,
        changes: std::rc::Rc<std::cell::RefCell<crate::automation_change::ChangeService>>,
        stop: Arc<AtomicBool>,
        source_read_busy: Arc<AtomicBool>,
        path: PathBuf,
        socket_identity: (u64, u64),
        worker: Option<thread::JoinHandle<()>>,
        wake: Arc<Mutex<Option<Wake>>>,
    }
    impl Endpoint {
        fn defer_cited_originals(
            &self,
            editor: &Editor,
            repository: Option<&crate::project_repository::Repository>,
            original: Request,
            commands: Vec<crate::command::ModelCommand>,
            reply: SyncSender<Value>,
        ) {
            let Some(repository) = repository else {
                let _ = reply.try_send(json!({"error":{"code":"CITED_ORIGINAL_UNAVAILABLE","message":"Reconnect the bound project sources or save the definition without running"},"operation_submitted":false}));
                return;
            };
            if self.source_read_busy.swap(true, Ordering::AcqRel) {
                let _ = reply.try_send(
                    json!({"error":{"code":"SOURCE_READER_BUSY"},"operation_submitted":false}),
                );
                return;
            }
            let repository = repository.clone();
            let sources = (
                repository.root().to_path_buf(),
                repository.source_context_identity().to_owned(),
            );
            let document_id = editor.project_identity().to_owned();
            let book = editor.project.rules.clone();
            let context = self.service.context_revision(editor);
            let sender = self.source_return.clone();
            let wake = self.wake.clone();
            let busy = self.source_read_busy.clone();
            let failed_reply = reply.clone();
            if thread::Builder::new()
                .name("openarchi-cited-admission".into())
                .spawn(move || {
                    let admission = crate::project_repository::constraints::guard_cited_runs(
                        &repository,
                        &document_id,
                        &book,
                        &commands,
                    );
                    let pending = Pending {
                        request: Request::CitedReady {
                            original: Box::new(original),
                            admission,
                            context,
                            sources,
                        },
                        reply,
                    };
                    if let Err(error) = sender.try_send(pending) {
                        busy.store(false, Ordering::Release);
                        let pending = match error {
                            mpsc::TrySendError::Full(p) | mpsc::TrySendError::Disconnected(p) => p,
                        };
                        let _ = pending.reply.try_send(
                            json!({"error":{"code":"HOST_BUSY"},"operation_submitted":false}),
                        );
                        return;
                    }
                    if let Some(callback) = wake.lock().ok().and_then(|v| v.clone()) {
                        callback();
                    }
                })
                .is_err()
            {
                self.source_read_busy.store(false, Ordering::Release);
                let _ = failed_reply.try_send(json!({"error":{"code":"SOURCE_READER_UNAVAILABLE"},"operation_submitted":false}));
            }
        }
        /// Read-only activity observation; never changes admission or authorization.
        pub fn source_reader_busy(&self) -> bool {
            self.source_read_busy.load(Ordering::Acquire)
        }
        pub(crate) fn socket_path(&self) -> &std::path::Path {
            &self.path
        }
        pub(crate) fn set_write_enabled(&mut self, enabled: bool, editor: &Editor) {
            if enabled {
                self.changes.borrow_mut().authorize_current(editor);
            } else {
                self.changes.borrow_mut().set_enabled(false);
            }
        }
        pub(crate) fn set_wake(&mut self, wake: impl Fn() + Send + Sync + 'static) {
            *self.wake.lock().expect("automation wake lock") = Some(Arc::new(wake));
        }
        pub(crate) fn changes(
            &mut self,
        ) -> std::cell::RefMut<'_, crate::automation_change::ChangeService> {
            self.changes.borrow_mut()
        }
        pub(crate) fn bind_for_host(
            path: PathBuf,
            write_enabled: bool,
            journal_directory: Option<PathBuf>,
            host: &mut crate::application_host::ApplicationHost,
        ) -> std::io::Result<Self> {
            host.configure_operation_journal(journal_directory.clone())
                .map_err(std::io::Error::other)?;
            let mut endpoint = Self::bind_configured(path, write_enabled, journal_directory)?;
            endpoint.service = ReadService::new(host.operation_target().instance_id);
            endpoint.changes = host.operations.clone();
            if !write_enabled {
                endpoint.changes.borrow_mut().set_enabled(false);
            } else {
                endpoint.changes.borrow_mut().set_enabled(true);
            }
            Ok(endpoint)
        }
        /// Default application transport on the UI thread: native proposals and
        /// remote dispatch share this host's coordinator, grant and journal.
        pub fn bind_application(
            path: PathBuf,
            host: &mut crate::application_host::ApplicationHost,
        ) -> std::io::Result<Self> {
            let enabled = std::env::var("OPENARCHI_AUTOMATION_WRITE").as_deref() == Ok("1");
            let directory = std::env::var_os("OPENARCHI_AUTOMATION_JOURNAL_DIR")
                .map(PathBuf::from)
                .or_else(|| crate::platform::config_dir().map(|p| p.join("automation-journal")));
            Self::bind_for_host(path, enabled, directory, host)
        }
        pub fn bind(path: PathBuf) -> std::io::Result<Self> {
            let write_enabled = std::env::var("OPENARCHI_AUTOMATION_WRITE").as_deref() == Ok("1");
            let journal_directory = std::env::var_os("OPENARCHI_AUTOMATION_JOURNAL_DIR")
                .map(PathBuf::from)
                .or_else(|| crate::platform::config_dir().map(|p| p.join("automation-journal")));
            Self::bind_configured(path, write_enabled, journal_directory)
        }
        pub(crate) fn bind_configured(
            path: PathBuf,
            write_enabled: bool,
            journal_directory: Option<PathBuf>,
        ) -> std::io::Result<Self> {
            if journal_directory.as_ref().is_some_and(|p| !p.is_absolute())
                || (write_enabled && journal_directory.is_none())
            {
                return Err(std::io::Error::other(
                    "Absolute automation journal directory required",
                ));
            }

            // Require a fresh private directory; never alter an existing directory's permissions.
            let parent = path
                .parent()
                .ok_or_else(|| std::io::Error::other("socket requires parent"))?;
            fs::DirBuilder::new().mode(0o700).create(parent)?;
            fs::set_permissions(parent, fs::Permissions::from_mode(0o700))?;
            let listener = UnixListener::bind(&path)?;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;
            listener.set_nonblocking(true)?;
            let mut random = [0u8; 16];
            fs::File::open("/dev/urandom")?.read_exact(&mut random)?;
            let identity = random
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>();
            let (sender, receiver) = mpsc::sync_channel::<Pending>(16);
            let source_return = sender.clone();
            let stop = Arc::new(AtomicBool::new(false));
            let flag = stop.clone();
            let wake = Arc::new(Mutex::new(None::<Wake>));
            let notify = wake.clone();
            let socket_metadata = fs::symlink_metadata(&path)?;
            let socket_identity = (socket_metadata.dev(), socket_metadata.ino());
            let worker = thread::spawn(move || {
                while !flag.load(Ordering::Relaxed) {
                    match listener.accept() {
                        Ok((mut stream, _)) => {
                            if stream.set_nonblocking(false).is_err() {
                                continue;
                            }
                            let _ = stream.set_read_timeout(Some(Duration::from_millis(100)));
                            let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));
                            let response = match read_frame(&mut stream, &flag).and_then(|line| {
                                serde_json::from_slice::<Request>(&line)
                                    .map_err(|_| "INVALID_REQUEST")
                            }) {
                                Err(code) => json!({"error":{"code":code}}),
                                Ok(request) => {
                                    let job_read = matches!(
                                        &request,
                                        Request::PluginJobs(_) | Request::PluginJob(_)
                                    );
                                    let source_read = matches!(
                                        &request,
                                        Request::ReadProjectSource(_) | Request::ProjectRules(_)
                                    );
                                    let (reply, rx) = mpsc::sync_channel(1);
                                    match sender.try_send(Pending { request, reply }) {
                                        Err(_) => json!({"error":{"code":"HOST_BUSY"}}),
                                        Ok(()) => {
                                            let callback =
                                                notify.lock().ok().and_then(|value| value.clone());
                                            if let Some(callback) = callback {
                                                callback();
                                            }
                                            let deadline =
                                                std::time::Instant::now() + Duration::from_secs(3);
                                            loop {
                                                if flag.load(Ordering::Relaxed) {
                                                    break json!({"error":{"code":"HOST_STOPPED"}});
                                                }
                                                match rx.recv_timeout(Duration::from_millis(50)) {
                                                    Ok(value) => break value,
                                                    Err(mpsc::RecvTimeoutError::Disconnected) => {
                                                        break json!({"error":{"code":"HOST_STOPPED"}});
                                                    }
                                                    Err(_) => {
                                                        if std::time::Instant::now() >= deadline {
                                                            if job_read {
                                                                break json!({"error":{"code":"JOB_READ_TIMEOUT","recovery":"Readonly query only; inspect the same original project/request identity. No dispatch or model effect."}});
                                                            }
                                                            if source_read {
                                                                break json!({"error":{"code":"SOURCE_READ_TIMEOUT","recovery":"Readonly; retry with the same source/context identity. No model effect."}});
                                                            }
                                                            break json!({"error":{"code":"OPERATION_OUTCOME_UNKNOWN","recovery":"Query the original request_id before replaying any write"}});
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            };
                            if flag.load(Ordering::Relaxed) {
                                break;
                            }
                            if let Ok(mut bytes) = serde_json::to_vec(&response) {
                                if bytes.len() > 128 * 1024 {
                                    bytes=serde_json::to_vec(&json!({"error":{"code":"OUTPUT_LIMIT","recovery":"Use a smaller projection/page or describe one registered command"}})).unwrap();
                                }
                                let _ = stream.write_all(&bytes);
                                let _ = stream.write_all(b"\n");
                            }
                        }
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            thread::sleep(Duration::from_millis(20))
                        }
                        Err(_) => break,
                    }
                }
            });
            let mut changes =
                crate::automation_change::ChangeService::new(identity.clone(), write_enabled);
            if let Some(directory) = journal_directory {
                changes = changes.with_journal_directory(directory);
            }
            Ok(Self {
                build_identity: executable_identity(),
                receiver,
                source_return,
                files: vec![],
                workbench_pending: vec![],
                workbench_receipts: Default::default(),
                documents: vec![],
                changes: std::rc::Rc::new(std::cell::RefCell::new(changes)),
                service: ReadService::new(identity),
                stop,
                source_read_busy: Arc::new(AtomicBool::new(false)),
                path,
                socket_identity,
                worker: Some(worker),
                wake,
            })
        }
        pub fn recover_document(
            &mut self,
            editor: &mut Editor,
            p: crate::automation_change::Recover,
        ) -> Value {
            self.changes.borrow_mut().recover_document(editor, p)
        }
        pub fn poll(&mut self, editor: &mut Editor) {
            self.poll_with_repository(editor, None);
        }
        pub fn poll_with_repository(
            &mut self,
            editor: &mut Editor,
            repository: Option<&crate::project_repository::Repository>,
        ) {
            self.poll_inner(editor, repository, None, None);
        }
        /// Default native transport sees the same registration and durable job
        /// store as the UI. Legacy editor-only test hosts expose no job storage.
        pub fn poll_with_host(
            &mut self,
            host: &mut crate::application_host::ApplicationHost,
            repository: Option<&crate::project_repository::Repository>,
        ) {
            self.poll_inner(
                &mut host.document.editor,
                repository,
                Some((&host.plugins, host.jobs.as_ref())),
                host.environment.as_ref(),
            );
        }
        fn poll_inner(
            &mut self,
            editor: &mut Editor,
            repository: Option<&crate::project_repository::Repository>,
            plugins: Option<(
                &crate::plugins::Registry,
                Option<&crate::plugins::jobs::Jobs>,
            )>,
            environment: Option<&crate::workbench::environment::EnvironmentService>,
        ) {
            self.changes.borrow_mut().observe_document(editor);
            for _ in 0..8 {
                let Ok(pending) = self.receiver.try_recv() else {
                    break;
                };
                let (original, admission) = match pending.request {
                    Request::CitedReady {
                        original,
                        admission,
                        context,
                        sources,
                    } => {
                        self.source_read_busy.store(false, Ordering::Release);
                        if context != self.service.context_revision(editor) {
                            let _ = pending.reply.try_send(json!({"error":{"code":"CONTEXT_CONFLICT","message":"Application context changed during original verification; original authorization is not renewed"},"operation_submitted":false}));
                            continue;
                        }
                        if repository.is_none_or(|r| {
                            r.root() != sources.0 || r.source_context_identity() != sources.1
                        }) {
                            let _ = pending.reply.try_send(json!({"error":{"code":"SOURCE_CONTEXT_CONFLICT"},"operation_submitted":false}));
                            continue;
                        }
                        match admission {
                            Ok(guard) => (*original, Some(guard)),
                            Err(message) => {
                                let _ = pending.reply.try_send(json!({"error":{"code":"CITED_ORIGINAL_UNAVAILABLE","message":message},"operation_submitted":false}));
                                continue;
                            }
                        }
                    }
                    request => (request, None),
                };
                let request = match original {
                    Request::ProjectRules(request) => {
                        let result = self.service.project_rules(editor, &request);
                        let Ok(mut value) = result else {
                            let _ = pending.reply.try_send(json!({"error":result.unwrap_err()}));
                            continue;
                        };
                        if !matches!(request.section, crate::automation::RulesSection::Checks) {
                            let _ = pending.reply.try_send(json!({"result":value}));
                            continue;
                        }
                        let ids = value["entries"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .filter_map(|v| v["definition"]["id"].as_str())
                            .collect::<std::collections::BTreeSet<_>>();
                        let plan = crate::constraints::originals::ReadPlan::new(
                            editor.project_identity(),
                            repository,
                            &editor.project.rules,
                            editor
                                .project
                                .rules
                                .checks
                                .iter()
                                .filter(|c| ids.contains(c.definition.id.as_str())),
                        );
                        if plan.is_empty() {
                            let _ = pending.reply.try_send(json!({"result":value}));
                            continue;
                        }
                        if self.source_read_busy.swap(true, Ordering::AcqRel) {
                            let _ = pending
                                .reply
                                .try_send(json!({"error":{"code":"SOURCE_READER_BUSY"}}));
                            continue;
                        }
                        let busy = self.source_read_busy.clone();
                        let reply = pending.reply.clone();
                        if thread::Builder::new()
                            .name("openarchi-check-originals".into())
                            .spawn(move || {
                                let observation = plan.execute();
                                if let Some(entries) = value["entries"].as_array_mut() {
                                    observation.apply_to_entries(entries);
                                }
                                busy.store(false, Ordering::Release);
                                let _ = reply.try_send(json!({"result":value}));
                            })
                            .is_err()
                        {
                            self.source_read_busy.store(false, Ordering::Release);
                            let _ = pending
                                .reply
                                .try_send(json!({"error":{"code":"SOURCE_READER_UNAVAILABLE"}}));
                        }
                        continue;
                    }
                    request @ (Request::EnvironmentInspect(_)
                    | Request::EnvironmentPropose(_)
                    | Request::EnvironmentProposal(_)
                    | Request::EnvironmentCancel(_)) => {
                        let result = self.environment_request(editor, environment, request);
                        let _ = pending.reply.try_send(result);
                        continue;
                    }
                    Request::PluginCommands(p) => {
                        let result = if self.service.target(editor) != p.target {
                            json!({"error":{"code":"TARGET_MISMATCH"}})
                        } else if let Some((registry, _)) = plugins {
                            crate::plugins::query::catalog(registry, p)
                        } else {
                            json!({"error":{"code":"PLUGIN_HOST_UNAVAILABLE"}})
                        };
                        let _ = pending.reply.try_send(result);
                        continue;
                    }
                    request @ (Request::PluginJobs(_) | Request::PluginJob(_)) => {
                        let (target, revision, context) = match &request {
                            Request::PluginJobs(p) => (&p.target, p.expected_revision, None),
                            Request::PluginJob(p) => (
                                &p.target,
                                p.expected_revision,
                                Some(&p.expected_context_revision),
                            ),
                            _ => unreachable!(),
                        };
                        let error = if self.service.target(editor) != *target {
                            Some("TARGET_MISMATCH")
                        } else if editor.revision != revision {
                            Some("REVISION_CONFLICT")
                        } else if context
                            .is_some_and(|value| *value != self.service.context_revision(editor))
                        {
                            Some("CONTEXT_CONFLICT")
                        } else {
                            None
                        };
                        if let Some(code) = error {
                            let _ = pending.reply.try_send(json!({"error":{"code":code}}));
                            continue;
                        }
                        let Some((_, Some(jobs))) = plugins else {
                            let _ = pending
                                .reply
                                .try_send(json!({"error":{"code":"PLUGIN_HOST_UNAVAILABLE"}}));
                            continue;
                        };
                        // Share the bounded reader slot with original source reads.
                        // No storage/query validation or quantity calculation on UI.
                        if self.source_read_busy.swap(true, Ordering::AcqRel) {
                            let _ = pending
                                .reply
                                .try_send(json!({"error":{"code":"SOURCE_READER_BUSY"}}));
                            continue;
                        }
                        let plan = match request {
                            Request::PluginJobs(p) => {
                                crate::plugins::query::Plan::list_with_archive(
                                    jobs,
                                    &editor.project.job_archive,
                                    p,
                                )
                            }
                            Request::PluginJob(p) => {
                                crate::plugins::query::Plan::read(jobs, editor, p)
                            }
                            _ => unreachable!(),
                        };
                        let busy = self.source_read_busy.clone();
                        let reply = pending.reply.clone();
                        if thread::Builder::new()
                            .name("openarchi-job-read".into())
                            .spawn(move || {
                                let response = plan.execute();
                                busy.store(false, Ordering::Release);
                                let _ = reply.try_send(response);
                            })
                            .is_err()
                        {
                            self.source_read_busy.store(false, Ordering::Release);
                            let _ = pending
                                .reply
                                .try_send(json!({"error":{"code":"SOURCE_READER_UNAVAILABLE"}}));
                        }
                        continue;
                    }
                    Request::QueryMassSpaces(p) => {
                        let result = if self.service.target(editor) != p.target {
                            json!({"error":{"code":"TARGET_MISMATCH"}})
                        } else if editor.revision != p.expected_revision {
                            json!({"error":{"code":"REVISION_CONFLICT"}})
                        } else {
                            match crate::mass_spaces::query(
                                &editor.project,
                                p.mass,
                                p.zone,
                                p.offset,
                                p.limit,
                            ) {
                                Ok(value) => json!({"result":value}),
                                Err(message) => {
                                    json!({"error":{"code":"INVALID_MASS_SPACE_QUERY","message":message}})
                                }
                            }
                        };
                        let _ = pending.reply.try_send(result);
                        continue;
                    }
                    Request::QueryModelFigures(p) => {
                        let result = if self.service.target(editor) != p.target {
                            json!({"error":{"code":"TARGET_MISMATCH"}})
                        } else if editor.revision != p.expected_revision {
                            json!({"error":{"code":"REVISION_CONFLICT"}})
                        } else {
                            match crate::diagrams::query(&editor.project, p.offset, p.limit) {
                                Ok(value) => json!({"result":value}),
                                Err(message) => {
                                    json!({"error":{"code":"INVALID_FIGURE_QUERY","message":message}})
                                }
                            }
                        };
                        let _ = pending.reply.try_send(result);
                        continue;
                    }
                    Request::QueryTaskUpdates(p) => {
                        let result = if self.service.target(editor) != p.target {
                            json!({"error":{"code":"TARGET_MISMATCH"}})
                        } else if editor.revision != p.expected_revision {
                            json!({"error":{"code":"REVISION_CONFLICT"}})
                        } else {
                            match crate::design_process::update::query(
                                &editor.project,
                                repository,
                                p.offset,
                                p.limit,
                            ) {
                                Ok(value) => json!({"result":value}),
                                Err(message) => {
                                    json!({"error":{"code":"INVALID_TASK_UPDATE_QUERY","message":message}})
                                }
                            }
                        };
                        let _ = pending.reply.try_send(result);
                        continue;
                    }
                    Request::PrepareTaskUpdate(p) => {
                        let result = if self.service.target(editor) != p.target {
                            json!({"error":{"code":"TARGET_MISMATCH"}})
                        } else if editor.revision != p.expected_revision {
                            json!({"error":{"code":"REVISION_CONFLICT"}})
                        } else if self.service.context_revision(editor)
                            != p.expected_context_revision
                        {
                            json!({"error":{"code":"CONTEXT_CONFLICT"}})
                        } else if crate::automation::sources::context(editor, repository)["sources_revision"]
                            != p.expected_sources_revision
                        {
                            json!({"error":{"code":"SOURCE_CONTEXT_CONFLICT"}})
                        } else {
                            let authorization = self.changes.borrow().write_authorization(editor);
                            match authorization {
                                Err(error) => error,
                                Ok(()) => match p.draft.lower(editor).and_then(|draft| {
                                    crate::design_process::update::prepare(
                                        editor,
                                        repository,
                                        &p.update_id,
                                        &p.task_id,
                                        draft,
                                    )
                                }) {
                                    Err(message) => {
                                        json!({"error":{"code":"INVALID_TASK_UPDATE","message":message}})
                                    }
                                    Ok(prepared) => {
                                        self.changes.borrow_mut().prepare_task_proposal(
                                            editor,
                                            p.target,
                                            &p.task_id,
                                            "Scoped task update",
                                            prepared,
                                        )
                                    }
                                },
                            }
                        };
                        let _ = pending.reply.try_send(result);
                        continue;
                    }
                    Request::ObjectTaskOperations(p) => {
                        let result = if self.service.target(editor) != p.target {
                            json!({"error":{"code":"TARGET_MISMATCH"}})
                        } else if editor.revision != p.expected_revision {
                            json!({"error":{"code":"REVISION_CONFLICT"}})
                        } else {
                            self.changes
                                .borrow_mut()
                                .object_task_operations(editor, &p.request_id)
                        };
                        let _ = pending.reply.try_send(result);
                        continue;
                    }
                    Request::PrepareParametric(p) => {
                        let result = if self.service.target(editor) != p.target {
                            json!({"error":{"code":"TARGET_MISMATCH"}})
                        } else if editor.revision != p.expected_revision {
                            json!({"error":{"code":"REVISION_CONFLICT"}})
                        } else if self.service.context_revision(editor)
                            != p.expected_context_revision
                        {
                            json!({"error":{"code":"CONTEXT_CONFLICT"}})
                        } else if crate::automation::sources::context(editor, repository)["sources_revision"]
                            != p.expected_sources_revision
                        {
                            json!({"error":{"code":"SOURCE_CONTEXT_CONFLICT"}})
                        } else {
                            let authorization = self.changes.borrow().write_authorization(editor);
                            match authorization {
                                Err(error) => error,
                                Ok(()) => match p.proposal.prepare(editor, &p.task_id) {
                                    Err(message) => {
                                        json!({"error":{"code":"INVALID_PARAMETRIC_PROPOSAL","message":message}})
                                    }
                                    Ok(prepared) => {
                                        let checks = prepared.declared_check_previews();
                                        let mut result =
                                            self.changes.borrow_mut().prepare_task_proposal(
                                                editor,
                                                p.target,
                                                &p.task_id,
                                                "Parametric two-wing proposal",
                                                prepared,
                                            );
                                        if let Some(value) = result.get_mut("result") {
                                            value["declared_check_preview"] = json!({"checks":checks,"stored_reports_changed":false,"external_originals_read":false,"compliance_verdict":"not_evaluated"});
                                        }
                                        result
                                    }
                                },
                            }
                        };
                        let _ = pending.reply.try_send(result);
                        continue;
                    }
                    Request::PrepareMassIntent(p) => {
                        let result = if self.service.target(editor) != p.target {
                            json!({"error":{"code":"TARGET_MISMATCH"}})
                        } else if editor.revision != p.expected_revision {
                            json!({"error":{"code":"REVISION_CONFLICT"}})
                        } else if self.service.context_revision(editor)
                            != p.expected_context_revision
                        {
                            json!({"error":{"code":"CONTEXT_CONFLICT"}})
                        } else if crate::automation::sources::context(editor, repository)["sources_revision"]
                            != p.expected_sources_revision
                        {
                            json!({"error":{"code":"SOURCE_CONTEXT_CONFLICT"}})
                        } else {
                            let authorization = self.changes.borrow().write_authorization(editor);
                            match authorization {
                                Err(error) => error,
                                Ok(()) => match crate::design_process::intent::prepare_mass(
                                    editor,
                                    &p.task_id,
                                    p.draft.lower(),
                                ) {
                                    Err(message) => {
                                        json!({"error":{"code":"INVALID_MASS_INTENT","message":message}})
                                    }
                                    Ok(prepared) => {
                                        self.changes.borrow_mut().prepare_task_proposal(
                                            editor,
                                            p.target,
                                            &p.task_id,
                                            "Mass intent candidate proposal",
                                            prepared,
                                        )
                                    }
                                },
                            }
                        };
                        let _ = pending.reply.try_send(result);
                        continue;
                    }
                    Request::PrepareSpatialIntent(p) => {
                        let result = if self.service.target(editor) != p.target {
                            json!({"error":{"code":"TARGET_MISMATCH"}})
                        } else if editor.revision != p.expected_revision {
                            json!({"error":{"code":"REVISION_CONFLICT"}})
                        } else if self.service.context_revision(editor)
                            != p.expected_context_revision
                        {
                            json!({"error":{"code":"CONTEXT_CONFLICT"}})
                        } else if crate::automation::sources::context(editor, repository)["sources_revision"]
                            != p.expected_sources_revision
                        {
                            json!({"error":{"code":"SOURCE_CONTEXT_CONFLICT"}})
                        } else {
                            let authorization = self.changes.borrow().write_authorization(editor);
                            match authorization {
                                Err(error) => error,
                                Ok(()) => match crate::design_process::intent::prepare(
                                    editor,
                                    &p.task_id,
                                    p.draft.lower(),
                                ) {
                                    Err(message) => {
                                        json!({"error":{"code":"INVALID_SPATIAL_INTENT","message":message}})
                                    }
                                    Ok(prepared) => {
                                        self.changes.borrow_mut().prepare_task_proposal(
                                            editor,
                                            p.target,
                                            &p.task_id,
                                            "Spatial intent candidate proposal",
                                            prepared,
                                        )
                                    }
                                },
                            }
                        };
                        let _ = pending.reply.try_send(result);
                        continue;
                    }
                    Request::ReadProjectSource(p) => {
                        match crate::automation::sources::prepare_read(
                            &self.service,
                            editor,
                            repository,
                            p,
                        ) {
                            Err(error) => {
                                let _ = pending.reply.try_send(json!({"error":error}));
                            }
                            Ok(plan) => {
                                if self.source_read_busy.swap(true, Ordering::AcqRel) {
                                    let _ = pending
                                        .reply
                                        .try_send(json!({"error":{"code":"SOURCE_READER_BUSY"}}));
                                } else {
                                    let busy = self.source_read_busy.clone();
                                    let reply = pending.reply.clone();
                                    if thread::Builder::new()
                                        .name("openarchi-source-read".into())
                                        .spawn(move || {
                                            let response = plan.execute();
                                            busy.store(false, Ordering::Release);
                                            let _ = reply.try_send(response);
                                        })
                                        .is_err()
                                    {
                                        self.source_read_busy.store(false, Ordering::Release);
                                        let _ = pending.reply.try_send(
                                            json!({"error":{"code":"SOURCE_READER_UNAVAILABLE"}}),
                                        );
                                    }
                                }
                            }
                        }
                        continue;
                    }
                    Request::RecoverDocument(p) => {
                        self.documents.push((p, pending.reply));
                        break;
                    }
                    Request::Recover(p) => {
                        let _ = pending
                            .reply
                            .try_send(self.changes.borrow_mut().recover(editor, p));
                        continue;
                    }
                    Request::Save(params) => {
                        if self.service.target(editor) != params.target {
                            let _ = pending
                                .reply
                                .try_send(json!({"error":{"code":"TARGET_MISMATCH"}}));
                            continue;
                        }
                        if let Err(error) = self
                            .changes
                            .borrow_mut()
                            .unrestricted_write_authorization(editor)
                        {
                            let _ = pending.reply.try_send(error);
                            continue;
                        }
                        self.files.push(FilePending {
                            params,
                            reopen: false,
                            reply: pending.reply,
                        });
                        break;
                    }
                    Request::Reopen(params) => {
                        if self.service.target(editor) != params.target {
                            let _ = pending
                                .reply
                                .try_send(json!({"error":{"code":"TARGET_MISMATCH"}}));
                            continue;
                        }
                        if let Err(error) = self
                            .changes
                            .borrow_mut()
                            .unrestricted_write_authorization(editor)
                        {
                            let _ = pending.reply.try_send(error);
                            continue;
                        }
                        self.files.push(FilePending {
                            params,
                            reopen: true,
                            reply: pending.reply,
                        });
                        break;
                    }
                    Request::InvokeWorkbench(p) => {
                        self.enqueue_workbench(editor, p, pending.reply);
                        break;
                    }
                    Request::WorkbenchOperation(p) => {
                        let _ = pending.reply.try_send(self.workbench_operation(p));
                        continue;
                    }
                    Request::WorkbenchCommands(target) => {
                        let response = if self.service.target(editor) != target {
                            json!({"error":{"code":"TARGET_MISMATCH"}})
                        } else {
                            json!({"result":{"target":target,"revision":editor.revision,"commands":crate::workbench::CommandRegistry::catalog(editor),"discovery_only":true}})
                        };
                        let _ = pending.reply.try_send(response);
                        continue;
                    }
                    Request::Instances => {
                        let _=pending.reply.try_send(json!({"result":{"instances":[{"target":self.service.target(editor),"build_version":env!("CARGO_PKG_VERSION"),"build_identity":self.build_identity,"capability_version":crate::automation::CAPABILITY_VERSION,"connected":true}]}}));
                        continue;
                    }
                    Request::NativeSchema => {
                        let _=pending.reply.try_send(json!({"result":{"schema":crate::automation_registry::schema(),"discovered_schema":crate::automation_registry::discovered_schema(),"commands":crate::automation_registry::command_names(),"prepare_output_schema":crate::automation_registry::preview_schema(),"spatial_intent_schema":crate::automation::intent::schema(),"mass_intent_schema":crate::automation::intent::mass_schema(),"parametric_proposal_schema":crate::automation::intent::parametric_schema(),"declared_check_preview_checks_schema":schemars::schema_for!(Vec<crate::constraints::Preview>),"task_update_schema":crate::automation::intent::update_schema(),"plugin_job_read_version":1,"mass_space_read_version":1,"environment_proposal_version":if environment.is_some(){1}else{0},"environment_proposal_schema":schemars::schema_for!(EnvironmentPropose),"version":1}}));
                        continue;
                    }
                    Request::Search(p) => {
                        let value = if self.service.target(editor) != p.target {
                            json!({"error":{"code":"TARGET_MISMATCH"}})
                        } else if !(1..=100).contains(&p.limit) {
                            json!({"error":{"code":"INVALID_LIMIT","minimum":1,"maximum":100}})
                        } else {
                            let matches: Vec<_> = crate::automation_registry::command_names()
                                .into_iter()
                                .filter(|name| name.contains(&p.query.to_lowercase()))
                                .collect();
                            let binding = format!(
                                "{:x}",
                                Sha256::digest(
                                    serde_json::to_vec(&(
                                        &p.target,
                                        &p.query,
                                        p.limit,
                                        crate::automation::CAPABILITY_VERSION
                                    ))
                                    .unwrap()
                                )
                            );
                            let offset = if let Some(cursor) = p.cursor {
                                match cursor.split_once(':').and_then(|(hash, offset)| {
                                    if hash == binding {
                                        offset.parse::<usize>().ok()
                                    } else {
                                        None
                                    }
                                }) {
                                    Some(offset) => offset,
                                    None => {
                                        let _ = pending
                                            .reply
                                            .try_send(json!({"error":{"code":"INVALID_CURSOR"}}));
                                        continue;
                                    }
                                }
                            } else {
                                0
                            };
                            if offset > matches.len() {
                                json!({"error":{"code":"INVALID_CURSOR"}})
                            } else {
                                let end = (offset + p.limit).min(matches.len());
                                json!({"result":{"commands":matches[offset..end].iter().map(|name|json!({"name":name,"version":1,"native_type":*name!="story_elevation"})).collect::<Vec<_>>(),"next_cursor":(end<matches.len()).then(||format!("{binding}:{end}"))}})
                            }
                        };
                        let _ = pending.reply.try_send(value);
                        continue;
                    }
                    Request::Describe(p) => {
                        let target = Target {
                            instance_id: p.instance_id,
                            project_id: p.project_id,
                            document_session_id: p.document_session_id,
                        };
                        let result = if self.service.target(editor) != target {
                            json!({"error":{"code":"TARGET_MISMATCH"}})
                        } else if let Some(name) = p.command.filter(|name| name != "batch_change") {
                            crate::automation_registry::describe(&name)
                                .map(|v| json!({"result":v}))
                                .unwrap_or_else(|| json!({"error":{"code":"COMMAND_NOT_FOUND"}}))
                        } else {
                            self.changes.borrow_mut().describe(editor, &target)
                        };
                        let _ = pending.reply.try_send(result);
                        continue;
                    }
                    Request::PrepareDiscovered(p) => {
                        let result = self.changes.borrow_mut().prepare_discovered_with_originals(
                            editor,
                            p.clone(),
                            admission.as_ref(),
                        );
                        if admission.is_none()
                            && result["error"]["code"] == "CITED_ORIGINAL_ADMISSION_REQUIRED"
                        {
                            match p.change.resolve().and_then(|native|native.lower(editor)).map_err(|message|json!({"error":{"code":"INVALID_NATIVE_COMMAND","message":message}})).and_then(|batch|editor.resolve_batch_commands(p.expected_revision,&batch).map(|(commands,_)|commands).map_err(|e|json!({"error":e}))) {
                                Ok(commands) => self.defer_cited_originals(editor, repository, Request::PrepareDiscovered(p), commands, pending.reply),
                                Err(error) => { let _ = pending.reply.try_send(error); }
                            }
                        } else {
                            let _ = pending.reply.try_send(result);
                        }
                        continue;
                    }
                    Request::PrepareNative(p) => {
                        let result = self.changes.borrow_mut().prepare_native_with_originals(
                            editor,
                            p.clone(),
                            admission.as_ref(),
                        );
                        if admission.is_none()
                            && result["error"]["code"] == "CITED_ORIGINAL_ADMISSION_REQUIRED"
                        {
                            match p.change.lower(editor).map_err(|message|json!({"error":{"code":"INVALID_NATIVE_COMMAND","message":message}})).and_then(|batch|editor.resolve_batch_commands(p.expected_revision,&batch).map(|(commands,_)|commands).map_err(|e|json!({"error":e}))) {
                                Ok(commands) => self.defer_cited_originals(editor, repository, Request::PrepareNative(p), commands, pending.reply),
                                Err(error) => { let _ = pending.reply.try_send(error); }
                            }
                        } else {
                            let _ = pending.reply.try_send(result);
                        }
                        continue;
                    }
                    Request::Prepare(p) => {
                        let result = self.changes.borrow_mut().prepare_with_originals(
                            editor,
                            p.clone(),
                            admission.as_ref(),
                        );
                        if admission.is_none()
                            && result["error"]["code"] == "CITED_ORIGINAL_ADMISSION_REQUIRED"
                        {
                            match editor.resolve_batch_commands(p.expected_revision, &p.change) {
                                Ok((commands, _)) => self.defer_cited_originals(
                                    editor,
                                    repository,
                                    Request::Prepare(p),
                                    commands,
                                    pending.reply,
                                ),
                                Err(error) => {
                                    let _ = pending.reply.try_send(json!({"error":error}));
                                }
                            }
                        } else {
                            let _ = pending.reply.try_send(result);
                        }
                        continue;
                    }
                    Request::Commit(p) => {
                        let result = self.changes.borrow_mut().commit_with_originals(
                            editor,
                            p.clone(),
                            false,
                            admission.as_ref(),
                        );
                        if admission.is_none()
                            && result["error"]["code"] == "CITED_ORIGINAL_RECHECK_REQUIRED"
                        {
                            let commands =
                                self.changes.borrow().cited_preview_commands(&p.preview_id);
                            if let Some(commands) = commands {
                                self.defer_cited_originals(
                                    editor,
                                    repository,
                                    Request::Commit(p),
                                    commands,
                                    pending.reply,
                                );
                            } else {
                                let _ = pending.reply.try_send(json!({"error":{"code":"PREVIEW_NOT_FOUND"},"operation_submitted":false}));
                            }
                        } else {
                            let _ = pending.reply.try_send(result);
                        }
                        continue;
                    }
                    Request::Operation(p) => {
                        let _ = pending
                            .reply
                            .try_send(self.changes.borrow_mut().operation(editor, p));
                        continue;
                    }
                    Request::Undo(p) => {
                        let _ = pending
                            .reply
                            .try_send(self.changes.borrow_mut().undo(editor, p));
                        continue;
                    }
                    Request::Cancel(p) => {
                        let _ = pending
                            .reply
                            .try_send(self.changes.borrow_mut().cancel(editor, p));
                        continue;
                    }
                    request => request,
                };
                let result = match request {
                    Request::ProjectSources(p) => {
                        crate::automation::sources::search(&self.service, editor, repository, &p)
                    }
                    Request::Validate(query) => self.service.validate(editor, &query),
                    Request::CitedReady { .. } => {
                        unreachable!("internal continuation was unwrapped before dispatch")
                    }
                    Request::Target => Ok(json!({"target":self.service.target(editor)})),
                    Request::Context(target) => {
                        self.service.context(editor, &target).map(|mut value| {
                            value["project_sources"] =
                                crate::automation::sources::context(editor, repository);
                            value["task_updates"] =
                                crate::design_process::update::summary(&editor.project);
                            value["design_task_operations"] =
                                self.changes.borrow_mut().task_checkpoint(editor);
                            value["write_authorization"] =
                                self.changes.borrow_mut().authorization_status(editor);
                            value["permissions"] = if self
                                .changes
                                .borrow_mut()
                                .write_authorization(editor)
                                .is_ok()
                            {
                                json!(["read", "write"])
                            } else {
                                json!(["read"])
                            };
                            value
                        })
                    }
                    Request::Query(query) => self.service.query(editor, &query),
                    Request::ProjectRules(request) => self.service.project_rules(editor, &request),
                    Request::SourceEvidence(request) => {
                        self.service.source_evidence(editor, &request)
                    }
                    Request::TerrainHeight(request) => {
                        self.service.terrain_height(editor, &request)
                    }
                    Request::AnalyzeSpaces(request) => {
                        self.service.analyze_spaces(editor, &request)
                    }
                    _ => unreachable!(),
                };
                let response = match result {
                    Ok(value) => json!({"result":value}),
                    Err(error) => json!({"error":error}),
                };
                let _ = pending.reply.try_send(response);
            }
        }
    }
    fn executable_identity() -> Value {
        let digest = || -> std::io::Result<String> {
            let mut file = fs::File::open(std::env::current_exe()?)?;
            let mut hasher = Sha256::new();
            let mut bytes = [0u8; 65536];
            loop {
                let n = file.read(&mut bytes)?;
                if n == 0 {
                    break;
                }
                hasher.update(&bytes[..n]);
            }
            Ok(format!("{:x}", hasher.finalize()))
        };
        match digest() {
            Ok(hash) => {
                json!({"status":"available","binary_sha256":hash,"package_version":env!("CARGO_PKG_VERSION"),"profile":if cfg!(debug_assertions){"debug"}else{"release"}})
            }
            Err(_) => json!({"status":"unknown","package_version":env!("CARGO_PKG_VERSION")}),
        }
    }
    fn read_frame(
        stream: &mut std::os::unix::net::UnixStream,
        stop: &AtomicBool,
    ) -> Result<Vec<u8>, &'static str> {
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        let mut bytes = Vec::new();
        let mut buffer = [0u8; 4096];
        loop {
            if stop.load(Ordering::Relaxed) {
                return Err("HOST_STOPPED");
            }
            if std::time::Instant::now() >= deadline {
                return Err("FRAME_TIMEOUT");
            }
            match stream.read(&mut buffer) {
                Ok(0) => return Err("INVALID_FRAME"),
                Ok(count) => {
                    bytes.extend_from_slice(&buffer[..count]);
                    if bytes.len() > 65536 {
                        return Err("INVALID_FRAME");
                    }
                    if let Some(end) = bytes.iter().position(|b| *b == b'\n') {
                        if end + 1 != bytes.len() {
                            return Err("INVALID_FRAME");
                        }
                        return Ok(bytes);
                    }
                }
                Err(e)
                    if matches!(
                        e.kind(),
                        std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
                    ) => {}
                Err(_) => return Err("INVALID_FRAME"),
            }
        }
    }
    impl Drop for Endpoint {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::Relaxed);
            if let Some(worker) = self.worker.take() {
                let _ = worker.join();
            }
            if fs::symlink_metadata(&self.path)
                .is_ok_and(|m| (m.dev(), m.ino()) == self.socket_identity)
            {
                let _ = fs::remove_file(&self.path);
            }
        }
    }
    #[cfg(test)]
    mod gate_tests {
        use super::*;
        use std::{
            io::{BufRead, BufReader},
            os::unix::net::UnixStream,
            path::Path,
            sync::mpsc,
            time::Instant,
        };
        fn call(endpoint: &mut Endpoint, editor: &mut Editor, path: &Path, value: Value) -> Value {
            let path = path.to_owned();
            let (tx, rx) = mpsc::channel();
            let worker = std::thread::spawn(move || {
                let mut s = UnixStream::connect(path).unwrap();
                s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
                writeln!(s, "{value}").unwrap();
                let mut line = String::new();
                BufReader::new(s).read_line(&mut line).unwrap();
                tx.send(serde_json::from_str::<Value>(&line).unwrap())
                    .unwrap();
            });
            let start = Instant::now();
            let result = loop {
                endpoint.poll(editor);
                if let Ok(v) = rx.try_recv() {
                    break v;
                }
                assert!(start.elapsed() < Duration::from_secs(5));
                std::thread::sleep(Duration::from_millis(2));
            };
            worker.join().unwrap();
            result
        }

        #[test]
        fn mass_space_default_host_socket_reads_previews_and_suspends_after_user_edit() {
            use crate::{
                application_host::ApplicationHost,
                command::ModelCommand,
                mass_spaces,
                model::{Material, Point, Project, Shape},
            };
            fn request(
                endpoint: &mut Endpoint,
                host: &mut ApplicationHost,
                path: &std::path::Path,
                value: Value,
            ) -> Value {
                let path = path.to_owned();
                let (tx, rx) = mpsc::channel();
                let worker = thread::spawn(move || {
                    let mut stream = UnixStream::connect(path).unwrap();
                    stream
                        .set_read_timeout(Some(Duration::from_secs(5)))
                        .unwrap();
                    writeln!(stream, "{value}").unwrap();
                    let mut line = String::new();
                    BufReader::new(stream).read_line(&mut line).unwrap();
                    tx.send(serde_json::from_str::<Value>(&line).unwrap())
                        .unwrap();
                });
                let start = Instant::now();
                let response = loop {
                    endpoint.poll_with_host(host, None);
                    if let Ok(value) = rx.try_recv() {
                        break value;
                    }
                    assert!(start.elapsed() < Duration::from_secs(5));
                    thread::sleep(Duration::from_millis(2));
                };
                worker.join().unwrap();
                response
            }
            let root = PathBuf::from("/tmp").join(format!(
                "oa-ms-ipc-{}",
                &crate::storage::new_document_identity()[..16]
            ));
            fs::create_dir(&root).unwrap();
            let mut p = Project::default();
            p.add(
                0,
                Material::Plaster,
                Shape::ConceptMass {
                    spec: crate::concept_mass::ConceptMassSpec {
                        arcs: vec![],
                        hole_arcs: vec![],
                        outline: vec![
                            Point::new(0., 0.),
                            Point::new(4., 0.),
                            Point::new(4., 3.),
                            Point::new(0., 3.),
                        ],
                        holes: vec![],
                        height: 5.,
                        bottom_offset: 0.,
                    },
                },
            );
            let mut host = ApplicationHost::new(p);
            let path = root.join("ipc/app.sock");
            let mut endpoint =
                Endpoint::bind_for_host(path.clone(), false, Some(root.join("journal")), &mut host)
                    .unwrap();
            let target = request(
                &mut endpoint,
                &mut host,
                &path,
                json!({"method":"get_target"}),
            )["result"]["target"]
                .clone();
            let query = request(
                &mut endpoint,
                &mut host,
                &path,
                json!({"method":"query_mass_spaces","params":{"target":target,"expected_revision":0,"mass":1,"offset":0,"limit":4}}),
            );
            let stamp = query["result"]["source"]["expected_sha256"]
                .as_str()
                .unwrap();
            assert_eq!(query["result"]["source"]["authored_area_m2"], 12.);
            let change = json!({"version":1,"edits":[{"operation":"mass_space","edit":{"action":"create","mass":1,"expected_source_sha256":stamp,"name":"Assumed space","purpose":"Unknown","height":3.,"assumption":"Explicit concept assumption, not measured"}}]});
            let prepare = json!({"method":"prepare_native_change","params":{"target":target,"expected_revision":0,"change":change}});
            let denied = request(&mut endpoint, &mut host, &path, prepare.clone());
            assert_eq!(denied["error"]["code"], "WRITE_NOT_AUTHORIZED", "{denied}");
            endpoint.set_write_enabled(true, &host.document.editor); // explicit existing native authorization, no runner-only fence
            host.set_remote_interaction_blocked(true);
            let active = request(&mut endpoint, &mut host, &path, prepare.clone());
            assert_eq!(
                active["error"]["code"], "USER_INTERACTION_ACTIVE",
                "{active}"
            );
            host.set_remote_interaction_blocked(false);
            host.pause_remote_writes(true);
            let paused = request(&mut endpoint, &mut host, &path, prepare.clone());
            assert_eq!(paused["error"]["code"], "REMOTE_WRITES_PAUSED");
            let still_readable = request(
                &mut endpoint,
                &mut host,
                &path,
                json!({"method":"get_target"}),
            );
            assert!(still_readable.get("result").is_some());
            assert_eq!(host.document.editor.project.elements.len(), 1);
            host.pause_remote_writes(false);
            let preview = request(&mut endpoint, &mut host, &path, prepare.clone());
            assert_eq!(
                preview["result"]["mass_space_change"]["after"][0]["mass"], 1,
                "{preview}"
            );
            assert_eq!(host.document.editor.project.elements.len(), 1);
            let commit = request(
                &mut endpoint,
                &mut host,
                &path,
                json!({"method":"commit_change","params":{"target":target,"preview_id":preview["result"]["preview_id"],"request_id":"original-space-socket"}}),
            );
            assert_eq!(commit["result"]["status"], "committed", "{commit}");
            let before_user = host.document.editor.project.clone();
            let zone = before_user.design_process.mass_spaces[0].zone;
            let l = before_user.design_process.mass_spaces[0].clone();
            let mut user = host
                .document
                .editor
                .project
                .elements
                .iter()
                .find(|e| e.id == zone)
                .unwrap()
                .clone();
            user.name = "User protected name".into();
            host.document
                .editor
                .apply(ModelCommand::UpdateElement { element: user })
                .unwrap();
            let protected = host.document.editor.project.clone();
            let revision = host.document.editor.revision;
            let new_read = request(
                &mut endpoint,
                &mut host,
                &path,
                json!({"method":"query_mass_spaces","params":{"target":target,"expected_revision":revision,"zone":zone,"offset":0,"limit":4}}),
            );
            assert_eq!(
                new_read["result"]["links"][0]["status"],
                "Linked space has user edits"
            );
            let refresh = json!({"method":"prepare_native_change","params":{"target":target,"expected_revision":revision,"change":{"version":1,"edits":[{"operation":"mass_space","edit":{"action":"refresh","zone":zone,"expected_sha256":mass_spaces::expected(&l)}}]}}});
            let suspended = request(&mut endpoint, &mut host, &path, refresh.clone());
            assert_eq!(
                suspended["error"]["code"], "WRITE_AUTHORIZATION_SUSPENDED",
                "{suspended}"
            );
            endpoint.set_write_enabled(true, &host.document.editor); // even a new native grant cannot authorize geometric overwrite of an edited Zone
            let refused = request(&mut endpoint, &mut host, &path, refresh);
            assert!(refused["error"].is_object(), "{refused}");
            assert!(refused.to_string().contains("user edits"), "{refused}");
            assert_eq!(host.document.editor.project, protected);
            let stale = request(
                &mut endpoint,
                &mut host,
                &path,
                json!({"method":"query_mass_spaces","params":{"target":target,"expected_revision":0,"zone":zone,"offset":0,"limit":4}}),
            );
            assert_eq!(stale["error"]["code"], "REVISION_CONFLICT");
            drop(endpoint);
            drop(host);
            fs::remove_dir_all(root).unwrap();
        }
        #[test]
        fn automation_queued_request_wakes_owner_before_any_editor_poll() {
            let root = PathBuf::from("/tmp").join(format!(
                "oa-wake-{}",
                &crate::storage::new_document_identity()[..16]
            ));
            fs::create_dir(&root).unwrap();
            let path = root.join("ipc/app.sock");
            let mut endpoint = Endpoint::bind_configured(path.clone(), false, None).unwrap();
            let (notify, wake) = mpsc::channel();
            endpoint.set_wake(move || {
                notify.send(()).unwrap();
            });
            let worker = thread::spawn(move || {
                let mut stream = UnixStream::connect(path).unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                writeln!(stream, "{}", json!({"method":"get_target"})).unwrap();
                let mut line = String::new();
                BufReader::new(stream).read_line(&mut line).unwrap();
                serde_json::from_str::<Value>(&line).unwrap()
            });
            wake.recv_timeout(Duration::from_secs(2)).unwrap();
            let mut editor = Editor::new(crate::model::Project::default());
            let before = editor.project.clone();
            endpoint.poll(&mut editor);
            let response = worker.join().unwrap();
            assert!(response["result"]["target"].is_object(), "{response}");
            assert_eq!(editor.project, before);
            assert_eq!(editor.revision, 0);
            assert!(!editor.dirty());
            drop(endpoint);
            fs::remove_dir_all(root).unwrap();
        }
        #[test]
        fn mcp_gate_two_instances_sessions_and_readonly_writes_are_independent() {
            let root = PathBuf::from("/tmp").join(format!(
                "oa-gate-{}",
                &crate::storage::new_document_identity()[..16]
            ));
            fs::create_dir(&root).unwrap();
            let a_path = root.join("a/app.sock");
            let b_path = root.join("b/app.sock");
            let mut a = Endpoint::bind_configured(a_path.clone(), false, None).unwrap();
            let mut b = Endpoint::bind_configured(b_path.clone(), false, None).unwrap();
            let mut ea = Editor::new(crate::model::Project::default());
            let mut eb = Editor::new(ea.project.clone());
            eb.adopt_document_identity(ea.project_identity().into())
                .unwrap();
            let target_a = call(&mut a, &mut ea, &a_path, json!({"method":"get_target"}))["result"]
                ["target"]
                .clone();
            let target_b = call(&mut b, &mut eb, &b_path, json!({"method":"get_target"}))["result"]
                ["target"]
                .clone();
            assert_ne!(target_a["instance_id"], target_b["instance_id"]);
            assert_eq!(target_a["project_id"], target_b["project_id"]);
            let before = (
                ea.project.clone(),
                eb.project.clone(),
                ea.revision,
                eb.revision,
            );
            assert_eq!(
                call(
                    &mut b,
                    &mut eb,
                    &b_path,
                    json!({"method":"get_context","params":target_a})
                )["error"]["code"],
                "TARGET_MISMATCH"
            );
            let change = json!({"version":1,"creates":[{"name":"wall","story":0,"layer":1,"material":"concrete","geometry":{"type":"wall","start":{"x":0.,"y":0.},"end":{"x":4.,"y":0.},"thickness":0.2,"height":3.}}]});
            for (target, expected) in [
                (&target_a, "TARGET_MISMATCH"),
                (&target_b, "WRITE_NOT_AUTHORIZED"),
            ] {
                assert_eq!(
                    call(
                        &mut b,
                        &mut eb,
                        &b_path,
                        json!({"method":"prepare_native_change","params":{"target":target,"expected_revision":0,"change":change}})
                    )["error"]["code"],
                    expected
                );
            }
            let build = call(&mut a, &mut ea, &a_path, json!({"method":"list_instances"}));
            assert_eq!(
                build["result"]["instances"][0]["build_identity"]["binary_sha256"]
                    .as_str()
                    .unwrap()
                    .len(),
                64
            );
            assert_eq!(
                (
                    ea.project.clone(),
                    eb.project.clone(),
                    ea.revision,
                    eb.revision
                ),
                before
            );
            assert!(!ea.dirty() && !eb.dirty());
            assert!(ea.undo.is_empty() && eb.undo.is_empty());
            let mut next = Editor::new(eb.project.clone());
            next.adopt_document_identity(eb.project_identity().into())
                .unwrap();
            eb = next;
            assert_eq!(
                call(
                    &mut b,
                    &mut eb,
                    &b_path,
                    json!({"method":"get_context","params":target_b})
                )["error"]["code"],
                "TARGET_MISMATCH"
            );
            drop(a);
            drop(b);
            fs::remove_dir_all(root).unwrap();
        }
    }
}
#[cfg(unix)]
pub use unix::{Endpoint, FileParams, WorkbenchInvoke};
