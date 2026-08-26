use serde::Deserialize;
use std::fs;

use crate::types::{OcrArgs, ReconstructArgs};

#[derive(Debug, Deserialize, Default)]
pub(crate) struct AppConfig {
    pub ocr: Option<OcrCfg>,
    pub reconstruct: Option<ReconstructCfg>,
}

#[derive(Debug, Deserialize, Default)]
pub(crate) struct OcrCfg {
    pub pdf: Option<String>,
    pub outdir: Option<String>,
    pub total: Option<usize>,
    pub start: Option<usize>,
    pub dpi: Option<u32>,
    pub helper: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub(crate) struct ReconstructCfg {
    pub json_dir: Option<String>,
    pub source_pdf: Option<String>,
    pub outdir: Option<String>,
    pub api_key_env: Option<String>,
    pub env_file: Option<String>,
    pub base_url: Option<String>,
    pub model: Option<String>,
    pub concurrency: Option<usize>,
    pub reasoning_effort: Option<String>,
}

pub(crate) fn load(path: &str) -> Result<AppConfig, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("read config {}: {}", path, e))?;
    toml::from_str(&text).map_err(|e| format!("parse config {}: {}", path, e))
}

impl AppConfig {
    // CLI values win; config fills only clap defaults.
    pub(crate) fn reconstruct_or_default(&self, mut args: ReconstructArgs) -> ReconstructArgs {
        if let Some(cfg) = &self.reconstruct {
            if args.json_dir == "./json" {
                if let Some(v) = &cfg.json_dir {
                    args.json_dir = v.clone();
                }
            }
            if args.outdir == "./output" {
                if let Some(v) = &cfg.outdir {
                    args.outdir = v.clone();
                }
            }
            if args.source_pdf == "./input.pdf" {
                if let Some(v) = &cfg.source_pdf {
                    args.source_pdf = v.clone();
                }
            }
            if args.original_pdf == "./input.pdf" {
                if let Some(v) = &cfg.source_pdf {
                    args.original_pdf = v.clone();
                }
            }
            if args.env_file == "./.env" {
                if let Some(v) = &cfg.env_file {
                    args.env_file = v.clone();
                }
            }
            if args.api_key_env == "PDF2MD_API_KEY" {
                if let Some(v) = &cfg.api_key_env {
                    args.api_key_env = v.clone();
                }
            }
            if args.base_url.is_empty() {
                if let Some(v) = &cfg.base_url {
                    args.base_url = v.clone();
                }
            }
            if args.model.is_empty() {
                if let Some(v) = &cfg.model {
                    args.model = v.clone();
                }
            }
            if args.concurrency == 2 {
                if let Some(v) = cfg.concurrency {
                    args.concurrency = v.max(1);
                }
            }
            if args.reasoning_effort == "none" {
                if let Some(v) = &cfg.reasoning_effort {
                    args.reasoning_effort = v.clone();
                }
            }
        }
        if args.model.is_empty() {
            args.model = std::env::var("PDF2MD_MODEL").unwrap_or_default();
        }
        if args.base_url.is_empty() {
            args.base_url = std::env::var("PDF2MD_BASE_URL").unwrap_or_default();
        }
        if args.reasoning_effort == "none" {
            if let Ok(v) = std::env::var("PDF2MD_REASONING_EFFORT") {
                args.reasoning_effort = v;
            }
        }
        args
    }

    pub(crate) fn ocr_or_default(&self, mut args: OcrArgs) -> OcrArgs {
        if let Some(cfg) = &self.ocr {
            if args.pdf == "./input.pdf" {
                if let Some(v) = &cfg.pdf {
                    args.pdf = v.clone();
                }
            }
            if args.outdir == "./json" {
                if let Some(v) = &cfg.outdir {
                    args.outdir = v.clone();
                }
            }
            if args.total == 222 {
                if let Some(v) = cfg.total {
                    args.total = v;
                }
            }
            if args.start == 1 {
                if let Some(v) = cfg.start {
                    args.start = v;
                }
            }
            if args.dpi == 150 {
                if let Some(v) = cfg.dpi {
                    args.dpi = v;
                }
            }
            if args.helper == "scripts/ocr_helper.py" {
                if let Some(v) = &cfg.helper {
                    args.helper = v.clone();
                }
            }
        }
        args
    }
}
