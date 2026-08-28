# Legal-document regression fixture

This fixture records high-risk assertions from a structured-document sample without committing the source PDF or OCR artifacts.

The source PDF is supplied locally at runtime. `manifest.json` identifies the expected page count and protected tokens. A regression run must provide the PDF path and compare the generated JSON/Markdown against these assertions.

The fixture is intentionally not a full ground-truth transcription yet; it is a bounded integrity gate for page coverage, furniture removal, and structured-token retention.
