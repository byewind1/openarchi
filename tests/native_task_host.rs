mod common;
use atelier_bim::{
    application_host::ApplicationHost,
    command::ModelCommand,
    design_process::{Entry, service},
    editor::Editor,
    model::{Point, Project, Shape},
    storage,
};
use std::rc::Rc;

#[test]
fn native_report_without_task_cannot_admit_geometry_or_other_drawing_edits() {
    use atelier_bim::diagrams::{self, Definition, Domain, Edit};
    let mut e = Editor::new(common::synthetic_building_98());
    e.apply(ModelCommand::Diagram {
        edit: Edit::Create {
            name: "Existing model figure".into(),
            definition: Definition {
                story: 0,
                option: None,
                domain: Domain::Zones,
                presentation: Default::default(),
            },
        },
    })
    .unwrap();
    let mut host = ApplicationHost::new(e.project);
    let root = std::env::temp_dir().join(format!(
        "oa-native-report-{}",
        storage::new_document_identity()
    ));
    host.configure_operation_journal(Some(root)).unwrap();
    let figure = host.document.editor.project.drawings.diagrams[0].id;
    let report = ModelCommand::Diagram {
        edit: Edit::Report {
            edit: diagrams::report::Edit::Create {
                name: "Report without design task".into(),
                sections: vec![diagrams::report::Section {
                    heading: "Existing source".into(),
                    figure,
                }],
            },
        },
    };
    let prepared = host
        .document
        .editor
        .prepare_change(host.document.editor.revision, vec![report.clone()])
        .unwrap();
    let result = host.confirm_task_prepared(
        host.context(),
        "report-native-no-task",
        "Report metadata",
        Rc::new(prepared),
    );
    assert_eq!(result["result"]["status"], "committed", "{result}");
    let before = host.document.editor.project.clone();
    let mut drawings = before.drawings.clone();
    drawings.worksheets.push(atelier_bim::drawings::Worksheet {
        id: drawings.next_id().unwrap(),
        name: "Unrelated worksheet".into(),
    });
    for extra in [
        ModelCommand::MoveElements {
            ids: vec![before.elements[0].id],
            offset: Point::new(1., 0.),
        },
        ModelCommand::SetDrawings { document: drawings },
    ] {
        let proposal = host
            .document
            .editor
            .prepare_change(host.document.editor.revision, vec![extra, report.clone()])
            .unwrap();
        let result = host.confirm_task_prepared(
            host.context(),
            "must-refuse-wide-report",
            "Not a report-only edit",
            Rc::new(proposal),
        );
        assert_eq!(result["error"]["code"], "DESIGN_TASK_MISMATCH", "{result}");
        assert_eq!(host.document.editor.project, before);
    }
}

fn proposal(host: &ApplicationHost, task: &str) -> Rc<atelier_bim::model_service::PreparedChange> {
    Rc::new(
        service::prepare_create(
            &host.document.editor,
            task,
            service::Draft {
                name: "Native fixture".into(),
                question: "Keep original task operation across process restart".into(),
                statements: vec![],
                candidate_names: ["A".into(), "B".into()],
                source_ids: host
                    .document
                    .editor
                    .project
                    .elements
                    .iter()
                    .filter(|e| e.story == 0 && matches!(e.shape, Shape::Zone { .. }))
                    .map(|e| e.id)
                    .collect(),
                entry: Entry::ManualInterface,
            },
        )
        .unwrap(),
    )
}

