use crate::io::atomic_write;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub(crate) struct Manifest {
    pub input: String,
    pub output_dir: String,
    pub ok: usize,
    pub skipped: usize,
    pub failed: usize,
    pub quality_failed: usize,
    pub review_required: usize,
    pub vlm_candidates: usize,
    pub pages_total: usize,
    pub pages_empty: usize,
    pub content_integrity: String,
}

pub(crate) fn write(path: &str, m: &Manifest) -> Result<(), String> {
    let text = serde_json::to_string_pretty(m).map_err(|e| format!("serialize manifest: {}", e))?;
    atomic_write(path, text).map_err(|e| format!("write manifest {}: {}", path, e))
}
