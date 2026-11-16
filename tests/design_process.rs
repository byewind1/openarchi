mod common;
use atelier_bim::{
    command::ModelCommand,
    design_process::{self as dp, Entry, Statement, StatementKind, service},
    editor::Editor,
    model::{ModelAdapter, Point, Shape},
    storage,
};
fn id() -> String {
    storage::new_document_identity()
}
#[test]
fn manual_drawn_mass_candidates_need_no_image_and_preserve_shared_history() {
    use atelier_bim::{
        concept_mass::ConceptMassSpec,
        model::{Material, Project},
    };
    let mut p = Project::default();
    let a = p.add(
        0,
        Material::Plaster,
        Shape::ConceptMass {
            spec: ConceptMassSpec {
                arcs: vec![],
                hole_arcs: vec![],
                outline: vec![
                    Point::new(1., 1.),
                    Point::new(9., 1.),
                    Point::new(9., 5.),
                    Point::new(1., 5.),
                ],
                holes: vec![],
                height: 9.,
                bottom_offset: 0.,
            },
        },
    );
    let unrelated = p.add(
        0,
        Material::Plaster,
        Shape::Polyline {
            points: vec![Point::new(0., 0.), Point::new(20., 0.)],
            arcs: vec![],
        },
    );
    let mut e = Editor::new(p);
    let before = e.project.clone();
    let mut d = service::Draft {
        name: "Direct mass exploration".into(),
        question: "Compare two heights of existing authored mass; no survey or approval".into(),
        statements: vec![Statement {
            kind: StatementKind::Assumption,
            text: "All mass dimensions are conceptual design assumptions".into(),
        }],
        candidate_names: ["Nine metres".into(), "Six metres".into()],
        source_ids: vec![a],
        entry: Entry::ManualInterface,
    };
    let request = id();
    let change = service::prepare_create(&e, &request, d.clone())
        .expect("drawn mass should not require a fabricated image source");
    assert_eq!(e.project, before);
    assert!(change.project().drawings.pages.is_empty());
    e.commit_change(&request, &change).unwrap();
    let task = e.project.design_process.task().unwrap().clone();
    assert!(task.is_mass());
    assert!(task.intent.is_none());
    assert_eq!(task.source_mass_ids, vec![a]);
    assert!(task.source_zone_ids.is_empty());
    assert_eq!(
        e.project.elements.iter().find(|x| x.id == unrelated),
        before.elements.iter().find(|x| x.id == unrelated)
    );
    let original = e.project.clone();
    let mass = task.candidates[0].initial_mass_ids[0];
    let change = service::prepare_edit(&e, mass, Point::new(0., 0.), Some(6.)).unwrap();
    e.commit_change(&id(), &change).unwrap();
    let comparison = task.compare(&e.project).unwrap();
    assert!(comparison.different);
    assert_eq!(
        comparison.candidates[0]
            .mass_metrics
            .as_ref()
            .unwrap()
            .envelope_volume_sum_m3,
        192.
    );
    assert_eq!(
        comparison.candidates[1]
            .mass_metrics
            .as_ref()
            .unwrap()
            .envelope_volume_sum_m3,
        288.
    );
    let edited = e.project.clone();
    e.undo();
    assert_eq!(e.project, original);
    e.redo();
    assert_eq!(e.project, edited);
    let bytes = storage::JsonAdapter.export(&edited).unwrap();
    let mut j: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(j["schema_version"], 25);
    assert_eq!(storage::JsonAdapter.import(&bytes).unwrap(), edited);
    for marker in [10, 14, 19, 22, 23, 24] {
        j["schema_version"] = marker.into();
        assert!(
            storage::JsonAdapter
                .import(&serde_json::to_vec(&j).unwrap())
                .unwrap_err()
                .contains("version 25")
        );
    }
    let fresh = Editor::new(before.clone());
    d.entry = Entry::AgentProposal;
    assert!(service::prepare_create(&fresh, &id(), d.clone()).is_err());
    d.entry = Entry::ManualInterface;
    d.source_ids.push(unrelated);
    assert!(service::prepare_create(&fresh, &id(), d).is_err());
    assert_eq!(fresh.project, before);
}
fn draft(editor: &Editor) -> service::Draft {
    service::Draft {
        name: "空间组织试验".into(),
        question: "比较保存的空间边界与一个明确标识的试算方向".into(),
        statements: vec![
            Statement {
                kind: StatementKind::Assumption,
                text: "平移/净高仅用于受控试算，不是测绘或甲方确认".into(),
            },
            Statement {
                kind: StatementKind::Unknown,
                text: "本任务未判断法规或日照合规".into(),
            },
        ],
        candidate_names: ["保留边界".into(), "试算变化".into()],
        source_ids: editor
            .project
            .elements
            .iter()
            .filter(|e| e.story == 0 && matches!(e.shape, Shape::Zone { .. }))
            .map(|e| e.id)
            .collect(),
        entry: Entry::ManualInterface,
    }
}
fn create(editor: &mut Editor) -> dp::Task {
    let request = id();
    let before = editor.project.clone();
    let preview = service::prepare_create(editor, &request, draft(editor)).unwrap();
    assert_eq!(editor.project, before);
    assert_eq!(preview.project().elements.len(), before.elements.len() + 18);
    let receipt = editor.commit_change(&request, &preview).unwrap();
    assert!(receipt.design_process_changed);
    assert_eq!(editor.undo.len(), 1);
    editor.project.design_process.task().unwrap().clone()
}
fn activate(editor: &mut Editor, option: u32) {
    let p = service::prepare_activate(editor, Some(option)).unwrap();
    editor.commit_change(&id(), &p).unwrap();
}
#[test]
fn actual_example_candidates_are_editable_isolated_and_choice_versions_survive_reopen() {
    let mut editor = Editor::new(common::synthetic_building_98());
    let original = editor.project.clone();
    let task = create(&mut editor);
    for old in &original.elements {
        let mut expected = old.clone();
        if task.source_zone_ids.contains(&old.id) {
            expected.design_option = Some(task.baseline_option);
        }
        assert_eq!(
            editor.project.elements.iter().find(|e| e.id == old.id),
            Some(&expected)
        );
    }
    let a = task.candidates[0].option;
    let b = task.candidates[1].option;
    let ai = task.candidates[0].initial_zone_ids[0];
    let bi = task.candidates[1].initial_zone_ids[0];
    let same = task.compare(&editor.project).unwrap();
    assert!(!same.different);
    assert!(
        service::prepare_choose(
            &editor,
            &id(),
            &task.id,
            a,
            "not ready".into(),
            same.fingerprint,
            Entry::ManualInterface,
            "fixture manual path; not client approval".into()
        )
        .is_err()
    );
    let untouched_a = editor
        .project
        .elements
        .iter()
        .find(|e| e.id == ai)
        .unwrap()
        .clone();
    activate(&mut editor, b);
    let preview = service::prepare_edit(&editor, bi, Point::new(0.25, 0.), Some(3.15)).unwrap();
    editor.commit_change(&id(), &preview).unwrap();
    assert_eq!(
        editor.project.elements.iter().find(|e| e.id == ai),
        Some(&untouched_a)
    );
    let comparison = task.compare(&editor.project).unwrap();
    assert!(comparison.different && comparison.basis_current);
    let decision = id();
    let preview = service::prepare_choose(
        &editor,
        &decision,
        &task.id,
        a,
        "保留边界，另一个方向仅供试算且有待检查".into(),
        comparison.fingerprint,
        Entry::ManualInterface,
        "controlled fixture through manual service; not client approval".into(),
    )
    .unwrap();
    assert!(editor.project.design_process.decisions.is_empty());
    editor.commit_change(&decision, &preview).unwrap();
    assert_eq!(editor.project.plan_options.active_design_option, Some(a));
    assert_eq!(
        editor.project.design_process.current_choice,
        Some(decision.clone())
    );
    let frozen = editor.project.design_process.decisions[0].clone();
    editor.undo();
    assert!(editor.project.design_process.current_choice.is_none());
    assert_eq!(editor.project.design_process.decisions, [frozen.clone()]);
    editor.redo();
    assert_eq!(editor.project.design_process.current_choice, Some(decision));
    assert_eq!(editor.project.design_process.decisions, [frozen.clone()]);
    activate(&mut editor, b);
    let p = service::prepare_edit(&editor, bi, Point::new(0.1, 0.), None).unwrap();
    editor.commit_change(&id(), &p).unwrap();
    assert_eq!(editor.project.design_process.decisions, [frozen.clone()]);
    assert_ne!(
        task.compare(&editor.project).unwrap().fingerprint,
        frozen.comparison.fingerprint
    );
    let bytes = storage::JsonAdapter.export(&editor.project).unwrap();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&bytes).unwrap()["schema_version"],
        7
    );
    assert_eq!(storage::JsonAdapter.import(&bytes).unwrap(), editor.project);
    let mut old: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    for version in 2..=6 {
        old["schema_version"] = version.into();
        assert!(
            storage::JsonAdapter
                .import(&serde_json::to_vec(&old).unwrap())
                .is_err()
        );
    }
    let directory = std::env::temp_dir().join(format!("oa-design-process-{}", id()));
    std::fs::create_dir(&directory).unwrap();
    let file = directory.join("saved.json");
    storage::save_document(&editor.project, &file, editor.project_identity()).unwrap();
    let reopened = Editor::from_stored_document(&file).unwrap();
    assert_eq!(reopened.project, editor.project);
    assert_eq!(reopened.project_identity(), editor.project_identity());
    assert_eq!(reopened.project.design_process.decisions[0], frozen);
    std::fs::remove_dir_all(directory).unwrap();
}
#[test]
fn candidate_scope_locks_legacy_commits_and_common_changes_are_explicit() {
    let mut editor = Editor::new(common::synthetic_building_98());
    let task = create(&mut editor);
    let a = task.candidates[0].option;
    let b = task.candidates[1].option;
    let own = task.candidates[0].initial_zone_ids[0];
    let other = task.candidates[1].initial_zone_ids[0];
    let common = task.source_zone_ids[0];
    let before = editor.project.clone();
    assert!(
        editor
            .apply(ModelCommand::MoveElements {
                ids: vec![other],
                offset: Point::new(1., 0.)
            })
            .is_err()
    );
    assert_eq!(editor.project, before);
    assert!(
        editor
            .apply(ModelCommand::MoveElements {
                ids: vec![common],
                offset: Point::new(1., 0.)
            })
            .is_err()
    );
    assert_eq!(editor.project, before);
    editor
        .project
        .elements
        .iter_mut()
        .find(|e| e.id == other)
        .unwrap()
        .translate(Point::new(1., 0.));
    assert!(!editor.commit_checked(before.clone()));
    assert_eq!(editor.project, before);
    let p = service::prepare_lock(&editor, &task.id, a, true).unwrap();
    editor.commit_change(&id(), &p).unwrap();
    assert!(service::prepare_edit(&editor, own, Point::new(0.1, 0.), None).is_err());
    let p = service::prepare_lock(&editor, &task.id, a, false).unwrap();
    editor.commit_change(&id(), &p).unwrap();
    assert!(service::prepare_edit(&editor, own, Point::new(0.1, 0.), None).is_ok());
    let area = task.compare(&editor.project).unwrap().candidates[1]
        .metrics
        .clone();
    let new = editor
        .place_zone(atelier_bim::zones::ZoneSpec {
            surface: Default::default(),
            purpose: "controlled".into(),
            outline: vec![
                Point::new(30., 30.),
                Point::new(32., 30.),
                Point::new(32., 32.),
                Point::new(30., 32.),
            ],
            holes: vec![],
            height: 3.,
            bottom_offset: 0.,
            label_position: None,
        })
        .unwrap();
    assert_eq!(
        editor
            .project
            .elements
            .iter()
            .find(|e| e.id == new)
            .unwrap()
            .design_option,
        Some(a)
    );
    assert_eq!(
        task.compare(&editor.project).unwrap().candidates[1].metrics,
        area
    );
    assert!(task.compare(&editor.project).unwrap().basis_current);
    activate(&mut editor, task.baseline_option);
    editor
        .apply(ModelCommand::MoveElements {
            ids: vec![common],
            offset: Point::new(0.1, 0.),
        })
        .unwrap();
    assert!(!task.compare(&editor.project).unwrap().basis_current);
    activate(&mut editor, b);
    let cmp = task.compare(&editor.project).unwrap();
    assert!(
        service::prepare_choose(
            &editor,
            &id(),
            &task.id,
            b,
            "stale".into(),
            cmp.fingerprint,
            Entry::ManualInterface,
            "fixture".into()
        )
        .is_err()
    );
}
#[test]
fn undo_creation_retains_history_without_replaying_recorded_request() {
    let mut editor = Editor::new(common::synthetic_building_98());
    let original = editor.project.clone();
    let task = create(&mut editor);
    editor.undo();
    assert_eq!(editor.project.elements, original.elements);
    assert_eq!(editor.project.design_options, original.design_options);
    assert!(editor.project.design_process.current_task.is_none());
    assert_eq!(editor.project.design_process.tasks, [task.clone()]);
    assert!(task.compare(&editor.project).is_err());
    assert!(service::prepare_create(&editor, &task.id, draft(&editor)).is_err());
    editor.redo();
    assert_eq!(editor.project.design_process.tasks, [task]);
    assert!(
        editor
            .project
            .design_process
            .task()
            .unwrap()
            .compare(&editor.project)
            .unwrap()
            .basis_current
    );
}
#[test]
fn stale_create_and_failed_choice_never_consume_live_ids_or_audit_records() {
    let mut editor = Editor::new(common::synthetic_building_98());
    let request = id();
    let preview = service::prepare_create(&editor, &request, draft(&editor)).unwrap();
    editor.project.name = "User inserted edit".into();
    let user = editor.project.clone();
    assert!(editor.commit_change(&request, &preview).is_err());
    assert_eq!(editor.project, user);
    assert!(editor.project.design_process.is_empty());
    let task = create(&mut editor);
    activate(&mut editor, task.candidates[1].option);
    let p = service::prepare_edit(
        &editor,
        task.candidates[1].initial_zone_ids[0],
        Point::new(0.1, 0.),
        None,
    )
    .unwrap();
    editor.commit_change(&id(), &p).unwrap();
    let cmp = task.compare(&editor.project).unwrap();
    let before = editor.project.clone();
    let command = ModelCommand::DesignProcess {
        edit: dp::Edit::Choose {
            id: id(),
            task_id: task.id,
            option: task.candidates[0].option,
            reason: "x".repeat(4097),
            entry: Entry::ManualInterface,
            operator_label: "fixture".into(),
            expected_comparison: cmp.fingerprint,
            reviewed_basis: None,
            created_unix: service::now(),
        },
    };
    assert!(editor.apply(command).is_err());
    assert_eq!(editor.project, before);
}

