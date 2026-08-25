use std::fs;
use std::path::Path;

// ─── Progress & summary ───

pub(crate) fn print_init(pdf: &str, start: usize, total: usize, outdir: &str) {
    eprintln!("=== OCR Pipeline (Rust) ===");
    eprintln!("PDF: {}", pdf);
    eprintln!("Pages: {}..{}", start, total);
    eprintln!("Output: {}", outdir);
}

pub(crate) fn print_skip(page_num: usize, total: usize) {
    if page_num.is_multiple_of(10) {
        eprintln!("[skip] page {:03}/{} (already done)", page_num, total);
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn print_done(
    page_num: usize,
    total: usize,
    layout_count: usize,
    ocr_count: usize,
    page_time: f64,
    elapsed: f64,
    done: usize,
    outdir: &str,
) {
    let rate = done as f64 / elapsed.max(0.1);
    let eta = (total - page_num) as f64 / rate.max(0.01);
    eprintln!(
        "[done] page {:03}/{} regions={} boxes={} {:.1}s | ETA {:.0}min",
        page_num,
        total,
        layout_count,
        ocr_count,
        page_time,
        eta / 60.0
    );
    if page_num.is_multiple_of(10) {
        let done_count = fs::read_dir(outdir)
            .map(|d| {
                d.filter(|e| {
                    e.as_ref()
                        .is_ok_and(|e| e.file_name().to_string_lossy().ends_with(".json"))
                })
                .count()
            })
            .unwrap_or(0);
        eprintln!(
            "[progress] {}/{} ({}%)",
            done_count,
            total,
            done_count * 100 / total
        );
    }
}

pub(crate) fn print_error(page_num: usize, msg: &str) {
    eprintln!("[ERROR] page {:03}: {}", page_num, msg);
}

pub(crate) fn print_summary(
    elapsed: f64,
    total: usize,
    start: usize,
    skipped: usize,
    errors: usize,
    outdir: &str,
) {
    let files = fs::read_dir(outdir).map(|d| d.count()).unwrap_or(0);
    eprintln!("\n=== DONE ===");
    eprintln!(
        "Total: {} pages, {} skipped, {} errors",
        total - start + 1,
        skipped,
        errors
    );
    eprintln!("Time: {:.1} min", elapsed / 60.0);
    eprintln!("Output: {} ({} files)", outdir, files);
}

pub(crate) fn write_error_stub(json_path: &str, page_num: usize, error: &str) {
    let stub = serde_json::json!({
        "page": page_num,
        "error": error,
        "layout_regions": [],
        "ocr_boxes": []
    });
    fs::write(json_path, stub.to_string()).ok();
}

pub(crate) fn json_exists(json_path: &str) -> bool {
    Path::new(json_path).exists()
}

pub(crate) fn write_page_json(
    json_path: &str,
    page_json: &crate::types::PageJson,
) -> Result<(), String> {
    let json_str = serde_json::to_string_pretty(page_json)
        .map_err(|e| format!("JSON serialize failed: {}", e))?;
    fs::write(json_path, &json_str).map_err(|e| format!("write failed: {}", e))
}
