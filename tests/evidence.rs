mod common;
use atelier_bim::{
    application_host::{ApplicationHost, HostCommand},
    automation_registry::{DiscoveredBatch, NativeBatch},
    command::ModelCommand,
    editor::Editor,
    evidence::{Draft, EvidenceEdit, Kind, LinkStatus, Note, Quote, Target},
    model::{ModelAdapter, Point},
    project_repository::Repository,
    storage,
};
use std::{
    io::{Cursor, Write},
    path::PathBuf,
};
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path =
            std::env::temp_dir().join(format!("oa-evidence-{}", storage::new_document_identity()));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn docx() -> Vec<u8> {
    let mut zip = zip::ZipWriter::new(Cursor::new(vec![]));
    for (name, text) in [
        (
            "_rels/.rels",
            r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="main" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "[Content_Types].xml",
            r#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>Neighbor windows and heights require a survey.</w:t></w:r></w:p></w:body></w:document>"#,
        ),
    ] {
        zip.start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(text.as_bytes()).unwrap();
    }
    zip.finish().unwrap().into_inner()
}
fn draft(project: &atelier_bim::model::Project) -> Draft {
    Draft {
        id: "survey-question".into(),
        title: "Obtain measured neighbor conditions".into(),
        kind: Kind::Question,
        quote: Quote {
            source_sha256: "a".repeat(64),
            locator: "body/p[1]".into(),
            text: "Neighbor windows and heights require a survey.".into(),
        },
        annotation: "Not a measured height or approval".into(),
        targets: vec![Target::Element {
            id: project.elements[0].id,
        }],
        refresh_links: false,
    }
}
fn put(draft: Draft, expected_previous: Option<Note>) -> ModelCommand {
    ModelCommand::Evidence {
        edit: EvidenceEdit::Put {
            draft,
            expected_previous,
        },
    }
}
#[test]
fn link_changes_and_missing_objects_do_not_block_normal_model_editing_or_erase_quotes() {
    let mut editor = Editor::new(common::synthetic_building_98());
    let d = draft(&editor.project);
    let id = editor.project.elements[0].id;
    editor.apply(put(d, None)).unwrap();
    let note = editor.project.evidence.notes[0].clone();
    assert_eq!(note.links[0].status(&editor.project), LinkStatus::Unchanged);
    editor.selected_ids = vec![id];
    editor.move_selection(Point::new(1., 0.));
    assert_eq!(note.links[0].status(&editor.project), LinkStatus::Changed);
    assert_eq!(
        editor.project.evidence.status(&editor.project)["notes"][0]["links"][0]["status"],
        "changed"
    );
    assert_eq!(editor.project.evidence.notes[0], note);
    editor.undo();
    assert_eq!(note.links[0].status(&editor.project), LinkStatus::Unchanged);
    editor
        .apply(ModelCommand::RemoveElements { ids: vec![id] })
        .unwrap();
    editor.project.validate().unwrap();
    assert_eq!(note.links[0].status(&editor.project), LinkStatus::Missing);
    assert_eq!(
        editor.project.evidence.status(&editor.project)["notes"][0]["links"][0]["status"],
        "missing"
    );
    assert_eq!(editor.project.evidence.notes[0].quote, note.quote);
}
#[test]
fn annotation_edit_preserves_changed_baseline_until_explicit_refresh_and_missing_refresh_refuses() {
    let mut editor = Editor::new(common::synthetic_building_98());
    let mut d = draft(&editor.project);
    editor.apply(put(d.clone(), None)).unwrap();
    let old = editor.project.evidence.notes[0].clone();
    editor.selected_ids = vec![editor.project.elements[0].id];
    editor.move_selection(Point::new(2., 0.));
    d.annotation = "Keep waiting for measured survey".into();
    editor.apply(put(d.clone(), Some(old.clone()))).unwrap();
    let current = editor.project.evidence.notes[0].clone();
    assert_eq!(old.links, current.links);
    assert_eq!(
        current.links[0].status(&editor.project),
        LinkStatus::Changed
    );
    d.refresh_links = true;
    editor.apply(put(d.clone(), Some(current.clone()))).unwrap();
    let refreshed = editor.project.evidence.notes[0].clone();
    assert_ne!(refreshed.links, current.links);
    assert_eq!(
        refreshed.links[0].status(&editor.project),
        LinkStatus::Unchanged
    );
    assert!(editor.apply(put(d.clone(), Some(old))).is_err());
    let target = editor.project.elements[0].id;
    editor
        .apply(ModelCommand::RemoveElements { ids: vec![target] })
        .unwrap();
    assert!(
        editor
            .apply(put(d.clone(), Some(refreshed.clone())))
            .is_err()
    );
    d.refresh_links = false;
    editor.apply(put(d, Some(refreshed))).unwrap();
    assert_eq!(
        editor.project.evidence.notes[0].links[0].status(&editor.project),
        LinkStatus::Missing
    );
}
#[test]
fn program_source_or_row_changes_are_detected_without_touching_geometry() {
    use atelier_bim::program::{AreaBasis, AreaUnit, Mapping, ProgramEdit, workbook};
    let mut editor = Editor::new(common::synthetic_building_98());
    let proposal = workbook::map(
        &workbook::inspect(b"ID,Name\nA,Classroom\n", "csv").unwrap(),
        &"b".repeat(64),
        "brief".into(),
        "Brief".into(),
        Mapping {
            sheet: "CSV".into(),
            first_row: 2,
            last_row: 2,
            key_column: 1,
            label_column: 2,
            area_column: None,
            quantity_column: None,
            scope_column: None,
            note_column: None,
            area_unit: AreaUnit::SquareMetre,
            area_basis: AreaBasis::RowTotal,
        },
        None,
    )
    .unwrap();
    editor
        .apply(ModelCommand::Program {
            edit: ProgramEdit::Import { proposal },
        })
        .unwrap();
    let mut d = draft(&editor.project);
    d.targets = vec![Target::ProgramRow {
        sheet_id: "brief".into(),
        key: "A".into(),
    }];
    editor.apply(put(d, None)).unwrap();
    let note = editor.project.evidence.notes[0].clone();
    let sheet = editor.project.program.sheets[0].clone();
    let expected = sheet.rows[0].values.clone();
    let mut values = expected.clone();
    values.quantity = 2;
    editor
        .apply(ModelCommand::Program {
            edit: ProgramEdit::UpdateRow {
                sheet_id: "brief".into(),
                key: "A".into(),
                expected,
                values,
            },
        })
        .unwrap();
    assert_eq!(note.links[0].status(&editor.project), LinkStatus::Changed);
    editor.undo();
    assert_eq!(note.links[0].status(&editor.project), LinkStatus::Unchanged);
    editor
        .apply(ModelCommand::Program {
            edit: ProgramEdit::RemoveSheet { expected: sheet },
        })
        .unwrap();
    assert_eq!(note.links[0].status(&editor.project), LinkStatus::Missing);
    editor.project.validate().unwrap();
}
#[test]
fn host_receipt_undo_reopen_and_native_version_marker_preserve_legacy_documents() {
    let f = Fixture::new();
    let original = common::synthetic_building_98();
    let legacy = storage::JsonAdapter.export(&original).unwrap();
    let mut host = ApplicationHost::new(original.clone());
    let r = host
        .dispatch(
            host.context(),
            HostCommand::Apply {
                request_id: "source-note".into(),
                commands: vec![put(draft(&original), None)],
            },
        )
        .unwrap()
        .unwrap();
    assert!(
        r.evidence_changed
            && !r.program_changed
            && !r.site_context_changed
            && r.changed_ids.is_empty()
    );
    let after = host.document.editor.project.clone();
    host.dispatch(host.context(), HostCommand::Undo).unwrap();
    assert_eq!(
        storage::JsonAdapter
            .export(&host.document.editor.project)
            .unwrap(),
        legacy
    );
    host.dispatch(host.context(), HostCommand::Redo).unwrap();
    assert_eq!(host.document.editor.project, after);
    let path = f.0.join("model.json");
    storage::save_document(&after, &path, host.document.editor.project_identity()).unwrap();
    assert_eq!(Editor::from_stored_document(&path).unwrap().project, after);
    let mut value: serde_json::Value =
        serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    assert_eq!(value["schema_version"], 5);
    for version in 1..=4 {
        value["schema_version"] = serde_json::json!(version);
        assert!(
            storage::JsonAdapter
                .import(&serde_json::to_vec(&value).unwrap())
                .is_err()
        );
    }
}
#[test]
fn duplicate_targets_missing_new_targets_and_stale_note_removal_are_atomic() {
    let mut editor = Editor::new(common::synthetic_building_98());
    let original = editor.project.clone();
    let mut d = draft(&original);
    d.targets.push(d.targets[0].clone());
    assert!(editor.apply(put(d, None)).is_err());
    assert_eq!(editor.project, original);
    let mut d = draft(&original);
    d.targets = vec![Target::Element { id: u64::MAX }];
    assert!(editor.apply(put(d, None)).is_err());
    let d = draft(&original);
    editor.apply(put(d, None)).unwrap();
    let mut note = editor.project.evidence.notes[0].clone();
    let saved = note.clone();
    note.annotation = "Stale copy".into();
    assert!(
        editor
            .apply(ModelCommand::Evidence {
                edit: EvidenceEdit::Remove { expected: note }
            })
            .is_err()
    );
    editor
        .apply(ModelCommand::Evidence {
            edit: EvidenceEdit::Remove { expected: saved },
        })
        .unwrap();
    assert!(editor.project.evidence.is_empty());
    editor.undo();
    assert_eq!(editor.project.evidence.notes.len(), 1);
}
#[test]
fn source_guard_checks_original_text_identity_quality_lock_stale_and_portable_reopen() {
    let f = Fixture::new();
    let path = f.0.join("brief.docx");
    let bytes = docx();
    std::fs::write(&path, &bytes).unwrap();
    let mut host = ApplicationHost::new(common::synthetic_building_98());
    let mut repo = Repository::create_for_editor(&f.0.join("repo"), &host.document.editor).unwrap();
    let id = repo.add_source(&path).unwrap();
    let mut d = draft(&host.document.editor.project);
    d.quote.source_sha256 = id.clone();
    for (locator, text) in [
        ("body/p[2]", d.quote.text.clone()),
        ("body/p[1]", "Invented approval".into()),
    ] {
        let mut q = d.quote.clone();
        q.locator = locator.into();
        q.text = text;
        assert!(repo.guard_evidence_commit(&q).is_err());
    }
    let guard = repo.guard_evidence_commit(&d.quote).unwrap();
    assert!(repo.reject_source(&id, "lock test").is_err());
    host.dispatch(
        host.context(),
        HostCommand::Apply {
            request_id: "evidence-import".into(),
            commands: vec![put(d.clone(), None)],
        },
    )
    .unwrap();
    drop(guard);
    repo.checkpoint(
        &host.document.editor.project,
        host.document.editor.project_identity(),
        host.document.editor.revision,
    )
    .unwrap();
    let stale = Repository::open(repo.root()).unwrap();
    repo.confirm_unit(&id, None).unwrap();
    assert!(stale.guard_evidence_commit(&d.quote).is_err());
    let moved = f.0.join("moved");
    std::fs::rename(repo.root(), &moved).unwrap();
    let mut reopened = Repository::open(&moved).unwrap();
    assert_eq!(
        reopened.load_model().unwrap().project,
        host.document.editor.project
    );
    drop(reopened.guard_evidence_commit(&d.quote).unwrap());
    reopened
        .reject_source(&id, "Explicit fixture rejection")
        .unwrap();
    assert!(reopened.guard_evidence_commit(&d.quote).is_err());
    assert_eq!(std::fs::read(path).unwrap(), bytes);
}
#[test]
fn both_ai_adapters_preview_commit_and_read_report_declared_source_and_typed_link_status() {
    use atelier_bim::{
        automation::ReadService,
        automation_change::{ChangeService, Commit, Prepare},
    };
    let mut editor = Editor::new(common::synthetic_building_98());
    let original = editor.project.clone();
    let d = draft(&editor.project);
    let wire = serde_json::json!({"version":1,"edits":[{"operation":"evidence","edit":{"operation":"put","draft":d,"expected_previous":null}}]});
    let a: NativeBatch = serde_json::from_value(wire.clone()).unwrap();
    let b: DiscoveredBatch = serde_json::from_value(wire).unwrap();
    assert_eq!(
        a.lower(&editor).unwrap(),
        b.resolve().unwrap().lower(&editor).unwrap()
    );
    let read = ReadService::new("evidence".into());
    let target = read.target(&editor);
    let mut service = ChangeService::new("evidence".into(), true);
    let preview = service.prepare(
        &editor,
        Prepare {
            target: target.clone(),
            expected_revision: editor.revision,
            change: a.lower(&editor).unwrap(),
        },
    );
    assert_eq!(
        preview["result"]["evidence_change"]["after"]["notes"][0]["title"],
        d.title
    );
    assert_eq!(editor.project, original);
    let receipt = service.commit(
        &mut editor,
        Commit {
            target: target.clone(),
            preview_id: preview["result"]["preview_id"].as_str().unwrap().into(),
            request_id: "evidence-change".into(),
        },
    );
    assert_eq!(receipt["result"]["receipt"]["evidence_changed"], true);
    let context = read.context(&editor, &target).unwrap();
    assert_eq!(context["evidence_summary"]["note_count"], 1);
    let context = read
        .source_evidence(
            &editor,
            &atelier_bim::automation::EvidenceRequest {
                topics: false,
                object_targets: vec![],
                target: target.clone(),
                expected_revision: editor.revision,
                offset: 0,
                limit: 10,
            },
        )
        .unwrap();
    assert_eq!(context["notes"][0]["links"][0]["status"], "unchanged");
    assert!(
        context["source_verification"]
            .as_str()
            .unwrap()
            .contains("unverified")
    );
    assert_eq!(editor.project.elements, original.elements);
    assert!(atelier_bim::automation_registry::describe("evidence").is_some());
}

