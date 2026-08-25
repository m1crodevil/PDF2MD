use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use crate::manifest::{write as write_manifest, Manifest};
use crate::types::ReconstructArgs;

fn load_env_key(api_key_env: &str, env_file: &str) -> Result<String, String> {
    if let Ok(v) = env::var(api_key_env) {
        if !v.trim().is_empty() {
            return Ok(v);
        }
    }

    let path = env_file.strip_prefix("~/").map_or_else(
        || env_file.to_string(),
        |rest| format!("{}/{}", env::var("HOME").unwrap_or_default(), rest),
    );
    let content = fs::read_to_string(&path).map_err(|e| format!("read {}: {}", path, e))?;
    for line in content.lines() {
        let line = line.trim();
        if let Some(v) = line.strip_prefix(&format!("{}=", api_key_env)) {
            let v = v.trim().trim_matches('"');
            if !v.is_empty() {
                return Ok(v.to_string());
            }
        }
    }
    Err(format!("{} not found in env or file", api_key_env))
}

fn build_prompt(page_num: usize, markdown_hint: &str) -> String {
    format!(
        "You are a document reconstruction assistant for an Indonesian accounting textbook.\n\n\
Convert the OCR JSON page to clean Markdown.\n\n\
Rules:\n\
- Use proper heading hierarchy (#, ##, ###)\n\
- Reconstruct readable paragraphs from OCR fragments\n\
- Fix obvious OCR errors without changing meaning\n\
- Preserve Indonesian language\n\
- Add page marker: <!-- PAGE {} -->\n\
- Remove repeated headers/footers\n\
- Output ONLY markdown\n\n\
OCR JSON input:\n{}",
        page_num, markdown_hint
    )
}

fn reconstruct_one(
    json_path: &PathBuf,
    out_path: &PathBuf,
    api_key: &str,
    model: &str,
) -> Result<(), String> {
    let t = Instant::now();
    let input = fs::read_to_string(json_path)
        .map_err(|e| format!("read {}: {}", json_path.display(), e))?;
    let page_num: usize = json_path
        .file_stem()
        .and_then(|s| s.to_str())
        .and_then(|s| s.strip_prefix("page_"))
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| format!("bad filename: {}", json_path.display()))?;

    let prompt = build_prompt(page_num, &input);
    let payload = serde_json::json!({
        "model": model,
        "messages": [{"role": "user", "content": prompt}],
        "max_tokens": 4000,
        "temperature": 0,
    });

    let out = Command::new("curl")
        .args([
            "-s",
            &format!(
                "{}/chat/completions",
                "https://api.example.com/v1".trim_end_matches('/')
            ),
            "-H",
            &format!("Authorization: Bearer {}", api_key),
            "-H",
            "Content-Type: application/json",
            "-d",
            &payload.to_string(),
        ])
        .output()
        .map_err(|e| format!("curl failed: {}", e))?;

    if !out.status.success() {
        return Err(format!(
            "curl non-zero: {}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }

    let data: serde_json::Value =
        serde_json::from_slice(&out.stdout).map_err(|e| format!("parse llm json: {}", e))?;
    let md = data["choices"][0]["message"]["content"]
        .as_str()
        .ok_or_else(|| "missing content".to_string())?;
    fs::write(out_path, md).map_err(|e| format!("write {}: {}", out_path.display(), e))?;
    eprintln!(
        "[md] p{:03} {}s {} chars",
        page_num,
        t.elapsed().as_secs_f32(),
        md.len()
    );
    Ok(())
}

fn pdf_stem(source_pdf: &str) -> String {
    Path::new(source_pdf)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("pdf")
        .to_string()
}

pub(crate) fn run(args: &ReconstructArgs) -> Result<(), String> {
    let api_key = load_env_key(&args.api_key_env, &args.env_file)?;
    let pdf_name = pdf_stem(&args.source_pdf);
    let bundle_root = PathBuf::from(&args.outdir).join(&pdf_name);
    let json_root = bundle_root.join("json");
    let md_root = bundle_root.join("md");
    fs::create_dir_all(&json_root).map_err(|e| format!("mkdir {}: {}", json_root.display(), e))?;
    fs::create_dir_all(&md_root).map_err(|e| format!("mkdir {}: {}", md_root.display(), e))?;
    let original_copy = bundle_root.join("original.pdf");
    if !original_copy.exists() {
        fs::copy(&args.original_pdf, &original_copy).map_err(|e| {
            format!(
                "copy {} -> {}: {}",
                args.original_pdf,
                original_copy.display(),
                e
            )
        })?;
    }

    let mut files: Vec<PathBuf> = fs::read_dir(&args.json_dir)
        .map_err(|e| format!("read dir {}: {}", args.json_dir, e))?
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|s| s.to_str())
                .map(|s| s.starts_with("page_") && s.ends_with(".json"))
                .unwrap_or(false)
        })
        .collect();
    files.sort();

    let mut ok = 0usize;
    let mut skip = 0usize;
    let mut fail = 0usize;

    for json_path in files {
        let stem = json_path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        let out_path = md_root.join(format!("{}.md", stem));
        if out_path.exists() {
            skip += 1;
            continue;
        }
        match reconstruct_one(&json_path, &out_path, &api_key, &args.model) {
            Ok(_) => ok += 1,
            Err(e) => {
                fail += 1;
                eprintln!("[md][ERR] {}: {}", json_path.display(), e);
            }
        }
    }

    eprintln!("=== MD DONE === ok={} skip={} fail={}", ok, skip, fail);
    let manifest = Manifest {
        mode: "reconstruct".to_string(),
        input: args.json_dir.clone(),
        output_dir: bundle_root.display().to_string(),
        ok,
        skipped: skip,
        failed: fail,
    };
    let _ = write_manifest(
        &format!("{}/manifest.json", bundle_root.display()),
        &manifest,
    );
    Ok(())
}
