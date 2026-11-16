#[cfg(test)]
mod parametric_tests {
    use super::*;
    use crate::model::ModelAdapter;
    use crate::{
        application_host::ApplicationHost,
        command::ModelCommand,
        design_process::parametric::{Definition, Parameters},
        model::Point,
    };
    use std::{
        io::{BufRead, BufReader},
        os::unix::net::UnixStream,
        path::Path,
        time::Instant,
    };
    fn request(
        endpoint: &mut Endpoint,
        host: &mut ApplicationHost,
        path: &Path,
        value: Value,
    ) -> Value {
        let path = path.to_owned();
        let client = thread::spawn(move || {
            let mut s = UnixStream::connect(path).unwrap();
            s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            writeln!(s, "{value}").unwrap();
            let mut line = String::new();
            BufReader::new(s).read_line(&mut line).unwrap();
            serde_json::from_str(&line).unwrap()
        });
        let start = Instant::now();
        while !client.is_finished() {
            endpoint.poll_with_host(host, None);
            assert!(start.elapsed() < Duration::from_secs(5));
            thread::sleep(Duration::from_millis(1));
        }
        client.join().unwrap()
    }
    #[test]
    fn parametric_default_host_socket_preview_cancel_and_fresh_reads_do_not_reauthorize() {
        for enabled in [false, true] {
            let root = PathBuf::from("/tmp").join(format!(
                "oa-param-ipc-{}",
                crate::storage::new_document_identity()
            ));
            fs::create_dir(&root).unwrap();
            let mut host = ApplicationHost::new(crate::model::Project::demo());
            let socket = root.join("ipc/app.sock");
            let mut endpoint = Endpoint::bind_for_host(
                socket.clone(),
                enabled,
                Some(root.join("journal")),
                &mut host,
            )
            .unwrap();
            let target = request(
                &mut endpoint,
                &mut host,
                &socket,
                json!({"method":"get_target"}),
            )["result"]["target"]
                .clone();
            let schema = request(
                &mut endpoint,
                &mut host,
                &socket,
                json!({"method":"get_native_schema"}),
            );
            assert!(
                schema["result"]["parametric_proposal_schema"]["properties"]["proposal"]
                    .is_object()
            );
            let context = request(
                &mut endpoint,
                &mut host,
                &socket,
                json!({"method":"get_context","params":target}),
            )["result"]
                .clone();
            let p = &host.document.editor.project;
            let a = Parameters {
                origin_mm: [30000, 30000],
                court_width_mm: 20000,
                court_depth_mm: 14000,
                wing_depth_mm: 6000,
                storeys: 2,
                floor_height_mm: 3300,
            };
            let mut b = a.clone();
            b.court_width_mm = 18000;
            let def = Definition {
                rule_version: 1,
                candidates: [a, b],
                site_sha256: crate::automation_journal::canonical_hash(&p.site).unwrap(),
                sources: vec![],
                assumptions:
                    "All dimensions are authored assumptions, no source measurements or approval."
                        .into(),
            };
            let sources=crate::automation::sources::context(&host.document.editor,None)["sources_revision"].clone();
            let mut params = json!({"target":target,"expected_revision":context["revision"],"expected_context_revision":context["context_revision"],"expected_sources_revision":sources,"task_id":crate::storage::new_document_identity(),"proposal":{"action":"create","name":"Bounded study","question":"Compare two open court envelopes without changing existing work","definition":def,"story":0,"layer":0}});
            let before = host.document.editor.project.clone();
            let result = request(
                &mut endpoint,
                &mut host,
                &socket,
                json!({"method":"prepare_parametric_proposal","params":params}),
            );
            assert_eq!(host.document.editor.project, before);
            if !enabled {
                assert_eq!(result["error"]["code"], "WRITE_NOT_AUTHORIZED", "{result}");
                continue;
            }
            let preview = result["result"]["preview_id"]
                .as_str()
                .expect("validated shared preview");
            let cancelled = request(
                &mut endpoint,
                &mut host,
                &socket,
                json!({"method":"cancel_operation","params":{"target":target,"preview_id":preview}}),
            );
            assert!(cancelled.get("error").is_none(), "{cancelled}");
            // The same real endpoint may prepare again after explicit cancellation;
            // this is a new preview for a known unapplied task, not an unknown replay.
            let again = request(
                &mut endpoint,
                &mut host,
                &socket,
                json!({"method":"prepare_parametric_proposal","params":params}),
            );
            let commit = json!({"method":"commit_change","params":{"target":target,"preview_id":again["result"]["preview_id"],"request_id":params["task_id"]}});
            fs::write(
                root.join("original-agent-create.json"),
                serde_json::to_vec_pretty(&commit).unwrap(),
            )
            .unwrap();
            let committed = request(&mut endpoint, &mut host, &socket, commit);
            assert_eq!(committed["result"]["status"], "committed", "{committed}");
            assert_eq!(
                host.document
                    .editor
                    .project
                    .design_process
                    .task()
                    .unwrap()
                    .entry,
                crate::design_process::Entry::AgentProposal
            );
            let saved = crate::storage::JsonAdapter
                .export(&host.document.editor.project)
                .unwrap();
            let reopened = crate::storage::JsonAdapter.import(&saved).unwrap();
            assert_eq!(
                reopened.design_process.task().unwrap().entry,
                crate::design_process::Entry::AgentProposal
            );
            // A fresh unique task request must still be blocked after an intervening native edit.
            params["task_id"] = json!(crate::storage::new_document_identity());
            let element = host
                .document
                .editor
                .project
                .design_process
                .task()
                .unwrap()
                .candidates[0]
                .initial_mass_ids[0];
            host.document
                .editor
                .apply(ModelCommand::MoveElements {
                    ids: vec![element],
                    offset: Point::new(0.1, 0.),
                })
                .unwrap();
            let current = request(
                &mut endpoint,
                &mut host,
                &socket,
                json!({"method":"get_context","params":target}),
            )["result"]
                .clone();
            assert_eq!(current["write_authorization"]["status"], "suspended");
            params["expected_revision"] = current["revision"].clone();
            params["expected_context_revision"] = current["context_revision"].clone();
            let protected = host.document.editor.project.clone();
            let refused = request(
                &mut endpoint,
                &mut host,
                &socket,
                json!({"method":"prepare_parametric_proposal","params":params}),
            );
            assert_eq!(
                refused["error"]["code"], "WRITE_AUTHORIZATION_SUSPENDED",
                "{refused}"
            );
            assert_eq!(host.document.editor.project, protected);
        }
    }
    #[test]
    fn parameter_only_update_default_socket_commit_original_read_and_undo() {
        use crate::design_process::parametric as pm;
        let root = PathBuf::from("/tmp").join(format!(
            "oa-param-api-state-{}",
            crate::storage::new_document_identity()
        ));
        fs::create_dir(&root).unwrap();
        let mut host = ApplicationHost::new(crate::model::Project::default());
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
        b.court_width_mm = 18000;
        let task = crate::storage::new_document_identity();
        let def = Definition {
            rule_version: 1,
            candidates: [a.clone(), b],
            site_sha256: crate::automation_journal::canonical_hash(
                &host.document.editor.project.site,
            )
            .unwrap(),
            sources: vec![],
            assumptions: "Authored wire fixture, not AI/model acceptance".into(),
        };
        let prepared = pm::prepare(
            &host.document.editor,
            &task,
            "Metadata study".into(),
            "Same envelope with changed concept organization".into(),
            def,
            0,
            0,
        )
        .unwrap();
        let r = host.confirm_task_prepared(
            host.context(),
            &task,
            "Native fixture",
            std::rc::Rc::new(prepared),
        );
        assert_eq!(r["result"]["status"], "committed");
        let before = host.document.editor.project.clone();
        assert_eq!(
            before.design_process.task().unwrap().entry,
            crate::design_process::Entry::ManualInterface
        );
        let option = before.plan_options.active_design_option.unwrap();
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
        let context = request(
            &mut endpoint,
            &mut host,
            &socket,
            json!({"method":"get_context","params":target}),
        )["result"]
            .clone();
        assert_eq!(
            context["parametric_candidates"][0]["parameters"]["storeys"],
            2
        );
        let mut changed = a.clone();
        changed.storeys = 3;
        changed.floor_height_mm = 2200;
        let update = crate::storage::new_document_identity();
        let params = json!({"target":target,"expected_revision":context["revision"],"expected_context_revision":context["context_revision"],"expected_sources_revision":crate::automation::sources::context(&host.document.editor,None)["sources_revision"],"task_id":task,"proposal":{"action":"update","update_id":update,"instruction":"Keep 6.6m envelope, change concept storeys","parameters":changed}});
        let prepared = request(
            &mut endpoint,
            &mut host,
            &socket,
            json!({"method":"prepare_parametric_proposal","params":params}),
        );
        assert!(prepared["result"]["preview_id"].is_string(), "{prepared}");
        assert_eq!(host.document.editor.project, before);
        let commit = json!({"method":"commit_change","params":{"target":target,"preview_id":prepared["result"]["preview_id"],"request_id":update}});
        fs::write(
            root.join("original-commit.json"),
            serde_json::to_vec_pretty(&commit).unwrap(),
        )
        .unwrap();
        let committed = request(&mut endpoint, &mut host, &socket, commit);
        assert_eq!(committed["result"]["status"], "committed", "{committed}");
        assert_eq!(host.document.editor.project.elements, before.elements);
        assert_eq!(
            host.document
                .editor
                .project
                .design_process
                .updates
                .last()
                .unwrap()
                .entry,
            crate::design_process::Entry::AgentProposal
        );
        let read = request(
            &mut endpoint,
            &mut host,
            &socket,
            json!({"method":"get_context","params":target}),
        );
        assert_eq!(
            read["result"]["parametric_candidates"][0]["parameters"]["storeys"],
            3
        );
        let inspect = json!({"method":"get_operation","params":{"target":target,"request_id":update,"original_target":target}});
        assert_eq!(
            request(&mut endpoint, &mut host, &socket, inspect.clone())["result"]["status"],
            "committed"
        );
        let undone = request(
            &mut endpoint,
            &mut host,
            &socket,
            json!({"method":"undo_operation","params":{"target":target,"request_id":update,"original_target":target}}),
        );
        assert_eq!(undone["result"]["status"], "undone", "{undone}");
        assert_eq!(
            pm::current_parameters(&host.document.editor.project, &task, option).unwrap(),
            a
        );
        assert_eq!(host.document.editor.project.design_process.updates.len(), 1);
        assert_eq!(
            request(&mut endpoint, &mut host, &socket, inspect)["result"]["status"],
            "undone"
        );
    }
    include!("parametric_reviewed_tests.rs");
    include!("parametric_check_preview_tests.rs");
}
