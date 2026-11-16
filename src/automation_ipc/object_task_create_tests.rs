// Included inside the default Endpoint object-task tests. Public authored fixtures only.
fn submit_creation(h: &mut ApplicationHost) -> (String, String) {
    let p = &h.document.editor.project;
    let target = p.design_process.task().unwrap().candidates[0].initial_mass_ids[0];
    let exchange = topics::capture(
        p,
        &[ObjectTarget::Element { id: target }],
        "Compare two new courtyard wings; retain this source and all existing alternatives".into(),
    )
    .unwrap();
    let study = crate::storage::new_document_identity();
    let req = crate::storage::new_document_identity();
    let command = ModelCommand::Evidence {
        edit: EvidenceEdit::SubmitTopicTask {
            id: crate::storage::new_document_identity(),
            exchange,
            expected_previous: None,
            request_id: req.clone(),
            mode: Mode::CreateTwoWing {
                task_id: study.clone(),
                story: 0,
                layer: 0,
                site_sha256: crate::automation_journal::canonical_hash(&p.site).unwrap(),
            },
        },
    };
    let change = h
        .document
        .editor
        .prepare_change(h.document.editor.revision, vec![command])
        .unwrap();
    let r = h.confirm_task_prepared(h.context(), &req, "Submit object task", Rc::new(change));
    assert_eq!(r["result"]["status"], "committed", "{r}");
    (req, study)
}
fn creation_params(
    e: &mut Endpoint,
    h: &mut ApplicationHost,
    socket: &std::path::Path,
    req: &str,
    study: &str,
) -> Value {
    let (target, c) = context(e, h, socket);
    let (_, _, exchange, _) = topic_tasks::find(&h.document.editor.project, req).unwrap();
    let a = pm::Parameters {
        origin_mm: [50000, 50000],
        court_width_mm: 24000,
        court_depth_mm: 16000,
        wing_depth_mm: 6000,
        storeys: 2,
        floor_height_mm: 3300,
    };
    let mut b = a.clone();
    b.wing_depth_mm = 7000;
    let definition = pm::Definition {
        rule_version: 1,
        candidates: [a, b],
        site_sha256: crate::automation_journal::canonical_hash(&h.document.editor.project.site)
            .unwrap(),
        sources: exchange.objects.iter().map(|o| o.link.clone()).collect(),
        assumptions: "Public authored concept assumptions; no survey, AI or compliance claim"
            .into(),
    };
    json!({"target":target,"expected_revision":c["revision"],"expected_context_revision":c["context_revision"],"expected_sources_revision":c["project_sources"]["sources_revision"],"task_id":study,"proposal":{"action":"create","name":"Object-bound new wings","question":exchange.question,"definition":definition,"story":0,"layer":0}})
}
fn create_preview(
    e: &mut Endpoint,
    h: &mut ApplicationHost,
    socket: &std::path::Path,
    req: &str,
    study: &str,
) -> Value {
    let params = creation_params(e, h, socket, req, study);
    rpc(
        e,
        h,
        socket,
        json!({"method":"prepare_parametric_proposal","params":params}),
    )
}
#[test]
fn object_task_first_create_default_endpoint_one_study_original_scope_and_reopen() {
    use crate::model::ModelAdapter;
    let (root, mut h, mut e) = setup();
    let socket = root.join("ipc/a.sock");
    let (req, study) = submit_creation(&mut h);
    let refused = create_preview(&mut e, &mut h, &socket, &req, &study);
    assert_eq!(
        refused["error"]["code"], "WRITE_NOT_AUTHORIZED",
        "{refused}"
    );
    claim(&mut e, &mut h, &socket, &req);
    let before = h.document.editor.project.clone();
    for variation in 0..3 {
        let mut p = creation_params(&mut e, &mut h, &socket, &req, &study);
        match variation {
            0 => p["task_id"] = json!(crate::storage::new_document_identity()),
            1 => p["proposal"]["question"] = json!("Retargeted question"),
            _ => p["proposal"]["definition"]["sources"] = json!([]),
        }
        let bad = rpc(
            &mut e,
            &mut h,
            &socket,
            json!({"method":"prepare_parametric_proposal","params":p}),
        );
        assert_eq!(bad["error"]["code"], "OBJECT_TASK_SCOPE_MISMATCH", "{bad}");
        assert_eq!(h.document.editor.project, before);
    }
    let preview = create_preview(&mut e, &mut h, &socket, &req, &study);
    assert!(preview["result"]["preview_id"].is_string(), "{preview}");
    let target = h.operation_target();
    let cancel = rpc(
        &mut e,
        &mut h,
        &socket,
        json!({"method":"cancel_operation","params":{"target":target,"preview_id":preview["result"]["preview_id"]}}),
    );
    assert!(cancel.get("error").is_none(), "{cancel}");
    assert_eq!(h.document.editor.project, before);
    let preview = create_preview(&mut e, &mut h, &socket, &req, &study);
    let result = commit(&mut e, &mut h, &socket, &preview, &study);
    assert_eq!(result["result"]["status"], "committed", "{result}");
    let after = h.document.editor.project.clone();
    assert_eq!(
        &after.elements[..before.elements.len()],
        before.elements.as_slice()
    );
    assert_eq!(after.elements.len(), before.elements.len() + 6);
    assert_eq!(
        after.design_process.tasks.len(),
        before.design_process.tasks.len() + 1
    );
    assert_eq!(
        after.design_process.current_task.as_deref(),
        Some(study.as_str())
    );
    let (t, c) = context(&mut e, &mut h, &socket);
    assert_eq!(
        c["write_authorization"]["object_task"]["status"], "authorized",
        "{c}"
    );
    assert_eq!(c["write_authorization"]["object_task"]["request_id"], req);
    let op = rpc(
        &mut e,
        &mut h,
        &socket,
        json!({"method":"get_operation","params":{"target":t,"original_target":target,"request_id":study}}),
    );
    assert_eq!(op["result"]["status"], "committed");
    let duplicate = create_preview(
        &mut e,
        &mut h,
        &socket,
        &req,
        &crate::storage::new_document_identity(),
    );
    assert_eq!(
        duplicate["error"]["code"], "OBJECT_TASK_CREATION_ALREADY_RECORDED",
        "{duplicate}"
    );
    let task = after.design_process.task().unwrap();
    let mut parameters = pm::current_parameters(&after, &study, task.candidates[0].option).unwrap();
    parameters.wing_depth_mm += 100;
    let forbidden = rpc(
        &mut e,
        &mut h,
        &socket,
        json!({"method":"prepare_parametric_proposal","params":{"target":t,"expected_revision":c["revision"],"expected_context_revision":c["context_revision"],"expected_sources_revision":c["project_sources"]["sources_revision"],"task_id":study,"proposal":{"action":"update","update_id":crate::storage::new_document_identity(),"instruction":"Creation is not permission to continue editing","parameters":parameters}}}),
    );
    assert_eq!(
        forbidden["error"]["code"], "OBJECT_TASK_CREATION_ALREADY_RECORDED",
        "{forbidden}"
    );
    assert_eq!(h.document.editor.project, after);
    let bytes = crate::storage::JsonAdapter.export(&after).unwrap();
    let mut raw: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(raw["schema_version"], 31);
    raw["schema_version"] = json!(30);
    assert!(
        crate::storage::JsonAdapter
            .import(&serde_json::to_vec(&raw).unwrap())
            .is_err()
    );
    let saved = root.join("created.openarchi.json");
    crate::storage::save_document(&after, &saved, h.document.editor.project_identity()).unwrap();
    drop(e);
    drop(h);
    let mut h = ApplicationHost::new(crate::model::Project::default());
    h.document.editor = crate::editor::Editor::from_stored_document(&saved).unwrap();
    h.configure_operation_journal(Some(root.join("journal")))
        .unwrap();
    assert_eq!(h.document.editor.project, after);
    let socket = root.join("reopen/a.sock");
    let mut e =
        Endpoint::bind_for_host(socket.clone(), false, Some(root.join("journal")), &mut h).unwrap();
    let (_, c) = context(&mut e, &mut h, &socket);
    assert_eq!(c["write_authorization"]["status"], "read_only");
    assert_eq!(
        create_preview(&mut e, &mut h, &socket, &req, &study)["error"]["code"],
        "WRITE_NOT_AUTHORIZED"
    );
    native_event(&mut h, &req, Event::Resume);
    assert_eq!(
        h.authorize_object_task(h.context(), &req)["result"]["status"],
        "authorized"
    );
    let (t, c) = context(&mut e, &mut h, &socket);
    let (topic, index, _, _) = topic_tasks::find(&h.document.editor.project, &req).unwrap();
    let reply=topics::AgentReply{id:crate::storage::new_document_identity(),agent_label:"Client fixture, not AI".into(),text:"The original study is saved with two editable rule candidates; previous work is unchanged.".into(),claims:vec![crate::design_process::intent::Claim{kind:crate::design_process::intent::ClaimKind::Observation,text:"Created model was read back; no code or capacity approval.".into()}]};
    let edit = EvidenceEdit::RecordTopicReply {
        id: topic.id.clone(),
        exchange_index: index,
        expected_topic_sha256: crate::automation_journal::canonical_hash(topic).unwrap(),
        reply: reply.clone(),
    };
    let p = rpc(
        &mut e,
        &mut h,
        &socket,
        json!({"method":"prepare_native_change","params":{"target":t,"expected_revision":c["revision"],"expected_context_revision":c["context_revision"],"change":{"version":1,"edits":[{"operation":"evidence","edit":edit}]}}}),
    );
    assert_eq!(
        commit(&mut e, &mut h, &socket, &p, &reply.id)["result"]["status"],
        "committed",
        "{p}"
    );
    let p = prepare_event(
        &mut e,
        &mut h,
        &socket,
        &req,
        Event::Complete {
            operation_ids: vec![study.clone(), reply.id.clone()],
        },
    );
    let done = commit(
        &mut e,
        &mut h,
        &socket,
        &p,
        &crate::storage::new_document_identity(),
    );
    assert_eq!(done["result"]["status"], "committed", "{done}");
    assert_eq!(
        topic_tasks::find(&h.document.editor.project, &req)
            .unwrap()
            .3
            .state()
            .unwrap()
            .0,
        topic_tasks::Phase::Completed
    );
    assert_eq!(h.document.editor.project.elements, after.elements);
    assert_ne!(h.object_task_authorization()["status"], "authorized");
}

