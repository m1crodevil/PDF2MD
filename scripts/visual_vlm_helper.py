#!/usr/bin/env python3
"""Selective visual companion artifact bridge; stdlib-only and [OI]-compatible."""
import argparse
import base64
import hashlib
import json
import os
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request
from pathlib import Path

SCHEMA_VERSION = "visual-v1"


def atomic_json(path: Path, value: dict) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    fd, name = tempfile.mkstemp(prefix=f".{path.name}.", dir=path.parent)
    try:
        with os.fdopen(fd, "w", encoding="utf-8") as handle:
            json.dump(value, handle, ensure_ascii=False, indent=2)
            handle.write("\n")
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(name, path)
    finally:
        try:
            os.unlink(name)
        except FileNotFoundError:
            pass


def extract_object(text: str) -> dict:
    text = text.strip()
    if text.startswith("```"):
        text = text.strip("`").strip()
        if text.lower().startswith("json"):
            text = text[4:].strip()
    try:
        value = json.loads(text)
    except json.JSONDecodeError:
        start, end = text.find("{"), text.rfind("}")
        if start < 0 or end <= start:
            raise RuntimeError("visual response is not JSON")
        value = json.loads(text[start:end + 1])
    if not isinstance(value, dict):
        raise RuntimeError("visual response root must be an object")
    return value


def validate(value: dict) -> dict:
    description = value.get("description", "")
    labels = value.get("labels", [])
    relationships = value.get("relationships", [])
    uncertainties = value.get("uncertainties", [])
    if not isinstance(description, str) or not description.strip():
        raise RuntimeError("visual description is empty")
    if not isinstance(labels, list) or not isinstance(relationships, list) or not isinstance(uncertainties, list):
        raise RuntimeError("visual schema fields must be arrays")
    for label in labels:
        if not isinstance(label, dict) or not isinstance(label.get("text"), str):
            raise RuntimeError("visual label requires text")
        if label.get("certainty", "visible") not in {"visible", "uncertain"}:
            raise RuntimeError("invalid label certainty")
    for rel in relationships:
        if not isinstance(rel, dict) or not isinstance(rel.get("from"), str) or not isinstance(rel.get("to"), str):
            raise RuntimeError("visual relationship requires from/to")
        if rel.get("certainty", "visible") not in {"visible", "inferred", "uncertain"}:
            raise RuntimeError("invalid relationship certainty")
    return {"description": description.strip(), "labels": labels, "relationships": relationships, "uncertainties": [str(x) for x in uncertainties]}


def to_markdown(value: dict) -> str:
    lines = ["### Visual interpretation", "", value["description"], "", "### Visible labels", ""]
    lines += [f"- `{x['text']}` ({x.get('certainty', 'visible')})" for x in value["labels"]] or ["- None reported."]
    lines += ["", "### Visible relationships", ""]
    lines += [f"- `{x['from']}` -> `{x['to']}`: {x.get('relation', 'connected')} ({x.get('certainty', 'visible')})" for x in value["relationships"]] or ["- None reported."]
    lines += ["", "### Uncertainties", ""]
    lines += [f"- {x}" for x in value["uncertainties"]] or ["- None reported."]
    lines += ["", "> Visual interpretation generated from the rendered page; it does not replace raw page JSON."]
    return "\n".join(lines)


def main() -> int:
    parser = argparse.ArgumentParser()
    for name in ("pdf", "json", "output", "base-url", "model"):
        parser.add_argument(f"--{name}", required=True)
    parser.add_argument("--page", type=int, required=True)
    args = parser.parse_args()
    key = os.environ.get("PDF2MD_API_KEY", "").strip()
    if not key:
        raise RuntimeError("PDF2MD_API_KEY is missing")
    page_json = json.loads(Path(args.json).read_text(encoding="utf-8"))
    with tempfile.TemporaryDirectory(prefix="pdf2md-vlm-") as temp:
        prefix = str(Path(temp) / "page")
        subprocess.run(["pdftoppm", "-f", str(args.page), "-l", str(args.page), "-png", "-r", "150", args.pdf, prefix], check=True, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
        image = Path(f"{prefix}-{args.page}.png")
        if not image.is_file() or image.stat().st_size == 0:
            raise RuntimeError("pdftoppm produced no page image")
        image_bytes = image.read_bytes()
    image_sha256 = hashlib.sha256(image_bytes).hexdigest()
    prompt = f"""Analyze only visible visual content in this PDF page. Return JSON only, matching this schema exactly:
{{"description":"string","labels":[{{"text":"string","certainty":"visible|uncertain"}}],"relationships":[{{"from":"string","to":"string","relation":"string","certainty":"visible|inferred|uncertain"}}],"uncertainties":["string"]}}
Preserve visible labels/numbers exactly. Never invent labels, values, arrows, directions, or relationships. Structural JSON is context only:
{json.dumps(page_json, ensure_ascii=False)}"""
    payload = {"model": args.model, "messages": [{"role": "user", "content": [{"type": "text", "text": prompt}, {"type": "image_url", "image_url": {"url": "data:image/png;base64," + base64.b64encode(image_bytes).decode("ascii")}}]}], "max_tokens": 1800, "temperature": 0}
    request = urllib.request.Request(args.base_url.rstrip("/") + "/chat/completions", data=json.dumps(payload).encode(), headers={"Authorization": "Bearer " + key, "Content-Type": "application/json"})
    body = None
    for attempt in range(3):
        try:
            with urllib.request.urlopen(request, timeout=300) as response:
                body = response.read()
            break
        except urllib.error.HTTPError as error:
            detail = error.read(512).decode("utf-8", errors="replace")
            if error.code not in {408, 429} and error.code < 500:
                raise RuntimeError(f"HTTP {error.code}: {detail}")
            if attempt == 2:
                raise RuntimeError(f"HTTP {error.code}: {detail}")
            time.sleep(2 ** attempt)
        except (TimeoutError, urllib.error.URLError) as error:
            if attempt == 2:
                raise RuntimeError(f"network error: {error}")
            time.sleep(2 ** attempt)
    if body is None:
        raise RuntimeError("provider returned no response")
    result = json.loads(body)
    content = result.get("choices", [{}])[0].get("message", {}).get("content", "")
    normalized = validate(extract_object(content))
    artifact = {"schema_version": SCHEMA_VERSION, "page": args.page, "status": "success", "method": "vision_llm", "model": args.model, "source_image_sha256": image_sha256, **normalized, "markdown": to_markdown(normalized)}
    atomic_json(Path(args.output), artifact)
    print(artifact["markdown"])
    return 0


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        print(str(error), file=sys.stderr)
        raise SystemExit(1)
