//! Opt-in native QA counters. No per-frame filesystem operations.
//! Measurements stay in memory until this isolated application exits normally.
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

pub(super) struct Metrics {
    output: Option<PathBuf>,
    start: Instant,
    frames: Vec<(f64, f64, bool)>,
    recovery_starts: Vec<f64>,
}
impl Default for Metrics {
    fn default() -> Self {
        Self {
            output: std::env::var_os("OPENARCHI_NATIVE_METRICS").map(PathBuf::from),
            start: Instant::now(),
            frames: vec![],
            recovery_starts: vec![],
        }
    }
}
impl Metrics {
    pub(super) fn frame(&mut self, elapsed: Duration, active: bool) {
        if self.output.is_some() && self.frames.len() < 100_000 {
            self.frames.push((
                self.start.elapsed().as_secs_f64(),
                elapsed.as_secs_f64() * 1000.,
                active,
            ));
        }
    }
    pub(super) fn recovery_start(&mut self) {
        if self.output.is_some() {
            self.recovery_starts
                .push(self.start.elapsed().as_secs_f64());
        }
    }
}
impl Drop for Metrics {
    fn drop(&mut self) {
        let Some(path) = &self.output else { return };
        let data = serde_json::json!({"version":env!("CARGO_PKG_VERSION"),"frames":self.frames,
            "recovery_write_starts_seconds":self.recovery_starts,
            "scope":"Real native UI callback CPU time and actual recovery-worker starts. No GPU-present or system input latency claim. Memory only until normal exit."});
        use std::io::Write;
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(path)
        {
            let _ = file.write_all(data.to_string().as_bytes());
        }
    }
}
