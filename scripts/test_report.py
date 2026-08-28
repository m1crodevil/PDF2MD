#!/usr/bin/env python3
"""Smoke-test manifest validation and deterministic report generation."""
from pathlib import Path
import json
import subprocess
import tempfile

with tempfile.TemporaryDirectory() as directory:
    root = Path(directory)
    manifest = root / "manifest.json"
    manifest.write_text(json.dumps({"schema_version":"pdf2md-manifest-v2","mode":"reconstruct","input":"fixture","output_dir":str(root),"ok":1,"skipped":0,"failed":0,"quality_failed":0,"review_required":0,"vlm_candidates":0,"pages_total":1,"pages_empty":0,"content_integrity":"complete"}))
    subprocess.run(["python3", "scripts/validate_manifest.py", str(manifest)], check=True)
    report = root / "report.tex"
    subprocess.run(["python3", "scripts/generate_report.py", str(manifest), str(report)], check=True)
    assert "PDF2MD Run Report" in report.read_text()
print("report_smoke: PASS")
