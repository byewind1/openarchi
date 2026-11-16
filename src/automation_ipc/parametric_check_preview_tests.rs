#[test]
fn declared_parameter_check_default_socket_matches_shared_preview_without_report_writes() {
    use crate::{constraints as c, design_process::parametric as pm};
    for enabled in [false, true] {
        let root = PathBuf::from("/tmp").join(format!(
            "oa-param-check-ipc-{}",
            &crate::storage::new_document_identity()[..12]
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
        let definition = Definition {
            rule_version: 1,
            candidates: [a.clone(), b],
            site_sha256: crate::automation_journal::canonical_hash(
                &host.document.editor.project.site,
            )
            .unwrap(),
            sources: vec![],
            assumptions: "Public test concept; measured and statutory conditions unknown".into(),
        };
        let created = pm::prepare(
            &host.document.editor,
            &task,
            "Fixture".into(),
            "One existing two-wing design".into(),
            definition,
            0,
            0,
        )
        .unwrap();
        assert_eq!(
            host.confirm_task_prepared(
                host.context(),
                &task,
                "Native fixture",
                std::rc::Rc::new(created)
            )["result"]["status"],
            "committed"
        );
        let d = c::Definition {
            id: "declared-height".into(),
            title: "Declared envelope maximum".into(),
            domain: c::Domain::ConceptMasses,
            scope: c::Scope::Candidate {
                story: 0,
                option: host
                    .document
                    .editor
                    .project
                    .plan_options
                    .active_design_option,
            },
            metric: c::Metric::Height,
            relation: c::Relation::AtMost,
            limit: Some(7.),
            basis: c::Basis::DesignAssumption {
                note: "Test preference only".into(),
            },
        };
        host.document
            .editor
            .apply(ModelCommand::Constraint {
                edit: c::Edit::Put {
                    expected_sha256: None,
                    definition: d,
                },
            })
            .unwrap();
        let stamp = c::expected(&host.document.editor.project.rules.checks[0]);
        host.document
            .editor
            .apply(ModelCommand::Constraint {
                edit: c::Edit::Run {
                    id: "declared-height".into(),
                    expected_sha256: stamp,
                },
            })
            .unwrap();
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
        let context = request(
            &mut endpoint,
            &mut host,
            &socket,
            json!({"method":"get_context","params":target}),
        )["result"]
            .clone();
        let schema = request(
            &mut endpoint,
            &mut host,
            &socket,
            json!({"method":"get_native_schema"}),
        );
        assert!(schema["result"]["declared_check_preview_checks_schema"].is_object());
        let original = host.document.editor.project.clone();
        let mut changed = a.clone();
        changed.floor_height_mm = 4000;
        let update = crate::storage::new_document_identity();
        let shared = pm::prepare_update(
            &host.document.editor,
            &update,
            &task,
            changed.clone(),
            "Review taller envelope".into(),
        )
        .unwrap()
        .declared_check_previews();
        assert_eq!(shared[0].state, c::State::Fail);
        let result = request(
            &mut endpoint,
            &mut host,
            &socket,
            json!({"method":"prepare_parametric_proposal","params":{"target":target,"expected_revision":context["revision"],"expected_context_revision":context["context_revision"],"expected_sources_revision":context["project_sources"]["sources_revision"],"task_id":task,"proposal":{"action":"update","update_id":update,"instruction":"Review taller envelope","parameters":changed}}}),
        );
        assert_eq!(host.document.editor.project, original);
        if enabled {
            assert_eq!(
                result["result"]["declared_check_preview"]["checks"],
                json!(shared)
            );
            assert_eq!(
                result["result"]["declared_check_preview"]["stored_reports_changed"],
                false
            );
            assert_eq!(
                result["result"]["declared_check_preview"]["external_originals_read"],
                false
            );
            let cancelled = request(
                &mut endpoint,
                &mut host,
                &socket,
                json!({"method":"cancel_operation","params":{"target":target,"preview_id":result["result"]["preview_id"]}}),
            );
            assert!(cancelled.get("error").is_none(), "{cancelled}");
        } else {
            assert_eq!(result["error"]["code"], "WRITE_NOT_AUTHORIZED");
            assert!(result.get("result").is_none());
        }
        assert_eq!(host.document.editor.project, original);
        drop(endpoint);
        std::fs::remove_dir_all(root).unwrap();
    }
}
