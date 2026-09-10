mod cli;
mod core;
mod detectors;
mod git;
mod ui;

use std::path::PathBuf;
use std::time::Duration;
use anyhow::{Context, Result};
use clap::Parser;
use console::style;
use indicatif::{ProgressBar, ProgressStyle};

use crate::cli::args::Args;
use crate::core::cleaner::Cleaner;
use crate::core::engine::ScanEngine;
use crate::core::size::format_bytes;
use crate::detectors::DetectorRegistry;
use crate::ui::terminal::{print_projects_table, print_scan_header};
use crate::ui::tui::{prompt_selection, SelectableArtifact};

fn main() -> Result<()> {
    let args = Args::parse();

    let registry = DetectorRegistry::for_type(&args.project_type).map_err(|e| {
        eprintln!("{} {}", style("Error:").red().bold(), e);
        std::process::exit(1);
    })?;

    let target_path: PathBuf = if args.path.is_relative() {
        std::env::current_dir()
            .context("Failed to get current working directory")?
            .join(&args.path)
    } else {
        args.path.clone()
    };

    if !target_path.exists() {
        eprintln!(
            "{} Target path '{}' does not exist.",
            style("Error:").red().bold(),
            target_path.display()
        );
        std::process::exit(1);
    }

    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏")
            .template("{spinner:.green} {msg}")
            .unwrap_or_else(|_| ProgressStyle::default_spinner()),
    );
    spinner.set_message(format!("Scanning {}...", target_path.display()));
    spinner.enable_steady_tick(Duration::from_millis(80));

    let engine = ScanEngine::new(registry).with_stale_filter(args.stale);
    let sp = spinner.clone();

    let (projects, stats) = engine.scan(
        &target_path,
        Some(move |count| {
            sp.set_message(format!("Scanning ({} dirs inspected)...", count));
        }),
    );

    spinner.finish_and_clear();

    if args.tui {
        return ui::tui::run_tui_app(projects, stats, args.dry_run);
    }

    print_scan_header(&target_path.display().to_string(), &stats);
    print_projects_table(&projects);

    if projects.is_empty() {
        return Ok(());
    }

    // Dry run check
    if args.dry_run {
        println!(
            "{} Dry-run mode enabled. No files were deleted.",
            style("[DRY-RUN]").cyan().bold()
        );
        return Ok(());
    }

    // Determine artifacts to clean
    let to_clean: Vec<SelectableArtifact> = if args.force {
        println!(
            "{}",
            style("[FORCE] Skipping confirmation prompt. Proceeding with deletion...").yellow()
        );
        projects
            .iter()
            .flat_map(|p| {
                p.artifacts.iter().map(move |a| SelectableArtifact {
                    project_root: p.root.clone(),
                    project_type: p.project_type.clone(),
                    artifact_path: a.abs_path.clone(),
                    artifact_name: a.target.name,
                    size_bytes: a.size_bytes,
                    git_info: p.git_info.clone(),
                })
            })
            .collect()
    } else {
        prompt_selection(&projects)?
    };

    if to_clean.is_empty() {
        println!("{}", style("No artifacts selected. Exiting safely.").dim());
        return Ok(());
    }

    // Execute Safe & Fast Deletion
    let cleaner = Cleaner::new();
    let mut renamed_items = Vec::new();

    for item in to_clean {
        match cleaner.rename_to_trash(&item.artifact_path, item.size_bytes) {
            Ok(clean_item) => renamed_items.push(clean_item),
            Err(e) => eprintln!("{} {}", style("Error:").red().bold(), e),
        }
    }

    if renamed_items.is_empty() {
        return Ok(());
    }

    // Background purge with spinner
    let purge_spinner = ProgressBar::new_spinner();
    purge_spinner.set_style(
        ProgressStyle::default_spinner()
            .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏")
            .template("{spinner:.green} {msg}")
            .unwrap_or_else(|_| ProgressStyle::default_spinner()),
    );
    purge_spinner.set_message("Purging trash files in background...");
    purge_spinner.enable_steady_tick(Duration::from_millis(80));

    let handle = cleaner.purge_items_in_background(renamed_items);
    let report = handle.join().expect("Cleaner worker thread panicked");

    purge_spinner.finish_and_clear();

    println!(
        "\n{} Successfully reclaimed {} across {} artifact(s)!\n",
        style("✔").green().bold(),
        style(format_bytes(report.total_bytes_reclaimed))
            .green()
            .bold(),
        report.items_cleaned
    );

    if !report.errors.is_empty() {
        eprintln!(
            "{} Encounted {} errors during deletion:",
            style("Warning:").yellow().bold(),
            report.errors.len()
        );
        for err in report.errors {
            eprintln!("  - {}", err);
        }
    }

    Ok(())
}
