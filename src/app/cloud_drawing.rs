//! Bounded HTTPS image download bridge. No simulated BIMcloud login or project list.
use super::*;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
#[derive(Default)]
pub(super) struct CloudDrawingUi {
    pub open: bool,
    url: String,
    error: String,
    task: Option<Download>,
}
struct Download {
    rx: mpsc::Receiver<Result<Vec<u8>, String>>,
    cancel: Arc<AtomicBool>,
    scope: Option<u64>,
    story: u32,
}
impl Drop for Download {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}
fn valid_url(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("https://") else {
        return false;
    };
    let host = rest.split('/').next().unwrap_or("");
    !host.is_empty()
        && !host.contains('@')
        && url.len() < 8192
        && !url.chars().any(|c| c.is_whitespace() || c.is_control())
}
fn download(url: String, cancel: Arc<AtomicBool>) -> Result<Vec<u8>, String> {
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let dir = crate::platform::config_dir()
        .ok_or("Drawing cache is unavailable")?
        .join("drawing-downloads");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(format!(
        "{}.{}.part",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    let _file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|e| e.to_string())?;
    let result = (|| {
        let mut child = std::process::Command::new("/usr/bin/curl")
            .args([
                "--fail",
                "--silent",
                "--location",
                "--proto",
                "=https",
                "--proto-redir",
                "=https",
                "--max-filesize",
                "33554432",
                "--connect-timeout",
                "10",
                "--max-time",
                "60",
                "--output",
            ])
            .arg(&path)
            .arg("--")
            .arg(url)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|_| "HTTPS downloader unavailable")?;
        loop {
            if cancel.load(Ordering::Relaxed) {
                let _ = child.kill();
                let _ = child.wait();
                return Err("Download cancelled".into());
            }
            if let Some(status) = child.try_wait().map_err(|_| "Download failed")? {
                if !status.success() {
                    return Err("Image download failed; verify the authorized download link".into());
                }
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        let bytes = crate::drawings::source_bytes(&path)?;
        crate::drawings::decode(&bytes)?;
        Ok(bytes)
    })();
    let _ = std::fs::remove_file(path);
    result
}
impl AtelierApp {
    pub(super) fn cloud_drawing_window(&mut self, ctx: &Context) {
        if self.cloud_drawing.task.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
        if let Some(task) = self.cloud_drawing.task.as_ref()
            && let Ok(result) = task.rx.try_recv()
        {
            let scope = task.scope;
            let story = task.story;
            self.cloud_drawing.task = None;
            let result = result.and_then(|bytes| {
                if self
                    .host
                    .document
                    .editor
                    .project
                    .plan_options
                    .active_worksheet
                    != scope
                    || self.host.document.editor.story != story
                {
                    return Err("Active canvas changed; start the placement again".into());
                }
                let path = crate::drawings::store(&bytes, &self.drawing_cache()?)?;
                self.drawings_ui.path = path.display().to_string();
                self.import_drawing()?;
                if let Some(page) = self.drawings_ui.pending.as_mut() {
                    page.name = "Downloaded image".into();
                    page.linked_source = None;
                }
                Ok(())
            });
            match result {
                Ok(()) => self.cloud_drawing.open = false,
                Err(e) => self.cloud_drawing.error = e,
            }
        }
        if !self.cloud_drawing.open {
            return;
        }
        let mut open = true;
        let lang = self.language;
        Window::new(lang.translate("Place External Drawing from BIMcloud…")).id(Id::new("cloud_drawing")).open(&mut open).default_width(550.).show(ctx,|ui|{
            ui.label(lang.translate("Use an authorized HTTPS image download link. BIMcloud account/project browsing is not connected."));
            ui.label(lang.translate("Download link (session only)"));ui.add(egui::TextEdit::singleline(&mut self.cloud_drawing.url).password(true));
            if ui.add_enabled(self.cloud_drawing.task.is_none(),Button::new(lang.translate("Download and place image"))).clicked(){
                let url=self.cloud_drawing.url.trim().to_owned();
                if !valid_url(&url){self.cloud_drawing.error="Enter an HTTPS link without embedded credentials".into();}
                else {
                    let (tx,rx)=mpsc::channel();let cancel=Arc::new(AtomicBool::new(false));let worker_cancel=cancel.clone();
                    std::thread::spawn(move||{let _=tx.send(download(url,worker_cancel));});
                    self.cloud_drawing.task=Some(Download{rx,cancel,scope:self.host.document.editor.project.plan_options.active_worksheet,story:self.host.document.editor.story});self.cloud_drawing.error.clear();
                }
            }
            if self.cloud_drawing.task.is_some(){ui.spinner();if ui.button(lang.translate("Cancel")).clicked(){self.cloud_drawing.task=None;}}
            if !self.cloud_drawing.error.is_empty(){ui.colored_label(Color32::RED,lang.translate(&self.cloud_drawing.error));}
        });
        self.cloud_drawing.open = open;
        if !open {
            self.cloud_drawing.task = None;
            self.cloud_drawing.url.clear();
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cloud_link_accepts_only_explicit_https_without_authority_credentials() {
        assert!(valid_url(
            "https://example.test/authorized.png?signature=abc"
        ));
        for s in [
            "http://example.test/a",
            "file:///tmp/a",
            "https://user:secret@example.test/a",
            "https://",
            "https://example.test/a\n",
        ] {
            assert!(!valid_url(s));
        }
    }
}
