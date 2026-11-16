mod common;
use atelier_bim::{
    application_host::{ApplicationHost, HostCommand},
    command::ModelCommand,
    evidence::{Link, Target},
    model::ModelAdapter,
    project_repository::{Repository, rules::inspect},
    rules::*,
    storage::{self, JsonAdapter},
};
use std::{fs, path::PathBuf};
fn root() -> PathBuf {
    let p = std::env::temp_dir().join(format!("oa-rules-{}", storage::new_document_identity()));
    fs::create_dir(&p).unwrap();
    p
}
fn source(id: &str, sha: &str) -> RuleSource {
    RuleSource {
        id: id.into(),
        title: "Controlled original".into(),
        code: "TEST-ONLY".into(),
        edition: id.into(),
        publisher: "Synthetic fixture".into(),
        jurisdiction: "fixture jurisdiction".into(),
        published_on: "2026-01-01".into(),
        effective_on: "2026-02-01".into(),
        original_sha256: sha.into(),
        origin_url: String::new(),
        origin: Origin::Unverified,
        verification_note: "Synthetic; not a real regulation or approval".into(),
        replaces: vec![],
        same_edition_as: None,
    }
}
fn requirement(book: &RuleBook) -> Requirement {
    Requirement {
        id: "condition".into(),
        title: "Check survey".into(),
        topic: Topic::TerrainNeighbors,
        citation: Citation {
            source_id: "v1".into(),
            locator: "text/line[1]".into(),
            text: "Actual terrain remains unknown.".into(),
        },
        interpretation: "Do not assume flat ground".into(),
        decision: Decision {
            applicability: Applicability::Unknown,
            adoption: Adoption::Pending,
            rationale: String::new(),
            missing: "No measured terrain or neighbor window heights".into(),
            basis_sha256: book.basis("v1").unwrap(),
        },
        links: vec![],
    }
}
fn apply(host: &mut ApplicationHost, repo: &Repository, edit: RuleEdit) {
    let guard = repo
        .guard_rule_edit(&host.document.editor.project.rules, &edit)
        .unwrap();
    let receipt = host
        .apply_rule_edit(
            host.context(),
            &storage::new_document_identity(),
            edit,
            guard,
        )
        .unwrap();
    assert!(receipt.rules_changed);
    assert!(receipt.changed_ids.is_empty());
}
#[test]
fn cited_rule_workflow_is_shared_guarded_undoable_and_portable() {
    let directory = root();
    let mut host = ApplicationHost::new(common::synthetic_building_98());
    let original = host.document.editor.project.clone();
    let original_ids = original.elements.iter().map(|e| e.id).collect::<Vec<_>>();
    let mut repo =
        Repository::create_for_editor(&directory.join("repo"), &host.document.editor).unwrap();
    let file = directory.join("conditions.txt");
    fs::write(&file, "Actual terrain remains unknown.\nSecond condition\n").unwrap();
    let id = repo.add_source(&file).unwrap();
    let edit = RuleEdit::AddSource {
        source: source("v1", &id),
    };
    assert!(
        host.dispatch(
            host.context(),
            HostCommand::Apply {
                request_id: "bypass".into(),
                commands: vec![ModelCommand::Rules { edit: edit.clone() }]
            }
        )
        .unwrap_err()
        .contains("admission")
    );
    assert_eq!(host.document.editor.project, original);
    let inspected = repo.inspect_rule_original(&id).unwrap();
    assert_eq!(
        inspected.document.quote("text/line[1]"),
        Some("Actual terrain remains unknown.")
    );
    let guard = repo.guard_rule_edit(&original.rules, &edit).unwrap();
    assert_eq!(host.document.editor.project, original);
    host.apply_rule_edit(host.context(), "source-v1", edit, guard)
        .unwrap();
    let mut row = requirement(&host.document.editor.project.rules);
    let target = Target::Element {
        id: original_ids[0],
    };
    row.links.push(Link {
        baseline_sha256: target.current_hash(&host.document.editor.project).unwrap(),
        target,
    });
    apply(
        &mut host,
        &repo,
        RuleEdit::PutRequirement {
            expected: None,
            requirement: row.clone(),
        },
    );
    assert_eq!(host.document.editor.project.elements, original.elements);
    assert_eq!(host.document.editor.project.rules.requirements.len(), 1);
    assert!(
        host.document
            .editor
            .project
            .rules
            .decision_state(&row)
            .contains("incomplete")
    );
    assert_eq!(
        host.document
            .editor
            .project
            .rules
            .report(&host.document.editor.project)["compliance_verdict"],
        "not_evaluated"
    );
    host.dispatch(host.context(), HostCommand::Undo).unwrap();
    assert!(host.document.editor.project.rules.requirements.is_empty());
    host.dispatch(host.context(), HostCommand::Redo).unwrap();
    assert_eq!(host.document.editor.project.rules.requirements[0], row);
    let saved = directory.join("rule-case.json");
    storage::save_document(
        &host.document.editor.project,
        &saved,
        &host.document.editor.project_identity(),
    )
    .unwrap();
    let value: serde_json::Value = serde_json::from_slice(&fs::read(&saved).unwrap()).unwrap();
    assert_eq!(value["schema_version"], 6);
    let reopened = JsonAdapter.import(&fs::read(&saved).unwrap()).unwrap();
    assert_eq!(reopened, host.document.editor.project);
    repo.checkpoint(
        &host.document.editor.project,
        &host.document.editor.project_identity(),
        host.document.editor.revision,
    )
    .unwrap();
    let moved = directory.join("moved");
    repo.copy_to(&moved).unwrap();
    let fresh = Repository::open(&moved).unwrap();
    fresh
        .verify_rule_citation(&reopened.rules, &row.citation)
        .unwrap();
    let original_text = fresh.inspect_rule_original(&id).unwrap();
    assert_eq!(
        original_text.document.quote(&row.citation.locator),
        Some(row.citation.text.as_str())
    );
    for old_version in [2, 3, 4, 5] {
        let mut bad = value.clone();
        bad["schema_version"] = old_version.into();
        assert!(
            JsonAdapter
                .import(&serde_json::to_vec(&bad).unwrap())
                .is_err()
        );
    }
    let mut changed = row.clone();
    changed.citation.text = "Invented survey completed".into();
    let edit = RuleEdit::PutRequirement {
        expected: Some(row.clone()),
        requirement: changed,
    };
    assert!(
        fresh
            .guard_rule_edit(&reopened.rules, &edit)
            .err()
            .unwrap()
            .contains("differs")
    );
    assert_eq!(host.document.editor.project.elements, original.elements);
    fs::remove_dir_all(directory).unwrap();
}
#[test]
fn context_revision_conflict_and_object_change_never_become_compliance_pass() {
    let mut project = common::synthetic_building_98();
    let book = &mut project.rules;
    book.sources.push(source("v1", &"a".repeat(64)));
    let row = requirement(book);
    book.requirements.push(row.clone());
    book.validate().unwrap();
    let context = ProjectContext {
        jurisdiction: "fixture jurisdiction".into(),
        uses: "Unapproved training use".into(),
        assessment_date: "2026-11-09".into(),
        stage: "Predesign".into(),
    };
    *book = atelier_bim::rules::apply(
        book,
        &RuleEdit::SetContext {
            expected: book.context.clone(),
            context,
        },
    )
    .unwrap();
    assert_eq!(book.decision_state(&row), "Rule decision is stale");
    let mut current = row.clone();
    current.decision.basis_sha256 = book.basis("v1").unwrap();
    current.decision.applicability = Applicability::Conflict;
    current.decision.rationale = "Survey conflicts with declared boundary area".into();
    current.decision.adoption = Adoption::Adopted;
    assert!(
        atelier_bim::rules::apply(
            book,
            &RuleEdit::PutRequirement {
                expected: Some(row.clone()),
                requirement: current.clone()
            }
        )
        .is_err()
    );
    current.decision.adoption = Adoption::Pending;
    *book = atelier_bim::rules::apply(
        book,
        &RuleEdit::PutRequirement {
            expected: Some(row),
            requirement: current.clone(),
        },
    )
    .unwrap();
    assert_eq!(book.decision_state(&current), "Applicability conflict");
    let mut second = source("v2", &"b".repeat(64));
    second.replaces = vec!["v1".into()];
    *book = atelier_bim::rules::apply(book, &RuleEdit::AddSource { source: second }).unwrap();
    assert_eq!(book.requirements[0].citation.source_id, "v1");
    assert_eq!(book.decision_state(&current), "Rule decision is stale");
    assert!(
        atelier_bim::rules::apply(
            book,
            &RuleEdit::RemoveSource {
                expected: book.sources[0].clone()
            }
        )
        .is_err()
    );
    let mut cycle = book.clone();
    cycle.sources[0].replaces.push("v2".into());
    assert!(cycle.validate().unwrap_err().contains("Cyclic"));
    let mut cross = book.clone();
    cross.sources[1].code = "DIFFERENT".into();
    assert!(cross.validate().unwrap_err().contains("relation"));
    let report = project.rules.report(&project);
    assert_eq!(report["coverage_is_exhaustive"], false);
    assert_eq!(report["coverage"].as_array().unwrap().len(), 13);
    assert!(
        report["coverage"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["status"] == "missing")
    );
}
#[test]
fn guard_cannot_be_reused_for_other_document_edit_or_after_source_corruption() {
    let directory = root();
    let mut host = ApplicationHost::new(common::synthetic_building_98());
    let mut repo =
        Repository::create_for_editor(&directory.join("repo"), &host.document.editor).unwrap();
    let file = directory.join("original.txt");
    fs::write(&file, "Original").unwrap();
    let id = repo.add_source(&file).unwrap();
    let edit = RuleEdit::AddSource {
        source: source("v1", &id),
    };
    let guard = repo
        .guard_rule_edit(&host.document.editor.project.rules, &edit)
        .unwrap();
    let mut other = ApplicationHost::new(host.document.editor.project.clone());
    assert!(
        other
            .apply_rule_edit(other.context(), "wrong-doc", edit.clone(), guard)
            .unwrap_err()
            .contains("admission")
    );
    let guard = repo
        .guard_rule_edit(&host.document.editor.project.rules, &edit)
        .unwrap();
    let changed = RuleEdit::AddSource {
        source: source("v2", &id),
    };
    assert!(
        host.apply_rule_edit(host.context(), "wrong-edit", changed, guard)
            .unwrap_err()
            .contains("admission")
    );
    assert!(host.document.editor.undo.is_empty());
    let blob = repo.root().join(&repo.manifest.sources[0].blob.path);
    fs::write(blob, "Corrupted").unwrap();
    assert!(
        repo.guard_rule_edit(&host.document.editor.project.rules, &edit)
            .is_err()
    );
    fs::remove_dir_all(directory).unwrap();
}
#[test]
fn static_html_has_stable_locators_and_never_executes_script_or_claims_visibility() {
    let raw=br#"<!doctype html><html><head><script>network()</script></head><body><h1>Title</h1><p>Rule <b>A</b> &amp; B<script>hidden()</script></p><p hidden>Unrendered source</p></body></html>"#;
    let a = inspect(raw, "html").unwrap();
    assert_eq!(a.paragraphs[0].text, "Title");
    assert_eq!(a.paragraphs[1].text, "Rule A & B");
    assert_eq!(a.quote("html/static[2]"), Some("Rule A & B"));
    assert_eq!(a.paragraphs[2].text, "Unrendered source");
    assert_eq!(a, inspect(raw, "html").unwrap());
    assert!(inspect(&[255], "txt").is_err());
    assert!(inspect(b"%PDF-1.7", "pdf").is_err());
    assert!(inspect(b"<script>only script</script>", "html").is_err());
}
#[test]
fn malformed_context_dates_and_unreasoned_na_are_rejected() {
    let mut book = RuleBook::default();
    book.sources.push(source("v1", &"a".repeat(64)));
    for bad in [
        "2026-02-29",
        "2026-13-01",
        "0000-01-01",
        "2026-1-01",
        "é026-01-01",
    ] {
        let mut b = book.clone();
        b.context.assessment_date = bad.into();
        assert!(b.validate().is_err(), "{bad}")
    }
    let mut row = requirement(&book);
    row.decision.applicability = Applicability::NotApplicable;
    assert!(
        atelier_bim::rules::apply(
            &book,
            &RuleEdit::PutRequirement {
                expected: None,
                requirement: row
            }
        )
        .is_err()
    );
    book.context.assessment_date = "2024-02-29".into();
    book.validate().unwrap();
}