#[test]
fn corresponding_space_uses_declared_copy_identity_and_never_guesses_a_replacement() {
    let mut editor = Editor::new(common::synthetic_building_98());
    let task = create(&mut editor);
    let a = task.candidates[0].option;
    let b = task.candidates[1].option;
    for index in 0..task.source_zone_ids.len() {
        assert_eq!(
            task.corresponding_space(
                &editor.project,
                task.candidates[0].initial_zone_ids[index],
                b
            ),
            Some(task.candidates[1].initial_zone_ids[index])
        );
        assert_eq!(
            task.corresponding_space(
                &editor.project,
                task.candidates[1].initial_zone_ids[index],
                a
            ),
            Some(task.candidates[0].initial_zone_ids[index])
        );
        assert_eq!(
            task.corresponding_space(
                &editor.project,
                task.candidates[0].initial_zone_ids[index],
                task.baseline_option
            ),
            Some(task.source_zone_ids[index])
        );
    }
    let source = task.candidates[0].initial_zone_ids[2];
    let removed = task.candidates[1].initial_zone_ids[2];
    activate(&mut editor, b);
    let mut replacement = editor
        .project
        .elements
        .iter()
        .find(|e| e.id == removed)
        .unwrap()
        .clone();
    editor
        .apply(ModelCommand::RemoveElements { ids: vec![removed] })
        .unwrap();
    replacement.id = editor.project.next_id;
    editor
        .apply(ModelCommand::AddElement {
            element: replacement,
        })
        .unwrap();
    assert_eq!(
        task.corresponding_space(&editor.project, source, b),
        None,
        "same name and geometry are not correspondence authority"
    );
    assert_eq!(task.corresponding_space(&editor.project, 0, a), None);
}

