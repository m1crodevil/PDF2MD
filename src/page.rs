use std::fs;
use std::io::BufRead;
use std::io::BufReader;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command};
use std::time::Instant;

use serde_json::Value;

use crate::cleanup::{round1, RegexFixes};
use crate::types::{LayoutRegion, OcrBox, PageJson, PageQuality, RegionTextBox, Timings};

// ─── Batch helper: persistent Python process ───

pub(crate) struct BatchHelper {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

// ─── Page rendering ───

pub(crate) fn render_page(
    pdf: &str,
    page_num: usize,
    dpi: u32,
    tmp_dir: &str,
) -> Result<(PathBuf, f64), String> {
    let png = PathBuf::from(format!("{}/page_{:03}.png", tmp_dir, page_num));
    if png.exists() {
        return Ok((png, 0.0));
    }
    let t = Instant::now();
    let out_prefix = format!("{}/page", tmp_dir);
    let output = Command::new("pdftoppm")
        .args([
            "-f",
            &page_num.to_string(),
            "-l",
            &page_num.to_string(),
            "-r",
            &dpi.to_string(),
            "-png",
            pdf,
            &out_prefix,
        ])
        .output()
        .map_err(|e| format!("pdftoppm failed: {}", e))?;
    if !output.status.success() {
        return Err(format!(
            "pdftoppm error: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    // pdftoppm pads output names based on total page count; glob instead of guessing.
    let src = fs::read_dir(tmp_dir)
        .map_err(|e| format!("read tmp dir: {}", e))?
        .flatten()
        .map(|e| e.path())
        .find(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("page-") && n.ends_with(".png"))
        })
        .ok_or_else(|| format!("pdftoppm output not found for page {}", page_num))?;
    if src != png {
        fs::rename(&src, &png).ok();
    }
    Ok((png, t.elapsed().as_secs_f64()))
}

pub(crate) fn is_blank_image(png: &Path) -> bool {
    // ponytail: use PIL via python one-liner instead of pulling image crate
    let out = Command::new("python3")
        .args([
            "-c",
            &format!(
                "from PIL import Image; im=Image.open('{}').convert('L'); p=list(im.getdata()); print('BLANK' if sum(x<245 for x in p)/len(p)<0.0005 else 'NO')",
                png.display()
            ),
        ])
        .output();
    match out {
        Ok(o) => String::from_utf8_lossy(&o.stdout).trim() == "BLANK",
        Err(_) => false,
    }
}

// ─── Batch helper ───

impl BatchHelper {
    pub(crate) fn new(helper: &str) -> Result<Self, String> {
        let py = std::env::var("PYTHON").unwrap_or_else(|_| "python3".to_string());
        let mut child = Command::new(&py)
            .arg(helper)
            .arg("--batch")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| format!("batch helper spawn failed: {}", e))?;
        let stdin = child.stdin.take().ok_or("failed to capture stdin")?;
        let stdout = child.stdout.take().ok_or("failed to capture stdout")?;
        Ok(Self {
            child,
            stdin,
            stdout: std::io::BufReader::new(stdout),
        })
    }

    pub(crate) fn process(&mut self, png: &Path) -> Result<Value, String> {
        let path = png.display().to_string();
        writeln!(self.stdin, "{}", path).map_err(|e| format!("stdin write failed: {}", e))?;
        let mut line = String::new();
        self.stdout
            .read_line(&mut line)
            .map_err(|e| format!("stdout read failed: {}", e))?;
        if line.is_empty() {
            return Err("batch helper closed (no output)".to_string());
        }
        serde_json::from_str::<Value>(&line).map_err(|e| format!("JSON parse error: {}", e))
    }
}

impl Drop for BatchHelper {
    fn drop(&mut self) {
        let _ = self.stdin.flush();
        let _ = self.child.kill();
    }
}

// ─── OCR-to-region mapping ───

pub(crate) fn map_ocr_to_regions(regions: &mut [LayoutRegion], ocr_boxes: &[OcrBox]) {
    for region in regions.iter_mut() {
        let (rx1, ry1, rx2, ry2) = (
            region.bbox[0],
            region.bbox[1],
            region.bbox[2],
            region.bbox[3],
        );
        let mut region_boxes: Vec<RegionTextBox> = Vec::new();
        for ob in ocr_boxes {
            let cx = (ob.bbox[0] + ob.bbox[2]) / 2.0;
            let cy = (ob.bbox[1] + ob.bbox[3]) / 2.0;
            if rx1 <= cx && cx <= rx2 && ry1 <= cy && cy <= ry2 {
                region_boxes.push(RegionTextBox {
                    text: ob.text.clone(),
                    confidence: ob.confidence,
                    x: cx,
                    y: cy,
                });
            }
        }
        // Sort: top-to-bottom (group by ~15px), then left-to-right
        region_boxes.sort_by(|a, b| {
            let row_a = (a.y / 15.0).round() as i64;
            let row_b = (b.y / 15.0).round() as i64;
            row_a
                .cmp(&row_b)
                .then(a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal))
        });
        region.text_combined = Some(
            region_boxes
                .iter()
                .map(|t| t.text.as_str())
                .collect::<Vec<_>>()
                .join(" "),
        );
        region.text_boxes = region_boxes;
    }
}

