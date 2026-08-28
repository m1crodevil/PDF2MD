# PDF2MD Universal Document Engine Plan

Status: staged implementation plan; each phase must pass its exit gate before the next begins.

## Goal

Turn PDF2MD into a universal, page-aware document transformation engine for native PDFs, scans, mixed PDFs, forms, tables, figures, slides, reports, and other structured pages. The engine must preserve content and structure without assuming a legal, academic, or business domain.

## Evidence-backed design decisions

- Document parsing must evaluate diverse page types and multiple dimensions, not OCR alone. OmniDocBench uses nine document categories, layout and recognition annotations, and end-to-end plus module-level evaluation.
- OCR is only one stage. Production transformation needs partitioning, reading order, tables/images, metadata, normalization, and explicit provenance.
- Tables should retain cell boundaries and headers; flattening them into prose is lossy.
- Native text and OCR must be separate acquisition paths, with per-page routing because mixed PDFs are common.
- End-to-end model output is useful for structure and difficult visual content, but deterministic evidence and validation must remain authoritative for retained text.

Sources:

- OmniDocBench, CVPR 2025: https://arxiv.org/abs/2412.07626
- OmniDocBench paper: https://openaccess.thecvf.com/content/CVPR2025/papers/Ouyang_OmniDocBench_Benchmarking_Diverse_PDF_Document_Parsing_with_Comprehensive_Annotations_CVPR_2025_paper.pdf
- PaddleOCR PP-StructureV3: https://paddlepaddle.github.io/PaddleOCR/latest/en/version3.x/pipeline_usage/PP-StructureV3.html
- PDFium: https://pdfium.googlesource.com/pdfium/+/main/README.md
- Unstructured document transformation guidance: https://unstructured.io/insights/how-to-transform-text-images-documents-for-ai

## Target architecture

```text
CLI/config
  -> inspect/probe
  -> page router
       -> native extraction (PDFOxide)
       -> rendered acquisition (PDFium + OCR)
       -> optional visual fallback
  -> mandatory layout/region analysis
  -> versioned document IR
  -> furniture + reading order + table/image normalization
  -> deterministic Markdown/JSON/asset renderers
  -> optional LLM/VLM enhancement
  -> retention, completeness, structure, and visual quality gates
  -> manifest + concise report + reproducible artifacts
```

All stages are independently rerunnable. `run` is only a convenience composition.

## Universal IR contract

The core schema must be domain-neutral:

- document: source hash, page count, schema version, tool/model provenance;
- page: page number, dimensions, acquisition route, timings, status;
- region: stable ID, category, bounding box, reading-order index, confidence;
- content: text spans, tables/cells, formulas, images, captions, lists, headers/footers;
- evidence: source artifact, exact text, coordinates, backend and model;
- quality: blank/sparse signals, protected-token results, review flags, unresolved regions.

Provider-specific fields belong in an extension map, not in the core contract.

## Staged execution

### Phase 0 — Baseline and portability

Freeze current tests, fixture contracts, dependency checks, and a synthetic multi-layout corpus. Remove domain-specific names from public docs and fixtures. Scan tracked files, Git history, and generated artifacts for secrets and personal paths.

Exit: clean portable tree, reproducible baseline, no private data in tracked content.

### Phase 1 — Acquisition and routing

Make probe results explicit and page-aware. Route native-text, scan, sparse, and mixed pages independently. Preserve route evidence and never turn an extraction error into a blank page.

Exit: routing tests cover native, scan, sparse, and mixed inputs.

### Phase 2 — IR and adapters

Move native and OCR outputs behind the versioned IR. Keep raw acquisition evidence separate from derived structure so later stages can rerun without reacquisition.

Exit: both current adapters produce the same IR and schema validation rejects malformed pages.

### Phase 3 — Structure

Make layout analysis, reading order, furniture classification, table handling, image/figure handling, and captions first-class stages. Ambiguous regions remain in output and receive review flags.

Exit: multi-column, table, list, figure, header/footer, and mixed-region fixtures preserve order and region identity.

### Phase 4 — Modular workers and cache

Split CLI orchestration into stage modules. Add deterministic cache keys using source hash, page, route, renderer settings, model identity/version, and configuration hash. Use bounded pools separately for CPU acquisition and network reconstruction. Retry only failed pages. Stream manifest updates and keep output ordering deterministic.

Exit: each stage reruns independently; interrupted runs resume; cache invalidation is tested; memory and concurrency stay bounded.

### Phase 5 — Output and quality

Produce deterministic Markdown and structured JSON, retaining tables as structured data and images as linked assets. Add universal quality metrics: page coverage, sparse-page rate, text CER/WER when gold text exists, token precision/recall, omission rate, reading-order errors, table structure score, region coverage, and unresolved visual regions.

Exit: quality gates fail closed on silent loss, malformed output, missing pages, or unresolved configured hard requirements. Thresholds are versioned per corpus, never invented globally.

### Phase 6 — Report and release

Generate a concise XeLaTeX/PDF report containing run ID, input hash, stage/model versions, route counts, timings, quality metrics, failures, review items, and artifact locations. Add CI checks for formatting, tests, hygiene, synthetic regression, and report compilation/content verification. Rewrite README around universal document transformation, not a specific domain.

Exit: report compiles in a clean environment, contents match manifest, CI catches deliberate regressions, README commands are reproducible.

## Universal quality policy

Hard gates:

- every requested page has a manifest result;
- no silent blank or dropped page;
- output schema and page order are valid;
- required evidence tokens retain exact normalized forms;
- tables/images/formulas are either represented or explicitly flagged;
- no unresolved secret/private-data scan finding.

Measured gates are corpus-specific and require ground truth. A model confidence value alone is never a pass gate.

## Implementation order for this repository

1. Replace domain-specific README and fixture terminology with `document` / `structured-content` terminology.
2. Audit existing `main.rs`, `manifest.rs`, `reconstruct.rs`, config, and CLI against Phase 4; reuse existing cache and bounded workers where they already satisfy the contract.
3. Add only missing stage boundaries and manifest fields.
4. Add synthetic universal fixtures and quality checks.
5. Update the report and CI.
6. Run full verification, commit, push, and compare local/remote SHA.
