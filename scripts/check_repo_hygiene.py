#!/usr/bin/env python3
"""Reject tracked files containing local paths or credential-shaped values."""
from __future__ import annotations

import re
import subprocess
import sys

PATTERNS = (
    re.compile(r"/(?:home|Users)/[A-Za-z0-9._-]+/"),
    re.compile(r"/(?:tmp|private|var/folders)/[A-Za-z0-9._/-]+"),
    re.compile(r"-----BEGIN (?:RSA|OPENSSH|EC|DSA) PRIVATE KEY-----"),
    re.compile(r"\bBearer\s+[A-Za-z0-9._~+/=-]{20,}"),
    re.compile(r"(?i)\b(?:api[_-]?key|access[_-]?token|secret)\s*[:=]\s*['\"][^'\"]{12,}['\"]"),
)


def main() -> int:
    files = subprocess.check_output(["git", "ls-files", "-z"], text=False).split(b"\0")
    findings: list[str] = []
    for raw_path in filter(None, files):
        path = raw_path.decode()
        try:
            text = open(path, encoding="utf-8").read()
        except (UnicodeDecodeError, OSError):
            continue
        for line_no, line in enumerate(text.splitlines(), 1):
            if any(pattern.search(line) for pattern in PATTERNS):
                findings.append(f"{path}:{line_no}")
    if findings:
        print("repository hygiene check failed:")
        print("\n".join(findings))
        return 1
    print(f"repository hygiene check passed ({len(list(filter(None, files)))} tracked files)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