#[test]
fn effective_dates_and_duplicate_editions_require_review() {
    let mut book = RuleBook::default();
    book.context = ProjectContext {
        jurisdiction: "fixture".into(),
        uses: "fixture".into(),
        assessment_date: "2026-01-01".into(),
        stage: "fixture".into(),
    };
    book.sources.push(source("v1", &"a".repeat(64)));
    let mut row = requirement(&book);
    assert_eq!(
        book.decision_state(&row),
        "Rule is not effective on the assessment date"
    );
    row.decision.applicability = Applicability::Applicable;
    row.decision.adoption = Adoption::Adopted;
    row.decision.rationale = "Unsupported future applicability".into();
    assert!(
        atelier_bim::rules::apply(
            &book,
            &RuleEdit::PutRequirement {
                expected: None,
                requirement: row.clone()
            }
        )
        .unwrap_err()
        .contains("effective date")
    );
    book.sources[0].effective_on.clear();
    row.decision.basis_sha256 = book.basis("v1").unwrap();
    assert_eq!(book.decision_state(&row), "Rule effective date is unknown");
    assert!(
        atelier_bim::rules::apply(
            &book,
            &RuleEdit::PutRequirement {
                expected: None,
                requirement: row
            }
        )
        .is_err()
    );
    let mut duplicate = book.sources[0].clone();
    duplicate.id = "different-id".into();
    assert!(
        atelier_bim::rules::apply(&book, &RuleEdit::AddSource { source: duplicate })
            .unwrap_err()
            .contains("duplicate revision")
    );
}
#[test]
fn paged_rule_reads_refuse_stale_and_wrong_target_and_expose_unknowns() {
    use atelier_bim::{
        automation::{ReadService, RulesRequest, RulesSection},
        editor::Editor,
    };
    let mut project = common::synthetic_building_98();
    project.rules.sources.push(source("v1", &"a".repeat(64)));
    let row = requirement(&project.rules);
    project.rules.requirements.push(row.clone());
    let editor = Editor::new(project);
    let service = ReadService::new("rules-reader".into());
    let mut request = RulesRequest {
        target: service.target(&editor),
        expected_revision: editor.revision,
        section: RulesSection::Requirements,
        offset: 0,
        limit: 1,
    };
    let before = editor.project.clone();
    let page = service.project_rules(&editor, &request).unwrap();
    assert_eq!(page["total"], 1);
    assert_eq!(
        page["entries"][0]["requirement"],
        serde_json::to_value(&row).unwrap()
    );
    assert_eq!(page["compliance_verdict"], "not_evaluated");
    assert_eq!(page["next_offset"], serde_json::Value::Null);
    assert_eq!(page["coverage"].as_array().unwrap().len(), 13);
    assert_eq!(editor.project, before);
    request.section = RulesSection::Sources;
    let page = service.project_rules(&editor, &request).unwrap();
    assert_eq!(page["entries"][0]["source"]["id"], "v1");
    request.limit = 11;
    assert_eq!(
        service.project_rules(&editor, &request).unwrap_err().code,
        "INVALID_LIMIT"
    );
    request.limit = 1;
    request.offset = 2;
    assert!(service.project_rules(&editor, &request).is_err());
    request.offset = 0;
    request.expected_revision += 1;
    assert_eq!(
        service.project_rules(&editor, &request).unwrap_err().code,
        "REVISION_CONFLICT"
    );
    request.expected_revision = editor.revision;
    request.target.instance_id = "other".into();
    assert_eq!(
        service.project_rules(&editor, &request).unwrap_err().code,
        "TARGET_MISMATCH"
    );
}

