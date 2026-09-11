use crate::core::engine::ScanStats;
use crate::core::size::format_bytes;
use crate::core::traits::{DiscoveredProject, GitStatus};
use console::{Alignment, pad_str, style};

pub fn print_scan_header(path_str: &str, stats: &ScanStats) {
    let banner = [
        r"███████╗██╗    ██╗███████╗███████╗██████╗       ██████╗ ███████╗",
        r"██╔════╝██║    ██║██╔════╝██╔════╝██╔══██╗      ██╔══██╗██╔════╝",
        r"███████╗██║ █╗ ██║█████╗  █████╗  ██████╔╝█████╗██████╔╝███████╗",
        r"╚════██║██║███╗██║██╔══╝  ██╔══╝  ██╔═══╝ ╚════╝██╔══██╗╚════██║",
        r"███████║╚███╔███╔╝███████╗███████╗██║           ██║  ██║███████║",
        r"╚══════╝ ╚══╝╚══╝ ╚══════╝╚══════╝╚═╝           ╚═╝  ╚═╝╚══════╝",
    ];

    println!();
    for line in banner {
        println!("{}", style(line).cyan().bold());
    }
    println!(
        "  {} {}",
        style("⚡").yellow().bold(),
        style("High-Performance Git-Aware Workspace Cleaner")
            .white()
            .bold()
    );
    println!(
        "  {} Target: {} │ Inspected {} dirs in {:.2}s\n",
        style("📂").cyan(),
        style(path_str).yellow(),
        style(stats.dirs_inspected).cyan().bold(),
        stats.duration.as_secs_f64()
    );
}

pub fn print_projects_table(projects: &[DiscoveredProject]) {
    if projects.is_empty() {
        println!("{}", style("No cleanable build artifacts found.").yellow());
        return;
    }

    println!(
        "{} {} {} {} {}",
        pad_str(
            &style("PROJECT PATH").bold().underlined().to_string(),
            38,
            Alignment::Left,
            None
        ),
        pad_str(
            &style("TYPE").bold().underlined().to_string(),
            8,
            Alignment::Left,
            None
        ),
        pad_str(
            &style("LAST COMMIT").bold().underlined().to_string(),
            15,
            Alignment::Left,
            None
        ),
        pad_str(
            &style("STATUS").bold().underlined().to_string(),
            18,
            Alignment::Left,
            None
        ),
        pad_str(
            &style("RECLAIMABLE").bold().underlined().to_string(),
            12,
            Alignment::Right,
            None
        )
    );

    let mut total_bytes = 0u64;

    for project in projects {
        let path_str = project.root.display().to_string();
        let display_path = if path_str.len() > 36 {
            format!("...{}", &path_str[path_str.len() - 33..])
        } else {
            path_str
        };

        let styled_path = style(display_path).white();
        let styled_type = style(project.project_type.to_string()).cyan();

        let (styled_commit, styled_status) = match &project.git_info {
            Some(info) => {
                let commit_str = match info.last_commit_days {
                    Some(0) => "Today".to_string(),
                    Some(1) => "1 day ago".to_string(),
                    Some(days) => format!("{} days ago", days),
                    None => "N/A".to_string(),
                };
                let s_commit = style(commit_str).dim();

                let s_status = if info.is_dirty {
                    match info.status {
                        GitStatus::Unknown => style("Dirty".to_string()).red().bold(),
                        other => style(format!("{} (Dirty)", other)).red().bold(),
                    }
                } else {
                    match info.status {
                        GitStatus::Stale => style("Stale".to_string()).green().bold(),
                        GitStatus::Active => style("Active".to_string()).yellow(),
                        GitStatus::Moderate => style("Moderate".to_string()).cyan(),
                        GitStatus::Unknown => style("Unknown".to_string()).dim(),
                    }
                };

                (s_commit, s_status)
            }
            None => (
                style("N/A".to_string()).dim(),
                style("No Git".to_string()).dim(),
            ),
        };

        let project_bytes: u64 = project.artifacts.iter().map(|a| a.size_bytes).sum();
        total_bytes += project_bytes;

        let styled_size = style(format_bytes(project_bytes)).yellow().bold();

        println!(
            "{} {} {} {} {}",
            pad_str(&styled_path.to_string(), 38, Alignment::Left, None),
            pad_str(&styled_type.to_string(), 8, Alignment::Left, None),
            pad_str(&styled_commit.to_string(), 15, Alignment::Left, None),
            pad_str(&styled_status.to_string(), 18, Alignment::Left, None),
            pad_str(&styled_size.to_string(), 12, Alignment::Right, None)
        );
    }

    println!("{}", style("-".repeat(96)).dim());
    println!(
        "{} {} ({})\n",
        style("Total Space Reclaimable:").bold(),
        style(format_bytes(total_bytes)).green().bold(),
        style(format!("{} projects found", projects.len())).dim()
    );
}
