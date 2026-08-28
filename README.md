# PDF2MD

PDF2MD is a small Rust pipeline that turns PDFs into page-aware JSON and Markdown.
It combines PDF rendering, OCR, layout detection, and optional LLM-assisted
reconstruction.

> **Important:** PDF2MD extracts text and layout. It does not yet understand the
> meaning of charts, diagrams, graphs, or image-only content. Review those pages
> manually or send them through a VLM in a later step.

## How it works

```text
PDF → probe → PDFOxide or OCR/PP-DocLayout → page JSON → Markdown reconstruction
```

- `ocr` probes each page first: text-rich pages use PDFOxide; image-only or
  text-insufficient pages use the existing PDF render + PP-DocLayout +
  `faster_paddle` medium route.
- `reconstruct` validates those JSON files and turns them into Markdown.
- Optional `pdfium` support renders bounded visual-QA artifacts when native
  extraction fails; it is not required for the default route.
- The final output includes per-page Markdown, `document.md`, and a manifest.
- Quality checks catch missing page markers, lost numeric tokens, malformed tables,
  and invalid OCR results.

The pipeline keeps raw OCR text, cleaned text, layout regions, confidence scores,
reading order, and review flags. These signals help route difficult pages; they
are not a guarantee that the document was understood correctly.

## What it handles well

- Scanned PDFs with ordinary text and common page layouts.
- Basic tables, figures, captions, and multi-region pages.
- Resumable reconstruction with bounded concurrency.
- Retry handling for temporary API failures.
- Page-level validation and a run manifest.

## Current limits

PDF2MD is not a replacement for visual document understanding. It may struggle
with charts, plots, diagrams, image-only information, complex figure/legend
relationships, cross-page tables, and unusual multi-column layouts. Those pages
are flagged for review instead of being presented as reliably understood.

## Quick start

```bash
cargo build --release
./scripts/check-deps.sh
./target/release/pdf2md ocr --pdf ./input.pdf --outdir ./json
./target/release/pdf2md reconstruct --concurrency 2
```

The dependency check covers `cargo`, `curl`, `pdfinfo`, `pdftoppm`, `python3`,
`paddleocr`, and `faster_paddle`. Page count is discovered automatically; use
`--start`, `--end`, and optionally `--total` for bounded ranges.

To build the optional PDFium path:

```bash
cargo check --features pdfium
PDFIUM_LIBRARY_PATH=/path/to/libpdfium.so cargo test --features pdfium --all-targets
```

## Configuration

The portable template is `config/pdf2md.toml`; machine-specific overrides belong
in ignored `config/pdf2md.local.toml`. Keep API keys in `.env` or the environment,
never in Git. `reconstruct` fails fast when endpoint or model configuration is missing.

## Output and resume behavior

OCR writes page JSON with layout regions, OCR boxes, confidence, cleaned/raw text,
quality data, and risk flags. Reconstruction writes per-page Markdown,
`document.md`, `manifest.json`, and `.cache/reconstruct/`. Existing Markdown and
cache entries are reused only after Markdown and retention validation.

## Universal content-quality contract

`faster_paddle` **medium** is the default OCR model for rendered pages. OCR acquisition
and optional reconstruction are separate stages. Numbers, dates, units, identifiers,
formulas, and other configured protected tokens found in source evidence must survive
reconstruction. Classified repeated page furniture may be excluded; uncertain content
is preserved.

If protected content is lost, reconstruction writes a deterministic source-text
fallback and marks the page as `quality_failed`/review candidate rather than silently
accepting incomplete Markdown. Cache hits are validated again. Confidence alone is
not a production gate, and tables, figures, forms, and visual regions may require review.

Run deterministic checks:

```bash
cargo fmt --all -- --check
cargo test --all-targets
cargo clippy --all-targets --all-features -- -D warnings
git diff --check
python3 scripts/evaluate_quality.py --fixtures tests/fixtures/quality
python3 scripts/check_regression_fixture.py tests/fixtures/document-regression
```

The document fixture records a regression contract while source PDFs and live OCR/LLM
outputs remain outside Git. Live benchmarks are separate because model/API results
vary; production validation needs a ground-truth corpus.

## Development

```bash
cargo fmt -- --check
cargo check
cargo test
cargo build --release
```

The deterministic evaluator checks CER/WER, numeric retention, Markdown validity,
page coverage, and tables. VLM processing remains opt-in for pages whose visual risk
signals justify the cost.

## Pipeline guarantees and release gate

The OCR command is page-aware and resumable: cached page JSON is skipped, while
reconstruction uses bounded concurrency and revalidates cached Markdown before reuse.
Each run writes page results and a manifest; failures remain explicit rather than
becoming blank pages. The CI gate runs formatting, tests, Clippy, repository hygiene,
quality fixtures, and the document regression contract. A release still requires a live
document smoke test because OCR and LLM output can vary by model and input.

## Upstream projects

- [PaddleOCR](https://github.com/PaddlePaddle/PaddleOCR)
- [PaddleX](https://github.com/PaddlePaddle/PaddleX)
- [FastDeploy](https://github.com/PaddlePaddle/FastDeploy)
- [faster-paddle](https://github.com/cnmoro/faster-paddle)
