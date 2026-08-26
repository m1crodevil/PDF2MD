use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc;
use std::thread;
use std::time::Instant;

use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::manifest::{write as write_manifest, Manifest};
use crate::types::PageJson;
use crate::types::ReconstructArgs;

#[derive(Deserialize)]
struct CompletionResponse {
    choices: Vec<CompletionChoice>,
}

#[derive(Deserialize)]
struct CompletionChoice {
    message: CompletionMessage,
}

#[derive(Deserialize)]
struct CompletionMessage {
    content: Option<String>,
}

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

fn load_env_value(name: &str, env_file: &str) -> Option<String> {
    if let Ok(value) = env::var(name) {
        if !value.trim().is_empty() {
            return Some(value);
        }
    }
    let path = env_file.strip_prefix("~/").map_or_else(
        || env_file.to_string(),
        |rest| format!("{}/{}", env::var("HOME").unwrap_or_default(), rest),
    );
    fs::read_to_string(path)
        .ok()?
        .lines()
        .find_map(|line| {
            let (key, value) = line.split_once('=')?;
            (key.trim() == name).then(|| value.trim().trim_matches('"').to_string())
        })
        .filter(|value| !value.trim().is_empty())
}

fn build_prompt(page_num: usize, markdown_hint: &str) -> String {
    format!(
        "You are a universal PDF page reconstruction and readability engine.\n\n\
Turn ONE supplied PDF-page JSON into Markdown that is faithful to the source and\neasy for a human to understand. Improve readability through structure, grouping,\nheadings, spacing, and clear Markdown — never by inventing facts or changing\nmeaning. This must work for lecture notes, books, reports, articles, forms,\ninvoices, slides, technical documents, and mixed-layout pages.\n\n\
SOURCE AND EVIDENCE:\n\
- Use OCR text as the source of textual content.\n\
- Use coordinates, bounding boxes, confidence scores, layout regions, labels, and\n  metadata to determine reading order and document structure.\n\
- Never use outside knowledge to complete, explain, translate, or correct content.\n\
- If evidence is ambiguous, preserve the original OCR rather than guessing.\n\n\
CONTENT FIDELITY:\n\
- Preserve the original language and meaning; never translate.\n\
- Preserve meaningful words, numbers, dates, amounts, units, punctuation, symbols,\n  names, labels, formulas, and page-specific text.\n\
- Correct only obvious OCR segmentation or spacing errors supported by nearby page\n  evidence. Never silently change uncertain numbers, names, formulas, or terms.\n\
- Do not summarize, paraphrase, editorialize, add commentary, or merge content\n  from another page.\n\n\
READABILITY AND STRUCTURE:\n\
- Infer natural reading order from layout evidence, not OCR array order.\n\
- Use a clear heading hierarchy when headings are visibly supported by the page.\n\
- Group related fragments into readable paragraphs, but preserve meaningful line\n  breaks, labels, callouts, and short standalone statements.\n\
- Represent lists as Markdown bullets or numbered lists when list structure is\n  supported. Keep list nesting when it is visible.\n\
- Represent tables as Markdown tables only when row and column relationships are\n  reliably recoverable. If uncertain, use ordered lines or a simple list instead\n  of inventing columns.\n\
- Preserve quotations, definitions, examples, warnings, formulas, code, and\n  symbolic expressions with the closest readable Markdown structure.\n\
HEADER AND FOOTER POLICY:\n- The input may contain repeated running headers, running footers, page numbers,\n  document titles, captions, and labels.\n- Preserve titles, chapter/section/article headings, captions, legal references,\n  dates, amounts, formulas, signatures, and page-specific content.\n- Remove only text explicitly classified as repeated page furniture by the\n  cross-page preprocessing step. Never infer a removal from position alone.\n- When classification is absent or uncertain, preserve the text.\n- Do not restore text marked as removed page furniture.\n\
- Use whitespace and concise structural Markdown to make the page easy to scan,\n  but do not add decorative content.\n\n\
NON-TEXT OBJECTS:\n\
- Do not invent descriptions or interpretations of images, charts, diagrams, or\n  logos. Preserve available captions, labels, and recoverable text.\n\
- If a visual object has no recoverable text, omit it rather than hallucinating.\n\n\
OUTPUT CONTRACT:\n\
- Output only the reconstructed Markdown. No preamble, explanation, JSON, YAML,\n  front matter, or fenced Markdown wrapper.\n\
- Add exactly one page marker as the first line: <!-- PAGE {} -->\n\
- Do not add any other page marker.\n\
- Do not add a title or heading unless supported by the page content.\n\n\
OCR and layout JSON input:\n{}",
        page_num, markdown_hint
    )
}

