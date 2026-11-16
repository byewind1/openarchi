//! Explicitly reviewed local executable boundary. Process separation is NOT an OS sandbox.
use super::{
    MAX_BYTES,
    jobs::{Bundle, Identity, Intent, ResultArtifact, Terminal},
    publish,
};
use crate::project_repository::conversion::CapturedLog;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Declaration {
    pub protocol_version: u32,
    pub timeout_ms: u64,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutableGrant {
    pub path: PathBuf,
    pub sha256: String,
}
fn executable_bytes(path: &Path) -> Result<Vec<u8>, String> {
    if !path.is_absolute() || path.is_symlink() {
        return Err("Choose an absolute regular executable, not a symlink".into());
    }
    // Reject devices/FIFOs before opening; the Unix flags also prevent a race
    // with a symlink/FIFO from blocking the native review action.
    let initial = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if !initial.is_file() || initial.len() > 32 * 1024 * 1024 {
        return Err("Local provider must be a regular file within 32 MiB".into());
    }
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let file = options.open(path).map_err(|e| e.to_string())?;
    let metadata = file.metadata().map_err(|e| e.to_string())?;
    if !metadata.is_file() || metadata.len() > 32 * 1024 * 1024 {
        return Err("Local provider must be a regular file within 32 MiB".into());
    }
    let mut bytes = vec![];
    file.take(32 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 32 * 1024 * 1024 {
        return Err("Provider changed beyond its size limit".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o111 == 0 {
            return Err("The reviewed file is not executable".into());
        }
    }
    Ok(bytes)
}
impl ExecutableGrant {
    /// Read identity only; does not launch, install or change the file.
    pub fn review(path: &Path) -> Result<Self, String> {
        let path = fs::canonicalize(path).map_err(|e| e.to_string())?;
        let bytes = executable_bytes(&path)?;
        Ok(Self {
            path,
            sha256: crate::drawings::digest(&bytes),
        })
    }
    pub fn validate(&self) -> Result<(), String> {
        if !self.path.is_absolute() || !super::jobs::digest(&self.sha256) {
            return Err("Invalid reviewed executable identity".into());
        }
        Ok(())
    }
    fn stage(&self, directory: &Path) -> Result<PathBuf, String> {
        self.validate()?;
        let bytes = executable_bytes(&self.path)?;
        if crate::drawings::digest(&bytes) != self.sha256 {
            return Err("Reviewed executable changed; explicit review is required again".into());
        }
        let path = directory.join("provider.bin");
        let mut f = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|e| e.to_string())?;
        f.write_all(&bytes)
            .and_then(|_| f.sync_all())
            .map_err(|e| e.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            f.set_permissions(fs::Permissions::from_mode(0o500))
                .map_err(|e| e.to_string())?;
        }
        Ok(path)
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub protocol_version: u32,
    pub intent: Intent,
}
#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Event {
    Progress {
        identity: Identity,
        completed: usize,
    },
    Result {
        artifact: ResultArtifact,
    },
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Capture {
    pub identity: Identity,
    pub pid: u32,
    pub exit_code: Option<i32>,
    pub timed_out: bool,
    pub cancelled: bool,
    pub protocol_error: Option<String>,
    pub stdout: CapturedLog,
    pub stderr: CapturedLog,
}
impl Capture {
    pub(super) fn validate(&self, identity: &Identity) -> Result<(), String> {
        if &self.identity != identity
            || self.pid == 0
            || self.pid > i32::MAX as u32
            || self.protocol_error.as_ref().is_some_and(|s| s.len() > 8192)
            || [&self.stdout, &self.stderr].iter().any(|log| {
                log.bytes.len() > 64 * 1024 || log.total_bytes < (log.bytes.len() as u64)
            })
        {
            return Err("Provider process receipt identity or bounds differ".into());
        }
        Ok(())
    }
}
pub(super) const OWNERSHIP_UNKNOWN: &str =
    "Original process ownership unknown; no safe termination receipt";
struct OwnedChild(Child, bool, Option<std::process::ExitStatus>);
impl OwnedChild {
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    fn exited(&mut self) -> Result<bool, String> {
        // Observe without reaping: the original child's PID cannot be recycled
        // before group cleanup. A PID copied from disk is never used here.
        let mut info = unsafe { std::mem::zeroed::<libc::siginfo_t>() };
        let result = unsafe {
            libc::waitid(
                libc::P_PID,
                self.0.id() as libc::id_t,
                &mut info,
                libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
            )
        };
        if result != 0 {
            return Err(format!(
                "{OWNERSHIP_UNKNOWN}: {}",
                std::io::Error::last_os_error()
            ));
        }
        #[cfg(target_os = "macos")]
        let pid = info.si_pid;
        #[cfg(target_os = "linux")]
        let pid = unsafe { info.si_pid() };
        if pid != 0 && pid != self.0.id() as i32 {
            return Err(OWNERSHIP_UNKNOWN.into());
        }
        Ok(pid != 0)
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    fn exited(&mut self) -> Result<bool, String> {
        self.2 = self.0.try_wait().map_err(|e| e.to_string())?;
        self.1 = self.2.is_some();
        Ok(self.1)
    }
    fn stop(&mut self) -> Result<(), String> {
        if self.1 {
            return Ok(());
        }
        self.exited()?;
        if self.1 {
            return Ok(());
        }
        #[cfg(unix)]
        unsafe {
            if libc::kill(-(self.0.id() as i32), libc::SIGKILL) != 0 {
                let error = std::io::Error::last_os_error();
                // Darwin reports EPERM for an owned zombie-only group. Never
                // generalize this to a live child or a PID read from storage.
                if error.raw_os_error() != Some(libc::ESRCH)
                    && !(error.raw_os_error() == Some(libc::EPERM) && self.exited()?)
                {
                    return Err(format!("{OWNERSHIP_UNKNOWN}: {error}"));
                }
            }
        }
        #[cfg(not(unix))]
        {
            let _ = self.0.kill();
        }
        self.2 = Some(
            self.0
                .wait()
                .map_err(|e| format!("{OWNERSHIP_UNKNOWN}: {e}"))?,
        );
        self.1 = true;
        Ok(())
    }
}
impl Drop for OwnedChild {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}
fn empty_log() -> CapturedLog {
    CapturedLog {
        bytes: vec![],
        total_bytes: 0,
    }
}
fn append(log: &mut CapturedLog, bytes: &[u8]) {
    log.total_bytes = log.total_bytes.saturating_add(bytes.len() as u64);
    let keep = bytes
        .len()
        .min((64 * 1024usize).saturating_sub(log.bytes.len()));
    log.bytes.extend_from_slice(&bytes[..keep]);
}
fn accept(
    event: Result<Event, String>,
    intent: &Intent,
    progress: &AtomicUsize,
    error: &mut Option<String>,
    result: &mut Option<ResultArtifact>,
) {
    match event {
        Err(e) => *error = Some(e),
        Ok(Event::Progress {
            identity,
            completed,
        }) => {
            if identity != intent.identity
                || completed > intent.input.elements.len()
                || completed < progress.load(Ordering::Acquire)
                || result.is_some()
            {
                *error = Some("Provider progress identity/order differs".into());
            } else {
                progress.store(completed, Ordering::Release);
            }
        }
        Ok(Event::Result { artifact }) => {
            if artifact.identity != intent.identity || result.is_some() {
                *error = Some("Provider result identity/order differs".into());
            } else {
                *result = Some(artifact);
            }
        }
    }
}
/// Runs once under an already persisted intent. Only the job worker calls this.
pub(super) fn execute(
    intent: &Intent,
    directory: &Path,
    cancel: &AtomicBool,
    progress: &AtomicUsize,
) -> Result<Terminal, String> {
    if cancel.load(Ordering::Acquire) {
        return Err("Job cancelled".into());
    }
    let declaration = intent
        .declaration
        .provider
        .as_ref()
        .ok_or("Missing provider declaration")?;
    let executable = intent
        .execution
        .as_ref()
        .ok_or("Executable was not explicitly authorized")?;
    let runtime = directory.join("runtime");
    fs::create_dir(&runtime).map_err(|e| e.to_string())?;
    let path = executable.stage(&runtime)?;
    if cancel.load(Ordering::Acquire) {
        return Err("Job cancelled".into());
    }
    let mut command = Command::new(&path);
    command
        .env_clear()
        .current_dir(&runtime)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let mut child = OwnedChild(
        command
            .spawn()
            .map_err(|e| format!("Provider could not start: {e}"))?,
        false,
        None,
    );
    let pid = child.0.id();
    publish(&directory.join("process.json"),&serde_json::to_vec(&serde_json::json!({"identity":intent.identity,"pid":pid,"executable_sha256":executable.sha256,"state":"spawned; query original result, never replay"})).map_err(|e|e.to_string())?,false)?;
    let mut stdin = child.0.stdin.take().ok_or("Missing provider stdin")?;
    let stdout = child.0.stdout.take().ok_or("Missing provider stdout")?;
    let stderr = child.0.stderr.take().ok_or("Missing provider stderr")?;
    let request = serde_json::to_vec(&Request {
        protocol_version: 1,
        intent: intent.clone(),
    })
    .map_err(|e| e.to_string())?;
    let writer = thread::spawn(move || {
        stdin
            .write_all(&request)
            .and_then(|_| stdin.write_all(b"\n"))
    });
    let out_log = Arc::new(Mutex::new(empty_log()));
    let err_log = Arc::new(Mutex::new(empty_log()));
    let overflow = Arc::new(AtomicBool::new(false));
    let (tx, rx) = mpsc::sync_channel(8);
    let out = out_log.clone();
    let over = overflow.clone();
    let reader = thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        let mut frames = 0;
        loop {
            let mut bytes = vec![];
            let read = (&mut reader)
                .take(MAX_BYTES + 1)
                .read_until(b'\n', &mut bytes);
            if matches!(read, Ok(0)) {
                break;
            }
            {
                let mut log = out.lock().unwrap();
                append(&mut log, &bytes);
                if log.total_bytes > 8 * 1024 * 1024 {
                    over.store(true, Ordering::Release);
                }
            }
            frames += 1;
            if read.is_err()
                || bytes.len() as u64 > MAX_BYTES
                || !bytes.ends_with(b"\n")
                || frames > 256
            {
                let _ = tx.send(Err(
                    "Provider frame/stream limit or incomplete frame".to_string()
                ));
                break;
            }
            let event = serde_json::from_slice::<Event>(&bytes)
                .map_err(|_| "Invalid provider protocol frame".to_string());
            let failed = event.is_err();
            if tx.send(event).is_err() || failed {
                break;
            }
        }
    });
    let err = err_log.clone();
    let over = overflow.clone();
    let errors = thread::spawn(move || {
        let mut stderr = stderr;
        let mut buf = [0u8; 8192];
        while let Ok(n) = stderr.read(&mut buf) {
            if n == 0 {
                break;
            }
            let mut log = err.lock().unwrap();
            append(&mut log, &buf[..n]);
            if log.total_bytes > 8 * 1024 * 1024 {
                over.store(true, Ordering::Release);
            }
        }
    });
    let start = Instant::now();
    let mut protocol_error = None;
    let mut result = None;
    let mut timed_out;
    let mut cancelled;
    loop {
        for event in rx.try_iter() {
            accept(event, intent, progress, &mut protocol_error, &mut result);
        }
        cancelled = cancel.load(Ordering::Acquire);
        timed_out = start.elapsed() >= Duration::from_millis(declaration.timeout_ms);
        if overflow.load(Ordering::Acquire) {
            protocol_error = Some("Provider output exceeds its stream budget".into());
        }
        if cancelled || timed_out || protocol_error.is_some() {
            child.stop()?;
            break;
        }
        if child.exited()? && reader.is_finished() && errors.is_finished() {
            break;
        }
        thread::sleep(Duration::from_millis(5));
    }
    child.stop()?;
    let status = child.2;
    // Keep draining the bounded channel during cleanup. If an unowned process
    // retained a pipe, leave the original outcome unknown instead of hanging
    // forever or claiming cancellation without a complete termination receipt.
    let cleanup_deadline = Instant::now() + Duration::from_secs(1);
    while !writer.is_finished() || !reader.is_finished() || !errors.is_finished() {
        for event in rx.try_iter() {
            accept(event, intent, progress, &mut protocol_error, &mut result);
        }
        if Instant::now() >= cleanup_deadline {
            return Err(format!("{OWNERSHIP_UNKNOWN}: original pipes did not close"));
        }
        thread::sleep(Duration::from_millis(1));
    }
    let write_result = writer.join().map_err(|_| "Provider input writer failed")?;
    reader.join().map_err(|_| "Provider output reader failed")?;
    errors.join().map_err(|_| "Provider error reader failed")?;
    // Reader may have queued the final frame between the last poll and exit.
    for event in rx.try_iter() {
        accept(event, intent, progress, &mut protocol_error, &mut result);
    }
    let capture = Capture {
        identity: intent.identity.clone(),
        pid,
        exit_code: status.and_then(|s| s.code()),
        timed_out,
        cancelled,
        protocol_error: protocol_error.clone(),
        stdout: Arc::try_unwrap(out_log)
            .map_err(|_| "Provider log still owned")?
            .into_inner()
            .map_err(|_| "Provider log unavailable")?,
        stderr: Arc::try_unwrap(err_log)
            .map_err(|_| "Provider log still owned")?
            .into_inner()
            .map_err(|_| "Provider log unavailable")?,
    };
    publish(
        &directory.join("process-result.json"),
        &serde_json::to_vec_pretty(&capture).map_err(|e| e.to_string())?,
        false,
    )?;
    if cancelled {
        return Err("Job cancelled".into());
    }
    if timed_out {
        return Err("Provider timed out; original process stopped".into());
    }
    if let Some(e) = protocol_error {
        return Err(e);
    }
    write_result.map_err(|e| e.to_string())?;
    if capture.exit_code != Some(0) {
        return Err("Provider exited unsuccessfully; inspect original process receipt".into());
    }
    let artifact = result.ok_or("Provider exited without a result")?;
    Bundle {
        intent: intent.clone(),
        result: Some(artifact.clone()),
        cancel_requested: false,
    }
    .validate()?;
    Ok(artifact.terminal)
}
