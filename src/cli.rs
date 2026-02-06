use clap::{Parser, Subcommand};

/// clearnear — Docker Private Registry Image Cleaner
#[derive(Parser, Debug)]
#[command(name = "clearnear", version, about)]
pub struct Cli {
    /// Registry URL (e.g., http://localhost:5000)
    #[arg(long, env = "CLEARNEAR_REGISTRY")]
    pub registry: String,

    /// Repository name (omit to process all repos from catalog)
    #[arg(long)]
    pub repo: Option<String>,

    /// Verbose output
    #[arg(short, long, default_value_t = false)]
    pub verbose: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// List all repositories and their tags
    List,

    /// Find repositories with no tags (dangling)
    Dangling,

    /// Clean up images by deleting old, excess, or pattern-matched tags
    Clean(CleanArgs),
}

#[derive(Parser, Debug)]
pub struct CleanArgs {
    /// Keep N most recent tags, delete the rest
    #[arg(long, group = "strategy")]
    pub keep: Option<usize>,

    /// Delete images older than N days
    #[arg(long, group = "strategy")]
    pub older_than: Option<u64>,

    /// Delete tags matching this regex pattern
    #[arg(long, group = "strategy")]
    pub pattern: Option<String>,

    /// Preview changes without deleting
    #[arg(long, default_value_t = false)]
    pub dry_run: bool,
}
