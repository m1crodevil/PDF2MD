use serde::Serialize;
use std::fs;

#[derive(Debug, Serialize)]
pub(crate) struct Manifest {
    pub mode: String,
    pub input: String,
    pub output_dir: String,
    pub ok: usize,
    pub skipped: usize,
    pub failed: usize,
}

pub(crate) fn write(path: &str, m: &Manifest) -> Result<(), String> {
    let text = serde_json::to_string_pretty(m).map_err(|e| format!("serialize manifest: {}", e))?;
    fs::write(path, text).map_err(|e| format!("write manifest {}: {}", path, e))
}