fn cache_key(model: &str, prompt: &str, input_json: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(model.as_bytes());
    hasher.update(b"\n");
    hasher.update(prompt.as_bytes());
    hasher.update(b"\n");
    hasher.update(input_json.as_bytes());
    hex_of(&hasher.finalize())
}

fn hex_of(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

fn reconstruct_one(
    json_path: PathBuf,
    out_path: PathBuf,
    cache_dir: PathBuf,
    api_key: String,
    model: String,
    base_url: String,
    reasoning_effort: String,
) -> Result<(usize, usize), String> {
    let t = Instant::now();
    let input = fs::read_to_string(&json_path)
        .map_err(|e| format!("read {}: {}", json_path.display(), e))?;
    let page_num: usize = json_path
        .file_stem()
        .and_then(|s| s.to_str())
        .and_then(|s| s.strip_prefix("page_"))
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| format!("bad filename: {}", json_path.display()))?;

    let prompt = build_prompt(page_num, &input);
    let key = cache_key(&model, &prompt, &input);
    let cache_path = cache_dir.join(format!("{}.md", key));
    if let Ok(cached) = fs::read_to_string(&cache_path) {
        if validate_markdown(&cached, page_num)
            .and_then(|_| validate_retention(&input, &cached, page_num))
            .is_ok()
        {
            fs::write(&out_path, &cached)
                .map_err(|e| format!("write {}: {}", out_path.display(), e))?;
            eprintln!("[md] p{:03} cache-hit {} chars", page_num, cached.len());
            return Ok((page_num, cached.len()));
        }
    }
    let payload = serde_json::json!({
        "model": model,
        "messages": [{"role": "user", "content": prompt}],
        "max_tokens": 4000,
        "temperature": 0,
        "reasoning_effort": reasoning_effort,
    });

    let mut out = None;
    for attempt in 1..=3 {
        let result = Command::new("curl")
            .args([
                "-sS",
                "--connect-timeout",
                "20",
                "--max-time",
                "300",
                &format!("{}/chat/completions", base_url.trim_end_matches('/')),
                "-H",
                &format!("Authorization: Bearer {}", api_key),
                "-H",
                "Content-Type: application/json",
                "-d",
                &payload.to_string(),
                "-w",
                "\n%{http_code}",
            ])
            .output()
            .map_err(|e| format!("curl failed: {}", e))?;
        let stdout = String::from_utf8_lossy(&result.stdout);
        let status = stdout
            .rsplit_once('\n')
            .and_then(|(_, code)| code.parse::<u16>().ok())
            .unwrap_or(0);
        let retryable = status == 0 || status == 408 || status == 429 || status >= 500;
        if status == 402 {
            return Err("HTTP 402: payment required; stopping retries".to_string());
        }
        if result.status.success() && (200..300).contains(&status) {
            out = Some(result);
            break;
        }
        if retryable && attempt < 3 {
            thread::sleep(std::time::Duration::from_secs(attempt));
        } else {
            return Err(format!(
                "HTTP {}: {}",
                status,
                String::from_utf8_lossy(&result.stderr)
            ));
        }
    }
    let out = out.ok_or_else(|| "curl retry loop ended without response".to_string())?;

    let mut body = out.stdout;
    if let Some(index) = body.iter().rposition(|byte| *byte == b'\n') {
        body.truncate(index);
    }
    let data: CompletionResponse =
        serde_json::from_slice(&body).map_err(|e| format!("parse llm response: {}", e))?;
    let md = data
        .choices
        .first()
        .and_then(|choice| choice.message.content.as_deref())
        .ok_or_else(|| "missing choices[0].message.content".to_string())?;
    if let Err(first) =
        validate_markdown(md, page_num).and_then(|_| validate_retention(&input, md, page_num))
    {
        return Err(format!("{}", first));
    }
    fs::write(&out_path, md).map_err(|e| format!("write {}: {}", out_path.display(), e))?;
    fs::write(&cache_path, md)
        .map_err(|e| format!("write cache {}: {}", cache_path.display(), e))?;
    eprintln!(
        "[md] p{:03} {}s {} chars",
        page_num,
        t.elapsed().as_secs_f32(),
        md.len()
    );
    Ok((page_num, md.len()))
}

