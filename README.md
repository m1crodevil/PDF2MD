# PDF2MD

PDF2MD is a Rust pipeline that converts PDF documents into page-aware JSON and Markdown. It combines native text extraction, OCR, layout analysis, optional LLM reconstruction, validation, and resumable output.

## Processing model

```text
PDF → page probe → native Markdown OR OCR JSON → optional LLM reconstruction → validation
```

- **Text-rich pages:** use `pdf_oxide` native Markdown conversion. This route is deterministic and does not require an API key.
- **Scanned or sparse-text pages:** use PDF rendering, PP-DocLayout, and `faster_paddle` OCR. Reconstruction uses the configured LLM when required.
- **Mixed documents:** route each page independently; native pages bypass the LLM.
- **Quality validation:** checks page markers, page coverage, protected tokens, Markdown structure, and retention. Failed validation remains explicit in the manifest.

Image-only pages, figures, charts, diagrams, complex tables, and unusual layouts may require manual review or an opt-in VLM workflow.

## Requirements

Run the dependency check before processing a new environment:

```bash
./scripts/check-deps.sh
```

The check covers Rust tooling, `curl`, Poppler utilities, Python, PaddleOCR, and `faster_paddle`.

## Quick start

```bash
cargo build --release

./target/release/pdf2md ocr \
  --pdf ./input.pdf \
  --outdir ./json

./target/release/pdf2md reconstruct \
  --json-dir ./json \
  --source-pdf ./input.pdf \
  --outdir ./output
```

`--source-pdf` is also used as the original-document path when `--original-pdf` is omitted. Provide `--original-pdf` only when the bundled source should be different.

Use `--start`, `--end`, and optionally `--total` to process a bounded page range.

## Configuration

The tracked template is `config/pdf2md.toml`. Put machine-specific overrides in the ignored `config/pdf2md.local.toml`.

Keep credentials in `.env` or the process environment; never commit them. The supported variables are:

```text
PDF2MD_API_KEY
PDF2MD_BASE_URL
PDF2MD_MODEL=cx/gpt-5.6-luna
PDF2MD_REASONING_EFFORT=none
```

Configuration precedence is:

```text
CLI options > local/config values > environment or .env
```

The endpoint must be configured explicitly for LLM reconstruction. Native-only documents can run without `PDF2MD_API_KEY`; OCR/LLM pages require a valid key.

## Output and resume behavior

OCR writes page JSON containing text, layout regions, OCR boxes, confidence, quality data, and risk flags. Reconstruction writes:

- per-page Markdown;
- `document.md`;
- `manifest.json`;
- `.cache/reconstruct/`;
- a copy of the source PDF.

Existing outputs are reused only after Markdown and retention validation. Writes use an atomic temporary-file-and-rename sequence to avoid partial artifacts.

## Quality and development checks

```bash
cargo fmt --all -- --check
cargo test --all-targets
cargo clippy --all-targets --all-features -- -D warnings
git diff --check
python3 scripts/evaluate_quality.py --fixtures tests/fixtures/quality
python3 scripts/check_regression_fixture.py tests/fixtures/document-regression
```

Live OCR/LLM runs are separate from deterministic CI because model output varies by provider and input. A release should include at least one representative document smoke test.

## Current limitations

PDF2MD is not a replacement for visual document understanding. Review pages containing charts, diagrams, image-only information, complex figure/legend relationships, forms, cross-page tables, or unusual multi-column layouts.

## Upstream projects

- [PaddleOCR](https://github.com/PaddlePaddle/PaddleOCR)
- [PaddleX](https://github.com/PaddlePaddle/PaddleX)
- [FastDeploy](https://github.com/PaddlePaddle/FastDeploy)
- [faster-paddle](https://github.com/cnmoro/faster-paddle)
