#![cfg(unix)]
use atelier_bim::{
    application_host::ApplicationHost,
    automation_ipc::Endpoint,
    command::ModelCommand,
    concept_mass::ConceptMassSpec,
    model::{Material, Point, Project, Shape},
    plugins::{Manifest, Registry, jobs::Bundle},
    storage,
};
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Write},
    os::unix::net::UnixStream,
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant},
};
fn root() -> PathBuf {
    let p =
        PathBuf::from("/tmp").join(format!("oa-jr-{}", &storage::new_document_identity()[..12]));
    std::fs::create_dir(&p).unwrap();
    p
}
fn host(r: &Path) -> ApplicationHost {
    let mut p = Project::default();
    p.add(
        0,
        Material::Timber,
        Shape::ConceptMass {
            spec: ConceptMassSpec {
                arcs: vec![],
                hole_arcs: vec![],
                outline: vec![
                    Point::new(30., 30.),
                    Point::new(38., 30.),
                    Point::new(38., 36.),
                    Point::new(30., 36.),
                ],
                holes: vec![vec![
                    Point::new(32., 32.),
                    Point::new(34., 32.),
                    Point::new(34., 34.),
                    Point::new(32., 34.),
                ]],
                height: 6.,
                bottom_offset: 0.5,
            },
        },
    );
    let mut h = ApplicationHost::new(p);
    h.configure_plugins(r).unwrap();
    h.plugins
        .enable(
            Manifest::parse(include_bytes!("../assets/plugins/example-quantities.json")).unwrap(),
        )
        .unwrap();
    h
}
fn call(ep: &mut Endpoint, h: &mut ApplicationHost, path: &Path, value: Value) -> Value {
    let path = path.to_owned();
    let client = thread::spawn(move || {
        let mut s = UnixStream::connect(path).unwrap();
        s.set_read_timeout(Some(Duration::from_secs(6))).unwrap();
        writeln!(s, "{value}").unwrap();
        let mut line = String::new();
        BufReader::new(s).read_line(&mut line).unwrap();
        serde_json::from_str(&line).unwrap()
    });
    let start = Instant::now();
    while !client.is_finished() {
        ep.poll_with_host(h, None);
        assert!(start.elapsed() < Duration::from_secs(6));
        thread::sleep(Duration::from_millis(1));
    }
    client.join().unwrap()
}
fn job(h: &mut ApplicationHost) -> String {
    let id = storage::new_document_identity();
    h.start_plugin_job("example.quantities.review", &id)
        .unwrap();
    let start = Instant::now();
    loop {
        let jobs = h.jobs.as_mut().unwrap();
        jobs.poll();
        if jobs
            .query(h.document.editor.project_identity(), &id)
            .unwrap()
            .result
            .is_some()
        {
            return id;
        }
        assert!(start.elapsed() < Duration::from_secs(4));
        thread::sleep(Duration::from_millis(1));
    }
}
fn params(ep: &mut Endpoint, h: &mut ApplicationHost, path: &Path, id: &str) -> Value {
    let target = h.operation_target();
    let ctx = call(ep, h, path, json!({"method":"get_context","params":target}));
    assert_eq!(ctx["result"]["permissions"], json!(["read"]));
    json!({"target":target,"expected_revision":h.document.editor.revision,
        "expected_context_revision":ctx["result"]["context_revision"],"request_id":id,"offset":0,"limit":127})
}
fn read(ep: &mut Endpoint, h: &mut ApplicationHost, path: &Path, p: Value) -> (Value, Bundle) {
    let mut p = p;
    let mut bytes = vec![];
    let mut first: Option<Value> = None;
    let mut pages = 0;
    loop {
        let r = call(ep, h, path, json!({"method":"read_plugin_job","params":p}));
        assert!(r.get("error").is_none(), "{r}");
        let v = r["result"].clone();
        assert_eq!(v["offset"], bytes.len());
        assert_eq!(v["dispatch"], false);
        for hex in v["data"].as_str().unwrap().as_bytes().chunks_exact(2) {
            bytes.push(u8::from_str_radix(std::str::from_utf8(hex).unwrap(), 16).unwrap());
        }
        if let Some(first) = &first {
            assert_eq!(v["content_sha256"], first["content_sha256"]);
        } else {
            first = Some(v.clone());
        }
        pages += 1;
        if v["next_offset"].is_null() {
            assert_eq!(v["total_bytes"], bytes.len());
            break;
        }
        p["offset"] = v["next_offset"].clone();
        p["content_sha256"] = v["content_sha256"].clone();
    }
    assert!(pages > 1);
    let f = first.unwrap();
    assert_eq!(f["content_sha256"], atelier_bim::drawings::digest(&bytes));
    (f, serde_json::from_slice(&bytes).unwrap())
}

