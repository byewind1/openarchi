//! User-facing control of the existing local MCP adapter and native endpoint.
use super::*;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    process::{Child, Command, Stdio},
    time::Duration,
};

fn terminate_adapter(child: &mut Child) {
    // Every adapter starts in its own process group. Killing only uv leaves its
    // Python descendant listening; never signal the application's process group.
    #[cfg(unix)]
    unsafe {
        libc::kill(-(child.id() as i32), libc::SIGKILL);
    }
    let _ = child.kill();
    let _ = child.wait();
}
fn adapter_error_tail(log: &str) -> String {
    log.lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("No adapter log")
        .chars()
        .take(300)
        .collect()
}

fn short_runtime_path() -> PathBuf {
    PathBuf::from("/tmp").join(format!(
        "oa-mcp-{}",
        &crate::storage::new_document_identity()[..16]
    ))
}

#[derive(Serialize, Deserialize)]
#[serde(default)]
struct Preferences {
    enabled: bool,
    port: u16,
    project_dir: String,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            enabled: false,
            port: 17890,
            project_dir: crate::platform::documents_dir()
                .unwrap_or_else(std::env::temp_dir)
                .join("OpenArchi Projects")
                .display()
                .to_string(),
        }
    }
}
#[derive(Default)]
pub(super) struct McpSettings {
    pub(super) open: bool,
    preferences: Preferences,
    child: Option<Child>,
    runtime: Option<PathBuf>,
    error: String,
    token: String,
    owns_endpoint: bool,
    managed: bool,
    authorized_project: Option<String>,
}
impl Drop for McpSettings {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            terminate_adapter(&mut child);
        }
    }
}
impl McpSettings {
    fn persist(&self) -> Result<(), String> {
        let directory =
            crate::platform::config_dir().ok_or("Configuration directory unavailable")?;
        fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
        let bytes = serde_json::to_vec_pretty(&self.preferences).map_err(|e| e.to_string())?;
        fs::write(directory.join("mcp-settings.json"), bytes).map_err(|e| e.to_string())
    }
    fn status(&mut self) -> serde_json::Value {
        if let Some(child) = &mut self.child {
            match child.try_wait() {
                Ok(Some(status)) => {
                    #[cfg(unix)]
                    unsafe {
                        libc::kill(-(child.id() as i32), libc::SIGKILL);
                    }
                    self.error = format!(
                        "MCP adapter exited: {status}. {}",
                        adapter_error_tail(
                            &self
                                .runtime
                                .as_ref()
                                .and_then(|p| fs::read_to_string(p.join("adapter.log")).ok())
                                .unwrap_or_default()
                        )
                    );
                    self.child = None;
                }
                Err(e) => {
                    self.error = e.to_string();
                    self.child = None;
                }
                _ => {}
            }
        }
        if self.child.is_none() {
            return serde_json::Value::Null;
        }
        self.runtime
            .as_ref()
            .and_then(|p| fs::read(p.join("status.json")).ok())
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }
    fn url(&self) -> String {
        format!("http://127.0.0.1:{}/mcp", self.preferences.port)
    }
    fn client_config(&self) -> String {
        serde_json::to_string_pretty(&serde_json::json!({"mcpServers":{"openarchi":{"type":"http","url":self.url(),"headers":{"Authorization":format!("Bearer {}", self.token)}}}})).unwrap()
    }
}
impl AtelierApp {
    #[cfg(unix)]
    pub(super) fn mcp_file_scope(&self) -> Option<Result<PathBuf, &'static str>> {
        if !self.mcp_settings.managed {
            return None;
        }
        Some(
            if self.mcp_settings.child.is_none()
                || self.mcp_settings.authorized_project.as_deref()
                    != Some(self.host.document.editor.project_identity())
            {
                Err("WRITE_NOT_AUTHORIZED")
            } else {
                PathBuf::from(&self.mcp_settings.preferences.project_dir)
                    .canonicalize()
                    .map_err(|_| "FILE_SCOPE_NOT_CONFIGURED")
            },
        )
    }
    pub(super) fn initialize_mcp_settings(&mut self, ctx: &Context) {
        if let Some(directory) = crate::platform::config_dir() {
            if let Ok(bytes) = fs::read(directory.join("mcp-settings.json")) {
                match serde_json::from_slice(&bytes) {
                    Ok(preferences) => self.mcp_settings.preferences = preferences,
                    Err(e) => self.mcp_settings.error = e.to_string(),
                }
            }
        }
        if self.mcp_settings.preferences.enabled {
            self.start_mcp_settings(ctx);
        }
    }
    #[cfg(unix)]
    fn start_mcp_settings(&mut self, ctx: &Context) {
        if self.mcp_settings.child.is_some() {
            return;
        }
        let result = (|| -> Result<(), String> {
            use std::io::{Read, Write};
            use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
            self.mcp_settings.managed = true;
            let prefs = &self.mcp_settings.preferences;
            if prefs.port < 1024 {
                return Err("Port must be between 1024 and 65535".into());
            }
            let project_dir = PathBuf::from(&prefs.project_dir);
            if !project_dir.is_absolute() {
                return Err("Choose an absolute project folder".into());
            }
            fs::create_dir_all(&project_dir).map_err(|e| e.to_string())?;
            let directory =
                crate::platform::config_dir().ok_or("Configuration directory unavailable")?;
            fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
            let token_file = directory.join("mcp-token");
            if !token_file.exists() {
                let mut random = [0u8; 32];
                fs::File::open("/dev/urandom")
                    .and_then(|mut f| f.read_exact(&mut random))
                    .map_err(|e| e.to_string())?;
                let token: String = random.iter().map(|b| format!("{b:02x}")).collect();
                let mut file = fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .mode(0o600)
                    .open(&token_file)
                    .map_err(|e| e.to_string())?;
                file.write_all(token.as_bytes())
                    .map_err(|e| e.to_string())?;
            }
            let metadata = fs::symlink_metadata(&token_file).map_err(|e| e.to_string())?;
            if !metadata.is_file()
                || metadata.file_type().is_symlink()
                || metadata.permissions().mode() & 0o077 != 0
            {
                return Err("MCP token must be a private regular file".into());
            }
            self.mcp_settings.token = fs::read_to_string(&token_file).map_err(|e| e.to_string())?;
            // macOS sun_path is only 104 bytes. Preferences may live at a long
            // user-selected path; keep transient IPC below a short private root.
            let runtime = short_runtime_path();
            fs::DirBuilder::new()
                .mode(0o700)
                .create(&runtime)
                .map_err(|e| e.to_string())?;
            fs::set_permissions(&runtime, fs::Permissions::from_mode(0o700))
                .map_err(|e| e.to_string())?;
            if self.automation.is_none() {
                let mut endpoint = crate::automation_ipc::Endpoint::bind_for_host(
                    runtime.join("ipc/app.sock"),
                    false,
                    Some(
                        std::env::var_os("OPENARCHI_AUTOMATION_JOURNAL_DIR")
                            .map(PathBuf::from)
                            .unwrap_or_else(|| directory.join("automation-journal")),
                    ),
                    &mut self.host,
                )
                .map_err(|e| e.to_string())?;
                let wake = ctx.clone();
                endpoint.set_wake(move || wake.request_repaint());
                self.automation = Some(endpoint);
                self.mcp_settings.owns_endpoint = true;
            }
            self.automation
                .as_mut()
                .unwrap()
                .set_write_enabled(false, &self.host.document.editor);
            self.mcp_settings.authorized_project = None;
            let socket = self.automation.as_ref().unwrap().socket_path().to_owned();
            let bundle = std::env::current_exe().ok().and_then(|p| {
                p.parent()?
                    .parent()
                    .map(|p| p.join("Resources/OpenArchiMCP"))
            });
            let root = bundle
                .filter(|p| p.join("tools/mcp-adapter/server.py").is_file())
                .or_else(|| {
                    #[cfg(debug_assertions)]
                    {
                        Some(PathBuf::from(env!("CARGO_MANIFEST_DIR")))
                    }
                    #[cfg(not(debug_assertions))]
                    {
                        None
                    }
                })
                .ok_or_else(|| {
                    self.language
                        .translate("Bundled MCP adapter is missing")
                        .to_owned()
                })?;
            let adapter = root.join("tools/mcp-adapter");
            let mut command = if adapter.join(".venv/bin/python").is_file() {
                Command::new(adapter.join(".venv/bin/python"))
            } else {
                let home = std::env::var_os("HOME").map(PathBuf::from);
                let uv = home
                    .map(|h| h.join(".local/bin/uv"))
                    .filter(|p| p.is_file())
                    .unwrap_or_else(|| PathBuf::from("uv"));
                let mut command = Command::new(uv);
                command
                    .args(["run", "--offline", "--locked", "--project"])
                    .arg(&adapter)
                    .args(["python", "-B"]);
                command.env("UV_PROJECT_ENVIRONMENT", directory.join("mcp-python"));
                command
            };
            let log = fs::File::create(runtime.join("adapter.log")).map_err(|e| e.to_string())?;
            use std::os::unix::process::CommandExt;
            command.process_group(0);
            command.arg("-B");
            // uv already supplies python -B; repeated -B is valid for Python.
            command
                .arg(adapter.join("server.py"))
                .arg("--socket")
                .arg(socket)
                .arg("--http-port")
                .arg(prefs.port.to_string())
                .arg("--token-file")
                .arg(token_file)
                .arg("--status-file")
                .arg(runtime.join("status.json"));
            command
                .env("PYTHONDONTWRITEBYTECODE", "1")
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(log);
            self.mcp_settings.child = Some(
                command
                    .spawn()
                    .map_err(|e| format!("MCP runtime unavailable: {e}"))?,
            );
            self.mcp_settings.runtime = Some(runtime);
            self.mcp_settings.preferences.enabled = true;
            self.mcp_settings.persist()?;
            Ok(())
        })();
        self.mcp_settings.error = result.err().unwrap_or_default();
    }
    #[cfg(not(unix))]
    fn start_mcp_settings(&mut self, _ctx: &Context) {
        self.mcp_settings.error = "MCP local transport is unavailable on this platform".into();
    }
    fn stop_mcp_settings(&mut self) {
        if let Some(mut child) = self.mcp_settings.child.take() {
            terminate_adapter(&mut child);
        }
        #[cfg(unix)]
        {
            if let Some(endpoint) = &mut self.automation {
                endpoint.set_write_enabled(false, &self.host.document.editor);
            }
            if self.mcp_settings.owns_endpoint {
                self.automation = None;
                self.mcp_settings.owns_endpoint = false;
            }
        }
        self.mcp_settings.authorized_project = None;
        self.mcp_settings.preferences.enabled = false;
        if let Err(e) = self.mcp_settings.persist() {
            self.mcp_settings.error = e;
        }
    }
    pub(super) fn note_object_task_consent(&mut self, allowed: bool) {
        self.mcp_settings.authorized_project =
            allowed.then(|| self.host.document.editor.project_identity().to_owned());
    }
    pub(super) fn refresh_mcp_permissions(&mut self) {
        let _ = self.mcp_settings.status();
        #[cfg(unix)]
        if self.mcp_settings.managed
            && (self.mcp_settings.child.is_none()
                || self
                    .mcp_settings
                    .authorized_project
                    .as_deref()
                    .is_some_and(|id| id != self.host.document.editor.project_identity()))
        {
            if let Some(endpoint) = &mut self.automation {
                endpoint.set_write_enabled(false, &self.host.document.editor);
            }
            self.mcp_settings.authorized_project = None;
        }
    }
    fn local_stdio_available(&self) -> bool {
        #[cfg(unix)]
        {
            self.automation.is_some()
        }
        #[cfg(not(unix))]
        {
            false
        }
    }
    pub(super) fn mcp_settings_window(&mut self, ctx: &Context) {
        let status = self.mcp_settings.status();
        #[cfg(unix)]
        if self
            .mcp_settings
            .authorized_project
            .as_deref()
            .is_some_and(|id| id != self.host.document.editor.project_identity())
        {
            if let Some(endpoint) = &mut self.automation {
                endpoint.set_write_enabled(false, &self.host.document.editor);
            }
            self.mcp_settings.authorized_project = None;
        }
        if !self.mcp_settings.open {
            return;
        }
        let language = self.language;
        let mut open = true;
        Window::new(language.translate("MCP Settings…")).id(Id::new("mcp_settings")).open(&mut open).default_width(590.).resizable(true).show(ctx, |ui| {
            ui.label(language.translate("Connect an external AI agent to the current project."));
            ui.label(format!("{}: {}", language.translate("Current project"), self.host.document.editor.project.name));
            let running = self.mcp_settings.child.is_some();
            let listening = running && status["listening"] == true;
            let state = if listening { if status["requests"].as_u64().unwrap_or(0)>0 { "Listening · requests received" } else { "Listening · waiting for an agent" } } else if running { "Starting MCP service…" } else if self.local_stdio_available() { "HTTP is off · local stdio available" } else { "MCP service is off" };
            ui.strong(language.translate(state));
            if running { ui.label(format!("{}: {}",language.translate("Authenticated requests"),status["requests"].as_u64().unwrap_or(0))); }
            ui.horizontal(|ui| {
                if !running && ui.button(language.translate("Start MCP service")).clicked() { self.start_mcp_settings(ctx); }
                if (running || self.mcp_settings.preferences.enabled || self.mcp_settings.owns_endpoint) && ui.button(language.translate("Stop MCP service")).clicked() { self.stop_mcp_settings(); }
            });
            ui.separator();
            ui.label(language.translate("Local connection · Streamable HTTP"));
            ui.horizontal_wrapped(|ui| {
                ui.monospace(self.mcp_settings.url());
                if ui.add_enabled(listening, Button::new(language.translate("Copy MCP address"))).clicked() { ctx.copy_text(self.mcp_settings.url()); }
            });
            ui.label(language.translate("Bearer authentication is required. The configuration includes your private connection token."));
            ui.horizontal(|ui| {
                if ui.add_enabled(listening, Button::new(language.translate("Copy MCP configuration"))).clicked() { ctx.copy_text(self.mcp_settings.client_config()); }
                if ui.add_enabled(listening, Button::new(language.translate("Copy connection token"))).clicked() { ctx.copy_text(self.mcp_settings.token.clone()); }
            });
            if ui.add_enabled(listening, Button::new(language.translate("Copy Codex configuration"))).clicked() {
                ctx.copy_text(format!("[mcp_servers.openarchi]\nurl = {:?}\nhttp_headers = {{ Authorization = {:?} }}\n", self.mcp_settings.url(), format!("Bearer {}", self.mcp_settings.token)));
            }
            ui.label(language.translate("In your agent, add a Streamable HTTP MCP server using this address and Bearer token."));
            ui.add_enabled_ui(!running, |ui| {
                ui.horizontal(|ui| { ui.label(language.translate("MCP port")); ui.add(DragValue::new(&mut self.mcp_settings.preferences.port).range(1024..=65535)); });
                ui.label(language.translate("MCP project save folder"));
                ui.horizontal(|ui| {
                    ui.add(TextEdit::singleline(&mut self.mcp_settings.preferences.project_dir).desired_width(420.));
                    if ui.button(language.translate("Choose folder…")).clicked() {
                        if let Some(path) = rfd::FileDialog::new().pick_folder() { self.mcp_settings.preferences.project_dir = path.display().to_string(); }
                    }
                });
            });
            ui.label(language.translate("Stop the service before changing the port or folder. Start it to save these settings."));
            #[cfg(unix)] {
                let can_authorize=listening || (!self.mcp_settings.managed && self.local_stdio_available());
                let mut writable = self.automation.as_mut().is_some_and(|e| e.changes().enabled());
                let task_control=self.host.operations.borrow().requires_object_task_control(&self.host.document.editor);
                if task_control {ui.label(language.translate("Use the original object task controls to resume its scope"));}
                if ui.add_enabled(can_authorize&&(!task_control||writable), Checkbox::new(&mut writable, language.translate("Allow AI to modify this project"))).changed() {
                    if let Some(endpoint) = &mut self.automation { endpoint.set_write_enabled(writable, &self.host.document.editor); }
                    self.mcp_settings.authorized_project = writable.then(||self.host.document.editor.project_identity().to_owned());
                }
                ui.label(language.translate("Starts read-only. Write permission resets when the app restarts or another project opens."));
                if self.automation.as_mut().is_some_and(|e| { let changes=e.changes(); changes.enabled() && changes.write_authorization(&self.host.document.editor).is_err() }) {
                    ui.label(language.translate("Remote writes paused after a model edit. Reading a new revision does not renew permission."));
                    if ui.add_enabled(can_authorize&&!task_control, Button::new(language.translate("Authorize the current model state"))).clicked() {
                        if let Some(endpoint)=&mut self.automation {endpoint.set_write_enabled(true,&self.host.document.editor);}
                        self.mcp_settings.authorized_project=Some(self.host.document.editor.project_identity().to_owned());
                    }
                }

                if let Some(endpoint) = &self.automation {
                    ui.collapsing(language.translate("Advanced · local stdio adapter"), |ui| { ui.monospace(endpoint.socket_path().display().to_string()); ui.label(language.translate("Existing stdio clients can use this native socket through the OpenArchi MCP adapter.")); });
                }
            }
            if !self.mcp_settings.error.is_empty() { ui.separator(); ui.label(language.translate("MCP service error")); ui.label(&self.mcp_settings.error); ui.label(language.translate("Check the port and local Python/uv runtime. No dependencies are downloaded automatically.")); }
            if ui.button(language.translate("AI Operations…")).clicked() { self.automation_window = true; }
        });
        self.mcp_settings.open = open;
        ctx.request_repaint_after(Duration::from_millis(250));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[cfg(unix)]
    fn stopping_adapter_terminates_its_owned_process_group() {
        use std::os::unix::process::CommandExt;
        let mut child = Command::new("/bin/sh")
            .args(["-c", "sleep 30 & wait"])
            .process_group(0)
            .spawn()
            .unwrap();
        assert_eq!(
            unsafe { libc::getpgid(child.id() as i32) },
            child.id() as i32
        );
        terminate_adapter(&mut child);
        assert!(child.try_wait().unwrap().is_some());
        assert_eq!(
            adapter_error_tail("Traceback\nnoise\nOSError: Address already in use\n"),
            "OSError: Address already in use"
        );
    }
    #[test]
    fn native_socket_stays_below_macos_path_limit() {
        let a = short_runtime_path();
        let b = short_runtime_path();
        assert_ne!(a, b);
        assert!(a.join("ipc/app.sock").as_os_str().len() < 80);
        assert!(!a.to_string_lossy().contains("Application Support"));
    }
    #[test]
    fn mcp_menu_is_bilingual_and_does_not_edit_project() {
        let ctx = Context::default();
        let mut app = AtelierApp::from_renderer(None, "test".into());
        let original = app.host.document.editor.project.clone();
        let session = app.host.document.editor.document_session();
        for language in [UiLanguage::English, UiLanguage::SimplifiedChinese] {
            app.language = language;
            app.open_command_preview("MCP Settings…".into());
            assert!(app.mcp_settings.open);
            app.mcp_settings.preferences.enabled = true;
            let mut output = None;
            for _ in 0..2 {
                let mut frame = ctx.run_ui(
                    RawInput {
                        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1400., 1000.))),
                        ..Default::default()
                    },
                    |ui| {
                        app.mcp_settings_window(ui.ctx());
                    },
                );
                frame.textures_delta.clear();
                output = Some(frame);
            }
            let mut text = Vec::new();
            super::super::ui_tests::collect_text(&output.unwrap().shapes, &mut text);
            assert!(
                text.iter()
                    .any(|s| s == language.translate("MCP service is off")),
                "{text:?}"
            );
            assert!(
                text.iter()
                    .any(|s| s == language.translate("Start MCP service"))
            );
            assert!(text.iter().any(|s| s.contains("127.0.0.1:17890/mcp")));
            assert!(
                text.iter()
                    .any(|s| s == language.translate("Stop MCP service")),
                "a failed auto-start must remain stoppable"
            );
            assert_eq!(app.host.document.editor.project, original);
            assert_eq!(app.host.document.editor.document_session(), session);
            assert!(!app.host.document.editor.dirty());
        }
    }
    #[test]
    #[cfg(unix)]
    fn managed_stopped_service_cannot_fall_back_to_environment_write_permission() {
        let mut app = AtelierApp::from_renderer(None, "test".into());
        assert!(app.mcp_file_scope().is_none());
        app.mcp_settings.managed = true;
        app.mcp_settings.authorized_project =
            Some(app.host.document.editor.project_identity().to_owned());
        assert!(matches!(
            app.mcp_file_scope(),
            Some(Err("WRITE_NOT_AUTHORIZED"))
        ));
        app.refresh_mcp_permissions();
        assert!(app.mcp_settings.authorized_project.is_none());
        let serialized = serde_json::to_value(&app.mcp_settings.preferences).unwrap();
        assert!(serialized.get("authorized_project").is_none());
        assert!(serialized.get("token").is_none());
    }
}