#[test]
fn all_batch_quotes_are_rechecked_and_missing_source_does_not_erase_standalone_notes() {
    let f = Fixture::new();
    let source = f.0.join("brief.docx");
    std::fs::write(&source, docx()).unwrap();
    let mut editor = Editor::new(common::synthetic_building_98());
    let mut repo = Repository::create_for_editor(&f.0.join("repo"), &editor).unwrap();
    let id = repo.add_source(&source).unwrap();
    let mut d = draft(&editor.project);
    d.quote.source_sha256 = id.clone();
    let mut wrong = d.quote.clone();
    wrong.text = "Another quote must also be checked".into();
    assert!(
        repo.guard_evidence_quotes(&[d.quote.clone(), wrong])
            .is_err()
    );
    let guard = repo
        .guard_evidence_quotes(&[d.quote.clone(), d.quote.clone()])
        .unwrap();
    editor.apply(put(d.clone(), None)).unwrap();
    drop(guard);
    let path = f.0.join("standalone.json");
    storage::save_document(&editor.project, &path, editor.project_identity()).unwrap();
    let stored = repo.manifest.sources.iter().find(|s| s.id == id).unwrap();
    std::fs::remove_file(repo.root().join(&stored.blob.path)).unwrap();
    assert!(repo.guard_evidence_commit(&d.quote).is_err());
    assert!(Repository::open(repo.root()).is_err());
    let reopened = Editor::from_stored_document(&path).unwrap();
    assert_eq!(reopened.project.evidence, editor.project.evidence);
    let mut wire = serde_json::to_value(&editor.project.evidence.notes[0]).unwrap();
    wire["approved"] = serde_json::json!(true);
    assert!(serde_json::from_value::<Note>(wire).is_err());
}

