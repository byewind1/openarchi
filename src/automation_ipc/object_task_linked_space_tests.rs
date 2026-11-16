// Included in the existing default Endpoint harness; no alternate permission guard.
fn submit_linked_space(h: &mut ApplicationHost) -> (String, Mode) {
    let p = &h.document.editor.project;
    let mass = p.design_process.task().unwrap().candidates[0].initial_mass_ids[0];
    let exchange = topics::capture(
        p,
        &[ObjectTarget::Element { id: mass }],
        "Create one editable shared teaching concept space, declare assumptions, preserve all work"
            .into(),
    )
    .unwrap();
    let mode = Mode::linked_space(p, &exchange).unwrap();
    let request = crate::storage::new_document_identity();
    let command = ModelCommand::Evidence {
        edit: EvidenceEdit::SubmitTopicTask {
            id: crate::storage::new_document_identity(),
            exchange,
            expected_previous: None,
            request_id: request.clone(),
            mode: mode.clone(),
        },
    };
    let change = h
        .document
        .editor
        .prepare_change(h.document.editor.revision, vec![command])
        .unwrap();
    let v = h.confirm_task_prepared(h.context(), &request, "Submit object task", Rc::new(change));
    assert_eq!(v["result"]["status"], "committed", "{v}");
    (request, mode)
}
fn linked_space_preview(
    e: &mut Endpoint,
    h: &mut ApplicationHost,
    s: &std::path::Path,
    mode: &Mode,
) -> Value {
    let Mode::CreateLinkedSpace {
        mass,
        source_sha256,
        ..
    } = mode
    else {
        panic!()
    };
    let (t, c) = context(e, h, s);
    rpc(
        e,
        h,
        s,
        json!({"method":"prepare_native_change","params":{
        "target":t,"expected_revision":c["revision"],"expected_context_revision":c["context_revision"],
        "change":{"version":1,"edits":[{"operation":"mass_space","edit":{
            "action":"create","mass":mass,"expected_source_sha256":source_sha256,
            "name":"Shared teaching concept","purpose":"Teaching exchange; approval unknown",
            "height":3.,"assumption":"One concept level, assumed clear height; not surveyed or approved use"
        }}]}}}),
    )
}
fn linked_space_answer(
    e: &mut Endpoint,
    h: &mut ApplicationHost,
    s: &std::path::Path,
    request: &str,
) -> String {
    let (t, c) = context(e, h, s);
    let (topic, index, _, _) = topic_tasks::find(&h.document.editor.project, request).unwrap();
    let reply = topics::AgentReply {
        id: crate::storage::new_document_identity(),
        agent_label: "Client fixture, not AI".into(),
        text: "One source-bound concept space with assumed height; existing work remains protected. Approval and capacity unknown.".into(),
        claims: vec![crate::design_process::intent::Claim {
            kind: crate::design_process::intent::ClaimKind::DesignCompletion,
            text: "Three metre concept clear height, not measured.".into(),
        }],
    };
    let edit = EvidenceEdit::RecordTopicReply {
        id: topic.id.clone(),
        exchange_index: index,
        expected_topic_sha256: crate::automation_journal::canonical_hash(topic).unwrap(),
        reply: reply.clone(),
    };
    let preview = rpc(
        e,
        h,
        s,
        json!({"method":"prepare_native_change","params":{
        "target":t,"expected_revision":c["revision"],"expected_context_revision":c["context_revision"],
        "change":{"version":1,"edits":[{"operation":"evidence","edit":edit}]}}}),
    );
    assert_eq!(
        commit(e, h, s, &preview, &reply.id)["result"]["status"],
        "committed",
        "{preview}"
    );
    reply.id
}
#[test]
fn object_task_linked_space_default_endpoint_one_result_identity_and_reopen() {
    use crate::model::{ModelAdapter, Shape};
    let (root, mut h, mut e) = setup();
    let socket = root.join("ipc/a.sock");
    let (request, mode) = submit_linked_space(&mut h);
    let Mode::CreateLinkedSpace {
        mass,
        zone,
        operation_id,
        ..
    } = &mode
    else {
        panic!()
    };
    let refused = linked_space_preview(&mut e, &mut h, &socket, &mode);
    assert_eq!(refused["error"]["code"], "WRITE_NOT_AUTHORIZED");
    claim(&mut e, &mut h, &socket, &request);
    let reply_id = linked_space_answer(&mut e, &mut h, &socket, &request);
    let empty = prepare_event(
        &mut e,
        &mut h,
        &socket,
        &request,
        Event::Complete {
            operation_ids: vec![reply_id.clone()],
        },
    );
    assert_eq!(empty["error"]["code"], "OBJECT_TASK_RESULT_MISSING");
    let before = h.document.editor.project.clone();
    let preview = linked_space_preview(&mut e, &mut h, &socket, &mode);
    assert!(preview["result"]["preview_id"].is_string(), "{preview}");
    let (target, _) = context(&mut e, &mut h, &socket);
    let cancel = rpc(
        &mut e,
        &mut h,
        &socket,
        json!({"method":"cancel_operation","params":{"target":target,"preview_id":preview["result"]["preview_id"]}}),
    );
    assert!(cancel.get("error").is_none(), "{cancel}");
    assert_eq!(h.document.editor.project, before);
    let preview = linked_space_preview(&mut e, &mut h, &socket, &mode);
    let wrong = commit(
        &mut e,
        &mut h,
        &socket,
        &preview,
        &crate::storage::new_document_identity(),
    );
    assert_eq!(
        wrong["error"]["code"], "OBJECT_TASK_CREATION_REQUEST_ID_MISMATCH",
        "{wrong}"
    );
    assert_eq!(h.document.editor.project, before);
    let created = commit(&mut e, &mut h, &socket, &preview, operation_id);
    assert_eq!(created["result"]["status"], "committed", "{created}");
    let after = h.document.editor.project.clone();
    assert_eq!(after.elements.len(), before.elements.len() + 1);
    for old in &before.elements {
        assert_eq!(after.elements.iter().find(|e| e.id == old.id), Some(old));
    }
    assert_eq!(after.site, before.site);
    assert_eq!(after.program, before.program);
    assert_eq!(after.design_process.tasks, before.design_process.tasks);
    let z = after.elements.iter().find(|e| e.id == *zone).unwrap();
    assert_eq!(z.name, "Shared teaching concept");
    assert!(
        matches!(&z.shape,Shape::Zone{spec} if (spec.area()-156.).abs()<1e-8 && spec.height==3. && spec.purpose=="Teaching exchange; approval unknown")
    );
    let link = after.design_process.mass_spaces.last().unwrap();
    assert_eq!(link.mass, *mass);
    assert_eq!(link.zone, *zone);
    let original = rpc(
        &mut e,
        &mut h,
        &socket,
        json!({"method":"get_operation","params":{"target":target,"original_target":target,"request_id":operation_id}}),
    );
    assert_eq!(original["result"]["status"], "committed");
    let duplicate = linked_space_preview(&mut e, &mut h, &socket, &mode);
    assert_eq!(
        duplicate["error"]["code"],
        "OBJECT_TASK_CREATION_ALREADY_RECORDED"
    );
    let mut raw: Value =
        serde_json::from_slice(&crate::storage::JsonAdapter.export(&after).unwrap()).unwrap();
    assert_eq!(raw["schema_version"], 33);
    for old_version in [30, 31, 32] {
        raw["schema_version"] = json!(old_version);
        assert!(
            crate::storage::JsonAdapter
                .import(&serde_json::to_vec(&raw).unwrap())
                .is_err()
        );
    }
    let saved = root.join("linked-space.openarchi.json");
    crate::storage::save_document(&after, &saved, h.document.editor.project_identity()).unwrap();
    drop(e);
    drop(h);
    let mut h = ApplicationHost::new(crate::model::Project::default());
    h.document.editor = crate::editor::Editor::from_stored_document(&saved).unwrap();
    h.configure_operation_journal(Some(root.join("journal")))
        .unwrap();
    assert_eq!(h.document.editor.project, after);
    let socket = root.join("reopened/a.sock");
    let mut e =
        Endpoint::bind_for_host(socket.clone(), false, Some(root.join("journal")), &mut h).unwrap();
    let (_, c) = context(&mut e, &mut h, &socket);
    assert_eq!(c["write_authorization"]["status"], "read_only");
    native_event(&mut h, &request, Event::Resume);
    assert_eq!(
        h.authorize_object_task(h.context(), &request)["result"]["status"],
        "authorized"
    );
    let duplicate = linked_space_preview(&mut e, &mut h, &socket, &mode);
    assert_eq!(
        duplicate["error"]["code"],
        "OBJECT_TASK_CREATION_ALREADY_RECORDED"
    );
    let done = prepare_event(
        &mut e,
        &mut h,
        &socket,
        &request,
        Event::Complete {
            operation_ids: vec![operation_id.clone(), reply_id],
        },
    );
    let v = commit(
        &mut e,
        &mut h,
        &socket,
        &done,
        &crate::storage::new_document_identity(),
    );
    assert_eq!(v["result"]["status"], "committed", "{v}");
    assert_eq!(h.document.editor.project.elements, after.elements);
    assert_ne!(h.object_task_authorization()["status"], "authorized");
}