#[test]
fn object_task_first_create_user_changes_never_gain_authority_from_fresh_context() {
    for created in [false, true] {
        let (root, mut h, mut e) = setup();
        let socket = root.join("ipc/a.sock");
        let (req, study) = submit_creation(&mut h);
        claim(&mut e, &mut h, &socket, &req);
        if created {
            let p = create_preview(&mut e, &mut h, &socket, &req, &study);
            assert_eq!(
                commit(&mut e, &mut h, &socket, &p, &study)["result"]["status"],
                "committed",
                "{p}"
            );
        }
        let id = h
            .document
            .editor
            .project
            .design_process
            .task()
            .unwrap()
            .candidates[0]
            .initial_mass_ids[0];
        h.document
            .editor
            .apply(ModelCommand::MoveElements {
                ids: vec![id],
                offset: crate::model::Point::new(0.1, 0.),
            })
            .unwrap();
        let protected = h.document.editor.project.clone();
        let refused = create_preview(
            &mut e,
            &mut h,
            &socket,
            &req,
            &crate::storage::new_document_identity(),
        );
        assert_eq!(
            refused["error"]["code"], "WRITE_AUTHORIZATION_SUSPENDED",
            "{refused}"
        );
        assert_eq!(
            h.authorize_object_task(h.context(), &req)["error"]["code"],
            "OBJECT_TASK_SCOPE_CHANGED"
        );
        assert_eq!(h.document.editor.project, protected);
        native_event(&mut h, &req, Event::Cancel);
        assert_eq!(h.document.editor.project.elements, protected.elements);
    }
}

