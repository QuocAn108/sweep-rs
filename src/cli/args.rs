use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug, Clone)]
#[command(
    name = "sweep-rs",
    version,
    about = "A fast, git-aware workspace cleanup tool written in Rust."
)]
pub struct Args {
    #[arg(short, long, default_value = ".")]
    pub path: PathBuf,

    #[arg(short, long, default_value_t = false)]
    pub dry_run: bool,

    #[arg(short, long, num_args = 0..=1, default_missing_value = "30")]
    pub stale: Option<u64>,

    #[arg(short = 't', long = "type", default_value = "all")]
    pub project_type: String,

    #[arg(short, long, default_value_t = false)]
    pub force: bool,

    #[arg(long, default_value_t = false)]
    pub tui: bool,

    #[arg(short, long, default_value_t = false)]
    pub verbose: bool,
}
