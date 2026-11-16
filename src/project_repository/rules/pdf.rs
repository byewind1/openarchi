//! Fixed-argument local text-PDF provider. No shell, password, OCR or legal verdict.
use crate::{
    project_repository::{
        conversion::{self, ToolIdentity},
        hash, read_bounded,
    },
    source_document::{Paragraph, SourceDocument},
};
use serde::Serialize;
#[cfg(unix)]
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    time::Duration,
};
const OUTPUT_LIMIT: usize = 4 * 1024 * 1024;
#[derive(Clone, Debug, Serialize)]
pub struct PdfTextEvidence {
    pub tool: ToolIdentity,
    pub recipe: String,
    pub layout_sha256: String,
    pub physical_pages: usize,
    pub empty_pages: Vec<usize>,
    pub warnings: String,
}
struct Input {
    root: PathBuf,
    path: PathBuf,
}
impl Input {
    fn create(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > 32 * 1024 * 1024 || !bytes.starts_with(b"%PDF-") {
            return Err("Choose a PDF original no larger than32 MiB".into());
        }
        let root = std::env::temp_dir().join(format!(
            "openarchi-rule-pdf-{}",
            crate::storage::new_document_identity()
        ));
        let mut directory = fs::DirBuilder::new();
        #[cfg(unix)]
        directory.mode(0o700);
        directory
            .create(&root)
            .map_err(|_| "Cannot create a temporary PDF inspection folder")?;
        let input = Self {
            path: root.join("original.pdf"),
            root,
        };
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        options.mode(0o600);
        let mut file = options
            .open(&input.path)
            .map_err(|_| "Cannot create temporary PDF input")?;
        file.write_all(bytes)
            .map_err(|_| "Cannot write temporary PDF input")?;
        Ok(input)
    }
}
impl Drop for Input {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
        let _ = fs::remove_dir(&self.root);
    }
}
pub fn discover() -> Option<PathBuf> {
    let mut dirs = std::env::var_os("PATH")
        .map(|p| {
            std::env::split_paths(&p)
                .filter(|p| p.is_absolute())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    dirs.extend([
        PathBuf::from("/opt/homebrew/bin"),
        PathBuf::from("/usr/local/bin"),
        PathBuf::from("/usr/bin"),
    ]);
    dirs.into_iter()
        .map(|p| p.join("pdftotext"))
        .find(|p| p.is_file())
}
pub fn inspect(bytes: &[u8]) -> Result<(SourceDocument, PdfTextEvidence), String> {
    let provider=discover().ok_or("PDF text provider unavailable; install local Poppler pdftotext and restart the application")?;
    inspect_with_provider(bytes, &provider, Duration::from_secs(30))
}
pub fn inspect_with_provider(
    bytes: &[u8],
    provider: &Path,
    timeout: Duration,
) -> Result<(SourceDocument, PdfTextEvidence), String> {
    if timeout.is_zero() || timeout > Duration::from_secs(30) {
        return Err("PDF inspection timeout must be within30 seconds".into());
    }
    let input = Input::create(bytes)?;
    let executable=fs::canonicalize(provider).map_err(|_|"PDF text provider unavailable; install local Poppler pdftotext and restart the application")?;
    let fingerprint = hash(&read_bounded(&executable, 64 * 1024 * 1024)?);
    let version = conversion::run_bounded(
        &executable,
        &["-v".into()],
        &input.root,
        timeout.min(Duration::from_secs(3)),
        64 * 1024,
    )
    .map_err(|_| "Cannot launch PDF text provider")?;
    if version.timed_out
        || version.exit_code != Some(0)
        || version.stdout.truncated()
        || version.stderr.truncated()
    {
        return Err("PDF text provider version probe failed".into());
    }
    let version_text = format!(
        "{}{}",
        String::from_utf8_lossy(&version.stdout.bytes),
        String::from_utf8_lossy(&version.stderr.bytes)
    );
    if !version_text
        .lines()
        .any(|line| line.starts_with("pdftotext version "))
    {
        return Err("Selected PDF text provider is not identified as pdftotext".into());
    }
    let arguments = [
        "-layout", "-enc", "UTF-8", "-eol", "unix", "-f", "1", "-l", "513",
    ]
    .iter()
    .map(|s| s.to_string())
    .chain([
        input
            .path
            .to_str()
            .ok_or("Non-Unicode PDF temporary path")?
            .into(),
        "-".into(),
    ])
    .collect::<Vec<_>>();
    let capture =
        conversion::run_bounded(&executable, &arguments, &input.root, timeout, OUTPUT_LIMIT)
            .map_err(|_| "Cannot launch PDF text provider")?;
    if capture.timed_out {
        return Err("PDF text inspection timed out".into());
    }
    if capture.exit_code != Some(0) {
        return Err(
            "PDF text inspection failed; the original may be restricted, encrypted or malformed"
                .into(),
        );
    }
    if capture.stdout.truncated() || capture.stderr.truncated() {
        return Err("PDF text inspection exceeded output limits".into());
    }
    // Be conservative with warnings: a successful process may still omit text.
    if capture
        .stderr
        .bytes
        .iter()
        .any(|c| !c.is_ascii_whitespace())
    {
        return Err(
            "PDF text provider reported warnings; review the original before citing".into(),
        );
    }
    let text = std::str::from_utf8(&capture.stdout.bytes)
        .map_err(|_| "PDF text provider output is not UTF-8")?;
    let (document, pages, empty_pages) = parse_layout(text)?;
    if hash(&read_bounded(&executable, 64 * 1024 * 1024)?) != fingerprint {
        return Err("PDF text provider changed during inspection".into());
    }
    Ok((document,PdfTextEvidence{tool:ToolIdentity{executable:executable.to_string_lossy().into(),sha256:fingerprint,version:version_text.trim().into(),license:"External local Poppler provider; GPL license obligations and deployment reviewed separately, no redistribution in this change".into()},recipe:"poppler-layout-utf8-pages-blocks-v1".into(),layout_sha256:hash(&capture.stdout.bytes),physical_pages:pages,empty_pages,warnings:String::new()}))
}
fn parse_layout(text: &str) -> Result<(SourceDocument, usize, Vec<usize>), String> {
    if text.len() > OUTPUT_LIMIT
        || !text.ends_with('\u{c}')
        || text
            .chars()
            .any(|c| c == '\u{fffd}' || (c.is_control() && !matches!(c, '\n' | '\t' | '\u{c}')))
    {
        return Err("PDF text lacks reliable page boundaries or Unicode text".into());
    }
    let pages = text.split_terminator('\u{c}').collect::<Vec<_>>();
    if pages.is_empty() || pages.len() > 512 {
        return Err("PDF original exceeds512 physical pages".into());
    }
    let mut document = SourceDocument {
        paragraphs: vec![],
        tables: vec![],
    };
    let mut empty = vec![];
    for (page_index, page) in pages.iter().enumerate() {
        let mut block = String::new();
        let mut block_index = 0;
        for line in page.lines().chain(std::iter::once("")) {
            if line.trim().is_empty() {
                if !block.is_empty() {
                    block_index += 1;
                    document.paragraphs.push(Paragraph {
                        locator: format!("pdf/page[{}]/block[{}]", page_index + 1, block_index),
                        text: std::mem::take(&mut block),
                    });
                    if document.paragraphs.len() > 10000 {
                        return Err("PDF original exceeds10000 text blocks".into());
                    }
                }
            } else {
                if !block.is_empty() {
                    block.push('\n')
                }
                block.push_str(line.trim_end());
                if block.len() > 16384 {
                    return Err("PDF text block exceeds16 KiB; inspect the original layout".into());
                }
            }
        }
        if block_index == 0 {
            empty.push(page_index + 1)
        }
    }
    if document.paragraphs.is_empty() {
        return Err(
            "PDF original has no extractable text; scanned pages require verified OCR".into(),
        );
    }
    Ok((document, pages.len(), empty))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    #[test]
    fn temporary_original_is_private_and_removed() {
        use std::os::unix::fs::PermissionsExt;
        let input = Input::create(b"%PDF-controlled-permissions-fixture").unwrap();
        let root = input.root.clone();
        let path = input.path.clone();
        assert_eq!(fs::metadata(&root).unwrap().permissions().mode() & 0o077, 0);
        assert_eq!(fs::metadata(&path).unwrap().permissions().mode() & 0o077, 0);
        drop(input);
        assert!(!path.exists() && !root.exists());
    }
    #[test]
    fn page_gaps_and_line_wraps_remain_explicit() {
        let (d, n, empty) =
            parse_layout("Title\n\n A\n B  \n\u{c}\n\u{c}Next page\n\u{c}").unwrap();
        assert_eq!(n, 3);
        assert_eq!(empty, [2]);
        assert_eq!(d.quote("pdf/page[1]/block[2]"), Some(" A\n B"));
        assert_eq!(d.quote("pdf/page[3]/block[1]"), Some("Next page"));
        assert_eq!(d.paragraphs.len(), 3);
    }
    #[test]
    fn no_text_partial_unicode_and_oversized_are_refused() {
        for text in [
            "",
            "\u{c}",
            "No formfeed",
            "Bad\u{fffd}\u{c}",
            "control\0\u{c}",
        ] {
            assert!(parse_layout(text).is_err(), "{text:?}")
        }
        assert!(parse_layout(&("x".repeat(16385) + "\u{c}")).is_err());
        assert!(parse_layout(&"\u{c}".repeat(513)).is_err());
    }
}