#[test]
fn complete_notes_page_below_transport_bound_without_truncating_text_or_links() {
    use atelier_bim::{
        automation::{EvidenceRequest, ReadService},
        evidence::Link,
    };
    let mut editor = Editor::new(common::synthetic_building_98());
    for index in 0..20 {
        let mut note = Note {
            id: format!("large-{index}"),
            title: "Large declared source".into(),
            kind: Kind::Question,
            quote: Quote {
                source_sha256: "a".repeat(64),
                locator: "body/p[1]".into(),
                text: "\"".repeat(16384),
            },
            annotation: "\"".repeat(4096),
            links: vec![],
        };
        for key in 0..100 {
            note.links.push(Link {
                target: Target::ProgramRow {
                    sheet_id: format!("{}{key:03}", "\"".repeat(250)),
                    key: "\"".repeat(256),
                },
                baseline_sha256: "b".repeat(64),
            });
        }
        editor.project.evidence.notes.push(note);
    }
    editor.project.validate().unwrap();
    let before = editor.project.clone();
    let read = ReadService::new("evidence-page".into());
    let target = read.target(&editor);
    let context = read.context(&editor, &target).unwrap();
    assert_eq!(context["evidence_summary"]["note_count"], 20);
    assert!(context.get("evidence").is_none());
    let mut request = EvidenceRequest {
        topics: false,
        object_targets: vec![],
        target,
        expected_revision: editor.revision,
        offset: 0,
        limit: 10,
    };
    let mut count = 0;
    loop {
        let page = read.source_evidence(&editor, &request).unwrap();
        assert!(serde_json::to_vec(&page).unwrap().len() < 1024 * 1024);
        let notes = page["notes"].as_array().unwrap();
        assert!(!notes.is_empty() && notes.len() < 10);
        for value in notes {
            assert_eq!(
                value["note"]["quote"]["text"].as_str().unwrap().len(),
                16384
            );
            assert_eq!(value["note"]["links"].as_array().unwrap().len(), 100);
            assert_eq!(value["links"].as_array().unwrap().len(), 100);
            count += 1;
        }
        let Some(next) = page["next_offset"].as_u64() else {
            break;
        };
        assert!(next > request.offset as u64);
        request.offset = next as usize;
    }
    assert_eq!(count, 20);
    assert_eq!(editor.project, before);
    request.limit = 11;
    assert_eq!(
        read.source_evidence(&editor, &request).unwrap_err().code,
        "INVALID_LIMIT"
    );
    request.limit = 1;
    request.expected_revision += 1;
    assert_eq!(
        read.source_evidence(&editor, &request).unwrap_err().code,
        "REVISION_CONFLICT"
    );
}

