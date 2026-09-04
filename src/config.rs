use serde::Deserialize;
use std::fs;

use crate::types::{OcrArgs, ReconstructArgs};

#[derive(Debug, Deserialize, Default)]
pub(crate) struct AppConfig {
    pub ocr: Option<OcrCfg>,
    pub reconstruct: Option<ReconstructCfg>,
}

#[derive(Debug, Deserialize, Default, Clone)]
pub(crate) struct OcrCfg {
    pub pdf: Option<String>,
    pub outdir: Option<String>,
    pub start: Option<usize>,
    pub dpi: Option<u32>,
    pub helper: Option<String>,
}

#[derive(Debug, Deserialize, Default, Clone)]
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

// Config overrides CLI defaults; explicit CLI flags win over config.
macro_rules! overlay {
    ($args:expr, $cfg:expr, $field:ident, $default:expr) => {
        if $args.$field == $default {
            if let Some(v) = &$cfg.$field {
                $args.$field = v.clone();
            }
        }
    };
    ($args:expr, $cfg:expr, $field:ident, $default:expr, opt) => {
        if $args.$field == $default {
            if let Some(v) = $cfg.$field {
                $args.$field = v;
            }
        }
    };
}

impl AppConfig {
    pub(crate) fn ocr_or_default(&self, mut args: OcrArgs) -> OcrArgs {
        if let Some(cfg) = &self.ocr {
            overlay!(args, cfg, pdf, "./input.pdf");
            overlay!(args, cfg, outdir, "./json");
            overlay!(args, cfg, start, 1, opt);
            overlay!(args, cfg, dpi, 150, opt);
            overlay!(args, cfg, helper, "scripts/ocr_helper.py");
        }
        args
    }

    pub(crate) fn reconstruct_or_default(&self, mut args: ReconstructArgs) -> ReconstructArgs {
        if let Some(cfg) = &self.reconstruct {
            overlay!(args, cfg, json_dir, "./json");
            overlay!(args, cfg, outdir, "./output");
            overlay!(args, cfg, source_pdf, "./input.pdf");
            overlay!(args, cfg, env_file, "./.env");
            overlay!(args, cfg, api_key_env, "PDF2MD_API_KEY");
            overlay!(args, cfg, base_url, "");
            overlay!(args, cfg, model, "");
            overlay!(args, cfg, concurrency, 2, opt);
            overlay!(args, cfg, reasoning_effort, "none");
        }
        args
    }
}
