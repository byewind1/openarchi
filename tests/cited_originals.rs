use atelier_bim::{
    automation::{ReadService, RulesRequest, RulesSection},
    command::ModelCommand,
    constraints::{self, Basis, Check, Definition, Domain, Edit, Metric, Relation, Scope, State},
    editor::Editor,
    project_repository::Repository,
    rules::*,
    storage,
};
use serde_json::{Value, json};
use std::{fs, path::PathBuf};

fn fixture() -> (PathBuf, Editor, Repository) {
    fixture_original(false)
}
fn fixture_original(large: bool) -> (PathBuf, Editor, Repository) {
    fixture_editor(Editor::new(storage::startup_example()), large)
}
fn fixture_editor(mut editor: Editor, large: bool) -> (PathBuf, Editor, Repository) {
    let root = std::env::temp_dir().join(format!(
        "oa-cited-original-{}",
        storage::new_document_identity()
    ));
    fs::create_dir(&root).unwrap();
    let mut repo = Repository::create_for_editor(&root.join("repository"), &editor).unwrap();
    let file = root.join("developer-test.txt");
    let mut original =
        "Developer test parameter-height limit100m, not an actual regulation.\n".to_owned();
    if large {
        original.push_str(
            &format!("{}\n", "Controlled software fixture padding. ".repeat(7)).repeat(4096),
        );
    }
    fs::write(&file, original).unwrap();
    let sha = repo.add_source(&file).unwrap();
    editor.project.rules.context = ProjectContext {
        jurisdiction: "Synthetic developer fixture".into(),
        uses: "Software test only".into(),
        assessment_date: "2026-11-12".into(),
        stage: "Not a real approval".into(),
    };
    editor.project.rules.sources.push(RuleSource {
        id: "developer-original".into(),
        title: "Controlled original".into(),
        code: "NOT-A-CODE".into(),
        edition: "test1".into(),
        publisher: "Regression fixture".into(),
        jurisdiction: "Synthetic developer fixture".into(),
        published_on: "2020-01-01".into(),
        effective_on: "2020-01-01".into(),
        original_sha256: sha,
        origin_url: String::new(),
        origin: Origin::Unverified,
        verification_note: "No real regulatory authority".into(),
        replaces: vec![],
        same_edition_as: None,
    });
    editor.project.rules.requirements.push(Requirement {
        id: "developer-height".into(),
        title: "Controlled height fixture".into(),
        topic: Topic::AreaMassing,
        citation: Citation {
            source_id: "developer-original".into(),
            locator: "text/line[1]".into(),
            text: "Developer test parameter-height limit100m, not an actual regulation.".into(),
        },
        interpretation: "Synthetic numerical fixture".into(),
        decision: Decision {
            applicability: Applicability::Applicable,
            adoption: Adoption::Adopted,
            rationale: "Only the controlled software test, no project approval".into(),
            missing: String::new(),
            basis_sha256: editor.project.rules.basis("developer-original").unwrap(),
        },
        links: vec![],
    });
    let definition = Definition {
        id: "cited-test".into(),
        title: "Cited parameter-height fixture".into(),
        domain: Domain::ConceptMasses,
        scope: Scope::Objects { ids: vec![133] },
        metric: Metric::Height,
        relation: Relation::AtMost,
        limit: Some(100.),
        basis: Basis::CitedRequirement {
            id: "developer-height".into(),
        },
    };
    let fresh = Check {
        definition: definition.clone(),
        report: None,
    };
    let commands = vec![
        ModelCommand::Constraint {
            edit: Edit::Put {
                expected_sha256: None,
                definition,
            },
        },
        ModelCommand::Constraint {
            edit: Edit::Run {
                id: "cited-test".into(),
                expected_sha256: constraints::expected(&fresh),
            },
        },
    ];
    let admission = atelier_bim::project_repository::constraints::guard_cited_runs(
        &repo,
        editor.project_identity(),
        &editor.project.rules,
        &commands,
    )
    .unwrap();
    let prepared = editor
        .prepare_constraint_change(0, commands, &admission)
        .unwrap();
    editor
        .commit_change("controlled-cited-check", &prepared)
        .unwrap();
    drop(admission);
    (root, editor, repo)
}
fn snapshot(editor: &Editor) -> Value {
    let read = ReadService::new("original-query-test".into());
    read.project_rules(
        editor,
        &RulesRequest {
            target: read.target(editor),
            expected_revision: editor.revision,
            section: RulesSection::Checks,
            offset: 6,
            limit: 1,
        },
    )
    .unwrap()
}
fn plan(editor: &Editor, repo: Option<&Repository>) -> constraints::originals::ReadPlan {
    constraints::originals::ReadPlan::new(
        editor.project_identity(),
        repo,
        &editor.project.rules,
        editor
            .project
            .rules
            .checks
            .iter()
            .filter(|c| c.definition.id == "cited-test"),
    )
}
fn observe(editor: &Editor, repo: Option<&Repository>) -> Value {
    let plan = plan(editor, repo);
    let observation = std::thread::spawn(move || plan.execute()).join().unwrap();
    let mut value = snapshot(editor);
    observation.apply_to_entries(value["entries"].as_array_mut().unwrap());
    value
}

