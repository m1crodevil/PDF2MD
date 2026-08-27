#!/usr/bin/env bash
set -euo pipefail

need() { command -v "$1" >/dev/null 2>&1 || { echo "missing: $1"; exit 1; }; }

need cargo
need curl
need pdfinfo
need pdftoppm
need python3

python3 - <<'PY'
import importlib.util
import sys

missing = [name for name in ("paddleocr", "faster_paddle")
           if importlib.util.find_spec(name) is None]
if missing:
    print("missing Python packages: " + ", ".join(missing))
    sys.exit(1)
PY

echo "ok: cargo curl pdftoppm python3 paddleocr faster_paddle"