#[test]
fn object_task_first_create_lost_result_recovers_original_without_second_geometry() {
    use crate::automation_change::{Commit, DurableStage};
    let (root, mut h, mut e) = setup();
    let socket = root.join("ipc/a.sock");
    let (req, study) = submit_creation(&mut h);
    claim(&mut e, &mut h, &socket, &req);
    let before = h.document.editor.project.clone();
    let preview = create_preview(&mut e, &mut h, &socket, &req, &study);
    assert!(preview["result"]["preview_id"].is_string(), "{preview}");
    let target = h.operation_target();
    let call =
        json!({"target":target,"preview_id":preview["result"]["preview_id"],"request_id":study});
    fs::write(
        root.join("original-create-fault.json"),
        serde_json::to_vec_pretty(&call).unwrap(),
    )
    .unwrap();
    h.operations.borrow_mut().set_durability_probe(|stage| {
        if stage == DurableStage::ModelCommitted {
            panic!("lost creation result after model commit")
        }
    });
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            h.operations.borrow_mut().commit(
                &mut h.document.editor,
                serde_json::from_value::<Commit>(call).unwrap(),
            )
        }))
        .is_err()
    );
    h.operations.borrow_mut().set_durability_probe(|_| {});
    let committed = h.document.editor.project.clone();
    assert_eq!(committed.elements.len(), before.elements.len() + 6);
    assert_eq!(
        h.authorize_object_task(h.context(), &req)["error"]["code"],
        "OBJECT_TASK_OPERATION_UNKNOWN"
    );
    let read = rpc(
        &mut e,
        &mut h,
        &socket,
        json!({"method":"get_operation","params":{"target":target,"original_target":target,"request_id":study}}),
    );
    assert!(read.get("result").is_some(), "{read}");
    let revision = h.document.editor.revision;
    let recovered = rpc(
        &mut e,
        &mut h,
        &socket,
        json!({"method":"recover_operation","params":{"target":target,"expected_revision":revision,"original_target":target,"request_id":study}}),
    );
    assert!(recovered.get("result").is_some(), "{recovered}");
    assert_eq!(h.document.editor.project, committed);
    assert_ne!(h.object_task_authorization()["status"], "authorized");
    native_event(&mut h, &req, Event::Resume);
    assert_eq!(
        h.authorize_object_task(h.context(), &req)["result"]["status"],
        "authorized"
    );
    let refused = create_preview(&mut e, &mut h, &socket, &req, &study);
    // The reserved identity is stopped earlier by the existing intent ledger.
    // This is stricter than reaching the task candidate scope with a new identity.
    assert_eq!(
        refused["error"]["code"], "INVALID_PARAMETRIC_PROPOSAL",
        "{refused}"
    );
    assert_eq!(
        refused["error"]["message"],
        "Intent request already recorded; inspect its original state"
    );
    let known = rpc(
        &mut e,
        &mut h,
        &socket,
        json!({"method":"get_operation","params":{"target":target,"original_target":target,"request_id":study}}),
    );
    assert_eq!(known["result"]["status"], "committed", "{known}");
    assert_eq!(
        h.document
            .editor
            .project
            .design_process
            .tasks
            .iter()
            .filter(|t| t.id == study)
            .count(),
        1
    );
    assert_eq!(h.document.editor.project.elements, committed.elements);
    native_event(&mut h, &req, Event::Cancel);
    assert_eq!(h.document.editor.project.elements, committed.elements);
}
