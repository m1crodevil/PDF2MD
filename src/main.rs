mod cleanup;
mod config;
mod furniture;
mod manifest;
mod page;
mod pdfoxide_backend;
mod reconstruct;
mod report;
mod types;

use std::fs;
use std::time::Instant;

use clap::Parser;

use cleanup::RegexFixes;
use config::load as load_config;

use furniture::annotate_directory;
use page::{process_page, BatchHelper};
use pdfoxide_backend::{extract_page, probe_page};
use reconstruct::run as run_reconstruct;
use report::*;
use types::{Cli, Commands, OcrArgs};

fn run_ocr(cli: &OcrArgs) -> Result<(), String> {
    preflight_ocr_dependencies(&cli.helper)?;
    let pdf_total = pdf_page_count(&cli.pdf)?;
    let configured_total = cli.total.unwrap_or(pdf_total);
    if configured_total == 0 || configured_total > pdf_total {
        return Err(format!(
            "invalid total {} for PDF with {} pages",
            configured_total, pdf_total
        ));
    }
    let total = cli.end.unwrap_or(configured_total);
    if cli.start == 0 || cli.start > total || total > configured_total {
        return Err(format!("invalid page range: {}..{}", cli.start, total));
    }
    if total < cli.start {
        return Err(format!("invalid page range: {}..{}", cli.start, total));
    }

    print_init(&cli.pdf, cli.start, total, &cli.outdir);

    fs::create_dir_all(&cli.outdir)
        .map_err(|e| format!("create output directory {}: {}", cli.outdir, e))?;
    let tmp_root = std::env::temp_dir().join(format!("pdf2md-{}", std::process::id()));
    fs::create_dir_all(&tmp_root)
        .map_err(|e| format!("create temp directory {}: {}", tmp_root.display(), e))?;
    let tmp_dir = tmp_root.to_string_lossy().into_owned();

    let regex_fixes = RegexFixes::new();
    let mut helper = BatchHelper::new(&cli.helper).unwrap_or_else(|e| {
        eprintln!("FATAL: {}", e);
        std::process::exit(1);
    });

    let mut skipped = 0;
    let mut errors = 0;
    let start_time = Instant::now();

    for page_num in cli.start..=total {
        let json_path = format!("{}/page_{:03}.json", cli.outdir, page_num);
        if json_exists(&json_path) {
            skipped += 1;
            print_skip(page_num, total);
            continue;
        }

        if let Ok(probe) = probe_page(std::path::Path::new(&cli.pdf), page_num - 1) {
            eprintln!(
                "page {} native text: {} chars; native_route: {}",
                page_num,
                probe.native_text_chars,
                probe.has_native_text(20)
            );
            if probe.has_native_text(20) {
                match extract_page(std::path::Path::new(&cli.pdf), page_num - 1)
                    .and_then(|page| write_page_json(&json_path, &page))
                {
                    Ok(_) => continue,
                    Err(error) => eprintln!("PDFOxide fallback to OCR: {}", error),
                }
            }
        }

        match process_page(cli, page_num, &tmp_dir, &regex_fixes, &mut helper) {
            Ok(page_json) => match write_page_json(&json_path, &page_json) {
                Ok(_) => {
                    let elapsed = start_time.elapsed().as_secs_f64();
                    let done = page_num - cli.start - skipped + 1;
                    print_done(&page_json, total, elapsed, done, &cli.outdir);
                }
                Err(e) => {
                    errors += 1;
                    print_error(page_num, &e);
                }
            },
            Err(e) => {
                errors += 1;
                print_error(page_num, &e);
            }
        }
    }

    if errors == 0 {
        annotate_directory(&cli.outdir).map_err(|e| format!("furniture annotation: {}", e))?;
    }
    print_summary(
        start_time.elapsed().as_secs_f64(),
        total,
        cli.start,
        skipped,
        errors,
        &cli.outdir,
    );
    if let Err(e) = fs::remove_dir_all(&tmp_root) {
        eprintln!(
            "WARNING: cleanup temp directory {}: {}",
            tmp_root.display(),
            e
        );
    }
    if errors > 0 {
        return Err(format!("OCR completed with {} page errors", errors));
    }
    Ok(())
}

fn preflight_ocr_dependencies(helper: &str) -> Result<(), String> {
    for (command, version_arg) in [
        ("pdfinfo", "-v"),
        ("pdftoppm", "-v"),
        ("python3", "--version"),
    ] {
        require_command(command, version_arg)?;
    }
    if !std::path::Path::new(helper).is_file() {
        return Err(format!("missing OCR helper script: {}", helper));
    }
    Ok(())
}

fn require_command(command: &str, version_arg: &str) -> Result<(), String> {
    let output = std::process::Command::new(command)
        .arg(version_arg)
        .output()
        .map_err(|e| format!("missing required dependency '{}': {}", command, e))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "required dependency '{}' is not executable: {}",
            command,
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

fn pdf_page_count(pdf: &str) -> Result<usize, String> {
    let output = std::process::Command::new("pdfinfo")
        .arg(pdf)
        .output()
        .map_err(|e| format!("pdfinfo failed: {}", e))?;
    if !output.status.success() {
        return Err(format!(
            "pdfinfo error: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .find_map(|line| line.strip_prefix("Pages:")?.trim().parse().ok())
        .ok_or_else(|| "pdfinfo output has no Pages field".to_string())
}

fn main() {
    let cli = Cli::parse();
    let cfg = if std::path::Path::new("config/pdf2md.local.toml").exists() {
        Some(load_config("config/pdf2md.local.toml").unwrap_or_else(|e| {
            eprintln!("FATAL: {}", e);
            std::process::exit(1);
        }))
    } else {
        load_config("config/pdf2md.toml").ok()
    };
    match cli.command {
        Commands::Ocr(args) => {
            if let Err(e) = run_ocr(
                &cfg.as_ref()
                    .map(|c| c.ocr_or_default(args.clone()))
                    .unwrap_or(args),
            ) {
                eprintln!("FATAL: {}", e);
                std::process::exit(1);
            }
        }
        Commands::Reconstruct(args) => {
            let args = cfg
                .as_ref()
                .map(|c| c.reconstruct_or_default(args.clone()))
                .unwrap_or(args);
            if let Err(e) = run_reconstruct(&args) {
                eprintln!("FATAL: {}", e);
                std::process::exit(1);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::cleanup::round1;

    #[test]
    fn test_round1() {
        assert_eq!(round1(4.567), 4.6);
        assert_eq!(round1(2.0), 2.0);
    }
}