#[test]
fn default_endpoint_reads_native_archive_without_job_store_or_provider() {
    let r = root();
    let mut source = host(&r.join("source"));
    let id = job(&mut source);
    let snapshot = source.project_snapshot_for_save().collect().unwrap();
    let pid = source.document.editor.project_identity().to_owned();
    let expected = snapshot.job_archive.records()[0].bundle.clone();
    let file = r.join("carried.json");
    storage::save_document(&snapshot, &file, &pid).unwrap();
    let bytes = std::fs::read(&file).unwrap();
    let mut h = ApplicationHost::new(Project::default());
    h.document.editor = atelier_bim::editor::Editor::from_stored_document(&file).unwrap();
    h.configure_plugins(&r.join("fresh")).unwrap();
    let context = h.context();
    let before = h.document.editor.project.clone();
    let path = r.join("ipc/app.sock");
    let mut ep = Endpoint::bind_application(path.clone(), &mut h).unwrap();
    let p = params(&mut ep, &mut h, &path, &id);
    let target = h.operation_target();
    let page = call(
        &mut ep,
        &mut h,
        &path,
        json!({"method":"list_plugin_jobs","params":{"target":target,"expected_revision":0,"offset":0,"limit":16}}),
    );
    assert_eq!(page["result"]["request_ids"], json!([id]));
    let (state, b) = read(&mut ep, &mut h, &path, p.clone());
    assert_eq!(state["input_status"], "current");
    assert_eq!(b, expected);
    assert!(h.jobs.as_ref().unwrap().ids(&pid).unwrap().is_empty());
    assert_eq!(h.plugins.entries().len(), 2);
    let mut process = p;
    process["artifact"] = json!("process");
    assert_eq!(
        call(
            &mut ep,
            &mut h,
            &path,
            json!({"method":"read_plugin_job","params":process})
        )["error"]["code"],
        "JOB_PROCESS_UNKNOWN"
    );
    assert_eq!(h.context(), context);
    assert_eq!(h.document.editor.project, before);
    assert_eq!(std::fs::read(file).unwrap(), bytes);
}