#[test]
fn cited_runs_require_live_exact_admission_and_previews_never_hold_the_repository_lock() {
    use atelier_bim::project_repository::constraints::guard_cited_runs;
    let (root, mut editor, mut repo) = fixture();
    let baseline = editor.project.clone();
    let c = editor.project.rules.checks.last().unwrap();
    let commands = vec![ModelCommand::Constraint {
        edit: Edit::Run {
            id: c.definition.id.clone(),
            expected_sha256: constraints::expected(c),
        },
    }];
    let error = editor
        .prepare_change(editor.revision, commands.clone())
        .err()
        .unwrap();
    assert_eq!(error.code, "CITED_ORIGINAL_ADMISSION_REQUIRED");
    let admission = guard_cited_runs(
        &repo,
        editor.project_identity(),
        &editor.project.rules,
        &commands,
    )
    .unwrap();
    let prepared = editor
        .prepare_constraint_change(editor.revision, commands.clone(), &admission)
        .unwrap();
    let mut other = commands.clone();
    if let ModelCommand::Constraint {
        edit: Edit::Run {
            expected_sha256, ..
        },
    } = &mut other[0]
    {
        *expected_sha256 = "0".repeat(64);
    }
    assert_eq!(
        editor
            .prepare_constraint_change(editor.revision, other, &admission)
            .err()
            .unwrap()
            .code,
        "CITED_ORIGINAL_ADMISSION_MISMATCH"
    );
    drop(admission);
    assert_eq!(
        editor
            .commit_change("expired-preview", &prepared)
            .unwrap_err()
            .code,
        "CITED_ORIGINAL_RECHECK_REQUIRED"
    );
    assert_eq!(editor.project, baseline);
    assert!(editor.change_receipt("expired-preview").is_none());
    // A displayed preview must not block a quality update. Fresh admission now
    // sees the rejection and cannot reuse its old successful observation.
    let source = repo.manifest.sources[0].id.clone();
    repo.reject_source(&source, "Developer test rejection after preview")
        .unwrap();
    assert!(
        guard_cited_runs(
            &repo,
            editor.project_identity(),
            &editor.project.rules,
            &commands
        )
        .is_err()
    );
    // Definition-only edits preserve the existing report without requiring IO.
    let old = editor.project.rules.checks.last().unwrap().clone();
    let mut definition = old.definition.clone();
    definition.title = "Definition edited with original unavailable".into();
    let definition_only = editor
        .prepare_change(
            editor.revision,
            vec![ModelCommand::Constraint {
                edit: Edit::Put {
                    expected_sha256: Some(constraints::expected(&old)),
                    definition,
                },
            }],
        )
        .unwrap();
    editor
        .commit_change("definition-only", &definition_only)
        .unwrap();
    assert_eq!(
        editor.project.rules.checks.last().unwrap().report,
        old.report
    );
    assert_eq!(
        editor
            .project
            .rules
            .checks
            .last()
            .unwrap()
            .state(&editor.project),
        State::Unknown
    );
    assert_eq!(editor.project.elements, baseline.elements);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn native_cited_confirmation_rechecks_before_intent_and_keeps_original_operation_identity() {
    use atelier_bim::{
        application_host::ApplicationHost, project_repository::constraints::guard_cited_runs,
    };
    let (root, editor, repo) = fixture();
    let mut host = ApplicationHost::new(editor.project.clone());
    host.document.editor = editor;
    host.configure_operation_journal(Some(root.join("native-journal")))
        .unwrap();
    let base = host.context();
    let before = host.document.editor.project.clone();
    let old = before.rules.checks.last().unwrap();
    let commands = vec![ModelCommand::Constraint {
        edit: Edit::Run {
            id: old.definition.id.clone(),
            expected_sha256: constraints::expected(old),
        },
    }];
    let admission = guard_cited_runs(
        &repo,
        host.document.editor.project_identity(),
        &before.rules,
        &commands,
    )
    .unwrap();
    let preview = host
        .document
        .editor
        .prepare_constraint_change(base.revision, commands.clone(), &admission)
        .unwrap();
    drop(admission);
    let refused = host.confirm_task_prepared(
        base,
        "original-cited-request",
        "Test cited run",
        std::rc::Rc::new(preview),
    );
    assert_eq!(
        refused["error"]["code"], "CITED_ORIGINAL_RECHECK_REQUIRED",
        "{refused}"
    );
    assert_eq!(refused["operation_submitted"], false);
    assert_eq!(host.document.editor.project, before);
    let admission = guard_cited_runs(
        &repo,
        host.document.editor.project_identity(),
        &before.rules,
        &commands,
    )
    .unwrap();
    let fresh = host
        .document
        .editor
        .prepare_constraint_change(base.revision, commands, &admission)
        .unwrap();
    let fresh = std::rc::Rc::new(fresh);
    let target = host.operation_target();
    let committed = host.confirm_task_prepared(
        base,
        "original-cited-request",
        "Test cited run",
        fresh.clone(),
    );
    assert_eq!(committed["result"]["status"], "committed", "{committed}");
    drop(admission);
    assert_eq!(
        host.inspect_operation(target, "original-cited-request")["result"]["status"],
        "committed"
    );
    // Receipt recovery precedes source revalidation, and never reapplies.
    let revision = host.document.editor.revision;
    let receipt = host
        .document
        .editor
        .commit_change("original-cited-request", &fresh)
        .unwrap();
    assert_eq!(receipt.request_id, "original-cited-request");
    assert_eq!(host.document.editor.revision, revision);
    assert_eq!(host.document.editor.project.elements, before.elements);
    fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
fn endpoint_request(
    ep: &mut atelier_bim::automation_ipc::Endpoint,
    host: &mut atelier_bim::application_host::ApplicationHost,
    repository: Option<&Repository>,
    socket: &std::path::Path,
    message: Value,
    mut during_read: Option<&mut dyn FnMut(&mut atelier_bim::application_host::ApplicationHost)>,
) -> Value {
    use std::{
        io::{BufRead, BufReader, Write},
        os::unix::net::UnixStream,
        thread,
        time::Duration,
    };
    let path = socket.to_owned();
    let worker = thread::spawn(move || {
        let mut stream = UnixStream::connect(path).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(6)))
            .unwrap();
        writeln!(stream, "{message}").unwrap();
        let mut line = String::new();
        BufReader::new(stream).read_line(&mut line).unwrap();
        serde_json::from_str::<Value>(&line).unwrap()
    });
    for _ in 0..600 {
        ep.poll_with_host(host, repository);
        if ep.source_reader_busy()
            && let Some(callback) = during_read.take()
        {
            callback(host);
        }
        if worker.is_finished() {
            assert!(
                during_read.is_none(),
                "IO boundary was not observed; cannot claim insertion during source verification"
            );
            return worker.join().unwrap();
        }
        thread::sleep(Duration::from_millis(5));
    }
    panic!("Original request did not settle; do not replay")
}

