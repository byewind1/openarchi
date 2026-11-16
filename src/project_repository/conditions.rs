//! One reviewed project-preparation batch; reuse each original-source validator
//! under one repository lock, rather than recursively locking the repository.
use super::*;
use crate::{command::ModelCommand, rules::RuleEdit};

pub struct ConditionsCommitGuard {
    _lock: Option<Lock>,
    document_id: String,
    condition_hash: String,
    commands: Vec<ModelCommand>,
}
fn stamp(p: &Project) -> Result<String, String> {
    crate::automation_journal::canonical_hash(&(&p.site, &p.rules, &p.program, &p.evidence))
}
pub fn needs_sources(commands: &[ModelCommand]) -> bool {
    commands.iter().any(|c| {
        matches!(
            c,
            ModelCommand::Program {
                edit: crate::program::ProgramEdit::Import { .. }
            } | ModelCommand::Evidence {
                edit: crate::evidence::EvidenceEdit::Put { .. }
            } | ModelCommand::Terrain {
                edit: crate::site::terrain::Edit::Import { .. }
            } | ModelCommand::ApplySiteBoundary { .. }
                | ModelCommand::ApplySiteControlRegion { .. }
                | ModelCommand::Rules {
                    edit: RuleEdit::AddSource { .. }
                        | RuleEdit::PutRequirement { .. }
                        | RuleEdit::PutLifecycle { .. }
                        | RuleEdit::PutCondition { .. }
                }
        )
    })
}
impl ConditionsCommitGuard {
    pub(crate) fn verify(&self, editor: &Editor, commands: &[ModelCommand]) -> Result<(), String> {
        if self.document_id != editor.project_identity()
            || self.commands != commands
            || self.condition_hash != stamp(&editor.project)?
        {
            return Err("Project conditions or reviewed changes have changed".into());
        }
        Ok(())
    }
}
pub fn guard_conditions(
    repository: Option<&Repository>,
    document_id: &str,
    project: &Project,
    commands: &[ModelCommand],
) -> Result<ConditionsCommitGuard, String> {
    if commands.is_empty() || commands.len() > 200 {
        return Err("Choose between 1 and 200 project condition changes".into());
    }
    let lock = if needs_sources(commands) {
        let r =
            repository.ok_or("Open the bound project repository to verify source-based changes")?;
        if r.manifest.document_id != document_id {
            return Err("Project repository belongs to another document".into());
        }
        let lock = Lock::acquire(&r.root)?;
        r.unchanged()?;
        Some(lock)
    } else {
        None
    };
    let mut book = project.rules.clone();
    for command in commands {
        match command {
            ModelCommand::SetSiteContext { .. }
            | ModelCommand::DetachSiteBoundary
            | ModelCommand::DetachSiteControlRegion => {}
            ModelCommand::Program { edit } => {
                if let crate::program::ProgramEdit::Import { proposal } = edit {
                    repository.unwrap().verify_program_unlocked(proposal)?;
                }
            }
            ModelCommand::Evidence { edit } => {
                if let crate::evidence::EvidenceEdit::Put { draft, .. } = edit {
                    repository
                        .unwrap()
                        .verify_evidence_unlocked(std::slice::from_ref(&draft.quote))?;
                }
            }
            ModelCommand::Terrain { edit } => {
                if let crate::site::terrain::Edit::Import { proposal } = edit {
                    repository.unwrap().verify_terrain_unlocked(proposal)?;
                }
            }
            ModelCommand::ApplySiteBoundary { proposal }
            | ModelCommand::ApplySiteControlRegion { proposal } => {
                repository.unwrap().verify_boundary_unlocked(proposal)?
            }
            ModelCommand::Rules { edit } => {
                if matches!(
                    edit,
                    RuleEdit::AddSource { .. }
                        | RuleEdit::PutRequirement { .. }
                        | RuleEdit::PutLifecycle { .. }
                        | RuleEdit::PutCondition { .. }
                ) {
                    repository.unwrap().verify_rule_unlocked(&book, edit)?;
                }
                book = crate::rules::apply(&book, edit)?;
            }
            _ => return Err("Command is outside project condition editing".into()),
        }
    }
    Ok(ConditionsCommitGuard {
        _lock: lock,
        document_id: document_id.into(),
        condition_hash: stamp(project)?,
        commands: commands.to_vec(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        application_host::ApplicationHost,
        program::{AreaBasis, AreaUnit, Mapping},
        rules::{Origin, RuleSource},
        storage,
    };
    fn root() -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "oa-conditions-{}",
            storage::new_document_identity()
        ));
        fs::create_dir(&p).unwrap();
        p
    }
    fn plain(host: &ApplicationHost) -> Vec<ModelCommand> {
        let mut site = host.document.editor.project.site.clone();
        site.control_tolerance_m = Some(0.02);
        let mut context = host.document.editor.project.rules.context.clone();
        context.stage = "Authored concept exploration".into();
        vec![
            ModelCommand::SetSiteContext { context: site },
            ModelCommand::Rules {
                edit: RuleEdit::SetContext {
                    expected: host.document.editor.project.rules.context.clone(),
                    context,
                },
            },
        ]
    }
    #[test]
    fn conditions_batch_native_admission_is_exact_durable_and_one_undo_without_remote_grant() {
        let p = root();
        let mut host = ApplicationHost::new(storage::startup_example());
        host.configure_operation_journal(Some(p.join("journal")))
            .unwrap();
        let before = host.document.editor.project.clone();
        let commands = plain(&host);
        let guard = guard_conditions(
            None,
            host.document.editor.project_identity(),
            &before,
            &commands,
        )
        .unwrap();
        let prepared = host
            .document
            .editor
            .prepare_conditions_change(0, commands.clone(), &guard)
            .unwrap();
        assert!(prepared.verified_conditions_command());
        assert_eq!(host.document.editor.project, before);
        let mut changed = commands.clone();
        if let ModelCommand::SetSiteContext { context } = &mut changed[0] {
            context.control_tolerance_m = Some(0.03);
        }
        assert!(
            host.document
                .editor
                .prepare_conditions_change(0, changed, &guard)
                .is_err()
        );
        let original_target = host.operation_target();
        let response = host.confirm_task_prepared(
            host.context(),
            "conditions-native-original",
            "Edit project conditions",
            std::rc::Rc::new(prepared),
        );
        assert_eq!(response["result"]["status"], "committed", "{response}");
        assert_eq!(host.document.editor.undo.len(), 1);
        assert_eq!(host.document.editor.project.elements, before.elements);
        assert_eq!(host.document.editor.project.site.true_north, None);
        let saved = host.document.editor.project.clone();
        host.document.editor.undo();
        assert_eq!(host.document.editor.project, before);
        host.document.editor.redo();
        assert_eq!(host.document.editor.project, saved);
        let original = host.inspect_operation(original_target, "conditions-native-original");
        assert_eq!(original["result"]["status"], "committed");
        assert_eq!(original["result"]["current_effect"], "AfterImage");
        drop(guard);
        drop(host);
        fs::remove_dir_all(p).unwrap();
    }
    #[test]
    fn conditions_batch_rechecks_multiple_sources_under_one_lock_and_rejects_forgery() {
        let p = root();
        let host = ApplicationHost::new(storage::startup_example());
        let mut repo =
            Repository::create_for_editor(&p.join("repo"), &host.document.editor).unwrap();
        let file = p.join("rooms.csv");
        fs::write(&file, "ID,Name,Area,Count\nA,Room,12,1\n").unwrap();
        let id = repo.add_source(&file).unwrap();
        let mapping = Mapping {
            sheet: "CSV".into(),
            first_row: 2,
            last_row: 2,
            key_column: 1,
            label_column: 2,
            area_column: Some(3),
            quantity_column: Some(4),
            scope_column: None,
            note_column: None,
            area_unit: AreaUnit::SquareMetre,
            area_basis: AreaBasis::PerUnit,
        };
        let a = repo
            .preview_program(&id, "a".into(), "Authored A".into(), mapping.clone(), None)
            .unwrap();
        let b = repo
            .preview_program(&id, "b".into(), "Authored B".into(), mapping, None)
            .unwrap();
        let text = p.join("original.txt");
        fs::write(&text, "A synthetic source, not a regulation.\n").unwrap();
        let text_id = repo.add_source(&text).unwrap();
        let source = RuleSource {
            id: "authored-source".into(),
            title: "Authored source".into(),
            code: "TEST".into(),
            edition: "v1".into(),
            publisher: "Synthetic fixture".into(),
            jurisdiction: String::new(),
            published_on: String::new(),
            effective_on: String::new(),
            original_sha256: text_id,
            origin_url: String::new(),
            origin: Origin::Unverified,
            verification_note: "Not measured or approved".into(),
            replaces: vec![],
            same_edition_as: None,
        };
        let mut commands = plain(&host);
        commands.extend([
            ModelCommand::Program {
                edit: crate::program::ProgramEdit::Import {
                    proposal: a.clone(),
                },
            },
            ModelCommand::Program {
                edit: crate::program::ProgramEdit::Import { proposal: b },
            },
            ModelCommand::Rules {
                edit: RuleEdit::AddSource { source },
            },
        ]);
        assert!(
            host.document
                .editor
                .prepare_change(0, commands.clone())
                .is_err()
        );
        assert!(
            guard_conditions(
                None,
                host.document.editor.project_identity(),
                &host.document.editor.project,
                &commands
            )
            .is_err()
        );
        let guard = guard_conditions(
            Some(&repo),
            host.document.editor.project_identity(),
            &host.document.editor.project,
            &commands,
        )
        .unwrap();
        assert!(repo.reject_source(&id, "locked fixture").is_err());
        let prepared = host
            .document
            .editor
            .prepare_conditions_change(0, commands.clone(), &guard)
            .unwrap();
        assert_eq!(prepared.project().program.sheets.len(), 2);
        assert_eq!(prepared.project().rules.sources.len(), 1);
        drop(guard);
        let mut forged = commands.clone();
        if let ModelCommand::Program {
            edit: crate::program::ProgramEdit::Import { proposal },
        } = &mut forged[2]
        {
            proposal.sheet.rows[0].values.label = "Forged".into();
            proposal.sheet.rows[0].imported = proposal.sheet.rows[0].values.clone();
        }
        assert!(
            guard_conditions(
                Some(&repo),
                host.document.editor.project_identity(),
                &host.document.editor.project,
                &forged
            )
            .is_err()
        );
        let single = repo.guard_program_commit(&a).unwrap();
        drop(single);
        repo.reject_source(&id, "Rejected synthetic source")
            .unwrap();
        assert!(
            guard_conditions(
                Some(&repo),
                host.document.editor.project_identity(),
                &host.document.editor.project,
                &commands
            )
            .is_err()
        );
        drop(repo);
        fs::remove_dir_all(p).unwrap();
    }
    #[test]
    fn conditions_batch_cannot_admit_geometry_or_forge_a_native_condition_marker() {
        let p = root();
        let mut host = ApplicationHost::new(storage::startup_example());
        host.configure_operation_journal(Some(p.join("journal")))
            .unwrap();
        let before = host.document.editor.project.clone();
        let commands = plain(&host);
        let forged = host
            .document
            .editor
            .prepare_change(0, commands.clone())
            .unwrap();
        assert!(!forged.verified_conditions_command());
        let response = host.confirm_task_prepared(
            host.context(),
            "not-conditions-admitted",
            "Edit project conditions",
            std::rc::Rc::new(forged),
        );
        assert_eq!(response["error"]["code"], "DESIGN_TASK_MISMATCH");
        assert_eq!(host.document.editor.project, before);
        let ids = vec![before.elements[0].id];
        let geometry = vec![ModelCommand::RemoveElements { ids }];
        assert!(
            guard_conditions(
                None,
                host.document.editor.project_identity(),
                &before,
                &geometry
            )
            .is_err()
        );
        assert_eq!(host.document.editor.project, before);
        drop(host);
        fs::remove_dir_all(p).unwrap();
    }
}
