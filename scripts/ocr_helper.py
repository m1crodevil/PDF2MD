#!/usr/bin/env python3
"""Python OCR helper for rust-ocr-pipeline.
Called by Rust orchestrator. Outputs JSON to stdout.

Usage:
  python3 ocr_helper.py <png_path>              # single page (legacy)
  python3 ocr_helper.py --batch                 # persistent: read paths from stdin, JSON per stdout line

  --layout: run PP-DocLayout only
  --ocr: run faster-paddle medium only
  (default: both)
"""
import sys, json, os, time, warnings
warnings.filterwarnings("ignore")
os.environ["GLOG_minloglevel"] = "3"
# ponytail: was "0" — MKLDNN is Intel CPU accel for PaddlePaddle, disabling it was the #1 bottleneck
os.environ["FLAGS_use_mkldnn"] = "1"

def run_layout(ld, png_path):
    t0 = time.time()
    results = list(ld.predict(png_path))
    elapsed = time.time() - t0
    regions = []
    for res in results:
        boxes = res.get("boxes", []) if isinstance(res, dict) else []
        for box in boxes:
            coord = box.get("coordinate", [])
            if len(coord) == 4:
                regions.append({
                    "label": box.get("label", "unknown"),
                    "score": float(box.get("score", 0)),
                    "bbox": [float(v) for v in coord],
                })
    return {"layout_regions": regions, "layout_time": round(elapsed, 1)}

def run_ocr(engine, png_path):
    t0 = time.time()
    with open(png_path, "rb") as f:
        img_bytes = f.read()
    result = engine.ocr(img_bytes)
    elapsed = time.time() - t0
    bounds = result.get("bounds", {})
    boxes = []
    for b in bounds.values():
        tl = b.get("topLeftCoord", (0, 0))
        br = b.get("bottomRightCoord", (0, 0))
        boxes.append({
            "text": b.get("text", ""),
            "confidence": float(b.get("confidence", 0)),
            "bbox": [float(tl[0]), float(tl[1]), float(br[0]), float(br[1])],
        })
    return {"ocr_boxes_raw": boxes, "ocr_time": round(elapsed, 1)}


def needs_medium(layout):
    # ponytail: table-only routing; expand labels only after measured misses justify it.
    return any(
        r.get("label", "").lower() == "table" and float(r.get("score", 0)) >= 0.5
        for r in layout.get("layout_regions", [])
    )


def run_page(ld, engines, png_path):
    layout = run_layout(ld, png_path)
    # Layout misses are not blank pages: retry full-page OCR with the stronger
    # model before allowing the Rust orchestrator to classify the page as blank.
    fallback = not layout.get("layout_regions")
    # ponytail: medium is the quality default; small remains available for explicit
    # future benchmarking, but legal-token loss is costlier than OCR throughput.
    size = "medium"
    if size not in engines:
        import faster_paddle
        engines[size] = faster_paddle.OcrEngine(model_size=size, threads=8)
    output = {"png": png_path, "ocr_model": size, "layout_fallback": fallback}
    output.update(layout)
    output.update(run_ocr(engines[size], png_path))
    return output
def main():
    args = sys.argv[1:]

    # ── Batch mode: init once, loop over stdin ──
    if "--batch" in args:
        from paddleocr import LayoutDetection
        ld = LayoutDetection()
        engines = {}
        for line in sys.stdin:
            png_path = line.strip()
            if not png_path:
                continue
            output = {"png": png_path}
            try:
                output = run_page(ld, engines, png_path)
            except Exception as e:
                output["error"] = str(e)
            print(json.dumps(output, ensure_ascii=False), flush=True)
        return

    # ── Legacy single-page mode ──
    if not args:
        print(json.dumps({"error": "usage: ocr_helper.py <png> [--layout] [--ocr] [--batch]"}))
        sys.exit(1)
    png_path = args[0]
    mode = "both"
    if "--layout" in args and "--ocr" not in args:
        mode = "layout"
    elif "--ocr" in args and "--layout" not in args:
        mode = "ocr"

    output = {"png": png_path}
    try:
        if mode in ("layout", "both"):
            from paddleocr import LayoutDetection
            ld = LayoutDetection()
            output.update(run_layout(ld, png_path))
        if mode == "ocr":
            import faster_paddle
            engines = {"medium": faster_paddle.OcrEngine(model_size="medium")}
            output.update(run_ocr(engines["medium"], png_path))
        elif mode == "both":
            engines = {}
            output.update(run_page(ld, engines, png_path))
    except Exception as e:
        output["error"] = str(e)
    print(json.dumps(output, ensure_ascii=False))

if __name__ == "__main__":
    main()