#[test]
fn linked_object_changes_and_rejected_originals_stay_explicit() {
    let directory = root();
    let mut host = ApplicationHost::new(common::synthetic_building_98());
    let mut repo =
        Repository::create_for_editor(&directory.join("repo"), &host.document.editor).unwrap();
    let file = directory.join("original.txt");
    fs::write(&file, "Actual terrain remains unknown.").unwrap();
    let sha = repo.add_source(&file).unwrap();
    apply(
        &mut host,
        &repo,
        RuleEdit::AddSource {
            source: source("v1", &sha),
        },
    );
    let mut row = requirement(&host.document.editor.project.rules);
    let target = Target::Element {
        id: host.document.editor.project.elements[0].id,
    };
    row.links.push(Link {
        baseline_sha256: target.current_hash(&host.document.editor.project).unwrap(),
        target,
    });
    apply(
        &mut host,
        &repo,
        RuleEdit::PutRequirement {
            expected: None,
            requirement: row.clone(),
        },
    );
    let mut changed = host.document.editor.project.clone();
    changed.elements[0].name.push_str(" changed");
    assert_eq!(
        changed.rules.report(&changed)["requirements"][0]["decision_state"],
        "Linked rule target changed or missing"
    );
    changed.elements.remove(0);
    assert_eq!(
        changed.rules.report(&changed)["requirements"][0]["links"][0]["status"],
        "missing"
    );
    let changed_editor = atelier_bim::editor::Editor::new(changed.clone());
    let reader = atelier_bim::automation::ReadService::new("linked-rule-read".into());
    let page = reader
        .project_rules(
            &changed_editor,
            &atelier_bim::automation::RulesRequest {
                target: reader.target(&changed_editor),
                expected_revision: changed_editor.revision,
                section: atelier_bim::automation::RulesSection::Requirements,
                offset: 0,
                limit: 1,
            },
        )
        .unwrap();
    assert_eq!(
        page["entries"][0]["decision_state"],
        "Linked rule target changed or missing"
    );
    repo.reject_source(&sha, "Controlled rejection").unwrap();
    assert!(repo.inspect_rule_original(&sha).is_ok());
    assert!(
        repo.verify_rule_citation(&host.document.editor.project.rules, &row.citation)
            .unwrap_err()
            .contains("Rejected")
    );
    let mut edited = row.clone();
    edited.interpretation = "Attempt after source rejection".into();
    assert!(
        repo.guard_rule_edit(
            &host.document.editor.project.rules,
            &RuleEdit::PutRequirement {
                expected: Some(row),
                requirement: edited
            }
        )
        .err()
        .unwrap()
        .contains("Rejected")
    );
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn alternate_original_is_same_edition_not_a_fabricated_revision() {
    let mut book = RuleBook::default();
    let original = source("v1", &"a".repeat(64));
    book.sources.push(original.clone());
    let old_basis = book.basis("v1").unwrap();
    let mut pdf = original.clone();
    pdf.id = "pdf-original".into();
    pdf.original_sha256 = "b".repeat(64);
    pdf.same_edition_as = Some(original.id.clone());
    book = atelier_bim::rules::apply(
        &book,
        &RuleEdit::AddSource {
            source: pdf.clone(),
        },
    )
    .unwrap();
    assert_eq!(old_basis, book.basis("v1").unwrap());
    let pdf_basis = book.basis("pdf-original").unwrap();
    assert!(
        atelier_bim::rules::apply(&book, &RuleEdit::RemoveSource { expected: original }).is_err()
    );
    let mut wrong = book.clone();
    wrong.sources[1].effective_on = "2026-09-01".into();
    assert!(wrong.validate().unwrap_err().contains("same-edition"));
    let mut chain = book.clone();
    chain.sources[0].same_edition_as = Some("pdf-original".into());
    assert!(chain.validate().is_err());
    let mut next = source("v2", &"c".repeat(64));
    next.replaces = vec!["pdf-original".into()];
    book = atelier_bim::rules::apply(&book, &RuleEdit::AddSource { source: next }).unwrap();
    assert_ne!(old_basis, book.basis("v1").unwrap());
    assert_ne!(pdf_basis, book.basis("pdf-original").unwrap());
    let mut cycle = book.clone();
    cycle.sources[0].replaces = vec!["v2".into()];
    assert!(cycle.validate().unwrap_err().contains("Cyclic"));
}
#[cfg(unix)]
#[test]
fn pdf_provider_failure_timeout_overflow_and_warning_are_not_empty_successes() {
    use atelier_bim::project_repository::rules::pdf::inspect_with_provider;
    use std::{os::unix::fs::PermissionsExt, time::Duration};
    let root = root();
    let path = root.join("controlled-provider");
    for (body, expected) in [
        ("exit 1", "failed"),
        (
            "printf 'untrusted warning' >&2; printf 'text\\f'",
            "warnings",
        ),
        ("sleep 2", "timed out"),
        ("printf 'text without page boundary'", "page boundaries"),
        ("printf '\\f'", "no extractable text"),
        ("head -c 5000000 /dev/zero", "output limits"),
    ] {
        let script = format!(
            "#!/bin/sh\nif [ \"$1\" = '-v' ]; then echo 'pdftotext version controlled-test-fixture' >&2; exit 0; fi\n{body}\n"
        );
        fs::write(&path, script).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        let error =
            inspect_with_provider(b"%PDF-controlled-test", &path, Duration::from_millis(100))
                .unwrap_err();
        assert!(error.contains(expected), "{body}: {error}");
    }
    fs::remove_dir_all(root).unwrap();
}
