use serde::Serialize;
use tauri::{Emitter, Window};

#[derive(Clone, Debug, Serialize)]
pub struct BackgroundJobHistoryEntry {
    pub index: usize,
    pub label: String,
    pub status: String,
    pub detail: Option<String>,
    pub elapsed_ms: Option<u64>,
}

#[derive(Clone, Debug, Serialize)]
pub struct BackgroundJobProgress {
    pub job_id: String,
    pub kind: String,
    pub label: String,
    pub status: String,
    pub phase: String,
    pub current: usize,
    pub total: usize,
    pub succeeded: usize,
    pub failed: usize,
    pub recovered: usize,
    pub started_at: i64,
    pub updated_at: i64,
    pub elapsed_ms: u64,
    pub estimated_remaining_ms: Option<u64>,
    pub detail: Option<String>,
    pub cancellable: bool,
    pub history: Vec<BackgroundJobHistoryEntry>,
}

pub fn emit_background_job_progress(window: &Window, progress: &BackgroundJobProgress) {
    let _ = window.emit("background-job-progress", progress);
}
