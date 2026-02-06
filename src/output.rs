use colored::Colorize;

use crate::models::{CleanupPlan, TagInfo};

/// Print the cleanup plan for a repository
pub fn print_plan(plan: &CleanupPlan, dry_run: bool) {
    let header = if dry_run {
        format!(" {} ", "DRY RUN".yellow().bold())
    } else {
        String::new()
    };

    println!(
        "\n{}Repository: {}{}",
        header,
        plan.repository.bold(),
        if dry_run { " (no changes will be made)" } else { "" }
    );
    println!("{}", "─".repeat(60));

    if !plan.to_delete.is_empty() {
        println!("  {} ({}):", "TO DELETE".red().bold(), plan.to_delete.len());
        for tag in &plan.to_delete {
            print_tag_line(tag, "DELETE");
        }
    }

    if !plan.to_keep.is_empty() {
        println!("  {} ({}):", "KEEP".green().bold(), plan.to_keep.len());
        for tag in &plan.to_keep {
            print_tag_line(tag, "KEEP");
        }
    }

    if plan.to_delete.is_empty() {
        println!("  {}", "Nothing to delete.".green());
    }
}

fn print_tag_line(tag: &TagInfo, action: &str) {
    let digest_short = truncate_digest(&tag.digest);
    let created_str = match &tag.created {
        Some(dt) => dt.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
        None => "unknown".to_string(),
    };

    let label = match action {
        "DELETE" => "DELETE".red().bold().to_string(),
        "KEEP" => "  KEEP".green().bold().to_string(),
        _ => action.to_string(),
    };

    println!(
        "    [{}] {:<30} {} {}",
        label,
        tag.tag,
        digest_short.dimmed(),
        created_str.dimmed(),
    );
}

fn truncate_digest(digest: &str) -> &str {
    if digest.len() > 19 {
        &digest[..19]
    } else {
        digest
    }
}

/// Print final summary
pub fn print_summary(
    deleted: usize,
    unique_digests_deleted: usize,
    kept: usize,
    errors: usize,
    dry_run: bool,
) {
    println!("\n{}", "═".repeat(60));
    if dry_run {
        println!(
            "{} Would delete {} tags ({} unique digests), keep {} tags, {} errors",
            "DRY RUN SUMMARY:".yellow().bold(),
            deleted.to_string().red().bold(),
            unique_digests_deleted,
            kept.to_string().green().bold(),
            if errors > 0 {
                errors.to_string().red().bold().to_string()
            } else {
                errors.to_string()
            }
        );
    } else {
        println!(
            "{} Deleted {} tags ({} unique digests), kept {} tags, {} errors",
            "SUMMARY:".bold(),
            deleted.to_string().red().bold(),
            unique_digests_deleted,
            kept.to_string().green().bold(),
            if errors > 0 {
                errors.to_string().red().bold().to_string()
            } else {
                errors.to_string()
            }
        );
        if deleted > 0 {
            println!(
                "\n{} Run registry garbage collection to reclaim disk space:",
                "REMINDER:".yellow().bold()
            );
            println!("  docker exec <registry-container> bin/registry garbage-collect /etc/docker/registry/config.yml");
        }
    }
}
