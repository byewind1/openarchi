mod common;
use atelier_bim::{
    application_host::{ApplicationHost, HostCommand},
    automation_registry::{DiscoveredBatch, NativeBatch},
    command::ModelCommand,
    editor::Editor,
    model::ModelAdapter,
    program::{self, Area, AreaBasis, AreaUnit, Mapping, ProgramEdit, workbook},
    project_repository::Repository,
    storage,
};
use std::{
    fs,
    io::{Cursor, Write},
    path::PathBuf,
};
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let p =
            std::env::temp_dir().join(format!("oa-program-{}", storage::new_document_identity()));
        fs::create_dir(&p).unwrap();
        Self(p)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn mapping(sheet: &str) -> Mapping {
    Mapping {
        sheet: sheet.into(),
        first_row: 2,
        last_row: 3,
        key_column: 1,
        label_column: 2,
        area_column: Some(3),
        quantity_column: Some(4),
        scope_column: None,
        note_column: None,
        area_unit: AreaUnit::SquareMetre,
        area_basis: AreaBasis::PerUnit,
    }
}
fn csv() -> Vec<u8> {
    "\u{feff}ID,Name,Area,Count\r\nA,\"Room, A\",10,2\r\nB,Hall,,1\r\n"
        .as_bytes()
        .to_vec()
}
fn proposal() -> program::ImportProposal {
    workbook::map(
        &workbook::inspect(&csv(), "csv").unwrap(),
        &"a".repeat(64),
        "rooms".into(),
        "Synthetic requirements".into(),
        mapping("CSV"),
        None,
    )
    .unwrap()
}
fn command(p: program::ImportProposal) -> ModelCommand {
    ModelCommand::Program {
        edit: ProgramEdit::Import { proposal: p },
    }
}
fn xlsx() -> Vec<u8> {
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let opt = zip::write::SimpleFileOptions::default();
    for (path, xml) in [
        (
            "[Content_Types].xml",
            r#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>"#,
        ),
        (
            "xl/workbook.xml",
            r#"<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets><sheet name="Rooms" sheetId="1" r:id="rId1"/></sheets></workbook>"#,
        ),
        (
            "xl/_rels/workbook.xml.rels",
            r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/sharedStrings" Target="sharedStrings.xml"/></Relationships>"#,
        ),
        (
            "xl/sharedStrings.xml",
            r#"<sst xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><si><t>Shared room</t></si></sst>"#,
        ),
        (
            "xl/worksheets/sheet1.xml",
            r#"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><dimension ref="A1:XFD1048576"/><sheetData><row r="2"><c r="A2" t="inlineStr"><is><t>A</t></is></c><c r="B2" t="s"><v>0</v></c><c r="C2"><v>10</v></c><c r="D2"><v>2</v></c></row><row r="3"><c r="A3" t="inlineStr"><is><t>B</t></is></c><c r="B3" t="inlineStr"><is><t>Hall</t></is></c><c r="C3"><f>1/0</f><v>999</v></c><c r="D3"><v>1</v></c></row></sheetData></worksheet>"#,
        ),
    ] {
        zip.start_file(path, opt).unwrap();
        zip.write_all(xml.as_bytes()).unwrap();
    }
    zip.finish().unwrap().into_inner()
}
#[test]
fn quoted_csv_units_basis_and_unknown_are_explicit() {
    let mut p = proposal();
    assert_eq!(p.sheet.rows[0].values.label, "Room, A");
    assert_eq!(p.sheet.rows[0].values.area, Area::Known { m2: 20. });
    assert!(matches!(p.sheet.rows[1].values.area, Area::Unknown { .. }));
    p.sheet.mapping.area_unit = AreaUnit::SquareFoot;
    p.sheet.mapping.area_basis = AreaBasis::RowTotal;
    let q = workbook::map(
        &workbook::inspect(&csv(), "csv").unwrap(),
        &"a".repeat(64),
        "rooms".into(),
        "Rooms".into(),
        p.sheet.mapping,
        None,
    )
    .unwrap();
    assert!(
        (match q.sheet.rows[0].values.area {
            Area::Known { m2 } => m2,
            _ => panic!(),
        } - 0.9290304)
            .abs()
            < 1e-12
    );
    assert_eq!(workbook::address(3, 28), "AB3");
}
#[test]
fn streaming_xlsx_retains_shared_text_and_never_trusts_formula_cache() {
    let book = workbook::inspect(&xlsx(), "xlsx").unwrap();
    assert_eq!(book.sheets[0].cells.len(), 8);
    assert_eq!(book.sheets[0].cell(3, 3).unwrap().value, "999");
    assert!(book.sheets[0].cell(3, 3).unwrap().formula);
    let p = workbook::map(
        &book,
        &"b".repeat(64),
        "rooms".into(),
        "Rooms".into(),
        mapping("Rooms"),
        None,
    )
    .unwrap();
    assert_eq!(p.sheet.rows[0].values.label, "Shared room");
    assert!(matches!(p.sheet.rows[1].values.area, Area::Unknown { .. }));
}
#[test]
fn malformed_duplicate_keys_invalid_counts_and_ranges_refuse() {
    for text in [
        "ID,Name,Area,Count\nA,One,10,1\nA,Two,10,1\n",
        "ID,Name,Area,Count\nA,One,10,0\nB,Two,10,1\n",
        "ID,Name,Area,Count\n,One,10,1\nB,Two,-10,1\n",
    ] {
        let b = workbook::inspect(text.as_bytes(), "csv").unwrap();
        assert!(
            workbook::map(
                &b,
                &"a".repeat(64),
                "rooms".into(),
                "Rooms".into(),
                mapping("CSV"),
                None
            )
            .is_err()
        );
    }
    let mut m = mapping("CSV");
    m.last_row = 10002;
    assert!(m.validate().is_err());
    assert!(workbook::inspect(&[0xff], "csv").is_err());
    assert!(workbook::inspect(&vec![0; 32 * 1024 * 1024 + 1], "csv").is_err());
}
#[test]
fn shared_transaction_preserves_geometry_undo_reopen_and_version_markers() {
    let f = Fixture::new();
    let original = common::synthetic_building_98();
    let mut host = ApplicationHost::new(original.clone());
    let r = host
        .dispatch(
            host.context(),
            HostCommand::Apply {
                request_id: "program".into(),
                commands: vec![command(proposal())],
            },
        )
        .unwrap()
        .unwrap();
    assert!(r.program_changed);
    assert!(!r.site_context_changed);
    assert!(r.changed_ids.is_empty());
    assert_eq!(host.document.editor.project.elements, original.elements);
    let after = host.document.editor.project.clone();
    host.dispatch(host.context(), HostCommand::Undo).unwrap();
    assert_eq!(host.document.editor.project, original);
    host.dispatch(host.context(), HostCommand::Redo).unwrap();
    assert_eq!(host.document.editor.project, after);
    let out = f.0.join("model.json");
    storage::save_document(&after, &out, host.document.editor.project_identity()).unwrap();
    assert_eq!(Editor::from_stored_document(&out).unwrap().project, after);
    let mut value: serde_json::Value = serde_json::from_slice(&fs::read(out).unwrap()).unwrap();
    assert_eq!(value["schema_version"], 4);
    value["schema_version"] = serde_json::json!(2);
    assert!(
        storage::JsonAdapter
            .import(&serde_json::to_vec(&value).unwrap())
            .is_err()
    );
    assert!(
        serde_json::to_value(original)
            .unwrap()
            .get("program")
            .is_none()
    );
    assert_eq!(after.program.summary()["unknown_area_rows"], 1);
}
#[test]
fn reimport_preserves_stable_keys_and_refuses_manual_edits_or_missing_rows() {
    let mut editor = Editor::new(common::synthetic_building_98());
    editor.apply(command(proposal())).unwrap();
    let old = editor.project.program.sheets[0].clone();
    let mut p = proposal();
    p.expected_previous = Some(old.clone());
    p.sheet.source_sha256 = "b".repeat(64);
    p.sheet.rows[0].values.area = Area::Known { m2: 21. };
    p.sheet.rows[0].imported = p.sheet.rows[0].values.clone();
    editor.apply(command(p.clone())).unwrap();
    assert_eq!(editor.project.program.sheets[0].rows[0].key, "A");
    editor.undo();
    let row = &old.rows[0];
    let mut values = row.values.clone();
    values.label = "User edit".into();
    editor
        .apply(ModelCommand::Program {
            edit: ProgramEdit::UpdateRow {
                sheet_id: old.id.clone(),
                key: row.key.clone(),
                expected: row.values.clone(),
                values,
            },
        })
        .unwrap();
    let mut stale = p.clone();
    stale.expected_previous = Some(editor.project.program.sheets[0].clone());
    assert!(editor.apply(command(stale)).is_err());
    editor.undo();
    p.sheet.rows.remove(1);
    p.sheet.blank_rows.push(3);
    assert!(editor.apply(command(p)).is_err());
    editor
        .apply(ModelCommand::Program {
            edit: ProgramEdit::RemoveSheet { expected: old },
        })
        .unwrap();
    assert!(editor.project.program.is_empty());
    editor.undo();
    assert_eq!(editor.project.program.sheets.len(), 1);
}
#[test]
fn source_guard_checks_exact_cells_rejection_stale_manifest_and_lock() {
    let f = Fixture::new();
    let editor = Editor::new(common::synthetic_building_98());
    let mut repo = Repository::create_for_editor(&f.0.join("repo"), &editor).unwrap();
    let path = f.0.join("rooms.csv");
    fs::write(&path, csv()).unwrap();
    let source = repo.add_source(&path).unwrap();
    let p = repo
        .preview_program(
            &source,
            "rooms".into(),
            "Rooms".into(),
            mapping("CSV"),
            None,
        )
        .unwrap();
    let guard = repo.guard_program_commit(&p).unwrap();
    assert!(repo.reject_source(&source, "test").is_err());
    drop(guard);
    let mut forged = p.clone();
    forged.sheet.rows[0].values.label = "Forged".into();
    forged.sheet.rows[0].imported = forged.sheet.rows[0].values.clone();
    assert!(repo.guard_program_commit(&forged).is_err());
    let stale = repo.clone();
    repo.reject_source(&source, "Rejected fixture").unwrap();
    assert!(stale.guard_program_commit(&p).is_err());
    assert!(repo.guard_program_commit(&p).is_err());
    assert!(
        repo.preview_program(
            &source,
            "rooms".into(),
            "Rooms".into(),
            mapping("CSV"),
            None
        )
        .is_err()
    );
    assert!(repo.inspect_program(&source).is_ok());
}
#[test]
fn both_ai_adapters_share_program_edit_and_preserve_legacy_wire_receipts() {
    let editor = Editor::new(common::synthetic_building_98());
    let wire = serde_json::json!({"version":1,"edits":[{"operation":"program","edit":{"operation":"import","proposal":proposal()}}]});
    let a: NativeBatch = serde_json::from_value(wire.clone()).unwrap();
    let b: DiscoveredBatch = serde_json::from_value(wire).unwrap();
    assert_eq!(
        a.lower(&editor).unwrap(),
        b.resolve().unwrap().lower(&editor).unwrap()
    );
    assert!(atelier_bim::automation_registry::describe("program").is_some());
    let old = serde_json::json!({"request_id":"old","document_session":1,"revision_before":0,"revision_after":1,"changed_ids":[],"resolved_references":{}});
    let receipt: atelier_bim::model_service::ChangeReceipt =
        serde_json::from_value(old.clone()).unwrap();
    assert!(!receipt.program_changed);
    assert_eq!(serde_json::to_value(receipt).unwrap(), old);
}

