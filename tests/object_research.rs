mod common;
use atelier_bim::{
    automation::{EvidenceRequest, ReadService},
    command::ModelCommand,
    editor::Editor,
    evidence::{EvidenceEdit, LinkStatus, Target, topics},
    model::{ModelAdapter, Point, Shape},
    storage,
};

#[test]
fn discussion_frozen_objects_history_conflict_and_native_roundtrip() {
    let mut e = Editor::new(common::synthetic_building_98());
    let id = e
        .project
        .elements
        .iter()
        .find(|e| matches!(e.shape, Shape::Zone { .. }))
        .unwrap()
        .id;
    let targets = vec![Target::Element { id }];
    let entry = topics::capture(
        &e.project,
        &targets,
        "Can this existing space be shared? Capacity remains unknown.".into(),
    )
    .unwrap();
    let tid = storage::new_document_identity();
    let command = ModelCommand::Evidence {
        edit: EvidenceEdit::RecordTopic {
            id: tid.clone(),
            exchange: entry.clone(),
            expected_previous: None,
        },
    };
    let geometry = e.project.elements.clone();
    let p = e.prepare_change(e.revision, vec![command.clone()]).unwrap();
    assert!(e.project.evidence.topics.is_empty(), "preview is read-only");
    e.set_selection(vec![]);
    e.commit_change("discussion-1", &p).unwrap();
    assert_eq!(e.project.elements, geometry);
    let topic = e.project.evidence.topics[0].clone();
    assert_eq!(topic.exchanges[0].objects[0].link.target, targets[0]);
    let s = entry.objects[0].summary().unwrap();
    assert!(s["footprint_m2"].as_f64().unwrap() > 0.);
    let bytes = storage::JsonAdapter.export(&e.project).unwrap();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&bytes).unwrap()["schema_version"],
        26
    );
    let reopened = storage::JsonAdapter.import(&bytes).unwrap();
    assert_eq!(reopened.evidence, e.project.evidence);
    let mut wrong: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    wrong["schema_version"] = 5.into();
    assert!(
        storage::JsonAdapter
            .import(&serde_json::to_vec(&wrong).unwrap())
            .unwrap_err()
            .contains("version 26")
    );
    let read = ReadService::new("topics-read".into());
    let page = read
        .source_evidence(
            &e,
            &EvidenceRequest {
                target: read.target(&e),
                expected_revision: e.revision,
                offset: 0,
                limit: 1,
                topics: true,
                object_targets: vec![],
            },
        )
        .unwrap();
    assert_eq!(page["topics"][0]["topic"]["id"], tid);
    assert_eq!(page["external_agent_dispatch"], "not_connected");
    e.undo();
    assert!(e.project.evidence.topics.is_empty());
    e.redo();
    assert_eq!(e.project.evidence.topics[0], topic);
    let before = e.project.clone();
    e.set_selection(vec![id]);
    e.move_selection(Point::new(0.25, 0.));
    assert_eq!(
        entry.objects[0].link.status(&e.project),
        LinkStatus::Changed
    );
    assert_eq!(
        e.project.evidence.topics[0], topic,
        "history never auto-rebases"
    );
    assert!(
        e.prepare_change(
            e.revision,
            vec![ModelCommand::Evidence {
                edit: EvidenceEdit::RecordTopic {
                    id: tid.clone(),
                    exchange: entry,
                    expected_previous: Some(topic.clone())
                }
            }]
        )
        .is_err()
    );
    let fresh = topics::capture(
        &e.project,
        &targets,
        "Review the moved space without changing the old discussion.".into(),
    )
    .unwrap();
    let p = e
        .prepare_change(
            e.revision,
            vec![ModelCommand::Evidence {
                edit: EvidenceEdit::RecordTopic {
                    id: tid,
                    exchange: fresh,
                    expected_previous: Some(topic.clone()),
                },
            }],
        )
        .unwrap();
    e.commit_change("discussion-2", &p).unwrap();
    assert_eq!(e.project.evidence.topics[0].exchanges.len(), 2);
    assert_eq!(
        e.project.evidence.topics[0].exchanges[0],
        topic.exchanges[0]
    );
    assert_ne!(e.project.elements, before.elements);
}

