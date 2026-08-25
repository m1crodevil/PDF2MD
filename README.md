# PDF2MD

A lightweight Rust pipeline that turns PDFs into page-aware Markdown and JSON for downstream agent workflows.

> **Current limitation:** PDF2MD is not a full document-vision system. It can extract text and layout metadata, but it cannot reliably understand the semantic content of embedded images, charts, diagrams, or other visually complex objects. VLM-based visual understanding is planned for a later phase.

## What it does

- `ocr` → scanned PDF to per-page JSON
- layout analysis → detects regions such as text, tables, figures, and captions
- adaptive OCR → selects the OCR model based on detected page layout
- `reconstruct` → page JSON to Markdown via a configurable LLM endpoint
- deterministic OCR cleanup while preserving raw OCR text
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
- raw and cleaned OCR text where available

Reconstruction produces Markdown and a manifest describing page-level results, including failures and quality-gate failures.

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

## Development status

The project is being developed incrementally:

1. text extraction and OCR;
2. layout-aware page JSON;
3. Markdown reconstruction and quality validation;
4. VLM integration for complex visual objects — planned.