// ─── Page processing ───

pub(crate) fn process_page(
    cli: &crate::types::OcrArgs,
    page_num: usize,
    tmp_dir: &str,
    regex_fixes: &RegexFixes,
    helper: &mut BatchHelper,
) -> Result<PageJson, String> {
    let t_total = Instant::now();

    // Step 1: Render
    let (png, render_time) = render_page(&cli.pdf, page_num, cli.dpi, tmp_dir)?;

    // Step 2: Blank check
    if is_blank_image(&png) {
        return Ok(PageJson {
            status: "success".to_string(),
            page: page_num,
            blank: true,
            png: Some(png.display().to_string()),
            dpi: cli.dpi,
            layout_regions: Vec::new(),
            ocr_boxes: Vec::new(),
            reading_order: Vec::new(),
            risk_flags: vec!["blank".to_string()],
            quality: PageQuality::default(),
            furniture: Vec::new(),
            filtered_ocr_boxes: None,
            ocr_model: None,
            timings: Timings {
                render: render_time,
                layout: 0.0,
                ocr: 0.0,
                cleanup: 0.0,
                total: t_total.elapsed().as_secs_f64(),
            },
        });
    }

    // Step 3: Python helper (layout + OCR) — batch mode, models already initialized
    let helper_out = helper.process(&png)?;
    if helper_out
        .get("layout_regions")
        .and_then(|v| v.as_array())
        .is_none()
    {
        return Err("OCR helper returned no PP-DocLayout result".to_string());
    }
    if helper_out
        .get("ocr_boxes_raw")
        .and_then(|v| v.as_array())
        .is_none()
    {
        return Err("OCR helper returned no OCR boxes".to_string());
    }
    let layout_time = helper_out["layout_time"].as_f64().unwrap_or(0.0);
    let ocr_time = helper_out["ocr_time"].as_f64().unwrap_or(0.0);
    let ocr_model = helper_out["ocr_model"].as_str().map(str::to_owned);
    if ocr_model.as_deref() != Some("medium") {
        return Err(format!(
            "OCR helper used unexpected model: {}",
            ocr_model.as_deref().unwrap_or("missing")
        ));
    }

    // Step 4: Parse layout regions
    let mut layout_regions: Vec<LayoutRegion> = Vec::new();
    if let Some(regions) = helper_out["layout_regions"].as_array() {
        for r in regions {
            let bbox = parse_bbox(r["bbox"].as_array());
            layout_regions.push(LayoutRegion {
                label: r["label"].as_str().unwrap_or("unknown").to_string(),
                score: r["score"].as_f64().unwrap_or(0.0),
                bbox,
                ..Default::default()
            });
        }
    }

    // Step 5: Parse OCR boxes + apply regex
    let t_cleanup = Instant::now();
    let mut ocr_boxes: Vec<OcrBox> = Vec::new();
    if let Some(boxes) = helper_out["ocr_boxes_raw"].as_array() {
        for b in boxes {
            let raw_text = b["text"].as_str().unwrap_or("").to_string();
            let final_text = regex_fixes.apply(&raw_text);
            let text_raw = if final_text != raw_text {
                Some(raw_text)
            } else {
                None
            };
            let bbox = parse_bbox(b["bbox"].as_array());
            ocr_boxes.push(OcrBox {
                text: final_text,
                text_raw,
                confidence: b["confidence"].as_f64().unwrap_or(0.0),
                bbox,
            });
        }
    }
    let cleanup_time = t_cleanup.elapsed().as_secs_f64();

    // Step 6: Map OCR boxes to layout regions
    map_ocr_to_regions(&mut layout_regions, &ocr_boxes);

    let mut reading_order: Vec<usize> = (0..layout_regions.len()).collect();
    reading_order.sort_by(|a, b| {
        layout_regions[*a].bbox[1]
            .partial_cmp(&layout_regions[*b].bbox[1])
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(
                layout_regions[*a].bbox[0]
                    .partial_cmp(&layout_regions[*b].bbox[0])
                    .unwrap_or(std::cmp::Ordering::Equal),
            )
    });
    let text_chars = ocr_boxes.iter().map(|b| b.text.chars().count()).sum();
    let mean_confidence = if ocr_boxes.is_empty() {
        0.0
    } else {
        ocr_boxes.iter().map(|b| b.confidence).sum::<f64>() / ocr_boxes.len() as f64
    };
    let low_count = ocr_boxes.iter().filter(|b| b.confidence < 0.5).count();
    let table_detected = layout_regions
        .iter()
        .any(|r| r.label.eq_ignore_ascii_case("table") && r.score >= 0.5);
    let visual_region_detected = layout_regions.iter().any(|r| {
        matches!(
            r.label.to_ascii_lowercase().as_str(),
            "figure" | "chart" | "diagram" | "image"
        )
    });
    let mut risk_flags = Vec::new();
    if table_detected {
        risk_flags.push("table_detected".to_string());
    }
    if visual_region_detected {
        risk_flags.push("visual_object".to_string());
    }
    if low_count > 0 {
        risk_flags.push("low_confidence".to_string());
    }
    if layout_regions.len() > 1
        && reading_order.windows(2).any(|w| {
            layout_regions[w[0]].bbox[0] > layout_regions[w[1]].bbox[0]
                && (layout_regions[w[0]].bbox[1] - layout_regions[w[1]].bbox[1]).abs() < 20.0
        })
    {
        risk_flags.push("ambiguous_reading_order".to_string());
    }
    let review_required = !risk_flags.is_empty();
    let ocr_box_count = ocr_boxes.len();
    let low_confidence_ratio = if ocr_box_count == 0 {
        0.0
    } else {
        low_count as f64 / ocr_box_count as f64
    };

    Ok(PageJson {
        status: "success".to_string(),
        page: page_num,
        blank: false,
        png: Some(png.display().to_string()),
        dpi: cli.dpi,
        layout_regions,
        ocr_boxes,
        reading_order,
        risk_flags,
        quality: PageQuality {
            text_chars,
            ocr_box_count,
            mean_confidence: round1(mean_confidence),
            low_confidence_ratio,
            table_detected,
            visual_region_detected,
            review_required,
        },
        furniture: Vec::new(),
        filtered_ocr_boxes: None,
        ocr_model,
        timings: Timings {
            render: render_time,
            layout: layout_time,
            ocr: ocr_time,
            cleanup: round1(cleanup_time),
            total: round1(t_total.elapsed().as_secs_f64()),
        },
    })
}

/// Extract 4-element bbox from JSON array, defaulting to [0,0,0,0]
fn parse_bbox(arr: Option<&Vec<serde_json::Value>>) -> [f64; 4] {
    match arr {
        Some(a) if a.len() == 4 => [
            a[0].as_f64().unwrap_or(0.0),
            a[1].as_f64().unwrap_or(0.0),
            a[2].as_f64().unwrap_or(0.0),
            a[3].as_f64().unwrap_or(0.0),
        ],
        _ => [0.0; 4],
    }
}
