use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

use crate::types::{FurnitureAnnotation, OcrBox, PageJson};

/// Annotate repeated edge text without mutating the raw OCR directory.
pub(crate) fn annotate_directory(input: &str) -> Result<(), String> {
    let mut pages = Vec::new();
    for entry in fs::read_dir(input).map_err(|e| format!("read OCR directory: {}", e))? {
        let path = entry.map_err(|e| e.to_string())?.path();
        if path.extension().and_then(|x| x.to_str()) != Some("json") {
            continue;
        }
        let page: PageJson =
            serde_json::from_str(&fs::read_to_string(&path).map_err(|e| e.to_string())?)
                .map_err(|e| format!("parse {}: {}", path.display(), e))?;
        pages.push(page);
    }
    let mut frequency = HashMap::<String, usize>::new();
    for page in &pages {
        let mut seen = HashSet::new();
        for b in page
            .ocr_boxes
            .iter()
            .filter(|b| is_edge(b, &page.ocr_boxes) && eligible_candidate(&b.text))
        {
            seen.insert(normalize(&b.text));
        }
        for text in seen {
            *frequency.entry(text).or_default() += 1;
        }
    }
    let output = Path::new(input).join("filtered");
    fs::create_dir_all(&output).map_err(|e| e.to_string())?;
    for mut page in pages {
        let mut furniture = Vec::new();
        let mut retained = Vec::new();
        for (idx, b) in page.ocr_boxes.iter().enumerate() {
            let repeated = frequency.get(&normalize(&b.text)).copied().unwrap_or(0) >= 3;
            if is_edge(b, &page.ocr_boxes) && eligible_candidate(&b.text) && repeated {
                furniture.push(FurnitureAnnotation {
                    text: b.text.clone(),
                    role: "repeated_page_furniture_candidate".into(),
                    confidence: 0.75,
                    reason: "normalized text repeats in page edge zones".into(),
                });
            } else {
                retained.push(idx);
            }
        }
        page.furniture = furniture;
        page.filtered_ocr_boxes = Some(retained);
        validate_filtered_page(&page)?;
        let path = output.join(format!("page_{:03}.json", page.page));
        let json = serde_json::to_string_pretty(&page)
            .map_err(|e| format!("serialize page {}: {}", page.page, e))?;
        fs::write(path, json).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn validate_filtered_page(page: &PageJson) -> Result<(), String> {
    let retained = page
        .filtered_ocr_boxes
        .as_ref()
        .ok_or_else(|| format!("page {} has no filtered box selection", page.page))?;
    let mut seen = HashSet::new();
    for &idx in retained {
        if idx >= page.ocr_boxes.len() || !seen.insert(idx) {
            return Err(format!(
                "page {} has invalid filtered OCR index {}",
                page.page, idx
            ));
        }
    }
    for annotation in &page.furniture {
        if annotation.role.is_empty() || annotation.reason.is_empty() {
            return Err(format!(
                "page {} has incomplete furniture metadata",
                page.page
            ));
        }
        if !page.ocr_boxes.iter().any(|b| b.text == annotation.text) {
            return Err(format!(
                "page {} furniture text is absent from raw OCR: {}",
                page.page, annotation.text
            ));
        }
        if annotation.confidence < 0.0 || annotation.confidence > 1.0 {
            return Err(format!(
                "page {} has invalid furniture confidence",
                page.page
            ));
        }
    }
    Ok(())
}

fn normalize(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}
fn eligible_candidate(text: &str) -> bool {
    let normalized = normalize(text);
    normalized.chars().count() >= 8
        && normalized.split_whitespace().count() >= 2
        && normalized.chars().any(|c| c.is_alphabetic())
}

fn is_edge(b: &OcrBox, boxes: &[OcrBox]) -> bool {
    let min = boxes
        .iter()
        .map(|x| x.bbox[1])
        .fold(f64::INFINITY, f64::min);
    let max = boxes
        .iter()
        .map(|x| x.bbox[3])
        .fold(f64::NEG_INFINITY, f64::max);
    let y = (b.bbox[1] + b.bbox[3]) / 2.0;
    let h = (max - min).max(1.0);
    y <= min + h * 0.08 || y >= max - h * 0.08
}

#[cfg(test)]
mod tests {
    use super::normalize;
    #[test]
    fn normalizes_only_for_comparison() {
        assert_eq!(normalize(" Header   2025 "), "header 2025");
    }
}