fn validate_markdown(markdown: &str, page_num: usize) -> Result<(), String> {
    let text = markdown.trim();
    let marker = format!("<!-- PAGE {} -->", page_num);
    let content = text
        .lines()
        .filter(|line| line.trim() != marker)
        .collect::<Vec<_>>()
        .join("\n");
    if text.is_empty() || content.trim().is_empty() {
        return Err(format!(
            "quality validation: page {} contains only a marker or whitespace",
            page_num
        ));
    }
    if !text.contains(&marker) {
        return Err(format!(
            "quality validation: page {} is missing page marker",
            page_num
        ));
    }
    Ok(())
}

fn finalize_document(markdown: &str) -> Result<String, String> {
    let finalized = markdown
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            !(trimmed.starts_with("<!-- PAGE ") && trimmed.ends_with(" -->"))
        })
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string();
    if finalized.is_empty() {
        return Err("finalize document: output is empty after removing metadata".to_string());
    }
    if finalized
        .lines()
        .any(|line| line.trim().starts_with("<!-- PAGE "))
    {
        return Err("finalize document: page marker leaked into final output".to_string());
    }
    Ok(finalized)
}

fn validate_retention(input: &str, markdown: &str, page_num: usize) -> Result<(), String> {
    let source: PageJson =
        serde_json::from_str(input).map_err(|e| format!("parse page JSON: {}", e))?;
    if source.status != "success" {
        return Err(format!("page {} is not a successful OCR result", page_num));
    }
    // layout_regions.text_combined duplicates ocr_boxes text; count boxes only
    let source_numbers = source
        .ocr_boxes
        .iter()
        .map(|box_| box_.text.as_str())
        .flat_map(str::split_whitespace)
        .filter(|s| s.chars().any(|c| c.is_ascii_digit()))
        .count();
    let output_numbers = markdown
        .split_whitespace()
        .filter(|s| s.chars().any(|c| c.is_ascii_digit()))
        .count();
    if source_numbers >= 3 && output_numbers * 2 < source_numbers {
        return Err(format!(
            "quality validation: page {} lost too many numeric tokens",
            page_num
        ));
    }
    if source.quality.table_detected && !markdown.contains('|') {
        return Err(format!(
            "quality validation: page {} detected a table but output has no table structure",
            page_num
        ));
    }
    Ok(())
}

fn valid_existing_output(path: &Path, page_num: usize) -> bool {
    fs::read_to_string(path)
        .ok()
        .and_then(|text| validate_markdown(&text, page_num).ok())
        .is_some()
}

fn pdf_stem(source_pdf: &str) -> String {
    Path::new(source_pdf)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("pdf")
        .to_string()
}

