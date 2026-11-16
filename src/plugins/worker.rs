//! One original read-only invocation, using this application's headless entry.
//! A saved intent/permit is not authority: the fresh launcher supplies an
//! in-memory nonce AND the original intent hash over a private stdin pipe.
use super::{
    jobs::{Identity, Intent, Jobs, ResultArtifact, Terminal},
    publish, read,
};
use serde::{Deserialize, Serialize};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Duration,
};

const FLAG: &str = "--openarchi-plugin-worker";
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Permit {
    schema_version: u32,
    identity: Identity,
    intent_sha256: String,
    nonce_sha256: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Handshake {
    nonce: String,
    intent_sha256: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Claim {
    schema_version: u32,
    identity: Identity,
    intent_sha256: String,
    nonce_sha256: String,
    pid: u32,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Progress {
    identity: Identity,
    completed: usize,
}
#[derive(Clone, Debug, Serialize)]
pub struct Observation {
    pub identity: Identity,
    pub status: &'static str,
    pub worker_pid: Option<u32>,
    pub completed: Option<usize>,
    pub ownership: &'static str,
}
fn json<T: Serialize>(value: &T) -> Result<Vec<u8>, String> {
    serde_json::to_vec_pretty(value).map_err(|e| e.to_string())
}
fn parse<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, String> {
    serde_json::from_slice(&read(path)?).map_err(|e| e.to_string())
}
fn intent_hash(intent: &Intent) -> Result<String, String> {
    crate::automation_journal::canonical_hash(intent)
}
fn lease(path: &Path) -> Result<File, String> {
    if !std::fs::symlink_metadata(path)
        .map_err(|e| e.to_string())?
        .is_file()
    {
        return Err("Original job lease is not a regular file".into());
    }
    let mut o = OpenOptions::new();
    o.read(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        o.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let f = o.open(path).map_err(|e| e.to_string())?;
    if !f.metadata().map_err(|e| e.to_string())?.is_file() {
        return Err("Original job lease is not a regular file".into());
    }
    Ok(f)
}
#[cfg(unix)]
fn nonce() -> Result<String, String> {
    let mut bytes = [0u8; 32];
    // Only OS entropy; no PID/time-derived execution capability.
    if unsafe { libc::getentropy(bytes.as_mut_ptr().cast(), bytes.len()) } != 0 {
        return Err(std::io::Error::last_os_error().to_string());
    }
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}
#[cfg(not(unix))]
fn nonce() -> Result<String, String> {
    Err("Persistent plugin execution is unsupported on this platform".into())
}

pub(super) fn launch(
    executable: &Path,
    root: &Path,
    intent: &Intent,
    directory: &Path,
) -> Result<Child, String> {
    let handshake = Handshake {
        nonce: nonce()?,
        intent_sha256: intent_hash(intent)?,
    };
    let permit = Permit {
        schema_version: 1,
        identity: intent.identity.clone(),
        intent_sha256: handshake.intent_sha256.clone(),
        nonce_sha256: crate::drawings::digest(handshake.nonce.as_bytes()),
    };
    publish(
        &directory.join("worker-permit.json"),
        &json(&permit)?,
        false,
    )?;
    publish(&directory.join("worker-lease"), b"", false)?;
    let stderr = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(directory.join("worker-stderr.log"))
        .map_err(|e| e.to_string())?;
    let mut command = Command::new(executable);
    command
        .arg(FLAG)
        .arg(std::path::absolute(root).map_err(|e| e.to_string())?)
        .arg(&intent.identity.project_id)
        .arg(&intent.identity.request_id)
        .env_clear()
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::from(stderr));
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let mut child = command
        .spawn()
        .map_err(|e| format!("Original worker launch unknown; do not replay: {e}"))?;
    // A broken pipe is unknown, never permission to spawn another worker.
    child
        .stdin
        .take()
        .ok_or("Missing original worker stdin")?
        .write_all(&json(&handshake)?)
        .map_err(|e| format!("Original worker handshake unknown; do not replay: {e}"))?;
    Ok(child)
}

pub(super) fn observe(jobs: &Jobs, identity: &Identity) -> Result<Observation, String> {
    let bundle = jobs.query(&identity.project_id, &identity.request_id)?;
    if bundle.intent.identity != *identity {
        return Err("Original worker identity differs".into());
    }
    let directory = jobs.operation(&identity.project_id, &identity.request_id)?;
    let mut observation = Observation {
        identity: identity.clone(),
        status: "OwnerUnknown",
        worker_pid: None,
        completed: None,
        ownership: "original file lease only; PID is diagnostic, never authority to adopt or signal",
    };
    if bundle.result.is_some() {
        observation.status = "Finished";
        return Ok(observation);
    }
    if !directory.join("worker-permit.json").exists() {
        return Ok(observation);
    }
    let permit: Permit = parse(&directory.join("worker-permit.json"))?;
    if permit.schema_version != 1
        || permit.identity != *identity
        || permit.intent_sha256 != intent_hash(&bundle.intent)?
        || !super::jobs::digest(&permit.nonce_sha256)
    {
        return Err("Original worker permit conflicts with its intent".into());
    }
    if !directory.join("worker-claimed.json").exists() {
        return Ok(observation);
    }
    let claim: Claim = parse(&directory.join("worker-claimed.json"))?;
    if claim.schema_version != 1
        || claim.identity != *identity
        || claim.intent_sha256 != permit.intent_sha256
        || claim.nonce_sha256 != permit.nonce_sha256
        || claim.pid == 0
        || claim.pid > i32::MAX as u32
    {
        return Err("Original worker claim conflicts with its intent".into());
    }
    observation.worker_pid = Some(claim.pid);
    let file = lease(&directory.join("worker-lease"))?;
    match file.try_lock() {
        Ok(()) => return Ok(observation),
        Err(std::fs::TryLockError::WouldBlock) => observation.status = "Running",
        Err(std::fs::TryLockError::Error(e)) => return Err(e.to_string()),
    }
    if directory.join("worker-progress.json").exists() {
        let p: Progress = parse(&directory.join("worker-progress.json"))?;
        if p.identity != *identity || p.completed > bundle.intent.input.work_units() {
            return Err("Original worker progress identity or bounds differ".into());
        }
        observation.completed = Some(p.completed);
    }
    Ok(observation)
}

/// Called before GUI setup; recognized malformed worker requests never fall
/// through into opening a window. No nonce is put in argv, logs or the project.
pub fn main_if_requested() -> Option<i32> {
    let args = std::env::args_os().skip(1).collect::<Vec<_>>();
    if args.first().is_none_or(|v| v != FLAG) {
        return None;
    }
    let result = (|| {
        if args.len() != 4 {
            return Err("Invalid original worker arguments".into());
        }
        let project = args[2].to_str().ok_or("Invalid project identity")?;
        let request = args[3].to_str().ok_or("Invalid request identity")?;
        let mut bytes = vec![];
        std::io::stdin()
            .take(1025)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() > 1024 {
            return Err("Invalid original worker handshake".into());
        }
        let handshake: Handshake = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        run(&PathBuf::from(&args[1]), project, request, handshake)
    })();
    Some(match result {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("{e}");
            2
        }
    })
}
fn run(root: &Path, project: &str, request: &str, handshake: Handshake) -> Result<(), String> {
    let jobs = Jobs::new(root.to_owned());
    let bundle = jobs.query(project, request)?;
    let directory = jobs.operation(project, request)?;
    let intent = bundle.intent;
    if bundle.result.is_some() || intent.execution.is_none() {
        return Err("Original request cannot be executed again".into());
    }
    let permit: Permit = parse(&directory.join("worker-permit.json"))?;
    if permit.schema_version != 1
        || permit.identity != intent.identity
        || !super::jobs::digest(&handshake.nonce)
        || handshake.intent_sha256 != intent_hash(&intent)?
        || permit.intent_sha256 != handshake.intent_sha256
        || permit.nonce_sha256 != crate::drawings::digest(handshake.nonce.as_bytes())
    {
        return Err("No fresh authority for this original worker".into());
    }
    let file = lease(&directory.join("worker-lease"))?;
    file.try_lock()
        .map_err(|_| "Original worker already owns execution".to_owned())?;
    publish(
        &directory.join("worker-claimed.json"),
        &json(&Claim {
            schema_version: 1,
            identity: intent.identity.clone(),
            intent_sha256: permit.intent_sha256,
            nonce_sha256: permit.nonce_sha256,
            pid: std::process::id(),
        })?,
        false,
    )?;
    let cancel = Arc::new(AtomicBool::new(bundle.cancel_requested));
    let progress = Arc::new(AtomicUsize::new(0));
    let done = Arc::new(AtomicBool::new(false));
    let watch_error = Arc::new(Mutex::new(None::<String>));
    let cc = cancel.clone();
    let pp = progress.clone();
    let dd = done.clone();
    let ee = watch_error.clone();
    let path = directory.clone();
    let identity = intent.identity.clone();
    let watcher = std::thread::spawn(move || {
        let mut last = None;
        while !dd.load(Ordering::Acquire) {
            let result = (|| -> Result<(), String> {
                if path.join("cancel.json").exists() {
                    let original: Identity = parse(&path.join("cancel.json"))?;
                    if original != identity {
                        return Err("Original cancellation identity differs".into());
                    }
                    cc.store(true, Ordering::Release);
                }
                let n = pp.load(Ordering::Acquire);
                if last != Some(n) {
                    publish(
                        &path.join("worker-progress.json"),
                        &json(&Progress {
                            identity: identity.clone(),
                            completed: n,
                        })?,
                        true,
                    )?;
                    last = Some(n);
                }
                Ok(())
            })();
            if let Err(e) = result {
                *ee.lock().unwrap() = Some(e);
                cc.store(true, Ordering::Release);
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    });
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        super::provider::execute(&intent, &directory, &cancel, &progress)
    }));
    done.store(true, Ordering::Release);
    watcher
        .join()
        .map_err(|_| "Original worker observation failed")?;
    if let Ok(Err(message)) = &outcome {
        if message.starts_with(super::provider::OWNERSHIP_UNKNOWN) {
            return Err(message.clone());
        }
    }
    let terminal = if let Some(message) = watch_error.lock().unwrap().take() {
        Terminal::Failed { message }
    } else if cancel.load(Ordering::Acquire) {
        Terminal::Cancelled
    } else {
        match outcome {
            Ok(Ok(terminal)) => terminal,
            Ok(Err(message)) => Terminal::Failed { message },
            Err(_) => Terminal::Failed {
                message: "Plugin provider panicked".into(),
            },
        }
    };
    // Terminal persistence belongs to the original worker, not the GUI observer.
    publish(
        &directory.join("result.json"),
        &json(&ResultArtifact {
            schema_version: 1,
            identity: intent.identity,
            terminal,
        })?,
        false,
    )
}