#[test]
#[cfg(unix)]
fn default_application_endpoint_rechecks_cited_runs_and_preserves_original_identity_and_user_edits()
{
    const CHILD: &str = "OPENARCHI_CITED_EXECUTION_CHILD";
    if std::env::var_os(CHILD).is_none() {
        let config =
            std::env::temp_dir().join(format!("oa-ce-config-{}", storage::new_document_identity()));
        fs::create_dir(&config).unwrap();
        let output=std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact","default_application_endpoint_rechecks_cited_runs_and_preserves_original_identity_and_user_edits","--nocapture"])
            .env(CHILD,"1").env("OPENARCHI_AUTOMATION_WRITE","1").env("OPENARCHI_AUTOMATION_JOURNAL_DIR",config.join("journal")).output().unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        fs::remove_dir_all(config).unwrap();
        return;
    }
    use atelier_bim::{
        application_host::{ApplicationHost, HostCommand},
        automation_ipc::Endpoint,
    };
    let destination = std::env::var_os("OPENARCHI_CITED_ENDPOINT_OUTPUT").map(PathBuf::from);
    if let Some(output) = std::env::var_os("OPENARCHI_CITED_SDK_FIXTURE").map(PathBuf::from) {
        fs::create_dir(&output)
            .expect("Keep previous fixture outcomes; choose a new output directory");
        let input = std::env::var_os("OPENARCHI_CITED_SDK_INPUT").map(PathBuf::from);
        let editor = input
            .as_ref()
            .map(|p| Editor::from_stored_document(p).unwrap())
            .unwrap_or_else(|| Editor::new(storage::startup_example()));
        let (root, editor, repository) = fixture_editor(editor, false);
        let stored = output.join("controlled.openarchi.json");
        storage::save_document(&editor.project, &stored, editor.project_identity()).unwrap();
        repository.copy_to(&output.join("repository")).unwrap();
        fs::write(output.join("fixture.json"),serde_json::to_vec_pretty(&json!({"input":input,"file":"controlled.openarchi.json","repository":"repository","check_id":"cited-test","check_offset":editor.project.rules.checks.len()-1,"original_sha256":repository.manifest.sources[0].id,"model_elements":editor.project.elements.len(),"developer_fixture_only":true,"authority":"not a regulation, approval or survey","optional_task_guard":false})).unwrap()).unwrap();
        fs::remove_dir_all(root).unwrap();
    }
    let mut evidence = vec![];
    for method in [
        "prepare_change",
        "prepare_native_change",
        "prepare_discovered_change",
    ] {
        let (root, editor, mut repo) = fixture();
        let mut host = ApplicationHost::new(editor.project.clone());
        host.document.editor = editor;
        let before = host.document.editor.project.clone();
        let revision = host.document.editor.revision;
        let socket = PathBuf::from("/tmp")
            .join(format!("oa-ce-{}", &storage::new_document_identity()[..12]))
            .join("app.sock");
        let mut ep = Endpoint::bind_application(socket.clone(), &mut host).unwrap();
        if method == "prepare_change" {
            let forged = endpoint_request(
                &mut ep,
                &mut host,
                Some(&repo),
                &socket,
                json!({"method":"CitedReady","params":{}}),
                None,
            );
            assert_eq!(
                forged["error"]["code"], "INVALID_REQUEST",
                "internal source admissions must not be wire inputs: {forged}"
            );
            assert_eq!(host.document.editor.project, before);
        }
        let target = endpoint_request(
            &mut ep,
            &mut host,
            Some(&repo),
            &socket,
            json!({"method":"get_target"}),
            None,
        )["result"]["target"]
            .clone();
        let context = endpoint_request(
            &mut ep,
            &mut host,
            Some(&repo),
            &socket,
            json!({"method":"get_context","params":target}),
            None,
        );
        assert!(
            context["result"]["permissions"]
                .as_array()
                .is_some_and(|p| p.contains(&json!("write"))),
            "{context}"
        );
        let old = before.rules.checks.last().unwrap();
        let change = json!({"version":1,"creates":[],"edits":[{"operation":"constraint","edit":{"action":"run","id":old.definition.id,"expected_sha256":constraints::expected(old)}}]});
        let params = if method == "prepare_change" {
            json!({"target":target,"expected_revision":revision,"change":change})
        } else {
            json!({"target":target,"expected_revision":revision,"expected_context_revision":context["result"]["context_revision"],"change":change})
        };
        let unavailable = endpoint_request(
            &mut ep,
            &mut host,
            None,
            &socket,
            json!({"method":method,"params":params}),
            None,
        );
        assert_eq!(
            unavailable["error"]["code"], "CITED_ORIGINAL_UNAVAILABLE",
            "{method}: {unavailable}"
        );
        assert_eq!(unavailable["operation_submitted"], false);
        let preview = endpoint_request(
            &mut ep,
            &mut host,
            Some(&repo),
            &socket,
            json!({"method":method,"params":params}),
            None,
        );
        let preview_id = preview["result"]["preview_id"]
            .as_str()
            .expect(&format!("{method}: {preview}"));
        let commit = json!({"method":"commit_change","params":{"target":target,"preview_id":preview_id,"request_id":"original-cited-request"}});
        let blob = repo.root().join(&repo.manifest.sources[0].blob.path);
        let bytes = fs::read(&blob).unwrap();
        fs::write(&blob, b"Changed after source-backed preview").unwrap();
        let failed = endpoint_request(
            &mut ep,
            &mut host,
            Some(&repo),
            &socket,
            commit.clone(),
            None,
        );
        assert_eq!(
            failed["error"]["code"], "CITED_ORIGINAL_UNAVAILABLE",
            "{failed}"
        );
        assert_eq!(failed["operation_submitted"], false);
        assert_eq!(host.document.editor.project, before);
        assert_eq!(host.document.editor.revision, revision);
        let lookup = json!({"method":"get_operation","params":{"target":target,"request_id":"original-cited-request"}});
        let not_submitted = endpoint_request(
            &mut ep,
            &mut host,
            Some(&repo),
            &socket,
            lookup.clone(),
            None,
        );
        assert_eq!(
            not_submitted["error"]["code"], "OPERATION_NOT_FOUND",
            "{not_submitted}"
        );
        fs::write(&blob, &bytes).unwrap();
        let committed = endpoint_request(
            &mut ep,
            &mut host,
            Some(&repo),
            &socket,
            commit.clone(),
            None,
        );
        assert_eq!(
            committed["result"]["status"], "committed",
            "{method}: {committed}"
        );
        let after = host.document.editor.project.clone();
        assert_eq!(after.elements, before.elements);
        assert_eq!(
            after
                .rules
                .checks
                .last()
                .unwrap()
                .report
                .as_ref()
                .unwrap()
                .generation,
            old.report.as_ref().unwrap().generation + 1
        );
        let owned_revision = host.document.editor.revision;
        // Lost-response recovery is a query of the original request, not a new
        // execution; it remains available even if the original later disappears.
        fs::write(&blob, b"Unavailable after known committed result").unwrap();
        let recovered = endpoint_request(&mut ep, &mut host, None, &socket, commit, None);
        assert_eq!(recovered["result"]["status"], "committed", "{recovered}");
        assert_eq!(host.document.editor.revision, owned_revision);
        fs::write(&blob, &bytes).unwrap();
        let context = endpoint_request(
            &mut ep,
            &mut host,
            Some(&repo),
            &socket,
            json!({"method":"get_context","params":target}),
            None,
        );
        let current = after.rules.checks.last().unwrap();
        let mut second = params.clone();
        second["expected_revision"] = json!(owned_revision);
        if method != "prepare_change" {
            second["expected_context_revision"] = context["result"]["context_revision"].clone();
        }
        second["change"]["edits"][0]["edit"]["expected_sha256"] =
            json!(constraints::expected(current));
        let preview2 = endpoint_request(
            &mut ep,
            &mut host,
            Some(&repo),
            &socket,
            json!({"method":method,"params":second}),
            None,
        );
        let second_id = preview2["result"]["preview_id"]
            .as_str()
            .expect(&format!("{preview2}"));
        // Previews do not hold repository locks. Rejection can be recorded, and
        // the original server preview cannot reuse its former source proof.
        let sid = repo.manifest.sources[0].id.clone();
        repo.reject_source(&sid, "Controlled rejection after preview")
            .unwrap();
        let rejected = endpoint_request(
            &mut ep,
            &mut host,
            Some(&repo),
            &socket,
            json!({"method":"commit_change","params":{"target":target,"preview_id":second_id,"request_id":"rejected-cited-request"}}),
            None,
        );
        assert_eq!(
            rejected["error"]["code"], "CITED_ORIGINAL_UNAVAILABLE",
            "{rejected}"
        );
        assert_eq!(host.document.editor.project, after);
        let cancelled = endpoint_request(
            &mut ep,
            &mut host,
            Some(&repo),
            &socket,
            json!({"method":"cancel_operation","params":{"target":target,"preview_id":second_id}}),
            None,
        );
        assert!(cancelled["result"].is_object(), "{cancelled}");
        let cancelled_commit = endpoint_request(
            &mut ep,
            &mut host,
            Some(&repo),
            &socket,
            json!({"method":"commit_change","params":{"target":target,"preview_id":second_id,"request_id":"rejected-cited-request"}}),
            None,
        );
        assert_eq!(
            cancelled_commit["error"]["code"], "PREVIEW_NOT_FOUND",
            "{cancelled_commit}"
        );
        evidence.push(json!({"entry":method,"target":target,"preview":preview,"changed_original_refused":failed,"original_request_commit":committed,"same_request_recovery":recovered,"rejected_original_refused":rejected,"cancelled_preview_refused":cancelled_commit,"geometry_unchanged":true,"optional_task_guard":false}));
        drop(ep);
        fs::remove_dir(socket.parent().unwrap()).unwrap();
        fs::remove_dir_all(root).unwrap();
    }
    // Observe a real source worker boundary before inserting a native user edit.
    let (root, editor, repo) = fixture_original(true);
    let mut host = ApplicationHost::new(editor.project.clone());
    host.document.editor = editor;
    let before = host.document.editor.project.clone();
    let revision = host.document.editor.revision;
    let socket = PathBuf::from("/tmp")
        .join(format!("oa-ce-{}", &storage::new_document_identity()[..12]))
        .join("app.sock");
    let mut ep = Endpoint::bind_application(socket.clone(), &mut host).unwrap();
    let target = endpoint_request(
        &mut ep,
        &mut host,
        Some(&repo),
        &socket,
        json!({"method":"get_target"}),
        None,
    )["result"]["target"]
        .clone();
    let old = before.rules.checks.last().unwrap();
    let change = json!({"version":1,"creates":[],"edits":[{"operation":"constraint","edit":{"action":"run","id":old.definition.id,"expected_sha256":constraints::expected(old)}}]});
    let params = json!({"target":target,"expected_revision":revision,"change":change});
    let mut inserted = false;
    let mut insert = |host: &mut ApplicationHost| {
        let mut site = host.document.editor.project.site.clone();
        site.control_tolerance_m = Some(0.03);
        host.dispatch(
            host.context(),
            HostCommand::Apply {
                request_id: "native-user-insertion".into(),
                commands: vec![ModelCommand::SetSiteContext { context: site }],
            },
        )
        .unwrap();
        inserted = true;
    };
    let stopped = endpoint_request(
        &mut ep,
        &mut host,
        Some(&repo),
        &socket,
        json!({"method":"prepare_change","params":params}),
        Some(&mut insert),
    );
    assert!(inserted);
    assert_eq!(stopped["error"]["code"], "CONTEXT_CONFLICT", "{stopped}");
    assert_eq!(host.document.editor.revision, revision + 1);
    assert_eq!(
        host.document.editor.project.site.control_tolerance_m,
        Some(0.03)
    );
    assert_eq!(host.document.editor.project.rules, before.rules);
    let context = endpoint_request(
        &mut ep,
        &mut host,
        Some(&repo),
        &socket,
        json!({"method":"get_context","params":target}),
        None,
    );
    assert_eq!(context["result"]["permissions"], json!(["read"]));
    let mut fresh = params;
    fresh["expected_revision"] = json!(host.document.editor.revision);
    let still_stopped = endpoint_request(
        &mut ep,
        &mut host,
        Some(&repo),
        &socket,
        json!({"method":"prepare_change","params":fresh}),
        None,
    );
    assert_eq!(
        still_stopped["error"]["code"], "WRITE_AUTHORIZATION_SUSPENDED",
        "{still_stopped}"
    );
    evidence.push(json!({"entry":"default raw Endpoint with native user insertion during observed source IO","stopped":stopped,"new_revision_read_does_not_renew":still_stopped,"source_worker_observed":true,"optional_task_guard":false}));
    drop(ep);
    fs::remove_dir(socket.parent().unwrap()).unwrap();
    fs::remove_dir_all(root).unwrap();
    if let Some(output) = destination {
        fs::create_dir_all(&output).unwrap();
        let path = output.join("default-endpoint.json");
        assert!(!path.exists());
        fs::write(path, serde_json::to_vec_pretty(&evidence).unwrap()).unwrap();
    }
}

