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
use crate::core::engine::ScanEngine;
use crate::detectors::DetectorRegistry;
use crate::ui::terminal::{print_projects_table, print_scan_header};

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
            .tick_chars("â ‹â ™â ¹â ¸â ¼â ´â ¦â §â ‡â ")
            .template("{spinner:.green} {msg}")
            .unwrap_or_else(|_| ProgressStyle::default_spinner()),
    );
    spinner.set_message(format!("Scanning {}...", target_path.display()));
    spinner.enable_steady_tick(Duration::from_millis(80));

    let engine = ScanEngine::new(registry);
    let sp = spinner.clone();

    let (projects, stats) = engine.scan(
        &target_path,
        Some(move |count| {
            sp.set_message(format!("Scanning ({} dirs inspected)...", count));
        }),
    );

    spinner.finish_and_clear();

    print_scan_header(&target_path.display().to_string(), &stats);
    print_projects_table(&projects);

    Ok(())
}
