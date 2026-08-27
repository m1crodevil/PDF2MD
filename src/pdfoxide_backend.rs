use std::path::Path;
use std::time::Instant;

use pdf_oxide::PdfDocument;

use crate::ir::PageProbe;
use crate::types::{LayoutRegion, OcrBox, PageJson, PageQuality, Timings};

pub(crate) fn probe_page(path: &Path, page: usize) -> Result<PageProbe, String> {
    let path = path
        .to_str()
        .ok_or_else(|| "PDF path is not valid UTF-8".to_string())?;
    let document = PdfDocument::open(path).map_err(|e| format!("PDFOxide open failed: {e}"))?;
    let text = document
        .extract_text(page)
        .map_err(|e| format!("PDFOxide page {page} probe failed: {e}"))?;
    Ok(PageProbe {
        page: page + 1,
        native_text_chars: text.chars().count(),
        page_area: 0.0,
        image_only: text.trim().is_empty(),
    })
}

pub(crate) fn extract_page(path: &Path, page: usize) -> Result<PageJson, String> {
    let started = Instant::now();
    let path = path
        .to_str()
        .ok_or_else(|| "PDF path is not valid UTF-8".to_string())?;
    let document = PdfDocument::open(path).map_err(|e| format!("PDFOxide open failed: {e}"))?;
    let text = document
        .extract_text(page)
        .map_err(|e| format!("PDFOxide page {page} extraction failed: {e}"))?;
    let chars = text.chars().count();
    Ok(PageJson {
        status: "success".to_string(),
        page: page + 1,
        blank: text.trim().is_empty(),
        png: None,
        dpi: 0,
        layout_regions: vec![LayoutRegion {
            label: "native_text".to_string(),
            score: 1.0,
            bbox: [0.0; 4],
            text_boxes: Vec::new(),
            text_combined: Some(text.clone()),
        }],
        ocr_boxes: vec![OcrBox {
            text,
            text_raw: None,
            confidence: 1.0,
            bbox: [0.0; 4],
        }],
        reading_order: vec![0],
        risk_flags: vec!["native_text_no_coordinates".to_string()],
        quality: PageQuality {
            text_chars: chars,
            ocr_box_count: 1,
            mean_confidence: 1.0,
            review_required: true,
            ..PageQuality::default()
        },
        furniture: Vec::new(),
        filtered_ocr_boxes: None,
        ocr_model: Some("pdf_oxide".to_string()),
        timings: Timings {
            render: 0.0,
            layout: 0.0,
            ocr: 0.0,
            cleanup: 0.0,
            total: started.elapsed().as_secs_f64(),
        },
    })
}