pub(crate) fn run(args: &ReconstructArgs) -> Result<(), String> {
    let mut args = args.clone();
    if args.model.trim().is_empty() {
        if let Some(value) = load_env_value("PDF2MD_MODEL", &args.env_file) {
            args.model = value;
        }
    }
    if args.base_url.trim().is_empty() {
        return Err(
            "missing reconstruct base_url: set it in config/pdf2md.toml or pass --base-url"
                .to_string(),
        );
    }
    if args.model.trim().is_empty() {
        return Err("missing reconstruct model: set PDF2MD_MODEL or pass --model".to_string());
    }
    let api_key = load_env_key(&args.api_key_env, &args.env_file)?;
    let pdf_name = pdf_stem(&args.source_pdf);
    let bundle_root = PathBuf::from(&args.outdir).join(&pdf_name);
    let md_root = bundle_root.join("md");
    let cache_dir = bundle_root.join(".cache").join("reconstruct");
    fs::create_dir_all(&cache_dir).map_err(|e| format!("mkdir {}: {}", cache_dir.display(), e))?;
    fs::create_dir_all(bundle_root.join("json"))
        .map_err(|e| format!("mkdir {}: {}", bundle_root.join("json").display(), e))?;
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

    let (tx, rx) = mpsc::channel();
    let mut active = 0usize;
    let mut next = 0usize;
    let mut ok = 0usize;
    let mut skip = 0usize;
    let mut fail = 0usize;
    let mut quality_failed = 0usize;
    let mut review_required = 0usize;
    let mut vlm_candidates = 0usize;
    let max_concurrency = args.concurrency.max(1);

    for json_path in &files {
        if let Ok(text) = fs::read_to_string(json_path) {
            if let Ok(page) = serde_json::from_str::<PageJson>(&text) {
                if page.quality.review_required {
                    review_required += 1;
                }
                if page.risk_flags.iter().any(|f| f == "visual_object") {
                    vlm_candidates += 1;
                }
            }
        }
    }

    while next < files.len() || active > 0 {
        while active < max_concurrency && next < files.len() {
            let json_path = files[next].clone();
            next += 1;
            let stem = json_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();
            let page_num = stem
                .strip_prefix("page_")
                .and_then(|value| value.parse::<usize>().ok())
                .unwrap_or(0);
            let out_path = md_root.join(format!("{}.md", stem));
            let input = match fs::read_to_string(&json_path) {
                Ok(value) => value,
                Err(e) => {
                    fail += 1;
                    eprintln!("[md][ERR] read {}: {}", json_path.display(), e);
                    continue;
                }
            };
            match serde_json::from_str::<PageJson>(&input) {
                Ok(page) if page.status == "success" && page.page == page_num => {}
                Ok(page) => {
                    fail += 1;
                    eprintln!(
                        "[md][ERR] {} is not a successful page result (status={}, page={})",
                        json_path.display(),
                        page.status,
                        page.page
                    );
                    continue;
                }
                Err(e) => {
                    fail += 1;
                    eprintln!("[md][ERR] invalid JSON {}: {}", json_path.display(), e);
                    continue;
                }
            }
            if out_path.exists() && valid_existing_output(&out_path, page_num) {
                skip += 1;
                continue;
            }
            active += 1;
            let tx = tx.clone();
            let api_key = api_key.clone();
            let model = args.model.clone();
            let base_url = args.base_url.clone();
            let reasoning_effort = args.reasoning_effort.clone();
            let cache_dir = cache_dir.clone();
            thread::spawn(move || {
                let res = reconstruct_one(
                    json_path,
                    out_path,
                    cache_dir,
                    api_key,
                    model,
                    base_url,
                    reasoning_effort,
                );
                let _ = tx.send(res);
            });
        }
        if active > 0 {
            match rx.recv() {
                Ok(Ok((_page, _chars))) => ok += 1,
                Ok(Err(e)) => {
                    if e.starts_with("quality validation:") {
                        quality_failed += 1;
                    }
                    fail += 1;
                    eprintln!("[md][ERR] {}", e);
                }
                Err(e) => {
                    fail += 1;
                    eprintln!("[md][ERR] worker channel: {}", e);
                }
            }
            active -= 1;
        }
    }

    eprintln!(
        "=== MD DONE === ok={} skip={} fail={} quality_failed={} review_required={} vlm_candidates={}",
        ok, skip, fail, quality_failed, review_required, vlm_candidates
    );
    let mut merged = String::new();
    for page in &files {
        let stem = page.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        let output = md_root.join(format!("{}.md", stem));
        if let Ok(text) = fs::read_to_string(output) {
            merged.push_str(&text);
            merged.push_str("\n\n");
        }
    }
    if !merged.trim().is_empty() {
        let finalized = finalize_document(&merged)?;
        fs::write(bundle_root.join("document.md"), finalized)
            .map_err(|e| format!("write document.md: {}", e))?;
    }
    let manifest = Manifest {
        mode: "reconstruct".to_string(),
        input: args.json_dir.clone(),
        output_dir: bundle_root.display().to_string(),
        ok,
        skipped: skip,
        failed: fail,
        quality_failed,
        review_required,
        vlm_candidates,
    };
    write_manifest(
        &format!("{}/manifest.json", bundle_root.display()),
        &manifest,
    )
    .map_err(|e| format!("write manifest: {}", e))?;
    if fail > 0 {
        return Err(format!("reconstruct completed with {} page failures", fail));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::cache_key;

    #[test]
    fn cache_key_is_deterministic_and_sensitive() {
        let k1 = cache_key("m", "p", "j");
        let k2 = cache_key("m", "p", "j");
        assert_eq!(k1, k2, "same inputs must map to same key");
        assert_eq!(k1.len(), 64, "sha256 hex is 64 chars");
        assert_ne!(k1, cache_key("m", "p", "j2"), "json change must change key");
        assert_ne!(
            k1,
            cache_key("m", "p2", "j"),
            "prompt change must change key"
        );
        assert_ne!(
            k1,
            cache_key("m2", "p", "j"),
            "model change must change key"
        );
    }
}
