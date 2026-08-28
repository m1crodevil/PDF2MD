#!/usr/bin/env python3
"""Validate a runtime PDF2MD manifest and its report inputs."""
from pathlib import Path
import json
import sys


def main() -> int:
    if len(sys.argv) != 2:
        print(f"usage: {sys.argv[0]} <manifest.json>", file=sys.stderr)
        return 2
    path = Path(sys.argv[1])
    data = json.loads(path.read_text(encoding="utf-8"))
    required = {"schema_version", "mode", "input", "output_dir", "pages_total", "ok", "skipped", "failed", "content_integrity"}
    missing = sorted(required - data.keys())
    if missing:
        raise SystemExit(f"missing manifest fields: {', '.join(missing)}")
    total = int(data["pages_total"])
    if int(data["ok"]) + int(data["skipped"]) + int(data["failed"]) != total:
        raise SystemExit("manifest counters do not equal pages_total")
    expected = "complete" if int(data["failed"]) == 0 and int(data.get("pages_empty", 0)) == 0 else "incomplete"
    if data["content_integrity"] != expected:
        raise SystemExit("content_integrity disagrees with counters")
    print(f"manifest_valid: true\npages_total: {total}\ncontent_integrity: {data['content_integrity']}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
