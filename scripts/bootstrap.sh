#!/usr/bin/env bash
set -euo pipefail

cargo build --release
./scripts/check-deps.sh

echo "ready"
