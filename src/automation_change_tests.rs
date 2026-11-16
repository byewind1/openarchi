use super::*;
use crate::{
    model::{Point, Project},
    model_service::{ElementTemplate, NamedElement},
    storage,
};
use std::{io::Write, path::PathBuf};
fn stage_name(stage: DurableStage) -> &'static str {
    match stage {
        DurableStage::IntentStored => "intent",
        DurableStage::ModelCommitted => "model",
        DurableStage::CommitStored => "marker",
        DurableStage::UndoIntentStored => "undo-intent",
        DurableStage::ModelUndone => "undo-model",
        DurableStage::UndoStored => "undo-marker",
    }
}
#[test]
#[ignore = "subprocess crash fixture; geometry and trusted-task cases from service_crash_matrix"]
fn service_crash_child() {
    let root = PathBuf::from(std::env::var_os("OA_M2_SERVICE_ROOT").unwrap());
    let wanted = std::env::var("OA_M2_SERVICE_STAGE").unwrap();
    let path = root.join("model.json");
    let mut editor = Editor::from_stored_document(&path).unwrap();
    let target = ReadService::new("crash-child".into()).target(&editor);
    let mut scope = std::fs::File::create(root.join("scope.json")).unwrap();
    scope
        .write_all(&serde_json::to_vec(&target).unwrap())
        .unwrap();
    scope.sync_all().unwrap();
    let mut service =
        ChangeService::new("crash-child".into(), true).with_journal_directory(root.join("journal"));
    let wanted_for_probe = wanted.clone();
    service.set_durability_probe(move |stage| {
        if stage_name(stage) == wanted_for_probe {
            std::process::exit(23);
        }
    });
    let mut wall = editor
        .wall_creation_candidates(vec![(Point::new(0., 0.), Point::new(6., 0.))])
        .unwrap()
        .remove(0);
    wall.id = 0;
    let preview = if std::env::var("OA_M2_SERVICE_TASK").as_deref() == Ok("update") {
        let task = editor.project.design_process.task().unwrap();
        let task_id = task.id.clone();
        let mut element = editor
            .project
            .elements
            .iter()
            .find(|e| Some(task.candidates[0].option) == e.design_option)
            .unwrap()
            .clone();
        if let crate::model::Shape::Zone { spec } = &mut element.shape {
            spec.height += 0.2;
        } else {
            panic!("fixture domain");
        }
        let page = &editor.project.drawings.pages[0];
        let prepared = crate::design_process::update::prepare(
            &editor,
            None,
            &"c".repeat(64),
            &task_id,
            crate::design_process::update::Draft {
                instruction: "Crash fixture: only declared member".into(),
                expected_result: "one existing geometry update and immutable source record".into(),
                sources: vec![crate::design_process::update::Source::Drawing {
                    page: page.id,
                    sha256: page.sha256.clone(),
                }],
                claims: vec![crate::design_process::intent::Claim {
                    kind: crate::design_process::intent::ClaimKind::DesignCompletion,
                    text: "Controlled fixture height; no AI or approval".into(),
                }],
                statements: vec![],
                elements: vec![element],
                expected_comparison: task.compare(&editor.project).unwrap().fingerprint,
                entry: crate::design_process::Entry::AgentProposal,
            },
        )
        .unwrap();
        service.prepare_task_proposal(
            &editor,
            target.clone(),
            &task_id,
            "Interrupted scoped task update",
            prepared,
        )
    } else if std::env::var_os("OA_M2_SERVICE_TASK").is_some() {
        let task_id = storage::new_document_identity();
        let prepared = crate::design_process::service::prepare_create(
            &editor,
            &task_id,
            crate::design_process::service::Draft {
                name: "Interrupted task".into(),
                question: "Preserve original intent and history".into(),
                statements: vec![],
                candidate_names: ["A".into(), "B".into()],
                source_ids: editor
                    .project
                    .elements
                    .iter()
                    .filter(|e| e.story == 0 && matches!(e.shape, crate::model::Shape::Zone { .. }))
                    .map(|e| e.id)
                    .collect(),
                entry: crate::design_process::Entry::ManualInterface,
            },
        )
        .unwrap();
        service.prepare_task_proposal(
            &editor,
            target.clone(),
            &task_id,
            "Interrupted trusted task proposal",
            prepared,
        )
    } else {
        service.prepare(
            &editor,
            Prepare {
                target: target.clone(),
                expected_revision: 0,
                change: BatchChange {
                    creates: vec![NamedElement {
                        name: "wall".into(),
                        template: ElementTemplate {
                            element: wall,
                            host: None,
                        },
                    }],
                    ..Default::default()
                },
            },
        )
    };
    let result = service.commit(
        &mut editor,
        Commit {
            target: target.clone(),
            preview_id: preview["result"]["preview_id"].as_str().unwrap().into(),
            request_id: "c".repeat(64),
        },
    );
    assert_eq!(result["result"]["status"], "committed");
    if wanted.starts_with("undo") {
        let baseline = storage::FileVersion::read(&path).unwrap();
        let version = storage::save_document_if_unchanged(
            &editor.project,
            &path,
            &baseline,
            editor.project_identity(),
        )
        .unwrap();
        editor.bind_file_generation(version.token());
        editor.mark_saved();
        service.undo(
            &mut editor,
            Operation {
                target,
                request_id: "c".repeat(64),
                original_target: None,
            },
        );
    }
    panic!("requested checkpoint was not reached");
}
#[test]
fn service_crash_matrix() {
    for mode in 0..3 {
        let task_proposal = mode == 1;
        let task_update = mode == 2;
        for stage in [
            "intent",
            "model",
            "marker",
            "undo-intent",
            "undo-model",
            "undo-marker",
        ] {
            let root = std::env::temp_dir().join(format!(
                "oa-service-crash-{}",
                storage::new_document_identity()
            ));
            std::fs::create_dir(&root).unwrap();
            let path = root.join("model.json");
            let mut original = Editor::new(if task_proposal || task_update {
                storage::synthetic_building_fixture()
            } else {
                Project::default()
            });
            if task_update {
                let image = root.join("fixture.png");
                image::RgbaImage::from_pixel(8, 8, image::Rgba([180, 200, 210, 255]))
                    .save(&image)
                    .unwrap();
                original
                    .project
                    .drawings
                    .pages
                    .push(crate::drawings::import(&image, &root.join("assets"), 1).unwrap());
                let tid = storage::new_document_identity();
                let prepared = crate::design_process::service::prepare_create(
                    &original,
                    &tid,
                    crate::design_process::service::Draft {
                        name: "Existing task".into(),
                        question: "Preserve existing alternatives on recovery".into(),
                        statements: vec![],
                        candidate_names: ["A".into(), "B".into()],
                        source_ids: original
                            .project
                            .elements
                            .iter()
                            .filter(|e| {
                                e.story == 0 && matches!(e.shape, crate::model::Shape::Zone { .. })
                            })
                            .map(|e| e.id)
                            .collect(),
                        entry: crate::design_process::Entry::ManualInterface,
                    },
                )
                .unwrap();
                original.commit_change(&tid, &prepared).unwrap();
            }
            storage::save_document(&original.project, &path, original.project_identity()).unwrap();
            let mut child = std::process::Command::new(std::env::current_exe().unwrap());
            if task_proposal || task_update {
                child.env(
                    "OA_M2_SERVICE_TASK",
                    if task_update { "update" } else { "create" },
                );
            }
            let status = child
                .args([
                    "--exact",
                    "automation_change::durability_tests::service_crash_child",
                    "--ignored",
                    "--nocapture",
                ])
                .env("OA_M2_SERVICE_ROOT", &root)
                .env("OA_M2_SERVICE_STAGE", stage)
                .status()
                .unwrap();
            assert_eq!(status.code(), Some(23), "{stage}");
            let original_target: Target =
                serde_json::from_slice(&std::fs::read(root.join("scope.json")).unwrap()).unwrap();
            let mut editor = Editor::from_stored_document(&path).unwrap();
            let target = ReadService::new("restart".into()).target(&editor);
            let mut service = ChangeService::new("restart".into(), true)
                .with_journal_directory(root.join("journal"));
            let status = service.operation(
                &editor,
                Operation {
                    target: target.clone(),
                    request_id: "c".repeat(64),
                    original_target: Some(original_target.clone()),
                },
            );
            assert_eq!(
                status["result"]["status"],
                if stage == "marker" {
                    "committed"
                } else if stage == "undo-marker" {
                    "undone"
                } else {
                    "unknown"
                },
                "{stage}: {status}"
            );
            let mut recovered = service.recover(
                &mut editor,
                Recover {
                    target: target.clone(),
                    expected_revision: 0,
                    original_target: original_target.clone(),
                    request_id: "c".repeat(64),
                },
            );
            let mut target = target;
            if recovered["error"]["code"] == "RECOVERY_DOCUMENT_REQUIRED" {
                let fresh = Editor::new(Project::default());
                editor = fresh;
                target = ReadService::new("restart".into()).target(&editor);
                recovered = service.recover_document(
                    &mut editor,
                    Recover {
                        target,
                        expected_revision: 0,
                        original_target: original_target.clone(),
                        request_id: "c".repeat(64),
                    },
                );
                target = ReadService::new("restart".into()).target(&editor);
            }
            assert_eq!(
                recovered["result"]["status"],
                if stage.starts_with("undo") {
                    "undone"
                } else {
                    "committed"
                },
                "{stage}: {recovered}"
            );
            assert_eq!(
                editor.project.elements.len(),
                original.project.elements.len()
                    + if stage.starts_with("undo") {
                        0
                    } else if task_update {
                        0
                    } else if task_proposal {
                        18
                    } else {
                        1
                    },
                "{stage}"
            );
            if task_proposal {
                assert_eq!(editor.project.design_process.tasks.len(), 1, "{stage}");
                assert_eq!(
                    editor.project.design_process.current_task.is_none(),
                    stage.starts_with("undo"),
                    "{stage}"
                );
            }
            if task_update {
                assert_eq!(editor.project.design_process.updates.len(), 1, "{stage}");
                let record = &editor.project.design_process.updates[0];
                assert_eq!(
                    record.effect(&editor.project),
                    if stage.starts_with("undo") {
                        "Task update is not applied"
                    } else {
                        "Task update matches current elements"
                    },
                    "{stage}"
                );
                assert_eq!(
                    editor.project.design_process.tasks,
                    original.project.design_process.tasks
                );
                for element in original
                    .project
                    .elements
                    .iter()
                    .filter(|e| !record.affected_ids.contains(&e.id))
                {
                    assert!(editor.project.elements.contains(element), "{stage}");
                }
            }
            let before_retry = editor.project.clone();
            let revision = editor.revision;
            let retry = service.recover(
                &mut editor,
                Recover {
                    target,
                    expected_revision: revision,
                    original_target,
                    request_id: "c".repeat(64),
                },
            );
            assert!(retry.get("error").is_none(), "{stage}: {retry}");
            assert_eq!(editor.project, before_retry);
            assert_eq!(editor.revision, revision);
            drop(service);
            std::fs::remove_dir_all(root).unwrap();
        }
    }
}
