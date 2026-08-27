use std::path::Path;

use pdfium_render::prelude::*;

/// Render one page for visual QA. Pdfium remains optional and is never used
/// by the default extraction route.
pub fn render_page(path: &Path, page: usize, output: &Path) -> Result<(), String> {
    let bindings = match std::env::var_os("PDFIUM_LIBRARY_PATH") {
        Some(path) => Pdfium::bind_to_library(path),
        None => Pdfium::bind_to_system_library(),
    }
    .map_err(|error| format!("load Pdfium library: {error}"))?;
    let pdfium = Pdfium::new(bindings);
    let document = pdfium
        .load_pdf_from_file(path, None)
        .map_err(|error| format!("load PDF with Pdfium: {error}"))?;
    let page_index = page;
    let page = document
        .pages()
        .get(page as i32)
        .map_err(|error| format!("load page {page_index}: {error}"))?;
    let result = page
        .render_with_config(
            &PdfRenderConfig::new()
                .set_target_width(1600)
                .set_maximum_height(2200),
        )
        .map_err(|error| format!("render page {page_index}: {error}"))?;
    let image = result
        .as_image()
        .map_err(|error| format!("convert rendered page {page_index}: {error}"))?;
    image
        .save(output)
        .map_err(|error| format!("save rendered page {}: {error}", output.display()))
}

#[cfg(test)]
mod tests {
    use super::render_page;

    #[test]
    fn renders_a_real_page() {
        let pdf = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("docs/pdf2md-pdfium-refactor-plan.pdf");
        let output =
            std::env::temp_dir().join(format!("pdf2md-pdfium-test-{}.png", std::process::id()));
        render_page(&pdf, 0, &output).expect("PDFium should render the plan PDF");
        assert!(output.is_file());
        std::fs::remove_file(output).expect("remove temporary render");
    }
}
