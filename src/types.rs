use serde::{Deserialize, Serialize};
use std::io::BufReader;
use std::process::ChildStdout;
use std::process::{Child, ChildStdin};

use clap::{Parser, Subcommand};

// ─── CLI ───

#[derive(Parser, Debug)]
#[command(name = "ocr-pipeline")]
#[command(about = "OCR + Markdown pipeline")]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub(crate) enum Commands {
    /// Run OCR → page JSON
    Ocr(OcrArgs),
    /// Run page JSON → Markdown
    Reconstruct(ReconstructArgs),
}

#[derive(Parser, Debug, Clone)]
pub(crate) struct OcrArgs {
    /// Source PDF path
    #[arg(long, default_value = "./input.pdf")]
    pub pdf: String,

    /// Output directory for JSON files
    #[arg(long, default_value = "./json")]
    pub outdir: String,

    /// Total pages to process
    #[arg(long, default_value_t = 222)]
    pub total: usize,

    /// Start page (1-indexed)
    #[arg(long, default_value_t = 1)]
    pub start: usize,

    /// End page (1-indexed, overrides --total)
    #[arg(long)]
    pub end: Option<usize>,

    /// DPI for rendering
    #[arg(long, default_value_t = 150)]
    pub dpi: u32,

    /// Path to Python helper script
    #[arg(long, default_value = "scripts/ocr_helper.py")]
    pub helper: String,
}

#[derive(Parser, Debug, Clone)]
pub(crate) struct ReconstructArgs {
    /// Input OCR JSON directory
    #[arg(long, default_value = "./json")]
    pub json_dir: String,

    /// Source PDF path used to name the output folder
    #[arg(long, default_value = "./input.pdf")]
    pub source_pdf: String,

    /// Output root directory
    #[arg(long, default_value = "./output")]
    pub outdir: String,

    /// Copy of the original PDF path in the output bundle
    #[arg(long, default_value = "./input.pdf")]
    pub original_pdf: String,

    /// API key env/file lookup key
    #[arg(long, default_value = "PDF2MD_API_KEY")]
    pub api_key_env: String,

    /// API key file path
    #[arg(long, default_value = "~/.config/pdf2md/env")]
    pub env_file: String,

    /// LLM base URL
    #[arg(long, default_value = "https://api.example.com/v1")]
    pub base_url: String,

    /// Model slug
    #[arg(long, default_value = "gpt-4o-mini")]
    pub model: String,
}

// ─── JSON output schema ───

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct OcrBox {
    pub text: String,
    pub text_raw: Option<String>,
    pub confidence: f64,
    pub bbox: [f64; 4],
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub(crate) struct LayoutRegion {
    pub label: String,
    pub score: f64,
    pub bbox: [f64; 4],
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub text_boxes: Vec<RegionTextBox>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_combined: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct RegionTextBox {
    pub text: String,
    pub confidence: f64,
    pub x: f64,
    pub y: f64,
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct PageJson {
    pub page: usize,
    pub blank: bool,
    pub png: Option<String>,
    pub dpi: u32,
    pub layout_regions: Vec<LayoutRegion>,
    pub ocr_boxes: Vec<OcrBox>,
    pub timings: Timings,
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct Timings {
    pub render: f64,
    pub layout: f64,
    pub ocr: f64,
    pub cleanup: f64,
    pub total: f64,
}

// ─── Batch helper: persistent Python process ───

pub(crate) struct BatchHelper {
    pub child: Child,
    pub stdin: ChildStdin,
    pub stdout: BufReader<ChildStdout>,
}
