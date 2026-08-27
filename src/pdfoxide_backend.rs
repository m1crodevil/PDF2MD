use std::path::Path;

use pdf_oxide::PdfDocument;

use crate::ir::PageProbe;

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