#[cfg(unix)]
#[test]
fn source_evidence_page_is_available_over_readonly_ipc_without_mutation() {
    use atelier_bim::automation_ipc::Endpoint;
    use std::{
        io::{BufRead, BufReader},
        os::unix::net::UnixStream,
        sync::mpsc,
        time::{Duration, Instant},
    };
    let root = std::env::temp_dir().join(format!("oe{}", &storage::new_document_identity()[..8]));
    std::fs::create_dir(&root).unwrap();
    let f = Fixture(root);
    let socket = f.0.join("ipc/a.sock");
    let mut endpoint = Endpoint::bind(socket.clone()).unwrap();
    let mut editor = Editor::new(common::synthetic_building_98());
    editor.apply(put(draft(&editor.project), None)).unwrap();
    let before = editor.project.clone();
    let revision = editor.revision;
    let (tx, rx) = mpsc::channel();
    let client = std::thread::spawn(move || {
        let request = |value: serde_json::Value| {
            let mut stream = UnixStream::connect(&socket).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            writeln!(stream, "{value}").unwrap();
            let mut line = String::new();
            BufReader::new(stream).read_line(&mut line).unwrap();
            serde_json::from_str::<serde_json::Value>(&line).unwrap()
        };
        let target =
            request(serde_json::json!({"method":"get_target"}))["result"]["target"].clone();
        tx.send(request(serde_json::json!({"method":"query_source_evidence","params":{"target":target,"expected_revision":revision,"offset":0,"limit":10}}))).unwrap();
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    let result = loop {
        endpoint.poll(&mut editor);
        if let Ok(value) = rx.try_recv() {
            break value;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    };
    client.join().unwrap();
    assert_eq!(
        result["result"]["notes"][0]["note"]["id"],
        "survey-question"
    );
    assert_eq!(
        result["result"]["notes"][0]["links"][0]["status"],
        "unchanged"
    );
    assert_eq!(editor.project, before);
    assert_eq!(editor.revision, revision);
}
