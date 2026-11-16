use atelier_bim::{
    application_host::{ApplicationHost, HostCommand},
    command::ModelCommand,
    program::{Area, AreaBasis, AreaUnit, Mapping, ProgramEdit, workbook},
    project_repository::Repository,
    source_document::inspect_docx,
    storage,
};
use std::{
    io::{Cursor, Write},
    path::PathBuf,
};
const W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
const RELS: &str = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="main" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;
const TYPES: &str = r#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#;
fn archive(parts: &[(&str, &str)]) -> Vec<u8> {
    let mut zip = zip::ZipWriter::new(Cursor::new(vec![]));
    for (name, text) in parts {
        zip.start_file(
            *name,
            zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated),
        )
        .unwrap();
        zip.write_all(text.as_bytes()).unwrap();
    }
    zip.finish().unwrap().into_inner()
}
fn document(body: &str) -> Vec<u8> {
    archive(&[
        ("_rels/.rels", RELS),
        ("[Content_Types].xml", TYPES),
        (
            "word/document.xml",
            &format!(r#"<w:document xmlns:w="{W}"><w:body>{body}</w:body></w:document>"#),
        ),
    ])
}
fn cell(text: &str) -> String {
    format!("<w:tc><w:p><w:r><w:t>{text}</w:t></w:r></w:p></w:tc>")
}
fn table() -> String {
    format!(
        "<w:tbl><w:tr>{}{}</w:tr><w:tr>{}{}</w:tr><w:tr>{}{}</w:tr></w:tbl>",
        cell("Function"),
        cell("Requirement"),
        cell("Teaching"),
        cell("Shared &amp; flexible"),
        cell("Lodging"),
        cell("Beds not confirmed")
    )
}
fn mapping() -> Mapping {
    Mapping {
        sheet: "Table 1".into(),
        first_row: 2,
        last_row: 3,
        key_column: 1,
        label_column: 1,
        area_column: None,
        quantity_column: None,
        scope_column: None,
        note_column: Some(2),
        area_unit: AreaUnit::SquareMetre,
        area_basis: AreaBasis::RowTotal,
    }
}
#[test]
fn xml_text_quotes_keep_source_positions_and_do_not_invent_approval_or_areas() {
    let bytes = document(&format!(
        "<w:p><w:r><w:t xml:space=\"preserve\"> Unconfirmed </w:t><w:tab/><w:t>A &amp; B</w:t><w:br/><w:t>Next</w:t></w:r></w:p><w:p/>{}<w:p><w:r><w:t>Later</w:t></w:r></w:p>{}",
        table(),
        table()
    ));
    let d = inspect_docx(&bytes).unwrap();
    assert_eq!(d.paragraphs.len(), 3);
    assert_eq!(d.tables.len(), 2);
    assert_eq!(d.quote("body/p[1]"), Some(" Unconfirmed \tA & B\nNext"));
    assert_eq!(d.quote("body/p[2]"), Some(""));
    assert_eq!(
        d.quote("body/tbl[2]/row[2]/cell[2]"),
        Some("Shared & flexible")
    );
    for locator in [
        "body/p[0]",
        "body/tbl[2]/row[0]/cell[1]",
        "body/tbl[2]/row[9]/cell[1]",
        "body/tbl[2]/row[2]/cell[2]/extra",
    ] {
        assert_eq!(d.quote(locator), None);
    }
    let book = workbook::inspect(&bytes, "docx").unwrap();
    let proposal = workbook::map(
        &book,
        &"a".repeat(64),
        "brief".into(),
        "Unapproved brief".into(),
        mapping(),
        None,
    )
    .unwrap();
    assert_eq!(proposal.sheet.rows.len(), 2);
    assert_eq!(proposal.sheet.rows[0].values.note, "Shared & flexible");
    assert!(
        proposal
            .sheet
            .rows
            .iter()
            .all(|r| r.values.quantity == 1 && matches!(r.values.area, Area::Unknown { .. }))
    );
    assert_eq!(proposal.sheet.rows[1].source_row, 3);
}
#[test]
fn ambiguous_word_content_is_rejected_instead_of_reading_cached_or_deleted_text() {
    for fragment in [
        "<w:fldSimple w:instr=\"SUM\"><w:r><w:t>999</w:t></w:r></w:fldSimple>",
        "<w:r><w:fldChar w:fldCharType=\"begin\"/><w:instrText>SUM</w:instrText><w:t>999</w:t></w:r>",
        "<w:ins><w:r><w:t>Not accepted</w:t></w:r></w:ins>",
        "<w:del><w:r><w:delText>Deleted</w:delText></w:r></w:del>",
        "<w:r><w:rPr><w:vanish/></w:rPr><w:t>Hidden</w:t></w:r>",
        "<w:sdt><w:sdtContent><w:r><w:t>Controlled</w:t></w:r></w:sdtContent></w:sdt>",
        "<w:r><w:drawing/></w:r>",
        "<w:r><w:sym w:char=\"0041\"/></w:r>",
        "<w:r xmlns:x=\"urn:unknown\"><x:AlternateContent><w:t>Alternate</w:t></x:AlternateContent></w:r>",
    ] {
        assert!(
            inspect_docx(&document(&format!("<w:p>{fragment}</w:p>{}", table()))).is_err(),
            "{fragment}"
        );
    }
    for property in ["gridSpan", "vMerge", "hMerge"] {
        let table = table().replacen(
            "<w:tc>",
            &format!("<w:tc><w:tcPr><w:{property} w:val=\"2\"/></w:tcPr>"),
            1,
        );
        assert!(inspect_docx(&document(&table)).is_err(), "{property}");
    }
    let nested = format!("<w:tbl><w:tr><w:tc>{}</w:tc></w:tr></w:tbl>", table());
    assert!(inspect_docx(&document(&nested)).is_err());
}
#[test]
fn package_identity_namespaces_and_external_main_are_checked() {
    let xml = format!("<w:document xmlns:w=\"{W}\"><w:body/></w:document>");
    for (rels, types, xml) in [
        (
            RELS.replace("Target=", "TargetMode=\"External\" Target="),
            TYPES.into(),
            xml.clone(),
        ),
        (
            RELS.replace("word/document.xml", "https://invalid.example/remote.xml"),
            TYPES.into(),
            xml.clone(),
        ),
        (
            RELS.into(),
            TYPES.replace("document.main+xml", "template.main+xml"),
            xml.clone(),
        ),
        (
            RELS.into(),
            TYPES.into(),
            xml.replace(W, "urn:unrecognized"),
        ),
        (
            RELS.into(),
            TYPES.into(),
            format!("<!DOCTYPE document>{xml}"),
        ),
    ] {
        assert!(
            inspect_docx(&archive(&[
                ("_rels/.rels", &rels),
                ("[Content_Types].xml", &types),
                ("word/document.xml", &xml)
            ]))
            .is_err()
        );
    }
    assert!(inspect_docx(b"not a zip").is_err());
    assert!(inspect_docx(&archive(&[("word/document.xml", &xml)])).is_err());
    let styles = format!(
        "<w:styles xmlns:w=\"{W}\"><w:style><w:rPr><w:vanish/></w:rPr></w:style></w:styles>"
    );
    assert!(
        inspect_docx(&archive(&[
            ("_rels/.rels", RELS),
            ("[Content_Types].xml", TYPES),
            ("word/document.xml", &xml),
            ("word/styles.xml", &styles)
        ]))
        .is_err()
    );
}
#[test]
fn oversized_and_ragged_sources_fail_explicitly() {
    assert!(inspect_docx(&vec![0; 32 * 1024 * 1024 + 1]).is_err());
    assert!(
        inspect_docx(&document(&format!(
            "<w:p><w:r><w:t>{}</w:t></w:r></w:p>",
            "a".repeat(16385)
        )))
        .is_err()
    );
    let ragged = format!(
        "<w:tbl><w:tr>{}</w:tr><w:tr>{}{}</w:tr></w:tbl>",
        cell("A"),
        cell("B"),
        cell("C")
    );
    assert!(inspect_docx(&document(&ragged)).is_err());
    let many_nodes = format!("<w:p>{}</w:p>", "<w:r/>".repeat(250_001));
    assert!(
        inspect_docx(&document(&many_nodes))
            .unwrap_err()
            .contains("nodes")
    );
    let bomb = "x".repeat(64 * 1024 * 1024 + 1);
    assert!(
        inspect_docx(&archive(&[("oversized", &bomb)]))
            .unwrap_err()
            .contains("64 MiB")
    );
}
struct Fixture(PathBuf);
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
#[test]
fn original_docx_guard_host_undo_edits_reimport_and_portable_reopen() {
    let f =
        Fixture(std::env::temp_dir().join(format!("oa-docx-{}", storage::new_document_identity())));
    std::fs::create_dir(&f.0).unwrap();
    let source = f.0.join("brief.docx");
    let bytes = document(&table());
    std::fs::write(&source, &bytes).unwrap();
    let mut host = ApplicationHost::new(atelier_bim::model::Project::default());
    let original = host.document.editor.project.clone();
    let mut repo =
        Repository::create_for_editor(&f.0.join("repository"), &host.document.editor).unwrap();
    let id = repo.add_source(&source).unwrap();
    let proposal = repo
        .preview_program(
            &id,
            "brief".into(),
            "Unapproved brief".into(),
            mapping(),
            None,
        )
        .unwrap();
    let mut forged = proposal.clone();
    forged.sheet.rows[0].values.note = "Invented approval".into();
    assert!(repo.guard_program_commit(&forged).is_err());
    let guard = repo.guard_program_commit(&proposal).unwrap();
    host.dispatch(
        host.context(),
        HostCommand::Apply {
            request_id: storage::new_document_identity(),
            commands: vec![ModelCommand::Program {
                edit: ProgramEdit::Import { proposal },
            }],
        },
    )
    .unwrap();
    drop(guard);
    let imported = host.document.editor.project.clone();
    host.dispatch(host.context(), HostCommand::Undo).unwrap();
    assert_eq!(host.document.editor.project, original);
    host.dispatch(host.context(), HostCommand::Redo).unwrap();
    assert_eq!(host.document.editor.project, imported);
    let expected = imported.program.sheets[0].rows[0].values.clone();
    let mut values = expected.clone();
    values.note = "User revised requirement".into();
    host.dispatch(
        host.context(),
        HostCommand::Apply {
            request_id: storage::new_document_identity(),
            commands: vec![ModelCommand::Program {
                edit: ProgramEdit::UpdateRow {
                    sheet_id: "brief".into(),
                    key: "Teaching".into(),
                    expected,
                    values,
                },
            }],
        },
    )
    .unwrap();
    let current = host.document.editor.project.program.sheets[0].clone();
    let proposal = repo
        .preview_program(
            &id,
            "brief".into(),
            "Unapproved brief".into(),
            mapping(),
            Some(current),
        )
        .unwrap();
    assert!(
        host.dispatch(
            host.context(),
            HostCommand::Apply {
                request_id: storage::new_document_identity(),
                commands: vec![ModelCommand::Program {
                    edit: ProgramEdit::Import { proposal }
                }]
            }
        )
        .is_err()
    );
    repo.checkpoint(
        &host.document.editor.project,
        host.document.editor.project_identity(),
        host.document.editor.revision,
    )
    .unwrap();
    let moved = f.0.join("moved");
    std::fs::rename(f.0.join("repository"), &moved).unwrap();
    let mut reopened = Repository::open(&moved).unwrap();
    assert_eq!(
        reopened.load_model().unwrap().project,
        host.document.editor.project
    );
    assert_eq!(
        reopened
            .inspect_program(&id)
            .unwrap()
            .document
            .unwrap()
            .quote("body/tbl[1]/row[3]/cell[2]"),
        Some("Beds not confirmed")
    );
    reopened
        .reject_source(&id, "Unverified original; test rejection")
        .unwrap();
    assert!(reopened.inspect_program(&id).is_ok());
    assert!(
        reopened
            .preview_program(&id, "brief".into(), "Brief".into(), mapping(), None)
            .is_err()
    );
    assert_eq!(std::fs::read(source).unwrap(), bytes);
}