#[test]
fn native_confirmation_without_transport_is_durable_and_original_request_survives_reopen() {
    let root = std::env::temp_dir().join(format!(
        "oa-native-host-{}",
        storage::new_document_identity()
    ));
    std::fs::create_dir(&root).unwrap();
    let mut host = ApplicationHost::new(common::synthetic_building_98());
    host.configure_operation_journal(Some(root.join("journal")))
        .unwrap();
    let before = host.document.editor.project.clone();
    let task = storage::new_document_identity();
    let original = host.operation_target();
    let prepared = proposal(&host, &task);
    let request = storage::new_document_identity();
    let result = host.confirm_task_prepared(
        host.context(),
        &request,
        "Native confirmed exploration",
        prepared.clone(),
    );
    assert_eq!(result["result"]["status"], "committed", "{result}");
    assert_eq!(
        host.document.editor.project.elements.len(),
        before.elements.len() + 18
    );
    assert_eq!(host.document.editor.undo.len(), 1);
    let duplicate = host.confirm_task_prepared(
        host.context(),
        &request,
        "Native confirmed exploration",
        prepared,
    );
    assert_eq!(duplicate["error"]["code"], "DOCUMENT_CONFLICT");
    let checkpoint = host.task_operations();
    assert_eq!(checkpoint["total"], 1);
    assert_eq!(checkpoint["operations"][0]["native_confirmation"], true);
    let saved = root.join("saved.json");
    storage::save_document(
        &host.document.editor.project,
        &saved,
        host.document.editor.project_identity(),
    )
    .unwrap();
    let expected = host.document.editor.project.clone();
    drop(host);
    let mut host = ApplicationHost::new(Project::default());
    host.document.editor = Editor::from_stored_document(&saved).unwrap();
    host.configure_operation_journal(Some(root.join("journal")))
        .unwrap();
    let lookup = host.inspect_operation(original.clone(), &request);
    assert_eq!(lookup["result"]["status"], "committed");
    assert_eq!(lookup["result"]["current_effect"], "AfterImage");
    assert_eq!(
        lookup["result"]["operation_identity"],
        result["result"]["operation_identity"]
    );
    assert_eq!(
        host.task_operations()["operations"][0]["original_target"],
        serde_json::json!(original)
    );
    assert_eq!(host.document.editor.project, expected);
    assert!(host.document.editor.undo.is_empty());
    let zone = host
        .document
        .editor
        .project
        .design_process
        .task()
        .unwrap()
        .candidates[0]
        .initial_zone_ids[0];
    // A new document session is not permission to retarget an old request ID.
    let prepared = Rc::new(
        service::prepare_edit(&host.document.editor, zone, Point::new(0.1, 0.), None).unwrap(),
    );
    let replay = host.confirm_task_prepared(
        host.context(),
        &request,
        "Must inspect original identity",
        prepared,
    );
    assert_eq!(replay["error"]["code"], "RECOVERY_REQUIRED");
    assert_eq!(
        replay["error"]["operation_identity"],
        result["result"]["operation_identity"]
    );
    assert_eq!(host.document.editor.project, expected);
    assert!(host.document.editor.undo.is_empty());
    drop(host);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn native_confirmation_rejects_stale_previews_and_unavailable_durability() {
    let root = std::env::temp_dir().join(format!(
        "oa-native-failure-{}",
        storage::new_document_identity()
    ));
    std::fs::create_dir(&root).unwrap();
    let mut host = ApplicationHost::new(common::synthetic_building_98());
    let task = storage::new_document_identity();
    let prepared = proposal(&host, &task);
    let before = host.document.editor.project.clone();
    let missing = host.confirm_task_prepared(
        host.context(),
        &task,
        "Native must not fall back to memory",
        prepared.clone(),
    );
    assert_eq!(missing["error"]["code"], "DURABILITY_NOT_CONFIGURED");
    assert_eq!(host.document.editor.project, before);
    std::fs::write(root.join("blocked"), b"not a directory").unwrap();
    host.configure_operation_journal(Some(root.join("blocked")))
        .unwrap();
    let blocked = host.confirm_task_prepared(
        host.context(),
        &task,
        "Native durability unavailable",
        prepared.clone(),
    );
    assert_eq!(blocked["error"]["code"], "JOURNAL_UNAVAILABLE");
    assert_eq!(host.document.editor.project, before);
    host.configure_operation_journal(Some(root.join("journal")))
        .unwrap();
    let context = host.context();
    let zone = host
        .document
        .editor
        .project
        .elements
        .iter()
        .find(|e| matches!(e.shape, Shape::Zone { .. }))
        .unwrap()
        .id;
    host.document
        .editor
        .apply(ModelCommand::MoveElements {
            ids: vec![zone],
            offset: Point::new(0.1, 0.),
        })
        .unwrap();
    let manual = host.document.editor.project.clone();
    let stale = host.confirm_task_prepared(context, &task, "Keep user edit", prepared);
    assert_eq!(stale["error"]["code"], "REVISION_CONFLICT");
    assert_eq!(host.document.editor.project, manual);
    assert_eq!(host.document.editor.undo.len(), 1);
    drop(host);
    std::fs::remove_dir_all(root).unwrap();
}

fn figure_proposal(host: &ApplicationHost) -> Rc<atelier_bim::model_service::PreparedChange> {
    Rc::new(
        host.document
            .editor
            .prepare_change(
                host.document.editor.revision,
                vec![ModelCommand::Diagram {
                    edit: atelier_bim::diagrams::Edit::Create {
                        name: "Native no-task site location".into(),
                        definition: atelier_bim::diagrams::Definition {
                            story: 0,
                            option: None,
                            domain: atelier_bim::diagrams::Domain::Site,
                            presentation: atelier_bim::diagrams::Presentation::SiteLocation,
                        },
                    },
                }],
            )
            .unwrap(),
    )
}
#[test]
fn native_figure_without_design_task_is_durable_and_reopens_under_original_operation_identity() {
    let root = std::env::temp_dir().join(format!(
        "oa-native-figure-{}",
        storage::new_document_identity()
    ));
    std::fs::create_dir(&root).unwrap();
    let mut host = ApplicationHost::new(common::synthetic_building_98());
    let before = host.document.editor.project.clone();
    assert!(before.design_process.task().is_none());
    let missing = host.confirm_task_prepared(
        host.context(),
        "no-journal",
        "Create linked model figure",
        figure_proposal(&host),
    );
    assert_eq!(missing["error"]["code"], "DURABILITY_NOT_CONFIGURED");
    assert_eq!(host.document.editor.project, before);
    host.configure_operation_journal(Some(root.join("journal")))
        .unwrap();
    let target = host.operation_target();
    let request = storage::new_document_identity();
    let prepared = figure_proposal(&host);
    let result = host.confirm_task_prepared(
        host.context(),
        &request,
        "Create linked model figure",
        prepared,
    );
    assert_eq!(result["result"]["status"], "committed", "{result}");
    assert_eq!(host.document.editor.project.elements, before.elements);
    assert_eq!(
        host.document.editor.project.design_process,
        before.design_process
    );
    let after = host.document.editor.project.clone();
    let file = root.join("figure.json");
    storage::save_document(&after, &file, host.document.editor.project_identity()).unwrap();
    drop(host);
    let mut host = ApplicationHost::new(Project::default());
    host.document.editor = Editor::from_stored_document(&file).unwrap();
    host.configure_operation_journal(Some(root.join("journal")))
        .unwrap();
    let original = host.inspect_operation(target.clone(), &request);
    assert_eq!(original["result"]["status"], "committed");
    assert_eq!(
        original["result"]["operation_identity"],
        result["result"]["operation_identity"]
    );
    assert_eq!(host.document.editor.project, after);
    let next = figure_proposal(&host);
    let duplicate =
        host.confirm_task_prepared(host.context(), &request, "Create linked model figure", next);
    assert_eq!(duplicate["error"]["code"], "RECOVERY_REQUIRED");
    assert_eq!(host.document.editor.project, after);
    assert!(host.document.editor.undo.is_empty());
    drop(host);
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn native_no_task_figure_admission_rejects_model_mixed_edits_and_stale_user_state() {
    let root = std::env::temp_dir().join(format!(
        "oa-native-figure-guard-{}",
        storage::new_document_identity()
    ));
    std::fs::create_dir(&root).unwrap();
    let mut host = ApplicationHost::new(common::synthetic_building_98());
    host.configure_operation_journal(Some(root.join("journal")))
        .unwrap();
    let before = host.document.editor.project.clone();
    let id = before.elements[0].id;
    let prepared = Rc::new(
        host.document
            .editor
            .prepare_change(
                0,
                vec![
                    ModelCommand::MoveElements {
                        ids: vec![id],
                        offset: Point::new(0.1, 0.),
                    },
                    ModelCommand::Diagram {
                        edit: atelier_bim::diagrams::Edit::Create {
                            name: "Mixed unauthorized shortcut".into(),
                            definition: atelier_bim::diagrams::Definition {
                                story: 0,
                                option: None,
                                domain: atelier_bim::diagrams::Domain::Site,
                                presentation: atelier_bim::diagrams::Presentation::SiteLocation,
                            },
                        },
                    },
                ],
            )
            .unwrap(),
    );
    let result =
        host.confirm_task_prepared(host.context(), "mixed", "Native figure only", prepared);
    assert_eq!(result["error"]["code"], "DESIGN_TASK_MISMATCH");
    assert_eq!(host.document.editor.project, before);
    let context = host.context();
    let pending = figure_proposal(&host);
    host.document
        .editor
        .apply(ModelCommand::MoveElements {
            ids: vec![id],
            offset: Point::new(0.1, 0.),
        })
        .unwrap();
    let user = host.document.editor.project.clone();
    let stale = host.confirm_task_prepared(context, "stale", "Native figure only", pending);
    assert_eq!(stale["error"]["code"], "REVISION_CONFLICT");
    assert_eq!(host.document.editor.project, user);
    assert_eq!(host.document.editor.undo.len(), 1);
    drop(host);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn standalone_reply_is_native_metadata_and_cannot_admit_unrelated_geometry() {
    use atelier_bim::design_process::intent::{Claim, ClaimKind};
    use atelier_bim::evidence::{EvidenceEdit, Target, topics};
    let mut host = ApplicationHost::new(common::synthetic_building_98());
    let root = std::env::temp_dir().join(format!(
        "oa-native-reply-{}",
        storage::new_document_identity()
    ));
    host.configure_operation_journal(Some(root)).unwrap();
    let zone = host
        .document
        .editor
        .project
        .elements
        .iter()
        .find(|e| matches!(e.shape, Shape::Zone { .. }))
        .unwrap()
        .id;
    let tid = storage::new_document_identity();
    let exchange = topics::capture(
        &host.document.editor.project,
        &[Target::Element { id: zone }],
        "Read this room, without geometry authority".into(),
    )
    .unwrap();
    host.document
        .editor
        .apply(ModelCommand::Evidence {
            edit: EvidenceEdit::RecordTopic {
                id: tid.clone(),
                exchange,
                expected_previous: None,
            },
        })
        .unwrap();
    let topic = host.document.editor.project.evidence.topics[0].clone();
    let command = ModelCommand::Evidence {
        edit: EvidenceEdit::RecordTopicReply {
            id: tid,
            exchange_index: 0,
            expected_topic_sha256: atelier_bim::automation_journal::canonical_hash(&topic).unwrap(),
            reply: topics::AgentReply {
                id: storage::new_document_identity(),
                agent_label: "Native guard regression, not AI".into(),
                text: "No geometry operation is authorized by an answer.".into(),
                claims: vec![Claim {
                    kind: ClaimKind::Observation,
                    text: "This answer is project metadata.".into(),
                }],
            },
        },
    };
    let before = host.document.editor.project.clone();
    let unrelated = before.elements.iter().find(|e| e.id != zone).unwrap().id;
    let wide = host
        .document
        .editor
        .prepare_change(
            host.document.editor.revision,
            vec![
                ModelCommand::MoveElements {
                    ids: vec![unrelated],
                    offset: Point::new(0.1, 0.),
                },
                command.clone(),
            ],
        )
        .unwrap();
    let rejected = host.confirm_task_prepared(
        host.context(),
        "reply-cannot-grant-geometry",
        "Not reply-only",
        Rc::new(wide),
    );
    assert_eq!(
        rejected["error"]["code"], "DESIGN_TASK_MISMATCH",
        "{rejected}"
    );
    assert_eq!(host.document.editor.project, before);
    let reply = host
        .document
        .editor
        .prepare_change(host.document.editor.revision, vec![command])
        .unwrap();
    let id = storage::new_document_identity();
    let result = host.confirm_task_prepared(
        host.context(),
        &id,
        "Only declared answer metadata",
        Rc::new(reply),
    );
    assert_eq!(result["result"]["status"], "committed", "{result}");
    assert_eq!(host.document.editor.project.elements, before.elements);
    host.document.editor.undo();
    assert_eq!(host.document.editor.project.evidence, before.evidence);
}
