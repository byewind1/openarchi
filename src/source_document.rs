//! Read-only XML text of a bounded ordinary DOCX body. Not a Word layout engine.
use roxmltree::{Document, Node, ParsingOptions};
use serde::Serialize;
use std::io::Read;
const W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
const REL: &str = "http://schemas.openxmlformats.org/package/2006/relationships";
const OFFICE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument";
const CT: &str = "http://schemas.openxmlformats.org/package/2006/content-types";
const MAIN: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml";
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Paragraph {
    pub locator: String,
    pub text: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Table {
    pub locator: String,
    pub rows: Vec<Vec<String>>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SourceDocument {
    pub paragraphs: Vec<Paragraph>,
    pub tables: Vec<Table>,
}
impl SourceDocument {
    /// Exact XML-text quote at an inspector locator; no pagination/visibility claim.
    pub fn quote(&self, locator: &str) -> Option<&str> {
        if let Some(p) = self.paragraphs.iter().find(|p| p.locator == locator) {
            return Some(&p.text);
        }
        for t in &self.tables {
            let Some(suffix) = locator.strip_prefix(&format!("{}/row[", t.locator)) else {
                continue;
            };
            let (row, col) = suffix.split_once("]/cell[")?;
            let row = row.parse::<usize>().ok()?.checked_sub(1)?;
            let col = col
                .strip_suffix(']')?
                .parse::<usize>()
                .ok()?
                .checked_sub(1)?;
            return t.rows.get(row)?.get(col).map(String::as_str);
        }
        None
    }
}
fn parse(text: &str) -> Result<Document<'_>, String> {
    if text.contains("<!DOCTYPE") {
        return Err("DOCX DTD is unsupported".into());
    }
    Document::parse_with_options(
        text,
        ParsingOptions {
            nodes_limit: 250_000,
            ..Default::default()
        },
    )
    .map_err(|e| format!("Invalid or oversized DOCX XML: {e}"))
}
fn part(
    archive: &mut zip::ZipArchive<std::io::Cursor<&[u8]>>,
    name: &str,
) -> Result<String, String> {
    let mut text = String::new();
    archive
        .by_name(name)
        .map_err(|e| format!("Missing DOCX part {name}: {e}"))?
        .read_to_string(&mut text)
        .map_err(|e| format!("DOCX requires UTF-8 XML: {e}"))?;
    Ok(text)
}
fn unsupported(name: &str) -> bool {
    matches!(
        name,
        "fldChar"
            | "fldSimple"
            | "instrText"
            | "delText"
            | "ins"
            | "del"
            | "moveFrom"
            | "moveTo"
            | "moveFromRangeStart"
            | "moveToRangeStart"
            | "vanish"
            | "webHidden"
            | "specVanish"
            | "gridSpan"
            | "vMerge"
            | "hMerge"
            | "gridBefore"
            | "gridAfter"
            | "sdt"
            | "customXml"
            | "altChunk"
            | "object"
            | "drawing"
            | "pict"
            | "subDoc"
            | "contentPart"
            | "txbxContent"
            | "cellIns"
            | "cellDel"
            | "cellMerge"
    ) || name.ends_with("Change")
}
fn text(node: Node<'_, '_>, out: &mut String) -> Result<(), String> {
    for n in node.children().filter(Node::is_element) {
        if n.tag_name().namespace() != Some(W) {
            return Err("DOCX alternate or non-Word content is unsupported".into());
        }
        match n.tag_name().name() {
            "r" | "hyperlink" => text(n, out)?,
            "t" => {
                if n.children().any(|c| c.is_element()) {
                    return Err("Invalid DOCX text element".into());
                }
                out.push_str(n.text().unwrap_or(""));
            }
            "tab" => out.push('\t'),
            "br" | "cr" => out.push('\n'),
            "noBreakHyphen" => out.push('\u{2011}'),
            "softHyphen" => out.push('\u{00ad}'),
            "pPr"
            | "rPr"
            | "bookmarkStart"
            | "bookmarkEnd"
            | "proofErr"
            | "commentRangeStart"
            | "commentRangeEnd"
            | "commentReference"
            | "lastRenderedPageBreak" => {}
            other => return Err(format!("Unsupported DOCX paragraph content: {other}")),
        }
        if out.len() > 16_384 {
            return Err("DOCX paragraph/cell exceeds 16 KiB".into());
        }
    }
    Ok(())
}
pub fn inspect_docx(bytes: &[u8]) -> Result<SourceDocument, String> {
    let mut archive = crate::source_archive::open(bytes)?;
    let relationships = part(&mut archive, "_rels/.rels")?;
    let relationships = parse(&relationships)?;
    if !relationships
        .root_element()
        .has_tag_name((REL, "Relationships"))
    {
        return Err("Invalid DOCX package relationships".into());
    }
    let mains = relationships
        .root_element()
        .children()
        .filter(|n| n.has_tag_name((REL, "Relationship")) && n.attribute("Type") == Some(OFFICE))
        .collect::<Vec<_>>();
    if mains.len() != 1
        || mains[0].attribute("Target") != Some("word/document.xml")
        || !matches!(mains[0].attribute("TargetMode"), None | Some("Internal"))
    {
        return Err("DOCX requires one internal word/document.xml main part".into());
    }
    let content_types = part(&mut archive, "[Content_Types].xml")?;
    let content_types = parse(&content_types)?;
    if !content_types.root_element().has_tag_name((CT, "Types")) {
        return Err("Invalid DOCX content types".into());
    }
    let types = content_types
        .root_element()
        .children()
        .filter(|n| {
            n.has_tag_name((CT, "Override"))
                && n.attribute("PartName") == Some("/word/document.xml")
        })
        .collect::<Vec<_>>();
    if types.len() != 1 || types[0].attribute("ContentType") != Some(MAIN) {
        return Err(
            "DOCX main content type is unsupported (macros/templates are not admitted)".into(),
        );
    }
    if archive
        .index_for_name("word/_rels/document.xml.rels")
        .is_some()
    {
        let rels = part(&mut archive, "word/_rels/document.xml.rels")?;
        let rels = parse(&rels)?;
        if !rels.root_element().has_tag_name((REL, "Relationships")) {
            return Err("Invalid DOCX document relationships".into());
        }
        let style_rels = rels.root_element().children().filter(|n|
            n.has_tag_name((REL, "Relationship")) && n.attribute("Type") == Some(
                "http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles"
            )).collect::<Vec<_>>();
        if style_rels.len() > 1
            || style_rels.iter().any(|n| {
                n.attribute("Target") != Some("styles.xml")
                    || !matches!(n.attribute("TargetMode"), None | Some("Internal"))
            })
        {
            return Err("DOCX requires local ordinary styles.xml; external/relocated styles are unsupported".into());
        }
        if !style_rels.is_empty() && archive.index_for_name("word/styles.xml").is_none() {
            return Err("Missing DOCX style part".into());
        }
    }
    // Style-inherited hidden text is as ambiguous as a hidden run. No style resolution guessed.
    if archive.index_for_name("word/styles.xml").is_some() {
        let styles = part(&mut archive, "word/styles.xml")?;
        let styles = parse(&styles)?;
        if styles.descendants().any(|n| {
            n.is_element()
                && n.tag_name().namespace() == Some(W)
                && matches!(n.tag_name().name(), "vanish" | "webHidden" | "specVanish")
        }) {
            return Err("DOCX hidden text styles are unsupported".into());
        }
    }
    let xml = part(&mut archive, "word/document.xml")?;
    let doc = parse(&xml)?;
    let root = doc.root_element();
    if !root.has_tag_name((W, "document")) {
        return Err("Unsupported DOCX WordprocessingML namespace/root".into());
    }
    let bodies = root
        .children()
        .filter(|n| n.has_tag_name((W, "body")))
        .collect::<Vec<_>>();
    if bodies.len() != 1 {
        return Err("DOCX requires exactly one body".into());
    }
    let body = bodies[0];
    for n in body.descendants().filter(Node::is_element) {
        if n.tag_name().namespace() != Some(W) || unsupported(n.tag_name().name()) {
            return Err(format!(
                "Ambiguous/unsupported DOCX body content: {}",
                n.tag_name().name()
            ));
        }
    }
    let mut result = SourceDocument {
        paragraphs: vec![],
        tables: vec![],
    };
    let mut cell_count = 0;
    for block in body.children().filter(Node::is_element) {
        match block.tag_name().name() {
            "p" => {
                if result.paragraphs.len() >= 20_000 {
                    return Err("DOCX exceeds 20000 body paragraphs".into());
                }
                let mut value = String::new();
                text(block, &mut value)?;
                result.paragraphs.push(Paragraph {
                    locator: format!("body/p[{}]", result.paragraphs.len() + 1),
                    text: value,
                });
            }
            "tbl" => {
                if result.tables.len() >= 100 {
                    return Err("DOCX exceeds 100 tables".into());
                }
                let mut rows = vec![];
                let grid_width = block
                    .children()
                    .find(|n| n.has_tag_name((W, "tblGrid")))
                    .map(|n| {
                        n.children()
                            .filter(|c| c.has_tag_name((W, "gridCol")))
                            .count()
                    });
                for child in block.children().filter(Node::is_element) {
                    if matches!(child.tag_name().name(), "tblPr" | "tblGrid") {
                        continue;
                    }
                    if !child.has_tag_name((W, "tr")) {
                        return Err("Unsupported DOCX table content".into());
                    }
                    let mut row = vec![];
                    for cell in child.children().filter(Node::is_element) {
                        if cell.has_tag_name((W, "trPr")) {
                            continue;
                        }
                        if !cell.has_tag_name((W, "tc")) {
                            return Err("Unsupported DOCX table row content".into());
                        }
                        let mut paragraphs = vec![];
                        for p in cell.children().filter(Node::is_element) {
                            if p.has_tag_name((W, "tcPr")) {
                                continue;
                            }
                            if !p.has_tag_name((W, "p")) {
                                return Err(
                                    "Nested/complex DOCX cell content is unsupported".into()
                                );
                            }
                            let mut value = String::new();
                            text(p, &mut value)?;
                            paragraphs.push(value);
                        }
                        let value = paragraphs.join("\n");
                        if paragraphs.is_empty() {
                            return Err("DOCX cell requires an ordinary paragraph".into());
                        }
                        if value.len() > 16_384 || row.len() >= 16384 || cell_count >= 200_000 {
                            return Err("DOCX table exceeds supported cell limits".into());
                        }
                        row.push(value);
                        cell_count += 1;
                    }
                    if row.is_empty()
                        || rows.len() >= 10000
                        || grid_width.is_some_and(|n| n != row.len())
                        || rows
                            .first()
                            .is_some_and(|r: &Vec<String>| r.len() != row.len())
                    {
                        return Err(
                            "DOCX requires ordinary rectangular tables of at most 10000 rows"
                                .into(),
                        );
                    }
                    rows.push(row);
                }
                if rows.is_empty() {
                    return Err("Empty DOCX table is unsupported".into());
                }
                result.tables.push(Table {
                    locator: format!("body/tbl[{}]", result.tables.len() + 1),
                    rows,
                });
            }
            "sectPr" => {}
            _ => return Err("Unsupported DOCX body block".into()),
        }
    }
    Ok(result)
}
