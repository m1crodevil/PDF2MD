#!/usr/bin/env python3
"""Validate the committed legal-document regression contract and optional output bundle."""
import json
import sys
from pathlib import Path

root = Path(sys.argv[1] if len(sys.argv) > 1 else "tests/fixtures/legal-regression")
manifest = json.loads((root / "manifest.json").read_text(encoding="utf-8"))
if manifest.get("pages_total", 0) <= 0 or not manifest.get("protected_tokens"):
    raise SystemExit("invalid regression contract")
output_root = Path(sys.argv[2]) if len(sys.argv) > 2 else root / "output"
files = sorted(output_root.glob("*.md")) if output_root.is_dir() else []
if not files:
    print(json.dumps({"contract_valid": True, "output_checked": False, "pages_total": manifest["pages_total"]}))
    raise SystemExit(0)
output = "\n".join(p.read_text(encoding="utf-8") for p in files)
missing = [t for t in manifest["protected_tokens"] if t.lower() not in output.lower()]
forbidden = [t for t in manifest["required_absent_tokens"] if t in output]
if missing or forbidden:
    print(json.dumps({"missing_protected_tokens": missing, "forbidden_tokens": forbidden}))
    raise SystemExit(1)
print(json.dumps({"contract_valid": True, "output_checked": True, "pages": len(files)}))