#[test]
fn object_task_linked_space_keeps_old_modes_narrow_and_stops_after_user_insertion() {
    let (root, mut h, mut e) = setup();
    let socket = root.join("ipc/a.sock");
    let (request, mode) = submit_linked_space(&mut h);
    claim(&mut e, &mut h, &socket, &request);
    let mut other = mode.clone();
    if let Mode::CreateLinkedSpace {
        mass,
        source_sha256,
        ..
    } = &mut other
    {
        *mass = h
            .document
            .editor
            .project
            .design_process
            .task()
            .unwrap()
            .candidates[0]
            .initial_mass_ids[1];
        *source_sha256 = crate::mass_spaces::element_stamp(
            &h.document.editor.project,
            h.document
                .editor
                .project
                .elements
                .iter()
                .find(|e| e.id == *mass)
                .unwrap(),
        )
        .unwrap();
    }
    let wrong = linked_space_preview(&mut e, &mut h, &socket, &other);
    assert_eq!(
        wrong["error"]["code"], "OBJECT_TASK_SCOPE_MISMATCH",
        "{wrong}"
    );
    let preview = linked_space_preview(&mut e, &mut h, &socket, &mode);
    assert!(preview["result"]["preview_id"].is_string());
    let mut site = h.document.editor.project.site.clone();
    site.control_tolerance_m = Some(0.02);
    h.document
        .editor
        .apply(ModelCommand::SetSiteContext { context: site })
        .unwrap();
    let (_, fresh) = context(&mut e, &mut h, &socket);
    assert_eq!(fresh["write_authorization"]["status"], "suspended");
    e.set_write_enabled(true, &h.document.editor);
    let refused = linked_space_preview(&mut e, &mut h, &socket, &mode);
    assert_eq!(
        refused["error"]["code"], "OBJECT_TASK_AUTHORIZATION_SUSPENDED",
        "{refused}"
    );
    let Mode::CreateLinkedSpace { operation_id, .. } = &mode else {
        panic!()
    };
    let v = commit(&mut e, &mut h, &socket, &preview, operation_id);
    assert!(v.get("error").is_some());
    assert!(
        h.document
            .editor
            .project
            .design_process
            .mass_spaces
            .is_empty()
    );
    native_event(&mut h, &request, Event::Cancel);
    let old = submit(&mut h, true);
    claim(&mut e, &mut h, &socket, &old);
    let refuse = linked_space_preview(&mut e, &mut h, &socket, &mode);
    assert_eq!(
        refuse["error"]["code"], "OBJECT_TASK_SCOPE_MISMATCH",
        "{refuse}"
    );
}