#[test]
fn agent_reply_is_frozen_undoable_and_versioned_without_geometry_authority() {
    use atelier_bim::design_process::intent::{Claim, ClaimKind};
    let mut e = Editor::new(common::synthetic_building_98());
    let id = e
        .project
        .elements
        .iter()
        .find(|e| matches!(e.shape, Shape::Zone { .. }))
        .unwrap()
        .id;
    let topic_id = storage::new_document_identity();
    let exchange = topics::capture(
        &e.project,
        &[Target::Element { id }],
        "Can this authored room support the intended use? Preserve unknowns.".into(),
    )
    .unwrap();
    e.apply(ModelCommand::Evidence {
        edit: EvidenceEdit::RecordTopic {
            id: topic_id.clone(),
            exchange,
            expected_previous: None,
        },
    })
    .unwrap();
    let topic = e.project.evidence.topics[0].clone();
    let reply=topics::AgentReply{id:storage::new_document_identity(),agent_label:"Wire-test contribution, not real AI".into(),text:"Authored geometry is readable; approved use and capacity remain unknown. This is a suggestion, not a model edit.".into(),claims:vec![Claim{kind:ClaimKind::Observation,text:"The selected object is an authored Zone.".into()},Claim{kind:ClaimKind::Inference,text:"No capacity conclusion follows from its footprint alone.".into()}]};
    let command = ModelCommand::Evidence {
        edit: EvidenceEdit::RecordTopicReply {
            id: topic_id.clone(),
            exchange_index: 0,
            expected_topic_sha256: atelier_bim::automation_journal::canonical_hash(&topic).unwrap(),
            reply: reply.clone(),
        },
    };
    let before = e.project.clone();
    let p = e.prepare_change(e.revision, vec![command.clone()]).unwrap();
    assert_eq!(e.project, before, "preparation cannot store an answer");
    e.set_selection(vec![]); // Selection does not retarget the captured object.
    e.commit_change("reply-original", &p).unwrap();
    assert_eq!(e.project.elements, before.elements);
    assert_eq!(
        e.project.evidence.topics[0].exchanges[0].reply,
        Some(reply.clone())
    );
    assert!(
        e.prepare_change(e.revision, vec![command.clone()]).is_err(),
        "cannot overwrite an existing answer"
    );
    let bytes = storage::JsonAdapter.export(&e.project).unwrap();
    let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(value["schema_version"], 29);
    assert_eq!(storage::JsonAdapter.import(&bytes).unwrap(), e.project);
    value["schema_version"] = 26.into();
    assert!(
        storage::JsonAdapter
            .import(&serde_json::to_vec(&value).unwrap())
            .unwrap_err()
            .contains("version 29")
    );
    e.undo();
    assert_eq!(e.project, before);
    e.redo();
    assert_eq!(e.project.evidence.topics[0].exchanges[0].reply, Some(reply));
    e.undo();
    e.apply(ModelCommand::MoveElements {
        ids: vec![id],
        offset: Point::new(0.1, 0.),
    })
    .unwrap();
    let protected = e.project.clone();
    assert!(e.prepare_change(e.revision, vec![command]).is_err());
    assert_eq!(e.project, protected);
    let fresh = topics::capture(
        &e.project,
        &[Target::Element { id }],
        "Explicit new question after the user edit".into(),
    )
    .unwrap();
    e.apply(ModelCommand::Evidence {
        edit: EvidenceEdit::RecordTopic {
            id: topic_id,
            exchange: fresh,
            expected_previous: Some(topic),
        },
    })
    .unwrap();
    assert_eq!(e.project.evidence.topics[0].exchanges.len(), 2);
    // No caller may bypass reply admission by embedding one in a new question.
    let mut forged = e.project.evidence.topics[0].exchanges[1].clone();
    forged.reply = Some(topics::AgentReply {
        id: storage::new_document_identity(),
        agent_label: "untrusted".into(),
        text: "injected".into(),
        claims: vec![Claim {
            kind: ClaimKind::Inference,
            text: "unsupported".into(),
        }],
    });
    assert!(
        e.prepare_change(
            e.revision,
            vec![ModelCommand::Evidence {
                edit: EvidenceEdit::RecordTopic {
                    id: storage::new_document_identity(),
                    exchange: forged,
                    expected_previous: None
                }
            }]
        )
        .is_err()
    );
}

