#![allow(dead_code)]

use std::path::Path;

use pdf_oxide::PdfDocument;

use crate::ir::{Backend, PageIr};

pub(crate) fn extract_page(path: &Path, page: usize) -> Result<PageIr, String> {
    let path = path
        .to_str()
        .ok_or_else(|| "PDF path is not valid UTF-8".to_string())?;
    let document = PdfDocument::open(path).map_err(|e| format!("PDFOxide open failed: {e}"))?;
    let text = document
        .extract_text(page)
        .map_err(|e| format!("PDFOxide page {page} extraction failed: {e}"))?;
    Ok(PageIr {
        schema: "pdf2md-page-ir-v1".to_string(),
        page: page + 1,
        backend: Backend::PdfOxide,
        text,
        width: 0.0,
        height: 0.0,
    })
}
