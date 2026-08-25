#!/usr/bin/env bash
set -euo pipefail

./scripts/check-deps.sh
cargo build --release

echo "ready"