#[test]
fn multi_cell_formula_scopes_cannot_masquerade_as_literal_result_cells() {
    use std::io::Read;
    for replacement in [
        "<f t=\"array\" ref=\"C3:C4\">1/0</f>",
        "<f t=\"dataTable\" ref=\"C3:C4\"/>",
        "<f t=\"futureType\">1/0</f>",
        "<calculatedColumnFormula>1/0</calculatedColumnFormula>",
    ] {
        let original = xlsx();
        let mut read = zip::ZipArchive::new(Cursor::new(original)).unwrap();
        let mut write = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for i in 0..read.len() {
            let mut f = read.by_index(i).unwrap();
            let name = f.name().unwrap().into_owned();
            let mut bytes = Vec::new();
            f.read_to_end(&mut bytes).unwrap();
            if name == "xl/worksheets/sheet1.xml" {
                bytes = String::from_utf8(bytes)
                    .unwrap()
                    .replace("<f>1/0</f>", replacement)
                    .into_bytes();
            }
            write
                .start_file(name, zip::write::SimpleFileOptions::default())
                .unwrap();
            write.write_all(&bytes).unwrap();
        }
        let bytes = write.finish().unwrap().into_inner();
        assert!(workbook::inspect(&bytes, "xlsx").is_err());
    }
}

