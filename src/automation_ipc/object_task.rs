#[cfg(test)]
mod object_task_tests {
    use super::*;
    use crate::{
        application_host::ApplicationHost,
        command::ModelCommand,
        design_process::parametric as pm,
        evidence::{
            EvidenceEdit, Target as ObjectTarget,
            topic_tasks::{self, Event, Mode},
            topics,
        },
    };
    use std::{
        io::{BufRead, BufReader},
        os::unix::net::UnixStream,
        rc::Rc,
        time::Instant,
    };
    fn rpc(e: &mut Endpoint, h: &mut ApplicationHost, s: &std::path::Path, v: Value) -> Value {
        let s = s.to_owned();
        let c = thread::spawn(move || {
            let mut stream = UnixStream::connect(s).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            writeln!(stream, "{v}").unwrap();
            let mut line = String::new();
            BufReader::new(stream).read_line(&mut line).unwrap();
            serde_json::from_str(&line).unwrap()
        });
        let start = Instant::now();
        while !c.is_finished() {
            e.poll_with_host(h, None);
            assert!(
                start.elapsed() < Duration::from_secs(5),
                "unknown response: inspect the original request"
            );
            thread::sleep(Duration::from_millis(1));
        }
        c.join().unwrap()
    }
    fn setup() -> (std::path::PathBuf, ApplicationHost, Endpoint) {
        let root = PathBuf::from("/tmp").join(format!(
            "oa-task-{}",
            &crate::storage::new_document_identity()[..12]
        ));
        fs::create_dir(&root).unwrap();
        let mut h = ApplicationHost::new(crate::model::Project::default());
        h.configure_operation_journal(Some(root.join("journal")))
            .unwrap();
        let a = pm::Parameters {
            origin_mm: [30000, 30000],
            court_width_mm: 20000,
            court_depth_mm: 14000,
            wing_depth_mm: 6000,
            storeys: 2,
            floor_height_mm: 3300,
        };
        let mut b = a.clone();
        b.court_width_mm = 18000;
        let def = pm::Definition {
            rule_version: 1,
            candidates: [a, b],
            site_sha256: crate::automation_journal::canonical_hash(&h.document.editor.project.site)
                .unwrap(),
            sources: vec![],
            assumptions: "Public concept fixture; not real AI or measured facts".into(),
        };
        let id = crate::storage::new_document_identity();
        let prepared = pm::prepare(
            &h.document.editor,
            &id,
            "Existing wings".into(),
            "Preserve both alternatives".into(),
            def,
            0,
            0,
        )
        .unwrap();
        assert_eq!(
            h.confirm_task_prepared(h.context(), &id, "Fixture source", Rc::new(prepared))["result"]
                ["status"],
            "committed"
        );
        let e = Endpoint::bind_for_host(
            root.join("ipc/a.sock"),
            false,
            Some(root.join("journal")),
            &mut h,
        )
        .unwrap();
        (root, h, e)
    }
    fn submit(h: &mut ApplicationHost, allow_model: bool) -> String {
        let p = &h.document.editor.project;
        let t = p.design_process.task().unwrap();
        let option = p.plan_options.active_design_option.unwrap();
        let id = t.candidates[0].initial_mass_ids[0];
        let mode = if allow_model {
            Mode::TwoWing {
                task_id: t.id.clone(),
                option,
            }
        } else {
            Mode::Research
        };
        let exchange = topics::capture(
            p,
            &[ObjectTarget::Element { id }],
            "Make a limited shallower study; preserve B and unknowns".into(),
        )
        .unwrap();
        let request = crate::storage::new_document_identity();
        let command = ModelCommand::Evidence {
            edit: EvidenceEdit::SubmitTopicTask {
                id: crate::storage::new_document_identity(),
                exchange,
                expected_previous: None,
                request_id: request.clone(),
                mode,
            },
        };
        let c = h
            .document
            .editor
            .prepare_change(h.document.editor.revision, vec![command])
            .unwrap();
        let r = h.confirm_task_prepared(h.context(), &request, "Submit object task", Rc::new(c));
        assert_eq!(r["result"]["status"], "committed", "{r}");
        request
    }
    fn native_event(h: &mut ApplicationHost, id: &str, event: Event) {
        let (t, index, _, _) = topic_tasks::find(&h.document.editor.project, id).unwrap();
        let cmd = ModelCommand::Evidence {
            edit: EvidenceEdit::TopicTaskEvent {
                id: t.id.clone(),
                exchange_index: index,
                expected_topic_sha256: crate::automation_journal::canonical_hash(t).unwrap(),
                request_id: id.into(),
                event,
            },
        };
        let c = h
            .document
            .editor
            .prepare_change(h.document.editor.revision, vec![cmd])
            .unwrap();
        let r = h.confirm_task_prepared(
            h.context(),
            &crate::storage::new_document_identity(),
            "Control original object task",
            Rc::new(c),
        );
        assert_eq!(r["result"]["status"], "committed", "{r}");
    }
    fn context(e: &mut Endpoint, h: &mut ApplicationHost, s: &std::path::Path) -> (Value, Value) {
        let t = rpc(e, h, s, json!({"method":"get_target"}))["result"]["target"].clone();
        let c = rpc(e, h, s, json!({"method":"get_context","params":t}))["result"].clone();
        (t, c)
    }
    fn prepare_event(
        e: &mut Endpoint,
        h: &mut ApplicationHost,
        s: &std::path::Path,
        id: &str,
        event: Event,
    ) -> Value {
        let (t, c) = context(e, h, s);
        let (topic, index, _, _) = topic_tasks::find(&h.document.editor.project, id).unwrap();
        let edit = EvidenceEdit::TopicTaskEvent {
            id: topic.id.clone(),
            exchange_index: index,
            expected_topic_sha256: crate::automation_journal::canonical_hash(topic).unwrap(),
            request_id: id.into(),
            event,
        };
        rpc(
            e,
            h,
            s,
            json!({"method":"prepare_native_change","params":{"target":t,"expected_revision":c["revision"],"expected_context_revision":c["context_revision"],"change":{"version":1,"edits":[{"operation":"evidence","edit":edit}]}}}),
        )
    }
    fn commit(
        e: &mut Endpoint,
        h: &mut ApplicationHost,
        s: &std::path::Path,
        preview: &Value,
        id: &str,
    ) -> Value {
        let target = rpc(e, h, s, json!({"method":"get_target"}))["result"]["target"].clone();
        let call = json!({"method":"commit_change","params":{"target":target,"preview_id":preview["result"]["preview_id"],"request_id":id}});
        fs::write(
            s.parent()
                .unwrap()
                .parent()
                .unwrap()
                .join(format!("{id}-original.json")),
            serde_json::to_vec_pretty(&call).unwrap(),
        )
        .unwrap();
        rpc(e, h, s, call)
    }
    #[test]
    fn default_object_task_scope_claim_parametric_pause_and_native_resume() {
        let (root, mut h, mut e) = setup();
        let s = root.join("ipc/a.sock");
        let req = submit(&mut h, true);
        let initial = h.document.editor.project.clone();
        // Even an existing whole-project grant cannot implicitly adopt a saved task.
        e.set_write_enabled(true, &h.document.editor);
        let refused = prepare_event(
            &mut e,
            &mut h,
            &s,
            &req,
            Event::Claim {
                agent_label: "Client fixture, not AI".into(),
            },
        );
        assert_eq!(
            refused["error"]["code"], "OBJECT_TASK_NATIVE_AUTHORITY_REQUIRED",
            "{refused}"
        );
        assert_eq!(h.document.editor.project, initial);
        assert_eq!(
            h.authorize_object_task(h.context(), &req)["result"]["status"],
            "authorized"
        );
        let claim = prepare_event(
            &mut e,
            &mut h,
            &s,
            &req,
            Event::Claim {
                agent_label: "Client fixture, not AI".into(),
            },
        );
        assert!(claim["result"]["preview_id"].is_string(), "{claim}");
        assert_eq!(
            commit(
                &mut e,
                &mut h,
                &s,
                &claim,
                &crate::storage::new_document_identity()
            )["result"]["status"],
            "committed"
        );
        let (t, c) = context(&mut e, &mut h, &s);
        let task = h
            .document
            .editor
            .project
            .design_process
            .task()
            .unwrap()
            .clone();
        let option = h
            .document
            .editor
            .project
            .plan_options
            .active_design_option
            .unwrap();
        let id = task.candidates[0].initial_mass_ids[0];
        let bad = rpc(
            &mut e,
            &mut h,
            &s,
            json!({"method":"prepare_native_change","params":{"target":t,"expected_revision":c["revision"],"expected_context_revision":c["context_revision"],"change":{"version":1,"edits":[{"operation":"move","targets":[{"kind":"existing","value":id}],"offset":{"x":0.1,"y":0.0}}]}}}),
        );
        assert_eq!(bad["error"]["code"], "OBJECT_TASK_SCOPE_MISMATCH", "{bad}");
        let file = rpc(
            &mut e,
            &mut h,
            &s,
            json!({"method":"save_project","params":{"target":t,"expected_revision":c["revision"],"filename":"not-task-authorized.json"}}),
        );
        assert_eq!(
            file["error"]["code"], "OBJECT_TASK_SCOPE_MISMATCH",
            "{file}"
        );
        assert!(e.files.is_empty());
        let mut parameters =
            pm::current_parameters(&h.document.editor.project, &task.id, option).unwrap();
        parameters.wing_depth_mm = 5500;
        let update = crate::storage::new_document_identity();
        let prepared = rpc(
            &mut e,
            &mut h,
            &s,
            json!({"method":"prepare_parametric_proposal","params":{"target":t,"expected_revision":c["revision"],"expected_context_revision":c["context_revision"],"expected_sources_revision":c["project_sources"]["sources_revision"],"task_id":task.id,"proposal":{"action":"update","update_id":update,"instruction":"Fixture shallow parameter update, unknown compliance","parameters":parameters}}}),
        );
        assert!(prepared["result"]["preview_id"].is_string(), "{prepared}");
        assert_eq!(
            commit(&mut e, &mut h, &s, &prepared, &update)["result"]["status"],
            "committed"
        );
        assert_eq!(
            pm::current_parameters(&h.document.editor.project, &task.id, option).unwrap(),
            parameters
        );
        h.document
            .editor
            .apply(ModelCommand::MoveElements {
                ids: vec![id],
                offset: crate::model::Point::new(0.1, 0.),
            })
            .unwrap();
        let (_, fresh) = context(&mut e, &mut h, &s);
        assert_eq!(fresh["write_authorization"]["status"], "suspended");
        e.set_write_enabled(true, &h.document.editor);
        let refused = prepare_event(
            &mut e,
            &mut h,
            &s,
            &req,
            Event::Progress {
                message: "Must not resume from a fresh revision".into(),
            },
        );
        assert_eq!(
            refused["error"]["code"], "OBJECT_TASK_AUTHORIZATION_SUSPENDED",
            "{refused}"
        );
        h.document.editor.undo();
        let (_, _) = context(&mut e, &mut h, &s);
        native_event(&mut h, &req, Event::Resume);
        assert_eq!(
            h.authorize_object_task(h.context(), &req)["result"]["status"],
            "authorized"
        );
        let progress = prepare_event(
            &mut e,
            &mut h,
            &s,
            &req,
            Event::Progress {
                message: "Explicit original task resume, no operation replay".into(),
            },
        );
        assert!(progress["result"]["preview_id"].is_string(), "{progress}");
        assert_eq!(
            commit(
                &mut e,
                &mut h,
                &s,
                &progress,
                &crate::storage::new_document_identity()
            )["result"]["status"],
            "committed"
        );
        let (t, c) = context(&mut e, &mut h, &s);
        let ops = rpc(
            &mut e,
            &mut h,
            &s,
            json!({"method":"query_object_task_operations","params":{"target":t,"expected_revision":c["revision"],"request_id":req}}),
        );
        assert!(
            ops["result"]["operations"]
                .as_array()
                .unwrap()
                .iter()
                .any(|r| r["request_id"] == update && r["phase"] == "committed")
        );
        let before_cancel = h.document.editor.project.elements.clone();
        native_event(&mut h, &req, Event::Cancel);
        h.operations
            .borrow_mut()
            .release_finished_object_task(&h.document.editor, &req);
        assert_eq!(h.document.editor.project.elements, before_cancel);
        let forbidden = prepare_event(
            &mut e,
            &mut h,
            &s,
            &req,
            Event::Progress {
                message: "cancel cannot renew".into(),
            },
        );
        assert!(forbidden.get("error").is_some());
    }
    fn claim(e: &mut Endpoint, h: &mut ApplicationHost, socket: &std::path::Path, req: &str) {
        assert_eq!(
            h.authorize_object_task(h.context(), req)["result"]["status"],
            "authorized"
        );
        let p = prepare_event(
            e,
            h,
            socket,
            req,
            Event::Claim {
                agent_label: "Client fixture, not AI".into(),
            },
        );
        let r = commit(e, h, socket, &p, &crate::storage::new_document_identity());
        assert_eq!(r["result"]["status"], "committed", "{r}");
    }
    #[test]
    fn object_task_research_reply_completion_and_saved_identity_without_saved_authority() {
        let (root, mut h, mut e) = setup();
        let socket = root.join("ipc/a.sock");
        let req = submit(&mut h, false);
        claim(&mut e, &mut h, &socket, &req);
        let file = root.join("saved.json");
        crate::storage::save_document(
            &h.document.editor.project,
            &file,
            h.document.editor.project_identity(),
        )
        .unwrap();
        let raw: Value = serde_json::from_slice(&fs::read(&file).unwrap()).unwrap();
        assert_eq!(raw["schema_version"], 30);
        let mut wrong = raw.clone();
        wrong["schema_version"] = json!(29);
        let wrongfile = root.join("wrong-marker.json");
        fs::write(&wrongfile, serde_json::to_vec(&wrong).unwrap()).unwrap();
        assert!(crate::editor::Editor::from_stored_document(&wrongfile).is_err());
        let original_elements = h.document.editor.project.elements.clone();
        drop(e);
        drop(h);
        let editor = crate::editor::Editor::from_stored_document(&file).unwrap();
        let mut fresh = ApplicationHost::new(crate::model::Project::default());
        fresh.document.editor = editor;
        fresh
            .configure_operation_journal(Some(root.join("journal")))
            .unwrap();
        let mut fresh_e = Endpoint::bind_for_host(
            root.join("ipc-b/b.sock"),
            false,
            Some(root.join("journal")),
            &mut fresh,
        )
        .unwrap();
        let sock = root.join("ipc-b/b.sock");
        let blocked = prepare_event(
            &mut fresh_e,
            &mut fresh,
            &sock,
            &req,
            Event::Progress {
                message: "Reads do not grant".into(),
            },
        );
        assert_eq!(
            blocked["error"]["code"], "WRITE_NOT_AUTHORIZED",
            "{blocked}"
        );
        native_event(&mut fresh, &req, Event::Resume);
        assert_eq!(
            fresh.authorize_object_task(fresh.context(), &req)["result"]["status"],
            "authorized"
        );
        let (t, c) = context(&mut fresh_e, &mut fresh, &sock);
        let task = fresh
            .document
            .editor
            .project
            .design_process
            .task()
            .unwrap()
            .clone();
        let option = fresh
            .document
            .editor
            .project
            .plan_options
            .active_design_option
            .unwrap();
        let mut parameters =
            pm::current_parameters(&fresh.document.editor.project, &task.id, option).unwrap();
        parameters.wing_depth_mm = 5000;
        let forbidden = rpc(
            &mut fresh_e,
            &mut fresh,
            &sock,
            json!({"method":"prepare_parametric_proposal","params":{"target":t,"expected_revision":c["revision"],"expected_context_revision":c["context_revision"],"expected_sources_revision":c["project_sources"]["sources_revision"],"task_id":task.id,"proposal":{"action":"update","update_id":crate::storage::new_document_identity(),"instruction":"Research permission cannot change geometry","parameters":parameters}}}),
        );
        assert_eq!(
            forbidden["error"]["code"], "OBJECT_TASK_SCOPE_MISMATCH",
            "{forbidden}"
        );
        let (topic, index, _, _) = topic_tasks::find(&fresh.document.editor.project, &req).unwrap();
        let reply=topics::AgentReply{id:crate::storage::new_document_identity(),agent_label:"Client fixture, not AI".into(),text:"Saved task resumed under its original identity. Quantities remain authored concepts.".into(),claims:vec![crate::design_process::intent::Claim{kind:crate::design_process::intent::ClaimKind::Observation,text:"The original two editable alternatives remain.".into()}]};
        let edit = EvidenceEdit::RecordTopicReply {
            id: topic.id.clone(),
            exchange_index: index,
            expected_topic_sha256: crate::automation_journal::canonical_hash(topic).unwrap(),
            reply: reply.clone(),
        };
        let preview = rpc(
            &mut fresh_e,
            &mut fresh,
            &sock,
            json!({"method":"prepare_native_change","params":{"target":t,"expected_revision":c["revision"],"expected_context_revision":c["context_revision"],"change":{"version":1,"edits":[{"operation":"evidence","edit":edit}]}}}),
        );
        assert!(preview["result"]["preview_id"].is_string(), "{preview}");
        // Reply identity is the coordinator operation identity, not an arbitrary claim.
        let bad = commit(
            &mut fresh_e,
            &mut fresh,
            &sock,
            &preview,
            &crate::storage::new_document_identity(),
        );
        assert_eq!(
            bad["error"]["code"], "OBJECT_TASK_REPLY_REQUEST_ID_MISMATCH",
            "{bad}"
        );
        assert_eq!(
            commit(&mut fresh_e, &mut fresh, &sock, &preview, &reply.id)["result"]["status"],
            "committed"
        );
        let bad = prepare_event(
            &mut fresh_e,
            &mut fresh,
            &sock,
            &req,
            Event::Complete {
                operation_ids: vec![reply.id.clone(), req.clone()],
            },
        );
        assert_eq!(
            bad["error"]["code"], "OBJECT_TASK_RESULT_OWNERSHIP_MISMATCH",
            "{bad}"
        );
        let done = prepare_event(
            &mut fresh_e,
            &mut fresh,
            &sock,
            &req,
            Event::Complete {
                operation_ids: vec![reply.id.clone()],
            },
        );
        let finished = commit(
            &mut fresh_e,
            &mut fresh,
            &sock,
            &done,
            &crate::storage::new_document_identity(),
        );
        assert_eq!(finished["result"]["status"], "committed", "{finished}");
        assert_eq!(
            topic_tasks::find(&fresh.document.editor.project, &req)
                .unwrap()
                .3
                .state()
                .unwrap()
                .0,
            topic_tasks::Phase::Completed
        );
        assert_eq!(fresh.document.editor.project.elements, original_elements);
        assert!(
            prepare_event(
                &mut fresh_e,
                &mut fresh,
                &sock,
                &req,
                Event::Progress {
                    message: "Completion is terminal".into()
                }
            )
            .get("error")
            .is_some()
        );
        crate::storage::save_document(
            &fresh.document.editor.project,
            &root.join("completed.json"),
            fresh.document.editor.project_identity(),
        )
        .unwrap();
        assert_eq!(
            crate::editor::Editor::from_stored_document(&root.join("completed.json"))
                .unwrap()
                .project,
            fresh.document.editor.project
        );
    }
    #[test]
    fn object_task_native_undo_revokes_and_manual_parameters_cannot_be_adopted() {
        let (root, mut h, mut e) = setup();
        let socket = root.join("ipc/a.sock");
        let req = submit(&mut h, true);
        assert_eq!(
            h.authorize_object_task(h.context(), &req)["result"]["status"],
            "authorized"
        );
        h.document.editor.undo();
        context(&mut e, &mut h, &socket);
        assert_eq!(h.object_task_authorization()["status"], "not_granted");
        h.document.editor.redo();
        context(&mut e, &mut h, &socket);
        assert_eq!(h.object_task_authorization()["status"], "not_granted");
        claim(&mut e, &mut h, &socket, &req);
        let t = h.document.editor.project.design_process.task().unwrap();
        let task = t.id.clone();
        let option = t.candidates[0].option;
        let mut parameters =
            pm::current_parameters(&h.document.editor.project, &task, option).unwrap();
        parameters.wing_depth_mm = 6100;
        let id = crate::storage::new_document_identity();
        let c = pm::prepare_update(
            &h.document.editor,
            &id,
            &task,
            parameters,
            "Human local change".into(),
        )
        .unwrap();
        assert_eq!(
            h.confirm_task_prepared(h.context(), &id, "Manual parameters", Rc::new(c))["result"]["status"],
            "committed"
        );
        context(&mut e, &mut h, &socket);
        native_event(&mut h, &req, Event::Resume);
        let denied = h.authorize_object_task(h.context(), &req);
        assert_eq!(
            denied["error"]["code"], "OBJECT_TASK_USER_PARAMETER_CHANGE",
            "{denied}"
        );
        assert_eq!(
            pm::current_parameters(&h.document.editor.project, &task, option)
                .unwrap()
                .wing_depth_mm,
            6100
        );
    }
    #[test]
    fn object_task_unknown_commit_queries_original_without_grant_or_replay() {
        use crate::automation_change::{Commit, DurableStage};
        let (root, mut h, mut e) = setup();
        let sock = root.join("ipc/a.sock");
        let req = submit(&mut h, false);
        claim(&mut e, &mut h, &sock, &req);
        let p = prepare_event(
            &mut e,
            &mut h,
            &sock,
            &req,
            Event::Progress {
                message: "Protected failure-injection copy only".into(),
            },
        );
        assert!(p["result"]["preview_id"].is_string(), "{p}");
        let target = h.operation_target();
        let op = crate::storage::new_document_identity();
        let call = json!({"target":target,"preview_id":p["result"]["preview_id"],"request_id":op});
        fs::write(
            root.join("original-fault-operation.json"),
            serde_json::to_vec_pretty(&call).unwrap(),
        )
        .unwrap();
        h.operations.borrow_mut().set_durability_probe(|stage| {
            if stage == DurableStage::ModelCommitted {
                panic!("simulated lost result after model commit")
            }
        });
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            h.operations.borrow_mut().commit(
                &mut h.document.editor,
                serde_json::from_value::<Commit>(call).unwrap(),
            )
        }));
        assert!(outcome.is_err());
        h.operations.borrow_mut().set_durability_probe(|_| {});
        let committed = h.document.editor.project.clone();
        assert_eq!(
            h.authorize_object_task(h.context(), &req)["error"]["code"],
            "OBJECT_TASK_OPERATION_UNKNOWN"
        );
        let read = rpc(
            &mut e,
            &mut h,
            &sock,
            json!({"method":"get_operation","params":{"target":target,"original_target":target,"request_id":op}}),
        );
        assert!(read.get("result").is_some(), "{read}");
        let revision = h.document.editor.revision;
        let recovered = rpc(
            &mut e,
            &mut h,
            &sock,
            json!({"method":"recover_operation","params":{"target":target,"expected_revision":revision,"original_target":target,"request_id":op}}),
        );
        assert!(recovered.get("result").is_some(), "{recovered}");
        assert_eq!(h.document.editor.project, committed);
        assert_ne!(h.object_task_authorization()["status"], "authorized");
        native_event(&mut h, &req, Event::Resume);
        assert_eq!(
            h.authorize_object_task(h.context(), &req)["result"]["status"],
            "authorized"
        );
        assert_eq!(
            topic_tasks::find(&h.document.editor.project, &req)
                .unwrap()
                .3
                .events
                .iter()
                .filter(|e| matches!(e, Event::Progress { .. }))
                .count(),
            1
        );
    }
    #[test]
    fn native_new_object_task_can_review_changed_inputs_without_renewing_after_insertion() {
        let (root, mut h, mut e) = setup();
        let socket = root.join("ipc/a.sock");
        let mut site = h.document.editor.project.site.clone();
        site.control_tolerance_m = Some(0.02);
        h.document
            .editor
            .apply(ModelCommand::SetSiteContext { context: site })
            .unwrap();
        // A newly submitted task records its actual current scope, not an old grant.
        let req = submit(&mut h, true);
        assert_eq!(
            h.authorize_object_task(h.context(), &req)["result"]["status"],
            "authorized"
        );
        let claimed = prepare_event(
            &mut e,
            &mut h,
            &socket,
            &req,
            Event::Claim {
                agent_label: "Public fixture, not AI execution".into(),
            },
        );
        assert!(claimed["result"]["preview_id"].is_string(), "{claimed}");
        assert_eq!(
            commit(
                &mut e,
                &mut h,
                &socket,
                &claimed,
                &crate::storage::new_document_identity()
            )["result"]["status"],
            "committed"
        );
        let (target, c) = context(&mut e, &mut h, &socket);
        let task = h
            .document
            .editor
            .project
            .design_process
            .task()
            .unwrap()
            .clone();
        let option = h
            .document
            .editor
            .project
            .plan_options
            .active_design_option
            .unwrap();
        let inputs = pm::capture_inputs(&h.document.editor.project, &task.id, option).unwrap();
        let mut parameters =
            pm::current_parameters(&h.document.editor.project, &task.id, option).unwrap();
        parameters.wing_depth_mm = 6400;
        let update = crate::storage::new_document_identity();
        let before = h.document.editor.project.clone();
        let prepared = rpc(
            &mut e,
            &mut h,
            &socket,
            json!({"method":"prepare_parametric_proposal","params":{"target":target,"expected_revision":c["revision"],"expected_context_revision":c["context_revision"],"expected_sources_revision":c["project_sources"]["sources_revision"],"task_id":task.id,"proposal":{"action":"update_reviewed_inputs","update_id":update,"instruction":"Only this candidate with explicitly reviewed declared inputs","parameters":parameters,"inputs":inputs}}}),
        );
        assert!(prepared["result"]["preview_id"].is_string(), "{prepared}");
        assert_eq!(h.document.editor.project, before);
        let committed = commit(&mut e, &mut h, &socket, &prepared, &update);
        assert_eq!(committed["result"]["status"], "committed", "{committed}");
        assert_eq!(
            pm::current_parameters(&h.document.editor.project, &task.id, option).unwrap(),
            parameters
        );
        assert_eq!(
            pm::current_inputs(&h.document.editor.project, &task.id, option).unwrap(),
            inputs
        );
        let ids = &task.candidates[0].initial_mass_ids;
        for old in &before.elements {
            if !ids.contains(&old.id) {
                assert_eq!(
                    h.document
                        .editor
                        .project
                        .elements
                        .iter()
                        .find(|x| x.id == old.id),
                    Some(old)
                );
            }
        }
        // A later user condition insertion must sticky-pause this same task.
        let mut site = h.document.editor.project.site.clone();
        site.control_tolerance_m = Some(0.03);
        h.document
            .editor
            .apply(ModelCommand::SetSiteContext { context: site })
            .unwrap();
        let (_, fresh) = context(&mut e, &mut h, &socket);
        assert_eq!(fresh["write_authorization"]["status"], "suspended");
        e.set_write_enabled(true, &h.document.editor);
        let refused = prepare_event(
            &mut e,
            &mut h,
            &socket,
            &req,
            Event::Progress {
                message: "Reading a new revision does not renew this task".into(),
            },
        );
        assert_eq!(
            refused["error"]["code"], "OBJECT_TASK_AUTHORIZATION_SUSPENDED",
            "{refused}"
        );
    }
    #[test]
    fn default_object_task_refreshes_only_its_explicit_common_mass_check() {
        use crate::constraints::{self as c, Basis, Definition, Domain, Metric, Relation, Scope};
        let (root, mut h, mut e) = setup();
        let socket = root.join("ipc/a.sock");
        let p = &h.document.editor.project;
        let task = p.design_process.task().unwrap();
        let member = task.candidates[0].initial_mass_ids[0];
        let other = task.candidates[1].initial_mass_ids[0];
        let mut common = p.elements.iter().find(|e| e.id == member).unwrap().clone();
        common.id = p.elements.iter().map(|e| e.id).max().unwrap() + 1;
        common.design_option = None;
        common.name = "Authored common context, not surveyed".into();
        let active_plan = h.document.editor.project.plan_options.clone();
        let mut main_plan = active_plan.clone();
        main_plan.active_design_option = None;
        h.document
            .editor
            .apply(ModelCommand::SetOptions {
                plan: Some(main_plan),
                design: None,
                guide: None,
                grid: None,
            })
            .unwrap();
        h.document
            .editor
            .apply(ModelCommand::AddElement {
                element: common.clone(),
            })
            .unwrap();
        h.document
            .editor
            .apply(ModelCommand::SetOptions {
                plan: Some(active_plan),
                design: None,
                guide: None,
                grid: None,
            })
            .unwrap();
        let mut unmapped = h
            .document
            .editor
            .project
            .elements
            .iter()
            .find(|e| e.id == member)
            .unwrap()
            .clone();
        unmapped.id = common.id + 1;
        h.document
            .editor
            .apply(ModelCommand::AddElement {
                element: unmapped.clone(),
            })
            .unwrap();
        for (id, candidate) in [
            ("mapped", member),
            ("other", other),
            ("unmapped", unmapped.id),
        ] {
            let definition = Definition {
                id: id.into(),
                title: id.into(),
                domain: Domain::ConceptMasses,
                scope: Scope::Objects {
                    ids: vec![candidate, common.id],
                },
                metric: Metric::HorizontalSpacing,
                relation: Relation::AtLeast,
                limit: Some(1.),
                basis: Basis::DesignAssumption {
                    note: "Public authored fixture, no legal conclusion".into(),
                },
            };
            h.document
                .editor
                .apply(ModelCommand::Constraint {
                    edit: c::Edit::Put {
                        expected_sha256: None,
                        definition,
                    },
                })
                .unwrap();
        }
        let req = submit(&mut h, true);
        claim(&mut e, &mut h, &socket, &req);
        // The endpoint was bound read-only. Only the existing native task grant applies.
        let prepare = |e: &mut Endpoint, h: &mut ApplicationHost, id: &str| {
            let (target, context) = context(e, h, &socket);
            let check = h
                .document
                .editor
                .project
                .rules
                .checks
                .iter()
                .find(|c| c.definition.id == id)
                .unwrap();
            rpc(
                e,
                h,
                &socket,
                json!({"method":"prepare_native_change","params":{"target":target,"expected_revision":context["revision"],"expected_context_revision":context["context_revision"],"change":{"version":1,"edits":[{"operation":"constraint","edit":c::Edit::Run{id:id.into(),expected_sha256:c::expected(check)}}]}}}),
            )
        };
        for id in ["other", "unmapped"] {
            let rejected = prepare(&mut e, &mut h, id);
            assert_eq!(
                rejected["error"]["code"], "OBJECT_TASK_SCOPE_MISMATCH",
                "{rejected}"
            );
        }
        let before = h.document.editor.project.clone();
        let preview = prepare(&mut e, &mut h, "mapped");
        assert!(preview["result"]["preview_id"].is_string(), "{preview}");
        assert_eq!(h.document.editor.project, before);
        let (target, _) = context(&mut e, &mut h, &socket);
        let cancelled = rpc(
            &mut e,
            &mut h,
            &socket,
            json!({"method":"cancel_operation","params":{"target":target,"preview_id":preview["result"]["preview_id"]}}),
        );
        assert!(cancelled.get("error").is_none(), "{cancelled}");
        assert_eq!(h.document.editor.project, before);
        let preview = prepare(&mut e, &mut h, "mapped");
        let committed = commit(
            &mut e,
            &mut h,
            &socket,
            &preview,
            &crate::storage::new_document_identity(),
        );
        assert_eq!(committed["result"]["status"], "committed", "{committed}");
        assert_eq!(h.document.editor.project.elements, before.elements);
        assert!(
            h.document
                .editor
                .project
                .rules
                .checks
                .iter()
                .find(|c| c.definition.id == "mapped")
                .unwrap()
                .report
                .is_some()
        );
        // A user edit revokes continuation; reading the current revision cannot renew it.
        let mut changed = before
            .elements
            .iter()
            .find(|x| x.id == member)
            .unwrap()
            .clone();
        changed.name = "User changed candidate".into();
        h.document
            .editor
            .apply(ModelCommand::UpdateElement { element: changed })
            .unwrap();
        let refused = prepare(&mut e, &mut h, "mapped");
        assert_eq!(
            refused["error"]["code"], "WRITE_AUTHORIZATION_SUSPENDED",
            "{refused}"
        );
        assert_eq!(
            h.document
                .editor
                .project
                .elements
                .iter()
                .find(|x| x.id == member)
                .unwrap()
                .name,
            "User changed candidate"
        );
        assert_eq!(
            h.document
                .editor
                .project
                .elements
                .iter()
                .find(|x| x.id == common.id),
            Some(&common)
        );
    }
    include!("object_task_create_tests.rs");
    include!("object_task_linked_space_tests.rs");
}
