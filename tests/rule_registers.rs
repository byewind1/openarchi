mod common;
use atelier_bim::{
    application_host::{ApplicationHost, HostCommand},
    automation::{ReadService, RulesRequest, RulesSection},
    command::ModelCommand,
    evidence::{Link, Target},
    model::ModelAdapter,
    project_repository::Repository,
    rules::*,
    storage::{self, JsonAdapter},
};
use std::{fs, path::PathBuf};
fn source(id: &str, code: &str, sha: &str) -> RuleSource {
    RuleSource {
        id: id.into(),
        title: format!("Synthetic {id}"),
        code: code.into(),
        edition: "fixture-1".into(),
        publisher: "Synthetic fixture".into(),
        jurisdiction: "fixture".into(),
        published_on: "2026-01-01".into(),
        effective_on: "2026-02-01".into(),
        original_sha256: sha.into(),
        origin_url: String::new(),
        origin: Origin::Unverified,
        verification_note: "Controlled data, not real law or project approval".into(),
        replaces: vec![],
        same_edition_as: None,
    }
}
fn fixture_context() -> ProjectContext {
    ProjectContext {
        jurisdiction: "fixture".into(),
        uses: "fixture".into(),
        assessment_date: "2026-11-09".into(),
        stage: "fixture".into(),
    }
}
fn fixture() -> atelier_bim::model::Project {
    let mut p = common::synthetic_building_98();
    p.rules.context = fixture_context();
    p.rules.sources = vec![
        source("old", "OLD-1", &"a".repeat(64)),
        source("new", "NEW-2", &"b".repeat(64)),
        source("notice", "NOTICE-3", &"c".repeat(64)),
    ];
    p
}
fn citation() -> Citation {
    Citation {
        source_id: "notice".into(),
        locator: "text/line[1]".into(),
        text: "Controlled notice withdraws or replaces old provisions.".into(),
    }
}
fn record() -> LifecycleRecord {
    LifecycleRecord {
        id: "event".into(),
        title: "Controlled cross-code replacement".into(),
        subject: DocumentRef::Recorded {
            source_id: "old".into(),
        },
        successor: Some(DocumentRef::Recorded {
            source_id: "new".into(),
        }),
        kind: LifecycleKind::Replaced,
        scope: LifecycleScope::Whole,
        effective_on: "2026-08-01".into(),
        role: EvidenceRole::Instrument,
        citation: citation(),
        corroboration: vec![],
        scope_note: "Synthetic whole-document event; no real legal conclusion".into(),
        missing: String::new(),
    }
}
fn requirement(p: &atelier_bim::model::Project) -> Requirement {
    let target = Target::Element {
        id: p.elements[0].id,
    };
    Requirement {
        id: "req".into(),
        title: "Controlled requirement".into(),
        topic: Topic::Planning,
        citation: Citation {
            source_id: "old".into(),
            locator: "text/line[1]".into(),
            text: "Controlled old clause.".into(),
        },
        interpretation: "Synthetic".into(),
        decision: Decision {
            applicability: Applicability::Applicable,
            adoption: Adoption::Adopted,
            rationale: "Controlled fixture, not real approval".into(),
            missing: String::new(),
            basis_sha256: p.rules.basis("old").unwrap(),
        },
        links: vec![Link {
            baseline_sha256: target.current_hash(p).unwrap(),
            target,
        }],
    }
}
fn need(book: &RuleBook) -> ConditionNeed {
    let mut n = ConditionNeed {
        id: "need".into(),
        title: "Controlled review".into(),
        topic: Topic::Planning,
        question: "Is the cited fixture current?".into(),
        originals: vec![DocumentRef::Recorded {
            source_id: "old".into(),
        }],
        requirement_ids: vec!["req".into()],
        citation: None,
        status: ConditionStatus::Addressed,
        rationale: "Synthetic review, not real approval".into(),
        missing: String::new(),
        basis_sha256: String::new(),
    };
    n.basis_sha256 = book.condition_basis(&n).unwrap();
    n
}
fn temp() -> PathBuf {
    let p = std::env::temp_dir().join(format!("oa-registers-{}", storage::new_document_identity()));
    fs::create_dir(&p).unwrap();
    p
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
    assert!(receipt.rules_changed && receipt.changed_ids.is_empty());
}
#[test]
fn cited_cross_code_replacement_and_same_edition_invalidate_without_migration() {
    let mut p = fixture();
    let mut alias = p.rules.sources[0].clone();
    alias.id = "alias".into();
    alias.original_sha256 = "d".repeat(64);
    alias.same_edition_as = Some("old".into());
    p.rules.sources.push(alias);
    let mut req = requirement(&p);
    p.rules.requirements.push(req.clone());
    let alias_basis = p.rules.basis("alias").unwrap();
    let new_basis = p.rules.basis("new").unwrap();
    let event = record();
    p.rules = atelier_bim::rules::apply(
        &p.rules,
        &RuleEdit::PutLifecycle {
            expected: None,
            record: event.clone(),
        },
    )
    .unwrap();
    assert_ne!(p.rules.basis("old").unwrap(), req.decision.basis_sha256);
    assert_ne!(p.rules.basis("alias").unwrap(), alias_basis);
    assert_eq!(p.rules.basis("new").unwrap(), new_basis);
    assert_eq!(p.rules.requirements[0], req);
    assert_eq!(p.rules.decision_state(&req), "Rule decision is stale");
    req.decision.basis_sha256 = p.rules.basis("old").unwrap();
    assert_eq!(
        p.rules.decision_state(&req),
        "Declared replaced on assessment date"
    );
    assert!(
        atelier_bim::rules::apply(
            &p.rules,
            &RuleEdit::PutRequirement {
                expected: Some(p.rules.requirements[0].clone()),
                requirement: req
            }
        )
        .unwrap_err()
        .contains("lifecycle")
    );
    assert!(
        atelier_bim::rules::apply(
            &p.rules,
            &RuleEdit::RemoveSource {
                expected: p.rules.sources[2].clone()
            }
        )
        .is_err()
    );
    let mut reverse = event.clone();
    reverse.id = "reverse".into();
    reverse.subject = DocumentRef::Recorded {
        source_id: "new".into(),
    };
    reverse.successor = Some(DocumentRef::Recorded {
        source_id: "alias".into(),
    });
    assert!(
        atelier_bim::rules::apply(
            &p.rules,
            &RuleEdit::PutLifecycle {
                expected: None,
                record: reverse
            }
        )
        .unwrap_err()
        .contains("Cyclic")
    );
    let mut same = event;
    same.id = "same".into();
    same.successor = Some(DocumentRef::Recorded {
        source_id: "alias".into(),
    });
    assert!(
        atelier_bim::rules::apply(
            &p.rules,
            &RuleEdit::PutLifecycle {
                expected: None,
                record: same
            }
        )
        .unwrap_err()
        .contains("same edition")
    );
}
#[test]
fn explanations_partial_unknown_and_future_dates_never_become_whole_current_repeals() {
    let mut p = fixture();
    let mut event = record();
    event.role = EvidenceRole::Explanation;
    event.missing = "Formal instrument unavailable".into();
    p.rules = atelier_bim::rules::apply(
        &p.rules,
        &RuleEdit::PutLifecycle {
            expected: None,
            record: event.clone(),
        },
    )
    .unwrap();
    assert_eq!(
        p.rules.lifecycle_state(&event),
        "Lifecycle report needs the original instrument"
    );
    event.role = EvidenceRole::Instrument;
    event.scope = LifecycleScope::Partial;
    assert_eq!(
        p.rules.lifecycle_state(&event),
        "Partial or unresolved lifecycle scope needs review"
    );
    event.scope = LifecycleScope::Whole;
    event.effective_on.clear();
    assert_eq!(
        p.rules.lifecycle_state(&event),
        "Lifecycle effective date is unknown"
    );
    event.effective_on = "2026-08".into();
    assert!(
        atelier_bim::rules::apply(
            &RuleBook {
                lifecycle: vec![],
                ..p.rules.clone()
            },
            &RuleEdit::PutLifecycle {
                expected: None,
                record: event.clone()
            }
        )
        .is_err()
    );
    event.effective_on = "2027-08-01".into();
    p.rules.lifecycle = vec![event.clone()];
    assert_eq!(
        p.rules.lifecycle_state(&event),
        "Lifecycle event is after the assessment date"
    );
    assert!(p.rules.lifecycle_restriction("old").is_none());
    event.subject = DocumentRef::Missing {
        title: "Unacquired old original".into(),
        code_hint: "OLD-1".into(),
        edition_hint: String::new(),
        jurisdiction_hint: String::new(),
    };
    event.missing = "Original target has not been acquired or bound".into();
    p.rules.lifecycle = vec![event.clone()];
    p.rules.validate().unwrap();
    assert_eq!(
        p.rules.lifecycle_state(&event),
        "Lifecycle subject original not acquired"
    );
    assert_eq!(
        p.rules.lifecycle_restriction("old"),
        Some("Potential lifecycle target is not bound to this original")
    );
    assert!(matches!(
        p.rules.lifecycle[0].subject,
        DocumentRef::Missing { .. }
    ));
}
#[test]
fn condition_reviews_preserve_history_after_context_lifecycle_and_object_change() {
    let mut p = fixture();
    let req = requirement(&p);
    p.rules.requirements.push(req);
    let condition = need(&p.rules);
    p.rules = atelier_bim::rules::apply(
        &p.rules,
        &RuleEdit::PutCondition {
            expected: None,
            condition: condition.clone(),
        },
    )
    .unwrap();
    assert_eq!(
        p.rules.condition_state(&p, &condition),
        "Declared condition addressed"
    );
    p.validate().unwrap();
    let mut context = p.rules.context.clone();
    context.stage = "Changed stage".into();
    p.rules = atelier_bim::rules::apply(
        &p.rules,
        &RuleEdit::SetContext {
            expected: p.rules.context.clone(),
            context,
        },
    )
    .unwrap();
    p.validate().unwrap();
    assert_eq!(
        p.rules.condition_state(&p, &condition),
        "Condition review is stale"
    );
    p.rules.context = fixture_context();
    p.elements[0].name.push_str(" changed");
    assert_eq!(
        p.rules.condition_state(&p, &condition),
        "Linked requirements changed or remain unreviewed"
    );
    assert!(
        p.rules
            .validate_edit_against_project(
                &p,
                &RuleEdit::PutCondition {
                    expected: Some(condition.clone()),
                    condition: condition.clone()
                }
            )
            .is_err()
    );
    p.validate().unwrap();
    p.rules = atelier_bim::rules::apply(
        &p.rules,
        &RuleEdit::PutLifecycle {
            expected: None,
            record: record(),
        },
    )
    .unwrap();
    p.validate().unwrap();
    assert_eq!(
        p.rules.condition_state(&p, &condition),
        "Condition review is stale"
    );
    assert_eq!(p.rules.conditions[0], condition);
    assert!(
        atelier_bim::rules::apply(
            &p.rules,
            &RuleEdit::RemoveRequirement {
                expected: p.rules.requirements[0].clone()
            }
        )
        .unwrap_err()
        .contains("Referenced")
    );
}
#[test]
fn missing_conditions_na_and_pending_links_cannot_be_declared_addressed() {
    let mut p = fixture();
    p.rules.requirements.push(requirement(&p));
    let mut n = need(&p.rules);
    n.originals.push(DocumentRef::Missing {
        title: "Approved project conditions".into(),
        code_hint: String::new(),
        edition_hint: String::new(),
        jurisdiction_hint: String::new(),
    });
    n.basis_sha256 = p.rules.condition_basis(&n).unwrap();
    assert!(
        atelier_bim::rules::apply(
            &p.rules,
            &RuleEdit::PutCondition {
                expected: None,
                condition: n.clone()
            }
        )
        .is_err()
    );
    n.status = ConditionStatus::Missing;
    n.missing = "Original approval missing".into();
    p.rules = atelier_bim::rules::apply(
        &p.rules,
        &RuleEdit::PutCondition {
            expected: None,
            condition: n.clone(),
        },
    )
    .unwrap();
    assert_eq!(
        p.rules.condition_state(&p, &n),
        "Condition original not acquired"
    );
    let mut na = n.clone();
    na.status = ConditionStatus::NotApplicable;
    na.rationale = "No source to justify NA".into();
    assert!(
        atelier_bim::rules::apply(
            &p.rules,
            &RuleEdit::PutCondition {
                expected: Some(n),
                condition: na
            }
        )
        .is_err()
    );
    let mut pending = p.rules.requirements[0].clone();
    pending.decision.applicability = Applicability::Unknown;
    pending.decision.adoption = Adoption::Pending;
    pending.decision.missing = "Actual use is unknown".into();
    p.rules.requirements[0] = pending;
    let mut closed = need(&p.rules);
    closed.missing.clear();
    assert!(
        atelier_bim::rules::apply(
            &p.rules,
            &RuleEdit::PutCondition {
                expected: Some(p.rules.conditions[0].clone()),
                condition: closed
            }
        )
        .unwrap_err()
        .contains("pending")
    );
}
#[test]
fn repository_guards_all_evidence_and_condition_commits_undo_save_move_and_page() {
    let directory = temp();
    let mut host = ApplicationHost::new(common::synthetic_building_98());
    let original = host.document.editor.project.clone();
    let mut repo =
        Repository::create_for_editor(&directory.join("repo"), &host.document.editor).unwrap();
    for (id, code, text) in [
        ("old", "OLD-1", "Controlled old clause."),
        ("new", "NEW-2", "Controlled new clause."),
        (
            "notice",
            "NOTICE-3",
            "Controlled notice withdraws or replaces old provisions.\nAdditional date and scope block.",
        ),
    ] {
        let path = directory.join(format!("{id}.txt"));
        fs::write(&path, text).unwrap();
        let sha = repo.add_source(&path).unwrap();
        apply(
            &mut host,
            &repo,
            RuleEdit::AddSource {
                source: source(id, code, &sha),
            },
        );
    }
    apply(
        &mut host,
        &repo,
        RuleEdit::SetContext {
            expected: ProjectContext::default(),
            context: fixture_context(),
        },
    );
    let req = requirement(&host.document.editor.project);
    apply(
        &mut host,
        &repo,
        RuleEdit::PutRequirement {
            expected: None,
            requirement: req,
        },
    );
    let condition = need(&host.document.editor.project.rules);
    apply(
        &mut host,
        &repo,
        RuleEdit::PutCondition {
            expected: None,
            condition: condition.clone(),
        },
    );
    let mut event = record();
    event.corroboration.push(Citation {
        source_id: "notice".into(),
        locator: "text/line[2]".into(),
        text: "Additional date and scope block.".into(),
    });
    let before = host.document.editor.project.clone();
    assert!(
        host.dispatch(
            host.context(),
            HostCommand::Apply {
                request_id: "bypass".into(),
                commands: vec![ModelCommand::Rules {
                    edit: RuleEdit::PutLifecycle {
                        expected: None,
                        record: event.clone()
                    }
                }]
            }
        )
        .unwrap_err()
        .contains("admission")
    );
    let mut forged = event.clone();
    forged.corroboration[0].text = "Invented date".into();
    assert!(
        repo.guard_rule_edit(
            &before.rules,
            &RuleEdit::PutLifecycle {
                expected: None,
                record: forged
            }
        )
        .is_err()
    );
    assert_eq!(host.document.editor.project, before);
    apply(
        &mut host,
        &repo,
        RuleEdit::PutLifecycle {
            expected: None,
            record: event,
        },
    );
    assert_eq!(host.document.editor.project.elements, original.elements);
    assert_eq!(
        host.document
            .editor
            .project
            .rules
            .condition_state(&host.document.editor.project, &condition),
        "Condition review is stale"
    );
    host.dispatch(host.context(), HostCommand::Undo).unwrap();
    assert_eq!(host.document.editor.project, before);
    host.dispatch(host.context(), HostCommand::Redo).unwrap();
    let after = host.document.editor.project.clone();
    let bytes = JsonAdapter.export(&after).unwrap();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&bytes).unwrap()["schema_version"],
        6
    );
    assert_eq!(JsonAdapter.import(&bytes).unwrap(), after);
    repo.checkpoint(
        &after,
        host.document.editor.project_identity(),
        host.document.editor.revision,
    )
    .unwrap();
    let moved = repo.copy_to(&directory.join("moved")).unwrap();
    assert_eq!(moved.load_model().unwrap().project, after);
    let read = ReadService::new("register-read".into());
    for (section, total) in [(RulesSection::Lifecycle, 1), (RulesSection::Conditions, 1)] {
        let q = RulesRequest {
            target: read.target(&host.document.editor),
            expected_revision: host.document.editor.revision,
            section,
            offset: 0,
            limit: 1,
        };
        let page = read.project_rules(&host.document.editor, &q).unwrap();
        assert_eq!(page["total"], total);
        assert_eq!(page["entries"].as_array().unwrap().len(), 1);
        assert_eq!(page["compliance_verdict"], "not_evaluated");
        assert_eq!(page["coverage_is_exhaustive"], false);
    }
    assert_eq!(host.document.editor.project, after);
    fs::remove_dir_all(directory).unwrap();
}
