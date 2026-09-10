use console::style;
use crate::core::engine::ScanStats;
use crate::core::size::format_bytes;
use crate::core::traits::DiscoveredProject;

pub fn print_scan_header(path_str: &str, stats: &ScanStats) {
    println!(
        "\n{} Scanning: {} ... {} in {:.2}s ({} dirs inspected)\n",
        style("[SWEEP-RS]").cyan().bold(),
        style(path_str).yellow(),
        style("Done").green().bold(),
        stats.duration.as_secs_f64(),
        stats.dirs_inspected
    );
}

pub fn print_projects_table(projects: &[DiscoveredProject]) {
    if projects.is_empty() {
        println!("{}", style("No cleanable build artifacts found.").yellow());
        return;
    }

    println!(
        "{:<45} {:<10} {:<15} {:>12}",
        style("PROJECT PATH").bold().underlined(),
        style("TYPE").bold().underlined(),
        style("ARTIFACT").bold().underlined(),
        style("RECLAIMABLE").bold().underlined()
    );

    let mut total_bytes = 0u64;

    for project in projects {
        for artifact in &project.artifacts {
            total_bytes += artifact.size_bytes;

            let path_str = project.root.display().to_string();
            let display_path = if path_str.len() > 43 {
                format!("...{}", &path_str[path_str.len() - 40..])
            } else {
                path_str
            };

            println!(
                "{:<45} {:<10} {:<15} {:>12}",
                style(display_path).white(),
                style(project.project_type.to_string()).cyan(),
                style(artifact.target.name).dim(),
                style(format_bytes(artifact.size_bytes)).yellow().bold()
            );
        }
    }

    println!("{}", style("-".repeat(85)).dim());
    println!(
        "{} {} ({})\n",
        style("Total Space Reclaimable:").bold(),
        style(format_bytes(total_bytes)).green().bold(),
        style(format!("{} projects found", projects.len())).dim()
    );
}
