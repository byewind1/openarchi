//! Explicit local CLI boundary: conversion is never a claim of lossless import.
use super::*;
use std::{
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};
const LOG_LIMIT: usize = 64 * 1024;
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CapturedLog {
    pub bytes: Vec<u8>,
    pub total_bytes: u64,
}
impl CapturedLog {
    pub fn truncated(&self) -> bool {
        self.total_bytes > self.bytes.len() as u64
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProcessCapture {
    pub exit_code: Option<i32>,
    pub timed_out: bool,
    pub stdout: CapturedLog,
    pub stderr: CapturedLog,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ToolIdentity {
    pub executable: String,
    pub sha256: String,
    pub version: String,
    pub license: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConversionReceipt {
    pub schema_version: u32,
    pub operation_id: String,
    pub source_id: String,
    pub tool: ToolIdentity,
    pub arguments: Vec<String>,
    pub capture: Option<ProcessCapture>,
    pub error: Option<String>,
    pub derived_source_id: Option<String>,
    pub status: String,
    pub coverage: String,
}
fn drain(mut reader: impl Read, limit: usize) -> CapturedLog {
    let mut bytes = Vec::new();
    let mut total = 0;
    let mut buffer = [0u8; 8192];
    while let Ok(n) = reader.read(&mut buffer) {
        if n == 0 {
            break;
        }
        total += n as u64;
        let keep = n.min(limit - bytes.len());
        bytes.extend_from_slice(&buffer[..keep]);
    }
    CapturedLog {
        bytes,
        total_bytes: total,
    }
}
fn stop_group(child: &mut std::process::Child) {
    #[cfg(unix)]
    unsafe {
        libc::kill(-(child.id() as i32), libc::SIGKILL);
    }
    #[cfg(not(unix))]
    {
        let _ = child.kill();
    }
}
fn run(
    executable: &Path,
    args: &[String],
    cwd: &Path,
    timeout: Duration,
) -> Result<ProcessCapture, String> {
    run_bounded(executable, args, cwd, timeout, LOG_LIMIT)
}
pub(crate) fn run_bounded(
    executable: &Path,
    args: &[String],
    cwd: &Path,
    timeout: Duration,
    stdout_limit: usize,
) -> Result<ProcessCapture, String> {
    if stdout_limit > 4 * 1024 * 1024 {
        return Err("Process capture limit exceeds4 MiB".into());
    }
    let mut command = Command::new(executable);
    command
        .args(args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let mut child = command.spawn().map_err(|e| e.to_string())?;
    let stdout = child.stdout.take().ok_or("No converter stdout")?;
    let stderr = child.stderr.take().ok_or("No converter stderr")?;
    let out = thread::spawn(move || drain(stdout, stdout_limit));
    let err = thread::spawn(move || drain(stderr, LOG_LIMIT));
    let start = Instant::now();
    let mut timed_out = false;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Ok(status),
            Ok(None) => {}
            Err(error) => break Err(error),
        }
        if start.elapsed() >= timeout {
            timed_out = true;
            stop_group(&mut child);
            break child.wait();
        }
        thread::sleep(Duration::from_millis(10));
    };
    // Own process group only, including descendants holding output pipes open.
    stop_group(&mut child);
    let _ = child.wait();
    let stdout = out.join().map_err(|_| "Converter stdout reader failed")?;
    let stderr = err.join().map_err(|_| "Converter stderr reader failed")?;
    Ok(ProcessCapture {
        exit_code: status.map_err(|e| e.to_string())?.code(),
        timed_out,
        stdout,
        stderr,
    })
}
impl Repository {
    pub fn latest_conversion(&self) -> Result<Option<ConversionReceipt>, String> {
        self.manifest
            .conversions
            .last()
            .map(|item| {
                resource(&self.root, item, 4 * 1024 * 1024)
                    .and_then(|bytes| serde_json::from_slice(&bytes).map_err(|e| e.to_string()))
            })
            .transpose()
    }
    /// Store receipt and any derived DXF in one guarded manifest publication.
    /// Failed jobs retain receipts but do not admit an output as a source.
    pub fn convert_dwg(
        &mut self,
        source_id: &str,
        executable: &Path,
        timeout: Duration,
    ) -> Result<ConversionReceipt, String> {
        if timeout.is_zero() || timeout > Duration::from_secs(300) {
            return Err("Conversion timeout must be within 300 seconds".into());
        }
        self.unchanged()?;
        self.validate()?;
        let source = self
            .manifest
            .sources
            .iter()
            .find(|s| s.id == source_id && s.format == "dwg")
            .ok_or("Select a stored DWG source")?;
        let input = resource(&self.root, &source.blob, MAX_SOURCE)?;
        if executable.as_os_str().is_empty() {
            return Err("Choose a local dwg2dxf executable".into());
        }
        let executable =
            fs::canonicalize(executable).map_err(|e| format!("Converter unavailable: {e}"))?;
        let binary = read_bounded(&executable, 32 * 1024 * 1024)?;
        let fingerprint = hash(&binary);
        let version = run(
            &executable,
            &["--version".into()],
            &self.root,
            timeout.min(Duration::from_secs(5)),
        )?;
        if version.exit_code != Some(0) || version.timed_out || version.stdout.truncated() {
            return Err("Converter version probe failed".into());
        }
        let tool=ToolIdentity{executable:executable.to_str().ok_or("Non-Unicode converter path")?.into(),sha256:fingerprint,version:String::from_utf8_lossy(&version.stdout.bytes).trim().into(),license:"External configured provider; GNU LibreDWG is GPL-3.0-or-later; no redistribution assumed".into()};
        let operation_id = storage::new_document_identity();
        let jobs = directory(&self.root, "jobs")?;
        let job = jobs.join(&operation_id);
        fs::create_dir(&job).map_err(|e| e.to_string())?;
        publish_blob(&self.root, format!("jobs/{operation_id}/input.dwg"), &input)?;
        let mut receipt=ConversionReceipt{schema_version:1,operation_id:operation_id.clone(),source_id:source_id.into(),tool,arguments:vec!["-v1".into(),"-o".into(),"output.dxf".into(),"input.dwg".into()],capture:None,error:None,derived_source_id:None,status:"failed".into(),coverage:"unknown: conversion and record inventory do not prove source object/geometry preservation".into()};
        publish_blob(
            &self.root,
            format!("jobs/{operation_id}/request.json"),
            &serde_json::to_vec_pretty(&receipt).map_err(|e| e.to_string())?,
        )?;
        match run(&executable, &receipt.arguments, &job, timeout) {
            Ok(capture) => {
                if capture.timed_out || capture.exit_code != Some(0) {
                    receipt.error = Some("Converter timed out or exited unsuccessfully".into());
                }
                receipt.capture = Some(capture);
            }
            Err(error) => receipt.error = Some(error),
        }
        if read_bounded(&executable, 32 * 1024 * 1024)
            .map(|bytes| hash(&bytes) != receipt.tool.sha256)
            .unwrap_or(true)
        {
            receipt.error = Some("Converter executable changed during execution".into());
        }
        if read_bounded(&job.join("input.dwg"), MAX_SOURCE)
            .map(|bytes| hash(&bytes) != source_id)
            .unwrap_or(true)
        {
            receipt.error = Some("Converter modified its input copy".into());
        }
        let mut output = None;
        if receipt.error.is_none() {
            match safe_path(&self.root, &format!("jobs/{operation_id}/output.dxf"))
                .and_then(|path| read_bounded(&path, MAX_SOURCE))
                .and_then(|bytes| inventory::inspect(&bytes).map(|_| bytes))
            {
                Ok(bytes) => output = Some(bytes),
                Err(error) => receipt.error = Some(format!("Invalid converted DXF: {error}")),
            }
        }
        if output.is_some() {
            receipt.status = "publication_pending".into();
        }
        publish_blob(
            &self.root,
            format!("jobs/{operation_id}/execution.json"),
            &serde_json::to_vec_pretty(&receipt).map_err(|e| e.to_string())?,
        )?;
        let _lock = Lock::acquire(&self.root)
            .map_err(|error| format!("{error}; inspect jobs/{operation_id}/execution.json"))?;
        if let Err(error) = self.unchanged().and_then(|()| self.validate()) {
            receipt.error = Some(error.clone());
            receipt.status = "failed".into();
            publish_blob(
                &self.root,
                format!("jobs/{operation_id}/receipt.json"),
                &serde_json::to_vec_pretty(&receipt).map_err(|e| e.to_string())?,
            )?;
            return Err(format!("{error}; inspect jobs/{operation_id}/receipt.json"));
        }
        let mut next = self.clone();
        if let Some(bytes) = output {
            let id = next.admit_bytes(
                format!("converted-{}.dxf", &source_id[..16]),
                "dxf".into(),
                &bytes,
            )?;
            receipt.derived_source_id = Some(id);
            receipt.status = "converted_unverified".into();
        }
        let item = publish_blob(
            &self.root,
            format!("jobs/{operation_id}/receipt.json"),
            &serde_json::to_vec_pretty(&receipt).map_err(|e| e.to_string())?,
        )?;
        next.manifest.conversions.push(item);
        next.manifest.revision = next
            .manifest
            .revision
            .checked_add(1)
            .ok_or("Repository revision overflow")?;
        next.publish()?;
        *self = next;
        Ok(receipt)
    }
}
