# PDF2MD

A lightweight Rust pipeline that turns PDFs into clean Markdown for downstream agent workflows.

## What it does

- `ocr` → PDF to per-page JSON
- `reconstruct` → page JSON to Markdown via a configurable LLM endpoint
- config via `config/pdf2md.toml`
- manifest output for run tracking

## Quick start

```bash
cargo build --release
./target/release/pdf2md --help
```

## Config

Default config lives at:

```bash
config/pdf2md.toml
```

## Dependency check

```bash
./scripts/check-deps.sh
```

## Notes

- Uses the existing OCR helper boundary.
- Reconstruct mode expects `curl` and an API key provided via config or environment.