#[test]
fn program_transport_preview_durable_receipt_recovery_and_undo_keep_geometry_unchanged() {
    use atelier_bim::{
        automation::ReadService,
        automation_change::{ChangeService, Commit, Operation, Prepare, Recover},
    };
    let fixture = Fixture::new();
    let mut editor = Editor::new(common::synthetic_building_98());
    let before = editor.project.clone();
    let target = ReadService::new("program".into()).target(&editor);
    let wire = serde_json::json!({"version":1,"edits":[{"operation":"program","edit":{"operation":"import","proposal":proposal()}}]});
    let batch: NativeBatch = serde_json::from_value(wire).unwrap();
    let mut service = ChangeService::new("program".into(), true)
        .with_journal_directory(fixture.0.join("journal"));
    let preview = service.prepare(
        &editor,
        Prepare {
            target: target.clone(),
            expected_revision: 0,
            change: batch.lower(&editor).unwrap(),
        },
    );
    assert_eq!(editor.project, before);
    assert_eq!(
        preview["result"]["program_change"]["before"]["sheets"],
        serde_json::json!([])
    );
    assert_eq!(
        preview["result"]["program_change"]["after"]["sheets"][0]["id"],
        "rooms"
    );
    let receipt = service.commit(
        &mut editor,
        Commit {
            target: target.clone(),
            preview_id: preview["result"]["preview_id"].as_str().unwrap().into(),
            request_id: "program-import".into(),
        },
    );
    assert_eq!(receipt["result"]["status"], "committed");
    assert_eq!(receipt["result"]["receipt"]["program_changed"], true);
    assert_eq!(editor.project.elements, before.elements);
    drop(service);
    let mut service = ChangeService::new("program".into(), true)
        .with_journal_directory(fixture.0.join("journal"));
    let recovered = service.operation(
        &editor,
        Operation {
            target: target.clone(),
            request_id: "program-import".into(),
            original_target: None,
        },
    );
    assert_eq!(recovered["result"]["receipt"], receipt["result"]["receipt"]);
    let after = editor.project.clone();
    let blocked = service.undo(
        &mut editor,
        Operation {
            target: target.clone(),
            request_id: "program-import".into(),
            original_target: None,
        },
    );
    assert_eq!(blocked["error"]["code"], "UNDO_HISTORY_UNAVAILABLE");
    assert_eq!(editor.project, after);
    let mut restored_editor = Editor::new(atelier_bim::model::Project::default());
    let current = ReadService::new("program".into()).target(&restored_editor);
    let restored = service.recover_document(
        &mut restored_editor,
        Recover {
            target: current,
            expected_revision: 0,
            original_target: target.clone(),
            request_id: "program-import".into(),
        },
    );
    assert_eq!(restored["result"]["status"], "committed");
    assert_eq!(restored_editor.project, after);
    let current = ReadService::new("program".into()).target(&restored_editor);
    let undone = service.undo(
        &mut restored_editor,
        Operation {
            target: current,
            request_id: "program-import".into(),
            original_target: Some(target),
        },
    );
    assert_eq!(undone["result"]["status"], "undone");
    assert_eq!(restored_editor.project, before);
}
