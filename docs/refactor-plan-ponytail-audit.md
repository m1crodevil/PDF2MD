# PDF2MD Refactor Plan — Ponytail Audit

Status: comprehensive plan, ordered by safety (deletions first) then impact (architectural last). Each phase is independently committable and must pass `cargo test` + `cargo clippy` before the next begins.

## Repo snapshot (pre-refactor)

- 2,697 lines Rust + Python (1,586 production, 453 test, 658 scripts/config)
- 11 Rust source files, 8 Python scripts
- 92 tracked JSON artifacts in `json/` + `runs/` (should be gitignored)
- 115,850-line dead `frequency_dict.txt` (~1 MB)
- Shadow repo at `/tmp/pdf2md-clean/`
- Deps: `regex serde serde_json sha2 clap toml pdf_oxide`
- `pdf_oxide = "0.3"` → resolves to 0.3.77 which ships `ConversionOptions` + `to_markdown()`

## Internet validation summary

| Component | Validated against | Verdict |
|---|---|---|
| pdf_oxide v0.3.77 `ConversionOptions` | docs.rs/pdf-oxide/latest/pdf_oxide/converters/struct.ConversionOptions.html | Ships `strip_running_headers_footers`, `extract_tables`, `detect_headings`, `reading_order_mode`, `exclude_regions` — superset of furniture.rs + reconstruct.rs prompt |
| `PdfDocument::to_markdown(page, &opts)` | docs.rs/pdf-oxide/latest/pdf_oxide/converters/ | Native markdown converter, no LLM needed for native-text PDFs |
| `BatchProcessor` | docs.rs/pdf-oxide/latest/pdf_oxide/batch/ | Parallel batch extraction built-in |
| `regex` crate | std Rust ecosystem | Correct choice |
| `sha2` cache pattern | Standard deterministic cache key | Correct pattern |
| PaddleOCR PP-DocLayout | OmniDocBench CVPR 2025, PaddleOCR docs | SOTA open-source layout detection |
| `clap` derive config layering | clap docs | Over-layered with `toml` (see finding #4) |

---

## Phase 0 — Safe deletions (zero code-risk, zero behavior change)

### 0.1 Delete tracked JSON artifacts from git

`json/` and `runs/` are already in `.gitignore` (lines 2-4) but 92 JSON files are still tracked from before the ignore was added. These are run artifacts, not source.

```bash
cd /home/microdevil/PDF2MD
git rm -r --cached json/ runs/
git commit -m "chore: untrack run artifacts (json/ runs/) — already in .gitignore"
```

**Lines removed from git tracking:** 92 files, ~0 source lines.

### 0.2 Delete shadow repo

`/tmp/pdf2md-clean/` is a stale checkout missing `furniture.rs`, `ir.rs`, `pdfoxide_backend.rs`. It's a distraction, not a backup.

```bash
rm -rf /tmp/pdf2md-clean
```

### 0.3 Delete dead `frequency_dict.txt`

Zero references across all `.rs` and `.py` files (verified via `search_files` — 0 matches). 115,850 lines, ~1 MB. Legacy from abandoned spell-check strategy.

```bash
git rm data/frequency_dict.txt
rmdir data/  # if empty after
git commit -m "chore: delete dead frequency_dict.txt (0 references, 115K lines)"
```

### 0.4 Delete completed planning doc

`docs/universal-document-engine-plan.md` — all phases implemented. It's a relic.

```bash
git rm docs/universal-document-engine-plan.md
git commit -m "docs: remove completed planning doc"
```

### Phase 0 exit gate

```bash
cargo test --all-targets && cargo clippy --all-targets -- -D warnings && cargo fmt --check
python3 scripts/test_report.py
python3 scripts/check_repo_hygiene.py
```

---

## Phase 1 — Shrink `render_page` filename guessing (12 → 2 lines)

**Finding #6.** `render_page()` in `page.rs:50-61` tries 3 possible pdftoppm output filenames:

```rust
// Current: 3-branch guess
let generated = PathBuf::from(format!("{}/page-{:03}.png", tmp_dir, page_num));
let alt = PathBuf::from(format!("{}/page-{}.png", tmp_dir, page_num));
let padded = PathBuf::from(format!("{}/page-{:02}.png", tmp_dir, page_num));
let src = if generated.exists() { generated }
    else if alt.exists() { alt }
    else if padded.exists() { padded }
    else { return Err(...); };
```

pdftoppm with `-png` and prefix `page` always outputs `page-NNN.png` where padding = digits in total page count. For single-page calls (`-f N -l N`), it's always `page-N.png` (no padding). The 3-branch is defense against a bug that doesn't exist.

**Fix:** glob for the generated file.

```rust
let src = fs::read_dir(tmp_dir)
    .map_err(|e| format!("read tmp dir: {}", e))?
    .flatten()
    .find(|e| {
        e.file_name().to_string_lossy()
            .strip_prefix("page-")
            .is_some_and(|s| s.strip_suffix(".png").is_some())
    })
    .map(|e| e.path())
    .ok_or_else(|| format!("pdftoppm output not found for page {}", page_num))?;
```

**Net:** -10 lines, more robust (handles any padding).

### Phase 1 exit gate

```bash
cargo test --all-targets && cargo clippy --all-targets -- -D warnings
```

---

## Phase 2 — Flatten config layer (138 → ~35 lines)

**Finding #4.** `config.rs` has `OcrCfg` and `ReconstructCfg` structs mirroring CLI field-for-field, each wrapped in `Option<T>`, with 6+ "if arg == sentinel value → use config" checks per struct. This reimplements what clap's `default_value` + env source already does.

**Root cause:** config was designed to override clap defaults, but clap defaults are static — they can't read a toml file. So the overlay is necessary in principle, but the implementation is boilerplate-heavy.

**Fix:** One shared helper, no per-field sentinel checks.

```rust
// config.rs — after
use serde::Deserialize;
use crate::types::{OcrArgs, ReconstructArgs};
use std::fs;

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
```

Then in `main.rs`, replace `reconstruct_or_default` / `ocr_or_default` with direct field assignment:

```rust
// main.rs — merge config into args (CLI already parsed by clap with defaults)
fn merge_reconstruct(args: &mut ReconstructArgs, cfg: Option<&ReconstructCfg>) {
    let Some(cfg) = cfg else { return };
    if args.json_dir == "./json" { args.json_dir = cfg.json_dir.clone().unwrap_or_else(|| args.json_dir.clone()); }
    if args.outdir == "./output" { args.outdir = cfg.outdir.clone().unwrap_or_else(|| args.outdir.clone()); }
    if args.source_pdf == "./input.pdf" { args.source_pdf = cfg.source_pdf.clone().unwrap_or_else(|| args.source_pdf.clone()); }
    if args.original_pdf == "./input.pdf" { args.original_pdf = cfg.source_pdf.clone().unwrap_or_else(|| args.original_pdf.clone()); }
    if args.env_file == "./.env" { args.env_file = cfg.env_file.clone().unwrap_or_else(|| args.env_file.clone()); }
    if args.api_key_env == "PDF2MD_API_KEY" { args.api_key_env = cfg.api_key_env.clone().unwrap_or_else(|| args.api_key_env.clone()); }
    if args.base_url.is_empty() { args.base_url = cfg.base_url.clone().unwrap_or_default(); }
    if args.model.is_empty() { args.model = cfg.model.clone().unwrap_or_default(); }
    if args.concurrency == 2 { args.concurrency = cfg.concurrency.unwrap_or(args.concurrency); }
    if args.reasoning_effort == "none" { args.reasoning_effort = cfg.reasoning_effort.clone().unwrap_or_else(|| args.reasoning_effort.clone()); }
}
```

Wait — that's the same boilerplate. The real lazy fix is: **delete the entire config overlay.** Use clap's `env` attribute for env vars, and require `--config` path in toml only for values that are truly machine-specific (paths). Clap already supports `#[arg(long, env = "PDF2MD_MODEL")]`.

**Actually lazier:** delete `config.rs` entirely. Move `pdf2md.toml` loading into `main.rs` as 5 lines: read toml, deserialize into the same `OcrArgs`/`ReconstructArgs` structs (which already derive `Deserialize`), and overlay with a macro or a small `merge` function that takes `&mut args` and `Option<&toml::Value>`.

**Decision:** Keep `config.rs` but collapse the two `*_or_default` methods into a single generic `fn merge<T: Deserialize>(args: &mut T, cfg: Option<&toml::Value>)`. ~138 → ~45 lines.

**Net:** -93 lines.

### Phase 2 exit gate

```bash
cargo test --all-targets && cargo clippy --all-targets -- -D warnings
python3 scripts/test_report.py
```

---

## Phase 3 — Remove dead `ir.rs` module (71 lines)

**Finding #5.** `ir.rs` defines `Backend` enum and `PageIr` struct, both `#[allow(dead_code)]`. `PageProbe::backend()` is called exactly once in `main.rs:77` for an `eprintln!` debug line — its return value is never used to route anything. The OCR loop always calls both `probe_page` and `extract_page` unconditionally.

**Root cause:** IR was scaffolded for a routing layer that was never built. The probe is useful (it tells us if a page has text), but the `Backend` enum routing is dead.

**Fix:**

1. Move `PageProbe` struct into `pdfoxide_backend.rs` (it's only used there + `main.rs`).
2. Delete `ir.rs` entirely.
3. Remove `mod ir;` from `main.rs`.
4. Update `main.rs:72-78` — keep the probe `eprintln!` but drop the `backend(20)` call (or inline the heuristic).

```rust
// pdfoxide_backend.rs — add PageProbe struct here
#[derive(Debug, Clone)]
pub(crate) struct PageProbe {
    pub page: usize,
    pub native_text_chars: usize,
    pub image_only: bool,
}
```

```rust
// main.rs — remove mod ir; and update probe usage
use pdfoxide_backend::{extract_page, probe_page, PageProbe};
```

**Net:** -71 lines, -1 file.

### Phase 3 exit gate

```bash
cargo test --all-targets && cargo clippy --all-targets -- -D warnings
```

---

## Phase 4 — Remove `furniture.rs` (251 lines)

**Finding #8.** `furniture.rs` implements custom header/footer detection (edge-zone heuristics, page-number detection, repeated-text frequency counting). This is fully replaced by:

1. `ConversionOptions { strip_running_headers_footers: true }` — pdf_oxide's native cross-page header/footer stripping (WS2.6 spec, confirmed in docs).
2. The LLM prompt in `reconstruct.rs:99` already says "preprocessing has already removed page furniture" — but furniture.rs writes to `filtered/` subdir, and `choose_input_dir` falls back to raw if `filtered/` doesn't exist. Two paths for the same job.

**Root cause:** furniture.rs was built before pdf_oxide shipped `strip_running_headers_footers`. Now it's redundant.

**Fix:**

1. Delete `furniture.rs` entirely.
2. Remove `mod furniture;` from `main.rs`.
3. Remove `furniture::annotate_directory` import and call from `main.rs:20`.
4. Simplify `choose_input_dir` in `reconstruct.rs:448-460`:

```rust
fn choose_input_dir(json_dir: &str) -> PathBuf {
    PathBuf::from(json_dir)
}
```

5. Keep `FurnitureAnnotation` struct in `types.rs` (it's in the JSON schema, needed for backward compat with existing JSON files).

**Net:** -251 lines, -1 file. The `filtered/` directory concept is gone — all pages come from `json/` directly.

### Phase 4 exit gate

```bash
cargo test --all-targets && cargo clippy --all-targets -- -D warnings
# Verify backward compat: existing json/ files still load (furniture field defaults to [])
```

---

## Phase 5 — Shrink env file parsing (73 → ~20 lines)

**Finding #9.** `load_env_key` and `load_env_value` in `reconstruct.rs:31-73` are two near-identical functions that manually parse `.env` files. They differ only in return type (`Result` vs `Option`) and the key-matching logic.

**Fix:** One shared helper.

```rust
fn read_env_file(path: &str) -> std::collections::HashMap<String, String> {
    let expanded = path.strip_prefix("~/").map_or_else(
        || path.to_string(),
        |rest| format!("{}/{}", std::env::var("HOME").unwrap_or_default(), rest),
    );
    let mut map = std::collections::HashMap::new();
    if let Ok(content) = fs::read_to_string(&expanded) {
        for line in content.lines() {
            if let Some((k, v)) = line.split_once('=') {
                let v = v.trim().trim_matches('"');
                if !v.is_empty() {
                    map.insert(k.trim().to_string(), v.to_string());
                }
            }
        }
    }
    map
}

fn load_env_key(api_key_env: &str, env_file: &str) -> Result<String, String> {
    if let Ok(v) = env::var(api_key_env) {
        if !v.trim().is_empty() { return Ok(v); }
    }
    read_env_file(env_file)
        .get(api_key_env)
        .cloned()
        .ok_or_else(|| format!("{} not found in env or file", api_key_env))
}

fn load_env_value(name: &str, env_file: &str) -> Option<String> {
    if let Ok(value) = env::var(name) {
        if !value.trim().is_empty() { return Some(value); }
    }
    read_env_file(env_file).get(name).cloned().filter(|v| !v.trim().is_empty())
}
```

**Net:** -53 lines, DRY.

### Phase 5 exit gate

```bash
cargo test --all-targets && cargo clippy --all-targets -- -D warnings
```

---

## Phase 6 — Move `BatchHelper` to `page.rs`, clean `types.rs`

**Finding #12.** `BatchHelper` (a child-process struct) lives in `types.rs` next to DTOs like `PageJson`. It's only used in `page.rs`.

**Fix:**

1. Move `BatchHelper` struct + `impl` block from `types.rs` to `page.rs` (where the `impl` already lives — `types.rs:188-192` only declares the struct, the `impl` is in `page.rs:87-126`).
2. Remove `use std::io::BufReader; use std::process::ChildStdout; use std::process::{Child, ChildStdin};` from `types.rs`.

**Net:** cleaner module boundaries, no line change.

### Phase 6 exit gate

```bash
cargo test --all-targets && cargo clippy --all-targets -- -D warnings
```

---

## Phase 7 — Shrink `report.rs` print_done (7 args → struct)

**Finding #13.** `print_done` takes 8 parameters and has `#[allow(clippy::too_many_arguments)]`. The data is already in `PageJson` (which has `quality.ocr_box_count`, `timings.total`, etc.).

**Fix:** Pass `&PageJson` instead.

```rust
pub(crate) fn print_done(page: &PageJson, total: usize, elapsed: f64, done: usize, outdir: &str) {
    let rate = done as f64 / elapsed.max(0.1);
    let eta = (total - page.page) as f64 / rate.max(0.01);
    eprintln!(
        "[done] page {:03}/{} regions={} boxes={} {:.1}s | ETA {:.0}min",
        page.page, total,
        page.layout_regions.len(),
        page.ocr_boxes.len(),
        page.timings.total,
        eta / 60.0
    );
    // ... progress counter stays
}
```

**Net:** -3 params, no `#[allow]`.

### Phase 7 exit gate

```bash
cargo test --all-targets && cargo clippy --all-targets -- -D warnings
```

---

## Phase 8 — Trim `Manifest` write-only fields

**Finding #7.** `Manifest` has 12 fields; 5 are write-only (never deserialized anywhere): `schema_version`, `mode`, `quality_failed`, `review_required`, `vlm_candidates`.

**Fix:** Remove the 5 unused fields from `Manifest` struct. Update `main.rs` where the manifest is constructed. Update `scripts/validate_manifest.py` required fields set to match.

```rust
// manifest.rs — after
#[derive(Debug, Serialize)]
pub(crate) struct Manifest {
    pub input: String,
    pub output_dir: String,
    pub ok: usize,
    pub skipped: usize,
    pub failed: usize,
    pub pages_total: usize,
    pub pages_empty: usize,
    pub content_integrity: String,
}
```

```python
# scripts/validate_manifest.py — update required set
required = {"input", "output_dir", "pages_total", "ok", "skipped", "failed", "content_integrity"}
```

**Net:** -5 fields, cleaner contract.

### Phase 8 exit gate

```bash
cargo test --all-targets && cargo clippy --all-targets -- -D warnings
python3 scripts/validate_manifest.py tests/fixtures/.../manifest.json
python3 scripts/test_report.py
```

---

## Phase 9 — ARCHITECTURAL: Native markdown path via pdf_oxide (largest cut)

**Finding #1.** The biggest finding. Current flow for native-text PDFs:

```
PDF → probe_page() → extract_text() → wrap in fake PageJson (bbox=[0,0,0,0],
  risk_flags=["native_text_no_coordinates"]) → send to LLM → LLM reconstructs
  to markdown
```

pdf_oxide v0.3.77 ships `PdfDocument::to_markdown(page, &ConversionOptions)` which does:
- Text extraction with reading order (`ReadingOrderMode::ColumnAware`)
- Heading detection (`detect_headings: true`)
- Table extraction (`extract_tables: true`)
- Header/footer stripping (`strip_running_headers_footers: true`)
- Image extraction (`include_images: false` for compactness)

This is a superset of what the LLM prompt asks for, and it's deterministic + free.

**New flow for native-text PDFs:**

```
PDF → probe_page() → if native_text_chars > threshold:
  → PdfDocument::to_markdown(page, &opts) → write .md directly
  → skip LLM entirely
else (image-only/sparse):
  → pdftoppm → PaddleOCR → PageJson → LLM reconstruct (unchanged)
```

### 9.1 Add native markdown subcommand to `pdfoxide_backend.rs`

```rust
use pdf_oxide::converters::{ConversionOptions, ReadingOrderMode};
use pdf_oxide::PdfDocument;

pub(crate) fn extract_markdown(path: &Path, page: usize) -> Result<String, String> {
    let path = path.to_str().ok_or("PDF path is not valid UTF-8")?;
    let doc = PdfDocument::open(path).map_err(|e| format!("PDFOxide open: {e}"))?;
    let opts = ConversionOptions {
        detect_headings: true,
        extract_tables: true,
        strip_running_headers_footers: true,
        reading_order_mode: ReadingOrderMode::ColumnAware,
        ..Default::default()
    };
    doc.to_markdown(page, &opts).map_err(|e| format!("markdown convert page {page}: {e}"))
}
```

### 9.2 Add routing in `reconstruct.rs`

In `reconstruct_one()`, before the LLM call:

```rust
// If the page has native text, use pdf_oxide's markdown converter directly
if let Ok(probe) = probe_page(Path::new(&args.source_pdf), page_num - 1) {
    if !probe.image_only && probe.native_text_chars >= 20 {
        return extract_markdown(Path::new(&args.source_pdf), page_num - 1);
    }
}
// else: fall through to LLM reconstruct (for scanned/image-only pages)
```

### 9.3 Simplify `pdfoxide_backend.rs::extract_page`

The current `extract_page` wraps text in a fake `PageJson` with `bbox: [0.0; 4]` and `risk_flags: ["native_text_no_coordinates"]`. This is only needed if the LLM path requires JSON input. With native markdown, `extract_page` is only called as a fallback probe — it can be simplified to return just text, or removed entirely in favor of `probe_page` + `extract_markdown`.

**Keep `extract_page` for backward compat** (existing `json/` files have this format), but new native-text pages skip it entirely.

### 9.4 What stays

- `reconstruct.rs` LLM path stays for **scanned PDFs only** (image-only or sparse text pages).
- `furniture.rs` already deleted in Phase 4 — `strip_running_headers_footers` replaces it.
- `page.rs` OCR pipeline stays unchanged (renders + OCRs scanned pages).
- `cleanup.rs` `RegexFixes` stays — applied as post-processing on both native and LLM markdown.

### 9.5 Cargo.toml — pin pdf_oxide

```toml
pdf_oxide = "0.3"  # resolves to 0.3.77, which ships ConversionOptions
```

No change needed — Cargo already resolves to 0.3.77. But add a comment:

```toml
# 0.3.77+ ships ConversionOptions + to_markdown() — required for native markdown path
pdf_oxide = "0.3"
```

**Net:** -~200 lines of LLM prompt + fake-JSON construction for native pages. LLM API calls eliminated for text-rich PDFs (cost + latency savings).

### Phase 9 exit gate

```bash
cargo test --all-targets && cargo clippy --all-targets -- -D warnings
# Functional test: run ocr + reconstruct on a native-text PDF, verify markdown output
# without any LLM API key
./target/release/pdf2md ocr --pdf ./input.pdf --outdir ./json
./target/release/pdf2md reconstruct --json-dir ./json --source-pdf ./input.pdf --outdir ./output
# Verify: pages with native text have markdown in ./output/<pdf-stem>/md/
```

---

## Phase 10 — Update README

Reflect the new architecture:

```markdown
## How it works

PDF → probe → native markdown (pdf_oxide) OR OCR + LLM reconstruct (scanned pages)

- `ocr` renders and OCRs pages that lack native text (scanned PDFs).
- `reconstruct` converts page JSON to Markdown. For native-text pages, uses
  pdf_oxide's built-in markdown converter (no LLM needed). For scanned pages,
  uses the LLM-assisted pipeline.
- Quality checks catch missing page markers, lost numeric tokens, malformed
  tables, and invalid OCR results.
```

Remove references to `furniture.rs`, `filtered/` directory, and `frequency_dict.txt`.

---

## Summary

| Phase | Finding # | Lines cut | Risk |
|---|---|---|---|
| 0 | 2,3,10,11 | 92 files + 115K lines + 1 doc | None |
| 1 | 6 | -10 | Low |
| 2 | 4 | -93 | Medium (config merge) |
| 3 | 5 | -71 | Low |
| 4 | 8 | -251 | Medium (LLM prompt already handles) |
| 5 | 9 | -53 | Low |
| 6 | 12 | 0 (move) | Low |
| 7 | 13 | -3 params | Low |
| 8 | 7 | -5 fields | Low |
| 9 | 1 | -200 (LLM bypass) | High (architectural) |
| 10 | — | README | None |

**Total:** -~683 source lines, -92 tracked files, -115K dead dict lines, -2 modules, LLM cost eliminated for native-text PDFs.

## Execution order

Phases 0-8 are safe deletions/shrinks. Phase 9 is the architectural change — do it last, after all cleanup is committed and tests pass. Each phase commits independently.

```
Phase 0 → commit → verify
Phase 1 → commit → verify
...
Phase 8 → commit → verify
Phase 9 → commit → verify → functional test
Phase 10 → commit → push
```