#[test]
fn current_candidate_filters_plan_hit_hover_snap_marquee_and_actual_model_scene() {
    use atelier_bim::{
        geometry,
        render::{CpuSceneCache, SceneView, prepare_scene},
        view_settings::DesignOptionScope,
    };
    let mut editor = Editor::new(common::synthetic_building_98());
    let task = create(&mut editor);
    let a = task.candidates[0].option;
    let b = task.candidates[1].option;
    let a_id = task.candidates[0].initial_zone_ids[0];
    let b_id = task.candidates[1].initial_zone_ids[0];
    let a_point = match &editor
        .project
        .elements
        .iter()
        .find(|e| e.id == a_id)
        .unwrap()
        .shape
    {
        Shape::Zone { spec } => spec.label_point(),
        _ => unreachable!(),
    };
    assert_eq!(
        geometry::hit_plan(&editor.project, 0, a_point, 0.01),
        Some(a_id)
    );
    activate(&mut editor, b);
    let edit = service::prepare_edit(&editor, b_id, Point::new(30., 0.), Some(4.)).unwrap();
    editor.commit_change(&id(), &edit).unwrap();
    let b_element = editor
        .project
        .elements
        .iter()
        .find(|e| e.id == b_id)
        .unwrap()
        .clone();
    let b_node = match &b_element.shape {
        Shape::Zone { spec } => spec.outline[0],
        _ => unreachable!(),
    };
    activate(&mut editor, a);
    assert_eq!(geometry::hit_plan(&editor.project, 0, b_node, 0.01), None);
    assert_eq!(
        geometry::plan_hover_kind(&editor.project, 0, b_node, 0.01, 0.01),
        None
    );
    assert_eq!(
        geometry::snap(&editor.project, 0, b_node, 0.01, true).1,
        geometry::SnapKind::Grid
    );
    editor.marquee = atelier_bim::editor::MarqueeArea::new(
        vec![
            Point::new(-1., -1.),
            Point::new(60., -1.),
            Point::new(60., 30.),
            Point::new(-1., 30.),
        ],
        atelier_bim::editor::MarqueeScope::CurrentStory,
        0,
    );
    editor.select_all_in_marquee();
    assert!(
        !editor
            .selection_ids()
            .iter()
            .any(|id| task.candidates[1].initial_zone_ids.contains(id)
                || task.source_zone_ids.contains(id))
    );
    let view = SceneView {
        selected: &[],
        hovered: None,
        story: Some(0),
        marquee: None,
        outside_marquee: false,
        selection_only: false,
    };
    let mut scenes = CpuSceneCache::default();
    let mut fingerprints = vec![];
    for option in [a, b] {
        if editor.project.plan_options.active_design_option != Some(option) {
            activate(&mut editor, option);
        }
        let cached = editor
            .with_geometry_read(|read| {
                scenes
                    .prepare(&editor.project, view, read)
                    .content_fingerprint()
            })
            .unwrap();
        assert_eq!(
            cached,
            prepare_scene(&editor.project, view, None).content_fingerprint()
        );
        let mut oracle = editor.project.clone();
        oracle
            .elements
            .retain(|e| DesignOptionScope::Option { id: option }.contains(e.design_option));
        assert_eq!(
            cached,
            prepare_scene(&oracle, view, None).content_fingerprint(),
            "actual CPU output must exclude hidden candidate geometry"
        );
        editor
            .with_geometry_read(|read| {
                for candidate in &task.candidates {
                    assert_eq!(
                        read.model_includes(
                            candidate.initial_zone_ids[0],
                            geometry::ModelViewFilter {
                                story: Some(0),
                                ..Default::default()
                            }
                        )
                        .unwrap(),
                        candidate.option == option
                    );
                }
            })
            .unwrap();
        fingerprints.push(cached);
    }
    assert_ne!(fingerprints[0], fingerprints[1]);
}

