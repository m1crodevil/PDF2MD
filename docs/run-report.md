# Run report

A runtime reconstruction writes `manifest.json`. Generate a deterministic XeLaTeX report from that manifest:

```bash
python3 scripts/validate_manifest.py /path/to/output/manifest.json
python3 scripts/generate_report.py /path/to/output/manifest.json /path/to/output/report.tex
python3 scripts/generate_report.py /path/to/output/manifest.json /path/to/output/report.tex --pdf
```

The `--pdf` form requires `xelatex`. The report records the manifest's input, output, stage counters, page coverage, integrity status, and review counters. It is an operational run report, not proof of semantic correctness; corpus-backed quality evaluation remains a separate gate.