#[test]
fn related_discussions_share_explicit_source_result_filter_and_keep_unfiltered_paging() {
    use atelier_bim::{
        concept_mass::ConceptMassSpec,
        mass_spaces,
        model::{Material, Project},
    };
    let mut p = Project::default();
    for x in [0., 20.] {
        p.add(
            0,
            Material::Plaster,
            Shape::ConceptMass {
                spec: ConceptMassSpec {
                    arcs: vec![],
                    hole_arcs: vec![],
                    outline: vec![
                        Point::new(x, 0.),
                        Point::new(x + 8., 0.),
                        Point::new(x + 8., 6.),
                        Point::new(x, 6.),
                    ],
                    holes: vec![],
                    height: 6.,
                    bottom_offset: 0.,
                },
            },
        );
    }
    let mut editor = Editor::new(p);
    let mut topic_ids = vec![];
    for mass in [1, 2, 1] {
        let id = storage::new_document_identity();
        let exchange = topics::capture(
            &editor.project,
            &[Target::Element { id: mass }],
            format!("Investigate source {mass}; concept only"),
        )
        .unwrap();
        editor
            .apply(ModelCommand::Evidence {
                edit: EvidenceEdit::RecordTopic {
                    id: id.clone(),
                    exchange,
                    expected_previous: None,
                },
            })
            .unwrap();
        topic_ids.push(id);
    }
    let stamp = mass_spaces::element_stamp(&editor.project, &editor.project.elements[0]).unwrap();
    editor
        .apply(ModelCommand::MassSpaces {
            edit: mass_spaces::Edit::Create {
                mass: 1,
                expected_source_sha256: stamp,
                name: "Linked result".into(),
                purpose: "Concept only".into(),
                height: 3.,
                assumption: "Public fixture; not an AI task or measured fact".into(),
            },
        })
        .unwrap();
    let before = editor.project.clone();
    let read = ReadService::new("related-read".into());
    let mut request = EvidenceRequest {
        target: read.target(&editor),
        expected_revision: editor.revision,
        topics: true,
        object_targets: vec![Target::Element { id: 3 }],
        offset: 0,
        limit: 1,
    };
    let first = read.source_evidence(&editor, &request).unwrap();
    assert_eq!(first["total"], 2);
    assert_eq!(first["topics"][0]["topic"]["id"], topic_ids[0]);
    request.offset = first["next_offset"].as_u64().unwrap() as usize;
    let second = read.source_evidence(&editor, &request).unwrap();
    assert_eq!(second["topics"][0]["topic"]["id"], topic_ids[2]);
    assert!(second["next_offset"].is_null());
    request.offset = 0;
    request.object_targets = vec![Target::Element { id: 1 }];
    request.limit = 10;
    let source = read.source_evidence(&editor, &request).unwrap();
    assert_eq!(source["topics"].as_array().unwrap().len(), 2);
    request.object_targets.clear();
    let all = read.source_evidence(&editor, &request).unwrap();
    assert_eq!(all["total"], 3);
    request.object_targets = vec![Target::Element { id: 0 }];
    assert_eq!(
        read.source_evidence(&editor, &request).unwrap_err().code,
        "INVALID_OBJECT_TARGETS"
    );
    request.object_targets = vec![Target::Element { id: 1 }; 2];
    assert_eq!(
        read.source_evidence(&editor, &request).unwrap_err().code,
        "INVALID_OBJECT_TARGETS"
    );
    request.object_targets = vec![Target::Element { id: 1 }];
    request.topics = false;
    assert_eq!(
        read.source_evidence(&editor, &request).unwrap_err().code,
        "INVALID_OBJECT_TARGETS"
    );
    // An explicitly recorded generated-result identity remains retrievable even
    // after the live association/result is absent. Relevance is not authority.
    let mut history = editor.project.evidence.topics[1].clone();
    history.exchanges[0].assignment = Some(atelier_bim::evidence::topic_tasks::Assignment {
        id: storage::new_document_identity(),
        mode: atelier_bim::evidence::topic_tasks::Mode::CreateLinkedSpace {
            mass: 2,
            source_sha256: mass_spaces::element_stamp(&editor.project, &editor.project.elements[1])
                .unwrap(),
            story: 0,
            option: 1,
            zone: 99,
            operation_id: storage::new_document_identity(),
        },
        events: vec![],
    });
    assert!(history.related_to(&editor.project, &[Target::Element { id: 99 }]));
    assert_eq!(
        history.exchanges[0].result_status(&editor.project),
        Some("Linked concept space is missing")
    );
    assert!(!history.related_to(&editor.project, &[Target::Element { id: 100 }]));
    let mut linked = history.clone();
    if let atelier_bim::evidence::topic_tasks::Mode::CreateLinkedSpace { mass, zone, .. } =
        &mut linked.exchanges[0].assignment.as_mut().unwrap().mode
    {
        *mass = 1;
        *zone = 3;
    }
    assert_eq!(
        linked.exchanges[0].result_status(&editor.project),
        Some("Mass-space link is current")
    );
    let mut modified = editor.project.clone();
    if let Shape::Zone { spec } = &mut modified
        .elements
        .iter_mut()
        .find(|e| e.id == 3)
        .unwrap()
        .shape
    {
        spec.height = 2.9;
    }
    assert_eq!(
        linked.exchanges[0].result_status(&modified),
        Some("Linked space has user edits")
    );
    assert_eq!(
        editor.project, before,
        "retrieval must never capture/rebase history or generate a task"
    );
}
