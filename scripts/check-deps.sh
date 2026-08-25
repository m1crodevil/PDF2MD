#!/usr/bin/env bash
set -euo pipefail

need() { command -v "$1" >/dev/null 2>&1 || { echo "missing: $1"; exit 1; }; }

need cargo
need curl
need pdftoppm
need python3

echo "ok: cargo curl pdftoppm python3"
