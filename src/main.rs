mod cleanup;
mod config;
mod manifest;
mod page;
mod reconstruct;
mod report;
mod types;

use std::fs;
use std::time::Instant;

use clap::Parser;

use cleanup::RegexFixes;
use config::load as load_config;

use page::process_page;
use reconstruct::run as run_reconstruct;
use report::*;
use types::{BatchHelper, Cli, Commands, OcrArgs};

fn run_ocr(cli: &OcrArgs) -> Result<(), String> {
    let total = cli.end.unwrap_or(cli.total);
    if cli.start == 0 || cli.start > total {
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

        match process_page(cli, page_num, &tmp_dir, &regex_fixes, &mut helper) {
            Ok(page_json) => match write_page_json(&json_path, &page_json) {
                Ok(_) => {
                    let elapsed = start_time.elapsed().as_secs_f64();
                    let done = page_num - cli.start - skipped + 1;
                    print_done(
                        page_num,
                        total,
                        page_json.layout_regions.len(),
                        page_json.ocr_boxes.len(),
                        page_json.timings.total,
                        elapsed,
                        done,
                        &cli.outdir,
                    );
                }
                Err(e) => {
                    errors += 1;
                    print_error(page_num, &e);
                }
            },
            Err(e) => {
                errors += 1;
                print_error(page_num, &e);
                write_error_stub(&json_path, page_num, &e)?;
            }
        }
    }

    print_summary(
        start_time.elapsed().as_secs_f64(),
        total,
        cli.start,
        skipped,
        errors,
        &cli.outdir,
    );
    Ok(())
}

fn main() {
    let cli = Cli::parse();
    let cfg = load_config("config/pdf2md.toml").ok();
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
