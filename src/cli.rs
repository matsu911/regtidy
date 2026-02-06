use clap::Parser;

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

    /// Verbose output
    #[arg(short, long, default_value_t = false)]
    pub verbose: bool,
}