#[test]
fn object_task_linked_space_undo_does_not_renew_creation_and_redo_keeps_owner() {
    let (root, mut h, mut e) = setup();
    let socket = root.join("ipc/a.sock");
    let (request, mode) = submit_linked_space(&mut h);
    claim(&mut e, &mut h, &socket, &request);
    let preview = linked_space_preview(&mut e, &mut h, &socket, &mode);
    let Mode::CreateLinkedSpace {
        operation_id, zone, ..
    } = &mode
    else {
        panic!()
    };
    let created = commit(&mut e, &mut h, &socket, &preview, operation_id);
    assert_eq!(created["result"]["status"], "committed", "{created}");
    let after = h.document.editor.project.clone();
    h.document.editor.undo();
    let (target, c) = context(&mut e, &mut h, &socket);
    assert_eq!(c["write_authorization"]["status"], "suspended");
    assert!(
        !h.document
            .editor
            .project
            .elements
            .iter()
            .any(|e| e.id == *zone)
    );
    let v = h.authorize_object_task(h.context(), &request);
    assert_eq!(
        v["error"]["code"], "OBJECT_TASK_CREATION_ALREADY_RECORDED",
        "{v}"
    );
    let operation = rpc(
        &mut e,
        &mut h,
        &socket,
        json!({"method":"get_operation","params":{
        "target":target,"original_target":target,"request_id":operation_id}}),
    );
    assert_eq!(operation["result"]["status"], "committed");
    h.document.editor.redo();
    assert_eq!(h.document.editor.project, after);
    native_event(&mut h, &request, Event::Resume);
    let v = h.authorize_object_task(h.context(), &request);
    assert_eq!(v["result"]["status"], "authorized", "{v}");
    let repeat = linked_space_preview(&mut e, &mut h, &socket, &mode);
    assert_eq!(
        repeat["error"]["code"], "OBJECT_TASK_CREATION_ALREADY_RECORDED",
        "{repeat}"
    );
    assert_eq!(h.document.editor.project.elements, after.elements);
}
