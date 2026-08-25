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
}

pub(crate) fn load(path: &str) -> Result<AppConfig, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("read config {}: {}", path, e))?;
    toml::from_str(&text).map_err(|e| format!("parse config {}: {}", path, e))
}

impl AppConfig {
    pub(crate) fn reconstruct_or_default(&self, mut args: ReconstructArgs) -> ReconstructArgs {
        if let Some(cfg) = &self.reconstruct {
            if let Some(v) = &cfg.json_dir {
                args.json_dir = v.clone();
            }
            if let Some(v) = &cfg.outdir {
                args.outdir = v.clone();
            }
            if let Some(v) = &cfg.source_pdf {
                args.source_pdf = v.clone();
            }
            if let Some(v) = &cfg.env_file {
                args.env_file = v.clone();
            }
            if let Some(v) = &cfg.api_key_env {
                args.api_key_env = v.clone();
            }
            if let Some(v) = &cfg.base_url {
                args.base_url = v.clone();
            }
            if let Some(v) = &cfg.model {
                args.model = v.clone();
            }
        }
        args
    }

    pub(crate) fn ocr_or_default(&self, mut args: OcrArgs) -> OcrArgs {
        if let Some(cfg) = &self.ocr {
            if let Some(v) = &cfg.pdf {
                args.pdf = v.clone();
            }
            if let Some(v) = &cfg.outdir {
                args.outdir = v.clone();
            }
            if let Some(v) = cfg.total {
                args.total = v;
            }
            if let Some(v) = cfg.start {
                args.start = v;
            }
            if let Some(v) = cfg.dpi {
                args.dpi = v;
            }
            if let Some(v) = &cfg.helper {
                args.helper = v.clone();
            }
        }
        args
    }
}
