# PDF2MD

PDF2MD is a small Rust pipeline that turns PDFs into page-aware JSON and Markdown.
It combines PDF rendering, OCR, layout detection, and optional LLM-assisted
reconstruction.

> **Important:** PDF2MD extracts text and layout. It does not yet understand the
> meaning of charts, diagrams, graphs, or image-only content. Review those pages
> manually or send them through a VLM in a later step.

## How it works

```text
PDF → render/OCR → page JSON → Markdown reconstruction → document.md
```

- `ocr` renders pages and writes one JSON file per page.
- `reconstruct` validates those JSON files and turns them into Markdown.
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
with:

- charts, plots, and diagrams;
- information stored only inside images;
- complex figure/legend relationships;
- cross-page tables and unusual multi-column layouts.

Those pages are flagged for review instead of being presented as reliably
understood.

## Quick start

Build the binary and check the local dependencies:

```bash
cargo build --release
./scripts/check-deps.sh
```

The dependency check covers `cargo`, `curl`, `pdfinfo`, `pdftoppm`, `python3`,
`paddleocr`, and `faster_paddle`.

Run OCR:

```bash
./target/release/pdf2md ocr \
  --pdf ./input.pdf \
  --outdir ./json
```

The page count is discovered from the PDF automatically. Use `--start`, `--end`,
and optionally `--total` to process a bounded range. Ranges are checked before
rendering starts, and any page error makes the command exit non-zero.

Reconstruct Markdown:

```bash
./target/release/pdf2md reconstruct --concurrency 2
```

Run `--help` on either command for the complete option list.

## Configuration

The portable template lives at:

```text
config/pdf2md.toml
```

Keep machine-specific settings in the ignored local override when needed:

```text
config/pdf2md.local.toml
```

If `config/pdf2md.local.toml` exists, it is authoritative and parse errors stop the
program instead of silently falling back. Without it, the portable template is used.
Generated run bundles under `runs/` are local artifacts and are ignored by Git.

Keep API keys in `.env` or the environment. Never commit credentials. A typical
local setup provides:

```dotenv
PDF2MD_API_KEY=...
PDF2MD_BASE_URL=https://your-llm-endpoint/v1
PDF2MD_MODEL=your-model
PDF2MD_REASONING_EFFORT=none
```

`reconstruct` fails fast when the endpoint or model is missing. It reads
`message.content` from the configured API response; provider reasoning fields are
not treated as Markdown.

## Output and resume behavior

OCR writes page JSON containing layout regions, OCR boxes, confidence, cleaned and
raw text, quality data, and risk flags.

Reconstruction writes:

```text
<outdir>/<pdf-stem>/
├── md/page_001.md
├── document.md
├── manifest.json
└── .cache/reconstruct/
```

Existing Markdown is reused only when it passes validation. Validated responses
are also cached by model, prompt, and page JSON, so changing any of those inputs
naturally creates a new cache entry.

Malformed JSON, a page-number mismatch, or a page status other than `success` is
rejected before an LLM request. A run can still write its manifest, but exits
non-zero when one or more pages fail.

## Development

Run the standard checks:

```bash
cargo fmt -- --check
cargo check
cargo test
cargo build --release
```

### Quality evaluation

The repository includes a deterministic, dependency-free evaluator for the first
quality gate. It measures normalized CER/WER, numeric-token retention, Markdown
sanity, page coverage, and table presence:

```bash
python3 scripts/evaluate_quality.py \
  --fixtures tests/fixtures/quality \
  --json target/quality-report.json \
  --max-cer 1.0 --max-wer 1.0 --min-numeric-recall 1.0
```

The committed fixture is a smoke test only; production claims require a licensed,
representative PDF corpus with human-verified gold text/layout and separate test
splits. Keep generated reports under `target/` and inspect failed pages manually.
Thresholds are explicit CLI inputs so CI can tighten them after a real baseline is
measured; the committed smoke fixture does not claim production OCR accuracy.

The project intentionally keeps deterministic extraction and validation as the
default path. VLM processing is planned for pages whose visual risk signals justify
the extra cost.

## Upstream projects

PDF2MD uses the Paddle ecosystem for OCR and layout detection:

- [PaddleOCR](https://github.com/PaddlePaddle/PaddleOCR)
- [PaddleX](https://github.com/PaddlePaddle/PaddleX)
- [FastDeploy](https://github.com/PaddlePaddle/FastDeploy)
- [faster-paddle](https://github.com/cnmoro/faster-paddle)
