#![cfg(unix)]
use atelier_bim::{
    application_host::ApplicationHost,
    command::ModelCommand,
    model::{Material, Point, Project, Shape},
    plugins::{
        Manifest, Registry,
        jobs::{Bundle, Identity, Input, Jobs, Terminal},
        provider::ExecutableGrant,
    },
    program::{
        self, Area, AreaBasis, AreaUnit, Mapping, Program, ProgramSheet, Requirement, Values,
    },
    storage,
    zones::ZoneSpec,
};
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    process::Command,
    thread,
    time::{Duration, Instant},
};
fn root() -> PathBuf {
    let r =
        PathBuf::from("/tmp").join(format!("oa-pv-{}", &storage::new_document_identity()[..12]));
    std::fs::create_dir(&r).unwrap();
    r
}
fn manifest(program: bool, timeout: u64) -> Manifest {
    let mut v: Value = serde_json::from_slice(include_bytes!(
        "../assets/plugins/example-zone-program.json"
    ))
    .unwrap();
    v["provider"]["timeout_ms"] = json!(timeout);
    if !program {
        v["commands"][0]["operation"] = json!("authored_quantities");
        v["permissions"] = json!(["read_authored_geometry", "execute_local_program"]);
    }
    Manifest::parse(&serde_json::to_vec(&v).unwrap()).unwrap()
}
fn host(r: &Path) -> ApplicationHost {
    let mut p = Project::default();
    p.add(
        0,
        Material::Timber,
        Shape::Zone {
            spec: ZoneSpec {
                surface: Default::default(),
                purpose: "Meeting".into(),
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
                height: 3.,
                bottom_offset: 0.,
                label_position: None,
            },
        },
    );
    let mut h = ApplicationHost::new(p);
    h.configure_plugins(r).unwrap();
    h
}
fn wait(h: &mut ApplicationHost, id: &str) -> Bundle {
    let until = Instant::now() + Duration::from_secs(6);
    loop {
        let jobs = h.jobs.as_mut().unwrap();
        jobs.poll();
        let b = jobs
            .query(h.document.editor.project_identity(), id)
            .unwrap();
        if b.result.is_some() {
            return b;
        }
        assert!(
            Instant::now() < until,
            "original worker not terminal; never replay: {:?}",
            jobs.diagnostics
        );
        thread::sleep(Duration::from_millis(2));
    }
}
fn operation(h: &ApplicationHost, r: &Path, id: &str) -> PathBuf {
    r.join("plugin-jobs")
        .join(h.document.editor.project_identity())
        .join(id)
}
fn identity(h: &ApplicationHost, id: &str) -> Identity {
    Identity {
        project_id: h.document.editor.project_identity().into(),
        request_id: id.into(),
        plugin_id: "example.zone_program".into(),
        plugin_version: 1,
        command_id: "example.zone_program.review".into(),
        input_sha256: Input::capture(&h.document.editor)
            .unwrap()
            .fingerprint()
            .unwrap(),
    }
}
fn fixture(r: &Path, mode: &str, identity: &Identity) -> PathBuf {
    let artifact = json!({"schema_version":1,"identity":identity,"terminal":{"phase":"completed","report":{"definition":"Authored boundary minus holes; overlaps counted separately within each domain. Zone area and mass footprint are separate; not net/legal floor area or compliance.","zones":{"element_ids":[1],"area_m2":44.,"volume_m3":132.},"concept_masses":{"element_ids":[],"area_m2":0.,"volume_m3":0.}}}});
    let progress = json!({"type":"progress","identity":identity,"completed":0}).to_string();
    let result = json!({"type":"result","artifact":artifact}).to_string();
    let body = match mode {
        "hang" => "std::thread::sleep(std::time::Duration::from_secs(60));".into(),
        "inherited-pipe" => {
            "let _child=std::process::Command::new(\"/bin/sleep\").arg(\"5\").spawn().unwrap();"
                .into()
        }
        "crash" => "std::process::exit(17);".into(),
        "bad" => "println!(\"not JSON\");".into(),
        "flood" => "std::io::stdout().write_all(&vec![b'x';4*1024*1024+1]).unwrap();".into(),
        "backpressure" => format!("for _ in 0..1000{{println!(\"{{}}\",{progress:?});}}"),
        "delayed" => format!(
            "std::thread::sleep(std::time::Duration::from_millis(250));println!(\"{{}}\",{result:?});"
        ),
        "self-cancel" => {
            let v = json!({"type":"result","artifact":{"schema_version":1,"identity":identity,"terminal":{"phase":"cancelled"}}});
            format!("println!(\"{{}}\",{:?});", v.to_string())
        }
        "wrong" => {
            let mut v: Value = serde_json::from_str(&result).unwrap();
            v["artifact"]["identity"]["request_id"] = json!("0".repeat(64));
            format!("println!(\"{{}}\",{:?});", v.to_string())
        }
        _ => format!("println!(\"{{}}\",{result:?});"),
    };
    let source = r.join(format!("fixture-{mode}.rs"));
    let binary = r.join(format!("fixture-{mode}"));
    std::fs::write(&source,format!("use std::io::{{Read,Write}};fn main(){{let mut input=Vec::new();std::io::stdin().read_to_end(&mut input).unwrap();assert!(!input.is_empty());{body}}}")).unwrap();
    let result = Command::new(std::env::var_os("RUSTC").unwrap_or("rustc".into()))
        .args(["--edition=2024", "--crate-name", "provider_fixture"])
        .arg(&source)
        .arg("-o")
        .arg(&binary)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    binary
}
#[test]
fn exited_provider_with_inherited_pipe_is_bounded_and_keeps_actual_exit_status() {
    let r = root();
    let mut h = host(&r);
    let id = storage::new_document_identity();
    let bin = fixture(&r, "inherited-pipe", &identity(&h, &id));
    h.plugins
        .enable_reviewed_executable(manifest(false, 200), ExecutableGrant::review(&bin).unwrap())
        .unwrap();
    let before = h.document.editor.project.clone();
    let start = Instant::now();
    let original = h
        .start_plugin_job("example.zone_program.review", &id)
        .unwrap();
    let result = wait(&mut h, &id);
    assert!(
        start.elapsed() < Duration::from_secs(3),
        "inherited pipe must not defeat the original deadline"
    );
    assert!(matches!(
        result.result.unwrap().terminal,
        Terminal::Failed { .. }
    ));
    let capture = h
        .jobs
        .as_ref()
        .unwrap()
        .process_receipt(&original)
        .unwrap()
        .unwrap();
    assert!(capture.timed_out && !capture.cancelled);
    assert_eq!(
        capture.exit_code,
        Some(0),
        "original parent exited normally; deadline covered its inherited pipes"
    );
    assert_eq!(h.document.editor.project, before);
}
#[test]
fn explicit_executable_grant_changed_bytes_and_invalid_protocol_are_rejected() {
    let r = root();
    use std::os::unix::ffi::OsStrExt;
    let fifo = r.join("not-executable.fifo");
    let name = std::ffi::CString::new(fifo.as_os_str().as_bytes()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
    assert!(ExecutableGrant::review(&fifo).is_err());
    let mut h = host(&r);
    let id = storage::new_document_identity();
    let m = manifest(false, 1000);
    let bin = fixture(&r, "ok", &identity(&h, &id));
    assert!(h.plugins.enable(m.clone()).is_err());
    let grant = ExecutableGrant::review(&bin).unwrap();
    h.plugins.enable_reviewed_executable(m, grant).unwrap();
    std::fs::write(&bin, b"changed after explicit review").unwrap();
    h.start_plugin_job("example.zone_program.review", &id)
        .unwrap();
    let b = wait(&mut h, &id);
    assert!(
        matches!(&b.result.unwrap().terminal,Terminal::Failed{message} if message.contains("changed"))
    );
    assert!(!operation(&h, &r, &id).join("process.json").exists());
    assert!(!h.document.editor.dirty());
    let mut raw = serde_json::to_value(manifest(false, 1000)).unwrap();
    raw["provider"]["protocol_version"] = json!(2);
    assert!(Manifest::parse(&serde_json::to_vec(&raw).unwrap()).is_err());
}
#[test]
fn delayed_result_preserves_intervening_user_edit_and_provider_cancellation_is_distinct() {
    for mode in ["delayed", "self-cancel"] {
        let r = root();
        let mut h = host(&r);
        let id = storage::new_document_identity();
        let bin = fixture(&r, mode, &identity(&h, &id));
        h.plugins
            .enable_reviewed_executable(
                manifest(false, 2000),
                ExecutableGrant::review(&bin).unwrap(),
            )
            .unwrap();
        h.start_plugin_job("example.zone_program.review", &id)
            .unwrap();
        let path = operation(&h, &r, &id);
        let until = Instant::now() + Duration::from_secs(2);
        while !path.join("process.json").exists() {
            assert!(Instant::now() < until);
            thread::sleep(Duration::from_millis(1));
        }
        let eid = h.document.editor.project.elements[0].id;
        h.document
            .editor
            .apply(ModelCommand::MoveElements {
                ids: vec![eid],
                offset: Point::new(0.25, 0.),
            })
            .unwrap();
        let user = h.document.editor.project.clone();
        let b = wait(&mut h, &id);
        assert_eq!(h.document.editor.project, user);
        assert!(!b.current(&h.document.editor));
        assert!(if mode == "self-cancel" {
            matches!(b.result.as_ref().unwrap().terminal, Terminal::Cancelled)
        } else {
            matches!(
                b.result.as_ref().unwrap().terminal,
                Terminal::Completed { .. }
            )
        });
        h.document.editor.undo();
        assert!(b.current(&h.document.editor));
    }
}
#[test]
fn real_process_failures_and_bounded_output_preserve_original_identity_and_model() {
    for mode in ["crash", "bad", "flood", "backpressure", "wrong"] {
        let r = root();
        let mut h = host(&r);
        let id = storage::new_document_identity();
        let bin = fixture(&r, mode, &identity(&h, &id));
        h.plugins
            .enable_reviewed_executable(
                manifest(false, 2000),
                ExecutableGrant::review(&bin).unwrap(),
            )
            .unwrap();
        let before = h.document.editor.project.clone();
        let original = h
            .start_plugin_job("example.zone_program.review", &id)
            .unwrap();
        let b = wait(&mut h, &id);
        assert!(
            matches!(b.result.unwrap().terminal, Terminal::Failed { .. }),
            "{mode}"
        );
        assert_eq!(
            h.start_plugin_job("example.zone_program.review", &id)
                .unwrap(),
            original
        );
        let path = operation(&h, &r, &id);
        let process: Value =
            serde_json::from_slice(&std::fs::read(path.join("process-result.json")).unwrap())
                .unwrap();
        let capture = h
            .jobs
            .as_ref()
            .unwrap()
            .process_receipt(&original)
            .unwrap()
            .unwrap();
        assert_eq!(capture.identity, original);
        if mode == "bad" {
            let file = path.join("process-result.json");
            let saved = std::fs::read(&file).unwrap();
            let mut tampered = process.clone();
            tampered["identity"]["request_id"] = json!("0".repeat(64));
            let bad = serde_json::to_vec(&tampered).unwrap();
            std::fs::write(&file, &bad).unwrap();
            assert!(h.jobs.as_ref().unwrap().process_receipt(&original).is_err());
            assert_eq!(std::fs::read(&file).unwrap(), bad);
            std::fs::write(file, saved).unwrap();
        }
        let pid = process["pid"].as_u64().unwrap() as i32;
        assert_eq!(unsafe { libc::kill(pid, 0) }, -1);
        assert_eq!(h.document.editor.project, before);
        assert!(h.document.editor.undo.is_empty());
        assert_eq!(
            h.jobs.as_ref().unwrap().ids(&original.project_id).unwrap(),
            vec![id]
        );
    }
}
#[test]
fn original_owned_process_cancel_and_timeout_stop_without_restart_or_model_undo() {
    for cancel in [true, false] {
        let r = root();
        let mut h = host(&r);
        let id = storage::new_document_identity();
        let bin = fixture(&r, "hang", &identity(&h, &id));
        h.plugins
            .enable_reviewed_executable(
                manifest(false, if cancel { 3000 } else { 100 }),
                ExecutableGrant::review(&bin).unwrap(),
            )
            .unwrap();
        h.start_plugin_job("example.zone_program.review", &id)
            .unwrap();
        let path = operation(&h, &r, &id);
        let until = Instant::now() + Duration::from_secs(3);
        while !path.join("process.json").exists() {
            assert!(Instant::now() < until);
            thread::sleep(Duration::from_millis(1));
        }
        let observer = Jobs::new(r.join("plugin-jobs"));
        assert!(
            observer
                .query(h.document.editor.project_identity(), &id)
                .unwrap()
                .result
                .is_none()
        );
        assert!(
            observer
                .progress(h.document.editor.project_identity(), &id)
                .is_none()
        );
        if cancel {
            h.jobs
                .as_mut()
                .unwrap()
                .cancel(h.document.editor.project_identity(), &id)
                .unwrap();
        }
        let b = wait(&mut h, &id);
        assert!(if cancel {
            matches!(b.result.unwrap().terminal, Terminal::Cancelled)
        } else {
            matches!(b.result.unwrap().terminal,Terminal::Failed{message} if message.contains("timed out"))
        });
        let receipt: Value =
            serde_json::from_slice(&std::fs::read(path.join("process-result.json")).unwrap())
                .unwrap();
        assert_eq!(receipt["cancelled"], cancel);
        assert_eq!(receipt["timed_out"], !cancel);
        assert_eq!(
            unsafe { libc::kill(receipt["pid"].as_u64().unwrap() as i32, 0) },
            -1
        );
        assert!(h.document.editor.undo.is_empty());
    }
}
fn declared_program() -> Program {
    let values = |label: &str, area: Area, quantity| Values {
        label: label.into(),
        scope: "declared concept".into(),
        note: "not approved".into(),
        quantity,
        area,
    };
    let a = values("Meeting", Area::Known { m2: 20. }, 2);
    let b = values(
        "Kitchen",
        Area::Unknown {
            reason: "no approved target".into(),
        },
        1,
    );
    Program {
        sheets: vec![ProgramSheet {
            id: "declared".into(),
            name: "Declared only".into(),
            source_sha256: "a".repeat(64),
            mapping: Mapping {
                sheet: "Sheet1".into(),
                first_row: 2,
                last_row: 3,
                key_column: 1,
                label_column: 2,
                area_column: Some(3),
                quantity_column: Some(4),
                scope_column: Some(5),
                note_column: None,
                area_unit: AreaUnit::SquareMetre,
                area_basis: AreaBasis::RowTotal,
            },
            rows: vec![
                Requirement {
                    key: "R1".into(),
                    source_row: 2,
                    values: a.clone(),
                    imported: a,
                },
                Requirement {
                    key: "R2".into(),
                    source_row: 3,
                    values: b.clone(),
                    imported: b,
                },
            ],
            blank_rows: vec![],
        }],
    }
}
#[test]
#[ignore = "Requires separately offline-built original provider; run with OPENARCHI_TEST_PROVIDER and --ignored"]
fn standalone_provider_declared_requirements_unknowns_stale_undo_and_missing_provider_reopen() {
    let binary = PathBuf::from(
        std::env::var_os("OPENARCHI_TEST_PROVIDER").expect("explicit original provider required"),
    );
    let r = root();
    let mut h = host(&r);
    h.document.editor.project.program = declared_program();
    h.document.editor.project.program.validate().unwrap();
    h.plugins
        .enable_reviewed_executable(
            manifest(true, 3000),
            ExecutableGrant::review(&binary).unwrap(),
        )
        .unwrap();
    let before = h.document.editor.project.clone();
    let id = storage::new_document_identity();
    h.start_plugin_job("example.zone_program.review", &id)
        .unwrap();
    let b = wait(&mut h, &id);
    let report = match &b.result.as_ref().unwrap().terminal {
        Terminal::Completed { report } => report,
        other => panic!("{other:?}"),
    };
    let review = report.program_review.as_ref().unwrap();
    assert_eq!(review.rows.len(), 2);
    assert_eq!(review.rows[0].candidate_area_m2, 44.);
    assert_eq!(review.rows[0].delta_m2, Some(24.));
    assert_eq!(review.rows[0].declared_quantity, 2);
    assert_eq!(review.rows[0].mapping_status, "unverified_label_match");
    assert!(review.rows[1].delta_m2.is_none());
    assert_eq!(review.rows[1].mapping_status, "no_label_match");
    assert_eq!(review.unassigned_zone_ids, vec![1]);
    assert_eq!(h.document.editor.project, before);
    assert!(b.current(&h.document.editor));
    let expected = before.program.sheets[0].rows[0].values.clone();
    let mut values = expected.clone();
    values.area = Area::Known { m2: 24. };
    h.document
        .editor
        .apply(ModelCommand::Program {
            edit: program::ProgramEdit::UpdateRow {
                sheet_id: "declared".into(),
                key: "R1".into(),
                expected,
                values,
            },
        })
        .unwrap();
    assert!(!b.current(&h.document.editor));
    h.document.editor.undo();
    assert!(b.current(&h.document.editor));
    let saved = r.join("saved.json");
    storage::save_document(
        &h.document.editor.project,
        &saved,
        h.document.editor.project_identity(),
    )
    .unwrap();
    let mut next = ApplicationHost::new(Project::default());
    next.document.editor = atelier_bim::editor::Editor::from_stored_document(&saved).unwrap();
    next.configure_plugins(&r).unwrap();
    next.plugins = Registry::default();
    let original = next
        .jobs
        .as_ref()
        .unwrap()
        .query(next.document.editor.project_identity(), &id)
        .unwrap();
    assert_eq!(original, b);
    assert!(original.current(&next.document.editor));
    assert_eq!(next.document.editor.project, before);
}

#[test]
fn explicit_rereview_same_declaration_preserves_original_jobs_and_configuration_cas() {
    let r = root();
    let mut h = host(&r);
    let first = storage::new_document_identity();
    let m = manifest(false, 2000);
    let old = fixture(&r, "ok", &identity(&h, &first));
    h.plugins
        .enable_reviewed_executable(m.clone(), ExecutableGrant::review(&old).unwrap())
        .unwrap();
    h.start_plugin_job("example.zone_program.review", &first)
        .unwrap();
    let original = wait(&mut h, &first);
    assert!(matches!(
        original.result.as_ref().unwrap().terminal,
        Terminal::Completed { .. }
    ));
    let original_bytes = std::fs::read(operation(&h, &r, &first).join("intent.json")).unwrap();
    let mut stale = Registry::open(&r.join("plugins.json")).unwrap();
    let second = storage::new_document_identity();
    let v2 = r.join("v2");
    std::fs::create_dir(&v2).unwrap();
    let new = fixture(&v2, "ok", &identity(&h, &second));
    let grant = ExecutableGrant::review(&new).unwrap();
    h.plugins
        .enable_reviewed_executable(m.clone(), grant.clone())
        .expect(
            "explicit re-review must replace only the same declaration's local execution grant",
        );
    assert_eq!(h.plugins.entries().len(), 3);
    assert_eq!(h.plugins.executable(&m.id), Some(&grant));
    assert!(
        stale
            .enable_reviewed_executable(m.clone(), ExecutableGrant::review(&old).unwrap())
            .is_err()
    );
    let bytes = std::fs::read(r.join("plugins.json")).unwrap();
    let mut migration = m;
    migration.version = 2;
    assert!(
        h.plugins
            .enable_reviewed_executable(migration, grant)
            .is_err()
    );
    assert_eq!(std::fs::read(r.join("plugins.json")).unwrap(), bytes);
    h.start_plugin_job("example.zone_program.review", &second)
        .unwrap();
    assert!(matches!(
        wait(&mut h, &second).result.unwrap().terminal,
        Terminal::Completed { .. }
    ));
    assert_eq!(
        h.jobs
            .as_ref()
            .unwrap()
            .query(h.document.editor.project_identity(), &first)
            .unwrap(),
        original
    );
    assert_eq!(
        std::fs::read(operation(&h, &r, &first).join("intent.json")).unwrap(),
        original_bytes
    );
    assert!(h.document.editor.undo.is_empty());
}
