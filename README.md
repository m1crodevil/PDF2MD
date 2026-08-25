# PDF2MD

A lightweight Rust pipeline that turns PDFs into page-aware Markdown and JSON for downstream agent workflows.

> **Current limitation:** PDF2MD is not a full document-vision system. It can extract text and layout metadata, but it cannot reliably understand the semantic content of embedded images, charts, diagrams, or other visually complex objects. VLM-based visual understanding is planned for a later phase.

## What it does

- `ocr` → scanned PDF to per-page JSON
- layout analysis → detects regions such as text, tables, figures, and captions
- adaptive OCR → selects the OCR model based on detected page layout
- `reconstruct` → page JSON to Markdown via a configurable LLM endpoint
- deterministic OCR cleanup while preserving raw OCR text
- page quality assessment and risk flags for tables, visual objects, confidence, and reading order
- Markdown validation for page markers, numeric-token retention, and detected table structure
- conservative document-level concatenation into `document.md`
- quality validation, retry handling, resume support, and run manifest output

## What it does not do yet

PDF2MD currently does **not** reliably:

- interpret the meaning of charts, graphs, plots, or diagrams;
- extract and explain information encoded only in an image;
- perform visual question answering over page objects;
- reconstruct complex visual relationships between figures, labels, legends, and surrounding text;
- replace a VLM for image-heavy or graph-heavy documents.

The current pipeline can preserve detected regions and OCR text as context, but that context is not equivalent to visual understanding. Pages containing complex visual objects should be treated as requiring review or VLM processing in the next development phase.

## Pipeline

```text
PDF
 ├─ text-based pages ──> text/layout extraction
 └─ scanned pages ─────> OCR + layout analysis
                              │
                              ├─ page JSON with OCR boxes and regions
                              └─ Markdown reconstruction via LLM

Future phase: VLM analysis for images, charts, diagrams, and visual relationships
```

## Quick start

```bash
cargo build --release
./target/release/pdf2md --help
```

Check local dependencies:

```bash
./scripts/check-deps.sh
```

## Input and output

OCR produces per-page JSON containing, among other fields:

- `layout_regions`
- `ocr_boxes`
- OCR confidence scores
- bounding boxes and reading-order context
- `ocr_model`
- `reading_order` — deterministic region order derived from layout coordinates
- `risk_flags` — for example `table_detected`, `visual_object`, `low_confidence`, `ambiguous_reading_order`, and `blank`
- `quality` — text length, OCR box count, mean confidence, low-confidence ratio, detected table/visual regions, and review status
- raw and cleaned OCR text where available

Reconstruction produces per-page Markdown, a conservative concatenated `document.md`, and a manifest describing page-level results. The manifest includes `quality_failed`, `review_required`, and `vlm_candidates`; these are routing and observability signals, not proof that the output is semantically correct.

## Configuration

Default configuration:

```text
config/pdf2md.toml
```

`reconstruct` expects `curl` and an API key supplied through the configured LLM endpoint or environment. Do not commit credentials.

## Upstream projects

PDF2MD uses the Paddle ecosystem for OCR and document-layout analysis. These are the relevant upstream repositories:

- [PaddleOCR](https://github.com/PaddlePaddle/PaddleOCR) — OCR and document-understanding toolkit, including PP-DocLayout-related components.
- [PaddleX](https://github.com/PaddlePaddle/PaddleX) — PaddlePaddle's broader AI development and deployment platform.
- [FastDeploy](https://github.com/PaddlePaddle/FastDeploy) — official PaddlePaddle inference/deployment toolkit.
- [faster-paddle](https://github.com/cnmoro/faster-paddle) — the faster-paddle repository used as the local OCR helper/runtime reference in this project.

The upstream OCR/layout components provide text and region detection. They do not, by themselves, provide complete semantic understanding of arbitrary charts, diagrams, or image-only objects. That gap is the scope of the planned VLM phase.

## Quality and routing behavior

PDF2MD separates **API success** from **content quality**. A reconstruction can fail the quality gate even when the LLM request itself succeeded.

Current checks include:

- empty or marker-only Markdown is rejected;
- the expected page marker must be present;
- pages with enough numeric evidence are checked for severe numeric-token loss;
- a page detected as a table must produce a Markdown table structure;
- existing output is resumed only when it passes validation;
- transient request failures are retried, while non-transient HTTP errors are not blindly retried.

Risk flags do not automatically invoke a VLM. They identify pages for review or a future selective VLM route. In particular, `vlm_candidates` means that a visual object was detected; it does not mean the visual object has already been understood.

The region reading order is currently a conservative bounding-box order. Complex multi-column layouts, cross-page tables, figures, charts, and diagrams can still require review.

## Development status

Implemented:

1. text extraction and OCR;
2. layout-aware page JSON;
3. adaptive OCR model routing;
4. deterministic cleanup with raw-text preservation;
5. page quality metadata and risk flags;
6. reconstruction quality validation;
7. explicit reading-order metadata;
8. conservative document-level `document.md` output;
9. retry, resume, temporary-directory cleanup, and manifest observability.

Planned:

1. benchmark fixtures and structural fidelity metrics;
2. stronger table structure validation and cross-page merge;
3. selective VLM integration for flagged visual pages;
4. semantic understanding of images, charts, diagrams, and complex visual relationships.

The project deliberately does not send every page to a VLM. Deterministic extraction and validation remain the default; expensive visual processing should be reserved for pages whose risk signals justify it.