#[test]
fn broken_process_diagnostic_does_not_hide_valid_original_input_and_result() {
    let r = root();
    let mut h = host(&r);
    let id = job(&mut h);
    let pid = h.document.editor.project_identity().to_owned();
    let broken = r
        .join("plugin-jobs")
        .join(&pid)
        .join(&id)
        .join("process-result.json");
    std::fs::write(&broken, b"bad diagnostic; preserve").unwrap();
    let path = r.join("ipc/app.sock");
    let mut ep = Endpoint::bind_application(path.clone(), &mut h).unwrap();
    let mut p = params(&mut ep, &mut h, &path, &id);
    let (v, _) = read(&mut ep, &mut h, &path, p.clone());
    assert_eq!(v["phase"], "Completed");
    p["artifact"] = json!("process");
    assert_eq!(
        call(
            &mut ep,
            &mut h,
            &path,
            json!({"method":"read_plugin_job","params":p})
        )["error"]["code"],
        "JOB_PROCESS_UNKNOWN"
    );
    assert!(h.project_snapshot_for_save().collect().is_err());
    assert_eq!(std::fs::read(broken).unwrap(), b"bad diagnostic; preserve");
}
#[test]
fn default_readonly_host_recovers_original_artifact_without_provider_and_tracks_user_scope() {
    let r = root();
    let mut h = host(&r);
    let id = job(&mut h);
    let before = h.document.editor.project.clone();
    h.set_plugin_enabled("example.quantities", false).unwrap();
    h.plugins = Registry::default(); // missing external provider
    let path = r.join("ipc/app.sock");
    let mut ep = Endpoint::bind_application(path.clone(), &mut h).unwrap();
    let p = params(&mut ep, &mut h, &path, &id);
    let (v, b) = read(&mut ep, &mut h, &path, p.clone());
    let mut process = p.clone();
    process["artifact"] = json!("process");
    assert_eq!(
        call(
            &mut ep,
            &mut h,
            &path,
            json!({"method":"read_plugin_job","params":process})
        )["error"]["code"],
        "JOB_PROCESS_UNKNOWN"
    );
    assert_eq!(v["input_status"], "current");
    assert_eq!(v["phase"], "Completed");
    assert_eq!(b.intent.identity.request_id, id);
    assert_eq!(v["dispatch"], false);
    let raw = serde_json::to_value(&b).unwrap();
    assert_eq!(
        raw["result"]["terminal"]["report"]["concept_masses"]["area_m2"],
        44.
    );
    assert_eq!(
        raw["result"]["terminal"]["report"]["concept_masses"]["volume_m3"],
        264.
    );
    assert_eq!(h.document.editor.project, before);
    assert!(h.document.editor.undo.is_empty());
    assert!(!h.document.editor.dirty());
    let source = before.elements[0].id;
    h.document
        .editor
        .apply(ModelCommand::MoveElements {
            ids: vec![source],
            offset: Point::new(0.25, 0.),
        })
        .unwrap();
    assert_eq!(
        call(
            &mut ep,
            &mut h,
            &path,
            json!({"method":"read_plugin_job","params":p})
        )["error"]["code"],
        "REVISION_CONFLICT"
    );
    let next = params(&mut ep, &mut h, &path, &id);
    let (v, stale) = read(&mut ep, &mut h, &path, next.clone());
    assert_eq!(v["input_status"], "stale");
    assert_eq!(stale, b);
    h.document.editor.undo();
    let next = params(&mut ep, &mut h, &path, &id);
    assert_eq!(
        read(&mut ep, &mut h, &path, next.clone()).0["input_status"],
        "current"
    );
    let mut bad = next.clone();
    bad["target"]["project_id"] = json!(storage::new_document_identity());
    assert_eq!(
        call(
            &mut ep,
            &mut h,
            &path,
            json!({"method":"read_plugin_job","params":bad})
        )["error"]["code"],
        "TARGET_MISMATCH"
    );
    let mut bad = next.clone();
    bad["expected_context_revision"] = json!("old-context");
    assert_eq!(
        call(
            &mut ep,
            &mut h,
            &path,
            json!({"method":"read_plugin_job","params":bad})
        )["error"]["code"],
        "CONTEXT_CONFLICT"
    );
    for (key, value, code) in [
        ("offset", json!(usize::MAX), "INVALID_PAGE"),
        ("limit", json!(32769), "INVALID_PAGE"),
        ("request_id", json!("../result"), "JOB_ARTIFACT_UNKNOWN"),
        ("content_sha256", json!("other"), "ARTIFACT_CHANGED"),
    ] {
        let mut bad = next.clone();
        bad[key] = value;
        assert_eq!(
            call(
                &mut ep,
                &mut h,
                &path,
                json!({"method":"read_plugin_job","params":bad})
            )["error"]["code"],
            code
        );
    }
    let mut bad = next.clone();
    bad["offset"] = json!(1);
    assert_eq!(
        call(
            &mut ep,
            &mut h,
            &path,
            json!({"method":"read_plugin_job","params":bad})
        )["error"]["code"],
        "ARTIFACT_CHANGED"
    );
    let saved = r.join("project.json");
    storage::save_document(
        &h.document.editor.project,
        &saved,
        h.document.editor.project_identity(),
    )
    .unwrap();
    drop(ep);
    let mut restored = ApplicationHost::new(Project::default());
    restored.document.editor = atelier_bim::editor::Editor::from_stored_document(&saved).unwrap();
    restored.configure_plugins(&r).unwrap();
    restored.plugins = Registry::default();
    let path = r.join("reopened/app.sock");
    let mut ep = Endpoint::bind_application(path.clone(), &mut restored).unwrap();
    let next = params(&mut ep, &mut restored, &path, &id);
    assert_eq!(read(&mut ep, &mut restored, &path, next).1, b);
    assert_eq!(
        restored
            .jobs
            .as_ref()
            .unwrap()
            .ids(restored.document.editor.project_identity())
            .unwrap(),
        vec![id]
    );
}
#[test]
fn public_catalog_and_index_pages_bind_content_unknown_results_never_dispatch() {
    let r = root();
    let mut h = host(&r);
    let id = job(&mut h);
    let path = r.join("ipc/app.sock");
    let mut ep = Endpoint::bind_application(path.clone(), &mut h).unwrap();
    let target = h.operation_target();
    let p = json!({"target":target,"offset":0,"limit":1});
    let first = call(
        &mut ep,
        &mut h,
        &path,
        json!({"method":"list_plugin_commands","params":p}),
    );
    assert_eq!(first["result"]["total"], 3);
    assert_eq!(first["result"]["remote_dispatch"], false);
    let second = json!({"target":target,"offset":1,"limit":1,"catalog_sha256":first["result"]["catalog_sha256"]});
    assert_eq!(
        call(
            &mut ep,
            &mut h,
            &path,
            json!({"method":"list_plugin_commands","params":second})
        )["result"]["entries"][0]["manifest"]["id"],
        "org.openarchi.solar"
    );
    let third = json!({"target":target,"offset":2,"limit":1,"catalog_sha256":first["result"]["catalog_sha256"]});
    assert_eq!(
        call(
            &mut ep,
            &mut h,
            &path,
            json!({"method":"list_plugin_commands","params":third})
        )["result"]["entries"][0]["manifest"]["id"],
        "example.quantities"
    );
    h.set_plugin_enabled("example.quantities", false).unwrap();
    assert_eq!(
        call(
            &mut ep,
            &mut h,
            &path,
            json!({"method":"list_plugin_commands","params":second})
        )["error"]["code"],
        "ARTIFACT_CHANGED"
    );
    let listing = json!({"target":target,"expected_revision":0,"offset":0,"limit":1});
    let index = call(
        &mut ep,
        &mut h,
        &path,
        json!({"method":"list_plugin_jobs","params":listing}),
    );
    assert_eq!(index["result"]["request_ids"], json!([id]));
    let result = r
        .join("plugin-jobs")
        .join(&target.project_id)
        .join(&id)
        .join("result.json");
    let original = std::fs::read(&result).unwrap();
    std::fs::remove_file(&result).unwrap();
    let p = params(&mut ep, &mut h, &path, &id);
    let (unknown, b) = read(&mut ep, &mut h, &path, p.clone());
    assert_eq!(unknown["phase"], "Unknown; inspect original job");
    assert!(b.result.is_none());
    assert!(!result.exists());
    std::fs::write(&result, b"broken result").unwrap();
    let bad = call(
        &mut ep,
        &mut h,
        &path,
        json!({"method":"read_plugin_job","params":p}),
    );
    assert_eq!(bad["error"]["code"], "JOB_ARTIFACT_UNKNOWN");
    assert_eq!(std::fs::read(&result).unwrap(), b"broken result");
    std::fs::write(result, original).unwrap();
    assert_eq!(
        h.jobs.as_ref().unwrap().ids(&target.project_id).unwrap(),
        vec![id]
    );
    assert!(h.document.editor.undo.is_empty());
}
#[test]
fn default_readonly_endpoint_recovers_actual_solar_record_and_marks_changed_model_stale() {
    let r = root();
    let mut h = host(&r);
    let v: Value =
        serde_json::from_str(include_str!("fixtures/solar/concept-scenario.json")).unwrap();
    let args =
        serde_json::from_value(json!({"request":v["request"],"assumed_north":v["assumed_north"]}))
            .unwrap();
    let id = storage::new_document_identity();
    h.start_solar_job(&args, &id).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        h.jobs.as_mut().unwrap().poll();
        if h.jobs
            .as_ref()
            .unwrap()
            .query(h.document.editor.project_identity(), &id)
            .unwrap()
            .result
            .is_some()
        {
            break;
        }
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(1));
    }
    let path = r.join("ipc/app.sock");
    let mut ep = Endpoint::bind_application(path.clone(), &mut h).unwrap();
    let mut p = params(&mut ep, &mut h, &path, &id);
    p["limit"] = 4096.into();
    let (current, b) = read(&mut ep, &mut h, &path, p);
    assert_eq!(current["input_status"], "current");
    assert_eq!(current["dispatch"], false);
    let atelier_bim::plugins::jobs::Terminal::Completed { report } =
        &b.result.as_ref().unwrap().terminal
    else {
        panic!("not completed")
    };
    assert_eq!(
        report.solar.as_ref().unwrap().points[0].sampled_sunlit_seconds,
        3600
    );
    let object = b.intent.input.elements[0].id;
    h.document
        .editor
        .apply(ModelCommand::MoveElements {
            ids: vec![object],
            offset: Point::new(0.2, 0.),
        })
        .unwrap();
    let mut p = params(&mut ep, &mut h, &path, &id);
    p["limit"] = 4096.into();
    let (stale, same) = read(&mut ep, &mut h, &path, p);
    assert_eq!(stale["input_status"], "stale");
    assert_eq!(same, b);
    assert_eq!(
        h.jobs
            .as_ref()
            .unwrap()
            .ids(h.document.editor.project_identity())
            .unwrap(),
        vec![id]
    );
}
