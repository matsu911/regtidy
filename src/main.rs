mod cli;
mod error;
mod models;
mod output;
mod registry;
mod strategy;

use std::collections::HashSet;
use std::process;

use anyhow::Result;
use clap::Parser;

use cli::Cli;
use output::{print_plan, print_summary};
use registry::RegistryClient;
use strategy::Strategy;

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("Error: {:#}", e);
        process::exit(1);
    }
}

async fn run() -> Result<()> {
    let cli = Cli::parse();

    let strategy = Strategy::from_cli(&cli)?;

    if cli.verbose {
        eprintln!("[DEBUG] Strategy: {:?}", strategy);
        eprintln!("[DEBUG] Registry: {}", cli.registry);
        eprintln!("[DEBUG] Dry run: {}", cli.dry_run);
    }

    let client = RegistryClient::new(&cli.registry, cli.verbose);

    // Determine which repositories to process
    let repos = match &cli.repo {
        Some(repo) => vec![repo.clone()],
        None => {
            if cli.verbose {
                eprintln!("[DEBUG] No --repo specified, fetching catalog...");
            }
            client.list_repositories().await?
        }
    };

    if repos.is_empty() {
        println!("No repositories found.");
        return Ok(());
    }

    let mut total_deleted: usize = 0;
    let mut total_kept: usize = 0;
    let mut total_errors: usize = 0;
    let mut all_deleted_digests: HashSet<String> = HashSet::new();

    for repo in &repos {
        if cli.verbose {
            eprintln!("[DEBUG] Processing repository: {}", repo);
        }

        // Resolve all tags
        let tags = match client.resolve_all_tags(repo).await {
            Ok(tags) => tags,
            Err(e) => {
                eprintln!("[ERROR] Failed to resolve tags for {}: {}", repo, e);
                total_errors += 1;
                continue;
            }
        };

        if tags.is_empty() {
            if cli.verbose {
                eprintln!("[DEBUG] No tags found for {}", repo);
            }
            continue;
        }

        // Apply strategy
        let plan = strategy.apply(repo, tags);

        // Print the plan
        print_plan(&plan, cli.dry_run);

        total_kept += plan.to_keep.len();

        // Execute deletions (unless dry-run)
        if cli.dry_run {
            total_deleted += plan.to_delete.len();
            for tag in &plan.to_delete {
                all_deleted_digests.insert(tag.digest.clone());
            }
        } else {
            // Collect unique digests to delete
            let mut digests_to_delete: Vec<String> = Vec::new();
            let mut seen: HashSet<String> = HashSet::new();

            for tag in &plan.to_delete {
                if seen.insert(tag.digest.clone()) {
                    digests_to_delete.push(tag.digest.clone());
                }
            }

            for digest in &digests_to_delete {
                match client.delete_manifest(repo, digest).await {
                    Ok(()) => {
                        if cli.verbose {
                            eprintln!("[DEBUG] Deleted digest {}", digest);
                        }
                        all_deleted_digests.insert(digest.clone());
                    }
                    Err(e) => {
                        eprintln!("[ERROR] Failed to delete digest {}: {}", digest, e);
                        total_errors += 1;
                    }
                }
            }

            total_deleted += plan.to_delete.len();
        }
    }

    print_summary(
        total_deleted,
        all_deleted_digests.len(),
        total_kept,
        total_errors,
        cli.dry_run,
    );

    if total_errors > 0 {
        process::exit(1);
    }

    Ok(())
}
