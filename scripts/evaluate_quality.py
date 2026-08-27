#!/usr/bin/env python3
"""Evaluate PDF2MD page JSON/Markdown bundles against small gold fixtures.

Usage:
  python3 scripts/evaluate_quality.py --fixtures tests/fixtures/quality
  python3 scripts/evaluate_quality.py --fixtures ... --json target/quality.json

The evaluator is intentionally dependency-free and deterministic. It reports CER,
WER, numeric-token retention, Markdown validity, page coverage, and table presence.
It is a baseline gate, not a replacement for human visual review.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
from dataclasses import asdict, dataclass
from pathlib import Path


def norm(text: str) -> str:
    return " ".join(text.replace("\r\n", "\n").split())


def distance(a: list[str], b: list[str]) -> int:
    prev = list(range(len(b) + 1))
    for i, x in enumerate(a, 1):
        cur = [i]
        for j, y in enumerate(b, 1):
            cur.append(min(cur[-1] + 1, prev[j] + 1, prev[j - 1] + (x != y)))
        prev = cur
    return prev[-1]


def tokens(text: str) -> list[str]:
    return norm(text).split()


def numeric_tokens(text: str) -> set[str]:
    return {t for t in tokens(text) if any(c.isdigit() for c in t)}


def markdown_errors(text: str) -> list[str]:
    errors: list[str] = []
    if not norm(text):
        errors.append("empty_markdown")
    if "<!-- PAGE " in text:
        errors.append("page_marker_leaked")
    fences = sum(1 for line in text.splitlines() if line.strip().startswith("```"))
    if fences % 2:
        errors.append("unbalanced_code_fence")
    table_rows = [line for line in text.splitlines() if line.strip().startswith("|")]
    if table_rows and any("|" not in line[1:] for line in table_rows):
        errors.append("malformed_table")
    return errors


@dataclass
class PageResult:
    name: str
    cer: float
    wer: float
    numeric_recall: float
    expected_table: bool
    output_table: bool
    markdown_errors: list[str]
    passed: bool


def evaluate_page(gold_txt: Path, output_md: Path, page_json: Path | None) -> PageResult:
    gold = gold_txt.read_text(encoding="utf-8")
    output = output_md.read_text(encoding="utf-8")
    gc, oc = list(norm(gold)), list(norm(output))
    gw, ow = tokens(gold), tokens(output)
    cer = distance(gc, oc) / max(1, len(gc))
    wer = distance(gw, ow) / max(1, len(gw))
    nums = numeric_tokens(gold)
    numeric_recall = sum(n in numeric_tokens(output) for n in nums) / max(1, len(nums))
    expected_table = False
    if page_json and page_json.exists():
        data = json.loads(page_json.read_text(encoding="utf-8"))
        expected_table = bool(data.get("quality", {}).get("table_detected", False))
    output_table = any(line.strip().startswith("|") for line in output.splitlines())
    errors = markdown_errors(output)
    if expected_table and not output_table:
        errors.append("expected_table_missing")
    return PageResult(
        output_md.stem, cer, wer, numeric_recall, expected_table, output_table,
        errors, not errors,
    )


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--fixtures", type=Path, required=True)
    ap.add_argument("--json", type=Path)
    ap.add_argument("--max-cer", type=float, default=1.0)
    ap.add_argument("--max-wer", type=float, default=1.0)
    ap.add_argument("--min-numeric-recall", type=float, default=1.0)
    args = ap.parse_args()
    root = args.fixtures
    gold_dir, output_dir = root / "gold", root / "output"
    if not gold_dir.is_dir() or not output_dir.is_dir():
        print(f"missing fixture directories under {root}", file=sys.stderr)
        return 2
    results: list[PageResult] = []
    for gold_txt in sorted(gold_dir.glob("*.txt")):
        output_md = output_dir / f"{gold_txt.stem}.md"
        if not output_md.exists():
            print(f"missing output for {gold_txt.stem}", file=sys.stderr)
            return 2
        page_json = root / "metadata" / f"{gold_txt.stem}.json"
        results.append(evaluate_page(gold_txt, output_md, page_json))
    if not results:
        print("no gold fixtures found", file=sys.stderr)
        return 2
    fixture_bytes = b"".join(
        path.read_bytes()
        for path in sorted(root.rglob("*"))
        if path.is_file() and path.name != (args.json.name if args.json else "")
    )
    metric_pass = (
        sum(r.cer for r in results) / len(results) <= args.max_cer
        and sum(r.wer for r in results) / len(results) <= args.max_wer
        and sum(r.numeric_recall for r in results) / len(results) >= args.min_numeric_recall
    )
    report = {
        "schema": "pdf2md-quality-v1",
        "fixture_sha256": hashlib.sha256(fixture_bytes).hexdigest(),
        "pages_total": len(results),
        "pages_passed": sum(r.passed for r in results),
        "pages_failed": sum(not r.passed for r in results),
        "cer": sum(r.cer for r in results) / len(results),
        "wer": sum(r.wer for r in results) / len(results),
        "numeric_recall": sum(r.numeric_recall for r in results) / len(results),
        "thresholds": {"max_cer": args.max_cer, "max_wer": args.max_wer,
                       "min_numeric_recall": args.min_numeric_recall},
        "metrics_passed": metric_pass,
        "results": [asdict(r) for r in results],
    }
    text = json.dumps(report, indent=2, ensure_ascii=False) + "\n"
    if args.json:
        args.json.parent.mkdir(parents=True, exist_ok=True)
        args.json.write_text(text, encoding="utf-8")
    print(text, end="")
    return 0 if report["pages_failed"] == 0 and metric_pass else 1


if __name__ == "__main__":
    raise SystemExit(main())