#[test]
fn changed_basis_choice_needs_exact_manual_review_and_keeps_original_task_and_unknowns() {
    let mut editor = Editor::new(common::synthetic_building_98());
    let task = create(&mut editor);
    let a = task.candidates[0].option;
    let source = task.candidates[0].initial_zone_ids[0];
    let change = service::prepare_edit(&editor, source, Point::new(0.1, 0.), None).unwrap();
    editor.commit_change(&id(), &change).unwrap();
    // Genuine shared condition change: this is not erased from the basis.
    let mut rules = editor.project.rules.context.clone();
    rules.stage = "changed concept-stage requirement".into();
    editor
        .apply(ModelCommand::Rules {
            edit: atelier_bim::rules::RuleEdit::SetContext {
                expected: editor.project.rules.context.clone(),
                context: rules,
            },
        })
        .unwrap();
    let comparison = task.compare(&editor.project).unwrap();
    assert!(!comparison.basis_current);
    assert!(
        service::prepare_choose(
            &editor,
            &id(),
            &task.id,
            a,
            "review".into(),
            comparison.fingerprint.clone(),
            Entry::ManualInterface,
            "manual".into()
        )
        .is_err()
    );
    let command = |entry, basis: String| ModelCommand::DesignProcess {
        edit: dp::Edit::Choose {
            id: id(),
            task_id: task.id.clone(),
            option: a,
            reason: String::new(),
            entry,
            operator_label: "manual acceptance fixture".into(),
            expected_comparison: comparison.fingerprint.clone(),
            reviewed_basis: Some(basis),
            created_unix: service::now(),
        },
    };
    let before = editor.project.clone();
    assert!(
        editor
            .prepare_change(
                editor.revision,
                vec![command(Entry::ManualInterface, "f".repeat(64))]
            )
            .is_err()
    );
    assert!(
        editor
            .prepare_change(
                editor.revision,
                vec![command(
                    Entry::AgentProposal,
                    comparison.current_basis_sha256.clone()
                )]
            )
            .is_err()
    );
    assert_eq!(editor.project, before);
    let prepared = editor
        .prepare_change(
            editor.revision,
            vec![command(
                Entry::ManualInterface,
                comparison.current_basis_sha256.clone(),
            )],
        )
        .unwrap();
    editor.commit_change(&id(), &prepared).unwrap();
    let decision = editor.project.design_process.decisions.last().unwrap();
    assert!(!decision.comparison.basis_current);
    assert_eq!(
        decision.comparison.current_basis_sha256,
        comparison.current_basis_sha256
    );
    assert_eq!(editor.project.design_process.tasks[0], task);
    assert_eq!(editor.project.elements, before.elements);
    assert_eq!(editor.project.rules, before.rules);
    let bytes = storage::JsonAdapter.export(&editor.project).unwrap();
    let mut raw: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(raw["schema_version"], 22);
    let round = storage::JsonAdapter.import(&bytes).unwrap();
    assert_eq!(round, editor.project);
    raw["schema_version"] = serde_json::json!(7);
    assert!(
        storage::JsonAdapter
            .import(&serde_json::to_vec(&raw).unwrap())
            .unwrap_err()
            .contains("version 22")
    );
    let prepared = editor
        .prepare_change(
            editor.revision,
            vec![command(
                Entry::ManualInterface,
                comparison.current_basis_sha256,
            )],
        )
        .unwrap();
    editor
        .apply(ModelCommand::MoveElements {
            ids: vec![source],
            offset: Point::new(0.1, 0.),
        })
        .unwrap();
    assert!(
        editor.commit_change(&id(), &prepared).is_err(),
        "a local edit cannot be overwritten by the old reviewed proposal"
    );
}