#[test]
fn exact_original_is_required_and_failed_reads_never_destroy_the_numerical_snapshot() {
    let (root, editor, mut repo) = fixture();
    let baseline = editor.project.clone();
    let missing = observe(&editor, None);
    assert_eq!(missing["entries"][0]["state"], "unknown");
    assert_eq!(missing["entries"][0]["numerical_state"], "pass");
    let healthy = observe(&editor, Some(&repo));
    assert_eq!(healthy["entries"][0]["state"], "pass");
    assert_eq!(
        healthy["entries"][0]["original_verification"]["status"],
        "verified"
    );
    assert_eq!(healthy["compliance_verdict"], "not_evaluated");
    let blob = repo.root().join(&repo.manifest.sources[0].blob.path);
    let bytes = fs::read(&blob).unwrap();
    // Even if a malformed in-memory envelope supplies a matching replacement
    // blob checksum and the quoted first line is unchanged, it cannot claim
    // the old immutable original identity.
    use sha2::{Digest, Sha256};
    let mut dishonest = repo.clone();
    let mut replacement = bytes.clone();
    replacement.extend_from_slice(b"Changed trailing content, not the recorded original.\n");
    fs::write(&blob, &replacement).unwrap();
    dishonest.manifest.sources[0].blob.sha256 = format!("{:x}", Sha256::digest(&replacement));
    dishonest.manifest.sources[0].blob.bytes = replacement.len() as u64;
    let mismatched = observe(&editor, Some(&dishonest));
    assert_eq!(mismatched["entries"][0]["state"], "unknown");
    assert!(
        mismatched["entries"][0]["original_verification"]["reason"]
            .as_str()
            .unwrap()
            .contains("identity")
    );
    fs::write(&blob, &bytes).unwrap();
    let frozen = plan(&editor, Some(&repo));
    fs::write(&blob, b"changed original").unwrap();
    let mut changed = snapshot(&editor);
    frozen
        .execute()
        .apply_to_entries(changed["entries"].as_array_mut().unwrap());
    assert_eq!(changed["entries"][0]["state"], "unknown");
    assert!(
        changed["entries"][0]["original_verification"]["reason"]
            .as_str()
            .unwrap()
            .contains("changed")
    );
    fs::remove_file(&blob).unwrap();
    assert_eq!(
        observe(&editor, Some(&repo))["entries"][0]["state"],
        "unknown"
    );
    fs::write(&blob, &bytes).unwrap();
    assert_eq!(observe(&editor, Some(&repo))["entries"][0]["state"], "pass");
    let old_repo = repo.clone();
    let source_id = repo.manifest.sources[0].id.clone();
    repo.reject_source(&source_id, "Controlled source rejection")
        .unwrap();
    assert_eq!(
        observe(&editor, Some(&old_repo))["entries"][0]["state"],
        "unknown"
    );
    let rejected = observe(&editor, Some(&repo));
    assert_eq!(rejected["entries"][0]["state"], "unknown");
    assert!(
        rejected["entries"][0]["original_verification"]["reason"]
            .as_str()
            .unwrap()
            .contains("Rejected")
    );
    assert_eq!(editor.project, baseline);
    assert_eq!(editor.project.elements.len(), 130);
    assert_eq!(
        editor
            .project
            .rules
            .checks
            .last()
            .unwrap()
            .report
            .as_ref()
            .unwrap()
            .rows[0]
            .state,
        State::Pass
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn wrong_document_quote_and_late_observation_cannot_certify_a_different_check() {
    let (root, mut editor, repo) = fixture();
    let (other_root, other, _) = fixture();
    let wrong = constraints::originals::ReadPlan::new(
        other.project_identity(),
        Some(&repo),
        &editor.project.rules,
        editor
            .project
            .rules
            .checks
            .iter()
            .filter(|c| c.definition.id == "cited-test"),
    )
    .execute();
    let mut view = snapshot(&editor);
    wrong.apply_to_entries(view["entries"].as_array_mut().unwrap());
    assert_eq!(view["entries"][0]["state"], "unknown");
    let old = plan(&editor, Some(&repo)).execute();
    editor
        .project
        .rules
        .requirements
        .last_mut()
        .unwrap()
        .citation
        .text = "Invented replacement citation".into();
    let mut view = snapshot(&editor);
    old.apply_to_entries(view["entries"].as_array_mut().unwrap());
    assert_eq!(view["entries"][0]["state"], "unknown");
    assert_eq!(
        view["entries"][0]["original_verification"]["status"],
        "not_checked"
    );
    let result = observe(&editor, Some(&repo));
    assert_eq!(
        result["entries"][0]["original_verification"]["status"],
        "unavailable"
    );
    fs::remove_dir_all(root).unwrap();
    fs::remove_dir_all(other_root).unwrap();
}

#[cfg(unix)]
#[test]
fn shared_native_endpoint_queries_current_originals_on_the_background_reader() {
    if std::env::var_os("OPENARCHI_CITED_ENDPOINT_CHILD").is_none() {
        let config = std::env::temp_dir().join(format!(
            "oa-cited-config-{}",
            storage::new_document_identity()
        ));
        fs::create_dir(&config).unwrap();
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "shared_native_endpoint_queries_current_originals_on_the_background_reader",
                "--nocapture",
            ])
            .env("OPENARCHI_CITED_ENDPOINT_CHILD", "1")
            .env("OPENARCHI_AUTOMATION_JOURNAL_DIR", config.join("journal"))
            .env("OPENARCHI_AUTOMATION_WRITE", "0")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        fs::remove_dir_all(config).unwrap();
        return;
    }
    use std::{
        io::{BufRead, BufReader, Write},
        os::unix::net::UnixStream,
        thread,
        time::Duration,
    };
    let (root, editor, mut repo) = fixture();
    let mut host = atelier_bim::application_host::ApplicationHost::new(editor.project.clone());
    host.document.editor = editor;
    let socket = PathBuf::from("/tmp")
        .join(format!("oa-cq-{}", &storage::new_document_identity()[..12]))
        .join("app.sock");
    let mut endpoint =
        atelier_bim::automation_ipc::Endpoint::bind_application(socket.clone(), &mut host).unwrap();
    fn request(
        endpoint: &mut atelier_bim::automation_ipc::Endpoint,
        host: &mut atelier_bim::application_host::ApplicationHost,
        repo: Option<&Repository>,
        socket: &std::path::Path,
        method: &str,
        params: Value,
    ) -> Value {
        let path = socket.to_owned();
        let method = method.to_owned();
        let worker = thread::spawn(move || {
            let mut stream = UnixStream::connect(path).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(6)))
                .unwrap();
            let request = if method == "get_target" {
                json!({"method":method})
            } else {
                json!({"method":method,"params":params})
            };
            writeln!(stream, "{}", request).unwrap();
            let mut line = String::new();
            BufReader::new(stream).read_line(&mut line).unwrap();
            serde_json::from_str::<Value>(&line).unwrap()
        });
        for _ in 0..600 {
            endpoint.poll_with_host(host, repo);
            if worker.is_finished() {
                return worker.join().unwrap();
            }
            thread::sleep(Duration::from_millis(5));
        }
        panic!("original query still running; do not replay")
    }
    let target = request(
        &mut endpoint,
        &mut host,
        Some(&repo),
        &socket,
        "get_target",
        json!({}),
    )["result"]["target"]
        .clone();
    let params = json!({"target":target,"expected_revision":host.document.editor.revision,"section":"checks","offset":6,"limit":1});
    let healthy = request(
        &mut endpoint,
        &mut host,
        Some(&repo),
        &socket,
        "query_project_rules",
        params.clone(),
    );
    assert_eq!(
        healthy["result"]["entries"][0]["state"], "pass",
        "{healthy}"
    );
    let unavailable = request(
        &mut endpoint,
        &mut host,
        None,
        &socket,
        "query_project_rules",
        params.clone(),
    );
    assert_eq!(unavailable["result"]["entries"][0]["state"], "unknown");
    let id = repo.manifest.sources[0].id.clone();
    repo.reject_source(&id, "Controlled rejection").unwrap();
    let rejected = request(
        &mut endpoint,
        &mut host,
        Some(&repo),
        &socket,
        "query_project_rules",
        params,
    );
    assert_eq!(rejected["result"]["entries"][0]["state"], "unknown");
    let ctx = request(
        &mut endpoint,
        &mut host,
        Some(&repo),
        &socket,
        "get_context",
        target,
    );
    assert_eq!(ctx["result"]["permissions"], json!(["read"]));
    drop(endpoint);
    if socket.parent().unwrap().exists() {
        fs::remove_dir(socket.parent().unwrap()).unwrap();
    }
    fs::remove_dir_all(root).unwrap();
}
