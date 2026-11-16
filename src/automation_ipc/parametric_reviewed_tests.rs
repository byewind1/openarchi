#[test]
fn reviewed_inputs_default_endpoint_requires_explicit_authority_and_exact_snapshot() {
    use crate::design_process::{Entry, parametric as pm};
    let root = PathBuf::from("/tmp").join(format!(
        "oa-review-api-{}",
        &crate::storage::new_document_identity()[..12]
    ));
    fs::create_dir(&root).unwrap();
    let mut host = ApplicationHost::new(crate::model::Project::demo());
    host.configure_operation_journal(Some(root.join("journal")))
        .unwrap();
    let a = Parameters {
        origin_mm: [30000, 30000],
        court_width_mm: 20000,
        court_depth_mm: 14000,
        wing_depth_mm: 6000,
        storeys: 2,
        floor_height_mm: 3300,
    };
    let mut b = a.clone();
    b.wing_depth_mm = 7000;
    let task = crate::storage::new_document_identity();
    let rule = Definition {
        rule_version: 1,
        candidates: [a.clone(), b],
        site_sha256: crate::automation_journal::canonical_hash(&host.document.editor.project.site)
            .unwrap(),
        sources: vec![],
        assumptions: "Fixture declared site, measurement and approval unknown; not AI execution"
            .into(),
    };
    let change = pm::prepare(
        &host.document.editor,
        &task,
        "Same rule".into(),
        "Review one declared condition".into(),
        rule,
        0,
        0,
    )
    .unwrap();
    let created = host.confirm_task_prepared(
        host.context(),
        &task,
        "Native fixture create",
        std::rc::Rc::new(change),
    );
    assert_eq!(created["result"]["status"], "committed");
    let socket = root.join("ipc/app.sock");
    let mut endpoint =
        Endpoint::bind_for_host(socket.clone(), true, Some(root.join("journal")), &mut host)
            .unwrap();
    let target = request(
        &mut endpoint,
        &mut host,
        &socket,
        json!({"method":"get_target"}),
    )["result"]["target"]
        .clone();
    let mut site = host.document.editor.project.site.clone();
    site.control_tolerance_m = Some(0.02);
    host.document
        .editor
        .apply(ModelCommand::SetSiteContext { context: site })
        .unwrap();
    let current = request(
        &mut endpoint,
        &mut host,
        &socket,
        json!({"method":"get_context","params":target}),
    )["result"]
        .clone();
    assert_eq!(current["write_authorization"]["status"], "suspended");
    assert_eq!(
        current["parametric_candidates"][0]["input_status"],
        "changed_or_missing"
    );
    let inputs = current["parametric_candidates"][0]["current_input_snapshot"].clone();
    assert!(inputs.is_object());
    let mut next = a.clone();
    next.wing_depth_mm = 6400;
    let update = crate::storage::new_document_identity();
    let params = json!({"target":target,"expected_revision":current["revision"],"expected_context_revision":current["context_revision"],"expected_sources_revision":crate::automation::sources::context(&host.document.editor,None)["sources_revision"],"task_id":task,"proposal":{"action":"update_reviewed_inputs","update_id":update,"instruction":"Use exactly reviewed declared site; preserve B and original work","parameters":next,"inputs":inputs}});
    let unchanged = host.document.editor.project.clone();
    let blocked = request(
        &mut endpoint,
        &mut host,
        &socket,
        json!({"method":"prepare_parametric_proposal","params":params}),
    );
    assert_eq!(
        blocked["error"]["code"], "WRITE_AUTHORIZATION_SUSPENDED",
        "{blocked}"
    );
    assert_eq!(host.document.editor.project, unchanged);
    // An explicit legacy native permission event; reading a new revision above did not grant it.
    // This test does not claim that a global toggle renews an object-task grant.
    endpoint.set_write_enabled(true, &host.document.editor);
    let prepared = request(
        &mut endpoint,
        &mut host,
        &socket,
        json!({"method":"prepare_parametric_proposal","params":params}),
    );
    assert!(prepared["result"]["preview_id"].is_string(), "{prepared}");
    assert_eq!(host.document.editor.project, unchanged);
    let cancel = request(
        &mut endpoint,
        &mut host,
        &socket,
        json!({"method":"cancel_operation","params":{"target":target,"preview_id":prepared["result"]["preview_id"]}}),
    );
    assert!(cancel.get("error").is_none(), "{cancel}");
    assert_eq!(host.document.editor.project, unchanged);
    let prepared = request(
        &mut endpoint,
        &mut host,
        &socket,
        json!({"method":"prepare_parametric_proposal","params":params}),
    );
    assert!(prepared["result"]["preview_id"].is_string(), "{prepared}");
    let commit = json!({"method":"commit_change","params":{"target":target,"preview_id":prepared["result"]["preview_id"],"request_id":update}});
    fs::write(
        root.join("original-reviewed-commit.json"),
        serde_json::to_vec_pretty(&commit).unwrap(),
    )
    .unwrap();
    let committed = request(&mut endpoint, &mut host, &socket, commit);
    assert_eq!(committed["result"]["status"], "committed", "{committed}");
    let option = host
        .document
        .editor
        .project
        .plan_options
        .active_design_option
        .unwrap();
    assert_eq!(
        pm::current_parameters(&host.document.editor.project, &task, option).unwrap(),
        next
    );
    assert_eq!(
        host.document
            .editor
            .project
            .design_process
            .updates
            .last()
            .unwrap()
            .entry,
        Entry::AgentProposal
    );
    let status = request(
        &mut endpoint,
        &mut host,
        &socket,
        json!({"method":"get_operation","params":{"target":target,"request_id":update,"original_target":target}}),
    );
    assert_eq!(status["result"]["status"], "committed", "{status}");
    assert_eq!(
        pm::readback(&host.document.editor.project)[0]["input_status"],
        "current"
    );
    assert_eq!(
        pm::readback(&host.document.editor.project)[1]["input_status"],
        "changed_or_missing"
    );
    let bytes = crate::storage::JsonAdapter
        .export(&host.document.editor.project)
        .unwrap();
    assert_eq!(
        serde_json::from_slice::<Value>(&bytes).unwrap()["schema_version"],
        32
    );
    let reopened = crate::storage::JsonAdapter.import(&bytes).unwrap();
    assert_eq!(
        pm::current_parameters(&reopened, &task, option).unwrap(),
        next
    );
}
