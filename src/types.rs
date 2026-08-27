use serde::{Deserialize, Serialize};
use std::io::BufReader;
use std::process::ChildStdout;
use std::process::{Child, ChildStdin};

use clap::{Parser, Subcommand};

// ─── CLI ───

#[derive(Parser, Debug)]
#[command(name = "pdf2md")]
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

    /// Total pages to process (defaults to the PDF's page count)
    #[arg(long)]
    pub total: Option<usize>,

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
    #[arg(long, default_value = "./.env")]
    pub env_file: String,

    /// LLM base URL
    #[arg(long, default_value = "")]
    pub base_url: String,

    /// Model slug
    #[arg(long, default_value = "")]
    pub model: String,

    /// Maximum parallel LLM requests
    #[arg(long, default_value_t = 2)]
    pub concurrency: usize,

    /// Provider reasoning effort (best effort; output always comes from content)
    #[arg(long, default_value = "none")]
    pub reasoning_effort: String,
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
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
    #[serde(default = "default_page_status")]
    pub status: String,
    pub page: usize,
    pub blank: bool,
    pub png: Option<String>,
    pub dpi: u32,
    pub layout_regions: Vec<LayoutRegion>,
    pub ocr_boxes: Vec<OcrBox>,
    #[serde(default)]
    pub reading_order: Vec<usize>,
    #[serde(default)]
    pub risk_flags: Vec<String>,
    #[serde(default)]
    pub quality: PageQuality,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub furniture: Vec<FurnitureAnnotation>,
    #[serde(default)]
    pub filtered_ocr_boxes: Option<Vec<usize>>,
    #[serde(default)]
    pub ocr_model: Option<String>,
    pub timings: Timings,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub(crate) struct FurnitureAnnotation {
    pub text: String,
    pub role: String,
    pub confidence: f64,
    pub reason: String,
}

fn default_page_status() -> String {
    "success".to_string()
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub(crate) struct PageQuality {
    pub text_chars: usize,
    pub ocr_box_count: usize,
    pub mean_confidence: f64,
    pub low_confidence_ratio: f64,
    pub table_detected: bool,
    pub visual_region_detected: bool,
    pub review_required: bool,
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
