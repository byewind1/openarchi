//! Protected application-exit fixture. Never targets an existing app or user program.
#![cfg(unix)]
use atelier_bim::{
    application_host::ApplicationHost,
    automation_ipc::Endpoint,
    command::ModelCommand,
    editor::Editor,
    model::{Material, Point, Project, Shape},
    plugins::{
        Manifest, Registry,
        jobs::{Input, Jobs},
        provider::ExecutableGrant,
    },
    storage,
    zones::ZoneSpec,
};
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Write},
    os::unix::net::UnixStream,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

fn manifest() -> Manifest {
    let mut v: Value = serde_json::from_slice(include_bytes!(
        "../assets/plugins/example-zone-program.json"
    ))
    .unwrap();
    v["provider"]["timeout_ms"] = json!(15000);
    v["commands"][0]["operation"] = json!("authored_quantities");
    v["permissions"] = json!(["read_authored_geometry", "execute_local_program"]);
    Manifest::parse(&serde_json::to_vec(&v).unwrap()).unwrap()
}
fn read_host(root: &Path) -> ApplicationHost {
    let mut h = ApplicationHost::new(Project::default());
    h.document.editor = Editor::from_stored_document(&root.join("project.json")).unwrap();
    h.configure_plugins(&root.join("profile")).unwrap();
    h
}
#[test]
#[ignore = "Owned subprocess fixture; only invoked with explicit protected root"]
fn crash_origin_child() {
    let Some(root) = std::env::var_os("OA_JOB_CRASH_ROOT") else {
        return;
    };
    let root = PathBuf::from(root);
    let mut h = read_host(&root);
    h.jobs
        .as_mut()
        .unwrap()
        .use_process_worker(Path::new(env!("CARGO_BIN_EXE_openarchi")))
        .unwrap();
    h.plugins
        .enable_reviewed_executable(
            manifest(),
            ExecutableGrant::review(&root.join("provider-fixture")).unwrap(),
        )
        .unwrap();
    let id = std::fs::read_to_string(root.join("original-request")).unwrap();
    h.start_plugin_job("example.zone_program.review", &id)
        .unwrap();
    loop {
        h.jobs.as_mut().unwrap().poll();
        if root.join("close-origin").exists() {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}
struct Release(PathBuf);
struct Origin(Child);
impl Drop for Origin {
    fn drop(&mut self) {
        if matches!(self.0.try_wait(), Ok(None)) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}
impl Drop for Release {
    fn drop(&mut self) {
        let _ = std::fs::write(&self.0, b"release original provider");
    }
}
#[test]
fn original_external_result_survives_application_crash_without_replay() {
    scenario("complete");
}
#[test]
fn original_cancel_after_application_crash_reaches_only_original_worker() {
    scenario("cancel");
}
#[test]
fn ordinary_close_persists_existing_cancellation_intent() {
    scenario("close");
}
fn call(ep: &mut Endpoint, h: &mut ApplicationHost, path: &Path, value: Value) -> Value {
    let path = path.to_owned();
    let client = std::thread::spawn(move || {
        let mut s = UnixStream::connect(path).unwrap();
        s.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
        writeln!(s, "{value}").unwrap();
        let mut line = String::new();
        BufReader::new(s).read_line(&mut line).unwrap();
        serde_json::from_str(&line).unwrap()
    });
    let until = Instant::now() + Duration::from_secs(3);
    while !client.is_finished() {
        ep.poll_with_host(h, None);
        assert!(Instant::now() < until);
        std::thread::sleep(Duration::from_millis(1));
    }
    client.join().unwrap()
}
fn scenario(mode: &str) {
    let root = PathBuf::from("/tmp").join(format!(
        "oa-worker-crash-{}",
        storage::new_document_identity()
    ));
    std::fs::create_dir(&root).unwrap();
    println!("WORKER_CRASH_ROOT={}", root.display());
    let _release = Release(root.join("release-provider"));
    let mut p = Project::default();
    p.add(
        0,
        Material::Timber,
        Shape::Zone {
            spec: ZoneSpec {
                surface: Default::default(),
                purpose: "Meeting".into(),
                outline: vec![
                    Point::new(0., 0.),
                    Point::new(4., 0.),
                    Point::new(4., 6.),
                    Point::new(0., 6.),
                ],
                holes: vec![],
                height: 3.,
                bottom_offset: 0.,
                label_position: None,
            },
        },
    );
    let h = ApplicationHost::new(p);
    storage::save_document(
        &h.document.editor.project,
        &root.join("project.json"),
        h.document.editor.project_identity(),
    )
    .unwrap();
    let input = Input::capture(&h.document.editor).unwrap();
    let request = storage::new_document_identity();
    std::fs::write(root.join("original-request"), &request).unwrap();
    let identity = json!({"project_id":h.document.editor.project_identity(),"request_id":request,"plugin_id":"example.zone_program","plugin_version":1,"command_id":"example.zone_program.review","input_sha256":input.fingerprint().unwrap()});
    let artifact = json!({"type":"result","artifact":{"schema_version":1,"identity":identity,"terminal":{"phase":"completed","report":{"definition":"Authored boundary minus holes; overlaps counted separately within each domain. Zone area and mass footprint are separate; not net/legal floor area or compliance.","zones":{"element_ids":[1],"area_m2":24.,"volume_m3":72.},"concept_masses":{"element_ids":[],"area_m2":0.,"volume_m3":0.}}}}});
    let source = format!(
        r#"use std::io::Read;fn main(){{
        let mut input=Vec::new();std::io::stdin().read_to_end(&mut input).unwrap();assert!(!input.is_empty());
        let root=std::path::Path::new({root:?});
        use std::io::Write;writeln!(std::fs::OpenOptions::new().write(true).create(true).append(true).open(root.join("provider-attempts")).unwrap(),"{{}}",std::process::id()).unwrap();
        std::fs::OpenOptions::new().write(true).create_new(true).open(root.join("provider-launched-once")).expect("original request may execute once only");
        std::fs::write(root.join("provider-ready"),std::process::id().to_string()).unwrap();
        let until=std::time::Instant::now()+std::time::Duration::from_secs(8);
        while !root.join("release-provider").exists(){{if std::time::Instant::now()>until{{std::process::exit(20);}}std::thread::sleep(std::time::Duration::from_millis(5));}}
        println!("{{}}",{result:?});
    }}"#,
        root = root.to_string_lossy(),
        result = artifact.to_string()
    );
    std::fs::write(root.join("provider-fixture.rs"), source).unwrap();
    let compiled = Command::new(std::env::var_os("RUSTC").unwrap_or("rustc".into()))
        .args(["--edition=2024", "--crate-name", "owned_provider_fixture"])
        .arg(root.join("provider-fixture.rs"))
        .arg("-o")
        .arg(root.join("provider-fixture"))
        .output()
        .unwrap();
    assert!(
        compiled.status.success(),
        "{}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    if mode == "complete" {
        let mut unavailable = read_host(&root);
        unavailable
            .plugins
            .enable_reviewed_executable(
                manifest(),
                ExecutableGrant::review(&root.join("provider-fixture")).unwrap(),
            )
            .unwrap();
        assert!(
            unavailable
                .jobs
                .as_mut()
                .unwrap()
                .use_process_worker(&root.join("missing-worker"))
                .is_err()
        );
        assert!(
            unavailable
                .start_plugin_job("example.zone_program.review", &request)
                .unwrap_err()
                .contains("no program started")
        );
        assert!(
            !root
                .join("profile/plugin-jobs")
                .join(h.document.editor.project_identity())
                .join(&request)
                .exists()
        );
    }
    let log = std::fs::File::create(root.join("origin-app.log")).unwrap();
    let mut child = Origin(
        Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "crash_origin_child", "--ignored", "--nocapture"])
            .env("OA_JOB_CRASH_ROOT", &root)
            .stdout(Stdio::from(log.try_clone().unwrap()))
            .stderr(Stdio::from(log))
            .spawn()
            .unwrap(),
    );
    let until = Instant::now() + Duration::from_secs(6);
    while !root.join("provider-ready").exists() {
        assert!(child.0.try_wait().unwrap().is_none());
        assert!(Instant::now() < until);
        std::thread::sleep(Duration::from_millis(5));
    }
    // This Child was created above and has not been reaped; never use a saved PID.
    if mode == "close" {
        std::fs::write(root.join("close-origin"), b"ordinary close").unwrap();
    } else {
        child.0.kill().unwrap();
    }
    let status = child.0.wait().unwrap();
    assert_eq!(status.success(), mode == "close");
    std::fs::write(
        root.join("origin-app-terminal.json"),
        serde_json::to_vec(
            &json!({"pid":child.0.id(),"killed_owned_fixture":mode!="close","status":format!("{status}")}),
        )
        .unwrap(),
    )
    .unwrap();
    let mut restored = read_host(&root);
    let project = restored.document.editor.project_identity().to_owned();
    let pending = restored
        .jobs
        .as_ref()
        .unwrap()
        .query(&project, &request)
        .unwrap();
    if mode != "close" {
        assert!(pending.result.is_none());
    }
    assert_eq!(
        serde_json::to_value(&pending.intent.identity).unwrap(),
        identity
    );
    let socket = root.join("ipc/app.sock");
    let mut endpoint = Endpoint::bind_application(socket.clone(), &mut restored).unwrap();
    let target = restored.operation_target();
    let context = call(
        &mut endpoint,
        &mut restored,
        &socket,
        json!({"method":"get_context","params":target}),
    );
    assert_eq!(context["result"]["permissions"], json!(["read"]));
    let params = json!({"target":target,"expected_revision":0,"expected_context_revision":context["result"]["context_revision"],"request_id":request,"offset":0,"limit":32768});
    if mode != "close" {
        let observed = restored
            .jobs
            .as_ref()
            .unwrap()
            .execution_observation(&pending.intent.identity)
            .unwrap();
        assert_eq!(observed.status, "Running");
        let read = call(
            &mut endpoint,
            &mut restored,
            &socket,
            json!({"method":"read_plugin_job","params":params}),
        );
        assert_eq!(
            read["result"]["execution_observation"], "running_observed_at_request",
            "{read}"
        );
    }
    if mode == "cancel" {
        assert!(
            restored
                .jobs
                .as_mut()
                .unwrap()
                .cancel(&project, &request)
                .unwrap()
                .cancel_requested
        );
    }
    if mode == "complete" {
        restored
            .document
            .editor
            .apply(ModelCommand::MoveElements {
                ids: vec![1],
                offset: Point::new(0.25, 0.),
            })
            .unwrap();
    }
    let before = restored.document.editor.project.clone();
    if mode == "complete" {
        std::fs::write(
            root.join("release-provider"),
            b"finish the original invocation",
        )
        .unwrap();
    }
    let until = Instant::now() + Duration::from_secs(4);
    let result = loop {
        restored.jobs.as_mut().unwrap().poll();
        let b = restored
            .jobs
            .as_ref()
            .unwrap()
            .query(&project, &request)
            .unwrap();
        if b.result.is_some() {
            break b;
        }
        if Instant::now() >= until {
            break b;
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    std::fs::write(
        root.join("recovered-original.json"),
        serde_json::to_vec_pretty(&result).unwrap(),
    )
    .unwrap();
    assert_eq!(restored.document.editor.project, before);
    assert_eq!(
        result.phase(),
        if mode == "complete" {
            "Completed"
        } else {
            "Cancelled"
        },
        "original result writer was lost; unknown must not be replayed: {}",
        root.display()
    );
    assert_eq!(result.intent, pending.intent);
    let receipt = restored
        .jobs
        .as_ref()
        .unwrap()
        .process_receipt(&result.intent.identity)
        .unwrap()
        .unwrap();
    assert_eq!(receipt.cancelled, mode != "complete");
    if mode == "complete" {
        assert_eq!(receipt.exit_code, Some(0));
        assert!(!result.current(&restored.document.editor));
        let stale = call(
            &mut endpoint,
            &mut restored,
            &socket,
            json!({"method":"read_plugin_job","params":params}),
        );
        assert_eq!(stale["error"]["code"], "REVISION_CONFLICT");
        let context = call(
            &mut endpoint,
            &mut restored,
            &socket,
            json!({"method":"get_context","params":target}),
        );
        assert_eq!(context["result"]["permissions"], json!(["read"]));
        let mut fresh = params.clone();
        fresh["expected_revision"] = json!(restored.document.editor.revision);
        fresh["expected_context_revision"] = context["result"]["context_revision"].clone();
        let original = call(
            &mut endpoint,
            &mut restored,
            &socket,
            json!({"method":"read_plugin_job","params":fresh}),
        );
        assert_eq!(original["result"]["input_status"], "stale");
        assert_eq!(original["result"]["identity"]["request_id"], request);
        assert_eq!(
            original["result"]["execution_observation"],
            "terminal_artifact"
        );
    }
    assert!(!receipt.timed_out);
    let previous = std::fs::read(root.join("recovered-original.json")).unwrap();
    let denied = Command::new(env!("CARGO_BIN_EXE_openarchi"))
        .args(["--openarchi-plugin-worker"])
        .arg(root.join("profile/plugin-jobs"))
        .arg(&project)
        .arg(&request)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(!denied.status.success());
    assert_eq!(
        std::fs::read_to_string(root.join("provider-attempts"))
            .unwrap()
            .lines()
            .count(),
        1
    );
    assert_eq!(
        std::fs::read(root.join("recovered-original.json")).unwrap(),
        previous
    );
    if mode == "complete" {
        copied_unfinished_receipts_are_not_execution_ownership(&root, &result.intent.identity);
    }
}

fn copied_unfinished_receipts_are_not_execution_ownership(
    root: &Path,
    identity: &atelier_bim::plugins::jobs::Identity,
) {
    let source = root
        .join("profile/plugin-jobs")
        .join(&identity.project_id)
        .join(&identity.request_id);
    let store = root.join("copied-unfinished-store");
    let copied = store.join(&identity.project_id).join(&identity.request_id);
    std::fs::create_dir_all(&copied).unwrap();
    for name in [
        "intent.json",
        "process.json",
        "worker-permit.json",
        "worker-claimed.json",
        "worker-lease",
        "worker-progress.json",
    ] {
        std::fs::copy(source.join(name), copied.join(name)).unwrap();
    }
    // A live, unrelated PID is deliberately only diagnostic. A copied/free
    // lease can neither adopt it nor authorize another invocation.
    let mut claim: Value =
        serde_json::from_slice(&std::fs::read(copied.join("worker-claimed.json")).unwrap())
            .unwrap();
    claim["pid"] = json!(std::process::id());
    std::fs::write(
        copied.join("worker-claimed.json"),
        serde_json::to_vec(&claim).unwrap(),
    )
    .unwrap();
    let mut jobs = Jobs::new(store.clone());
    jobs.use_process_worker(Path::new(env!("CARGO_BIN_EXE_openarchi")))
        .unwrap();
    assert_eq!(
        jobs.execution_observation(identity).unwrap().status,
        "OwnerUnknown"
    );
    assert!(
        jobs.progress(&identity.project_id, &identity.request_id)
            .is_none()
    );
    let mut registry = Registry::open(&root.join("copied-registry.json")).unwrap();
    registry
        .enable_reviewed_executable(
            manifest(),
            ExecutableGrant::review(&root.join("provider-fixture")).unwrap(),
        )
        .unwrap();
    let editor = Editor::from_stored_document(&root.join("project.json")).unwrap();
    assert_eq!(
        jobs.start(
            &registry,
            &editor,
            "example.zone_program.review",
            &identity.request_id
        )
        .unwrap(),
        *identity
    );
    assert!(
        jobs.query(&identity.project_id, &identity.request_id)
            .unwrap()
            .result
            .is_none()
    );
    let cancelled = jobs
        .cancel(&identity.project_id, &identity.request_id)
        .unwrap();
    assert_eq!(cancelled.phase(), "Cancel requested");
    assert_eq!(
        jobs.execution_observation(identity).unwrap().status,
        "OwnerUnknown"
    );
    let permit: Value =
        serde_json::from_slice(&std::fs::read(copied.join("worker-permit.json")).unwrap()).unwrap();
    let nonce = "0".repeat(64);
    assert_ne!(
        atelier_bim::drawings::digest(nonce.as_bytes()),
        permit["nonce_sha256"]
    );
    let mut worker = Command::new(env!("CARGO_BIN_EXE_openarchi"))
        .arg("--openarchi-plugin-worker")
        .arg(store)
        .arg(&identity.project_id)
        .arg(&identity.request_id)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    worker
        .stdin
        .take()
        .unwrap()
        .write_all(
            serde_json::to_string(&json!({"nonce":nonce,"intent_sha256":permit["intent_sha256"]}))
                .unwrap()
                .as_bytes(),
        )
        .unwrap();
    assert!(!worker.wait_with_output().unwrap().status.success());
    assert_eq!(
        std::fs::read_to_string(root.join("provider-attempts"))
            .unwrap()
            .lines()
            .count(),
        1
    );
    assert_eq!(unsafe { libc::kill(std::process::id() as i32, 0) }, 0);
    assert!(
        jobs.query(&identity.project_id, &identity.request_id)
            .unwrap()
            .result
            .is_none()
    );
}
