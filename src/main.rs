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

use cli::{Cli, Command};
use output::{print_plan, print_repo_tags, print_summary};
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

    match cli.command {
        Command::List => run_list(&client, &repos, cli.verbose).await,
        Command::Dangling => run_dangling(&client, &repos, cli.verbose).await,
        Command::Clean(args) => run_clean(&client, &repos, &args, cli.verbose).await,
    }
}

async fn run_dangling(client: &RegistryClient, repos: &[String], verbose: bool) -> Result<()> {
    let mut dangling: Vec<String> = Vec::new();

    for repo in repos {
        if verbose {
            eprintln!("[DEBUG] Checking repository: {}", repo);
        }

        let tags = match client.list_tags(repo).await {
            Ok(tags) => tags,
            Err(e) => {
                eprintln!("[ERROR] Failed to list tags for {}: {}", repo, e);
                continue;
            }
        };

        if tags.is_empty() {
            dangling.push(repo.clone());
        }
    }

    if dangling.is_empty() {
        println!("No dangling repositories found.");
    } else {
        println!(
            "Found {} dangling {} (no tags):",
            dangling.len(),
            if dangling.len() == 1 {
                "repository"
            } else {
                "repositories"
            }
        );
        for repo in &dangling {
            println!("  - {}", repo);
        }
        println!(
            "\nRun registry garbage collection to reclaim storage:"
        );
        println!(
            "  docker exec <registry-container> bin/registry garbage-collect /etc/docker/registry/config.yml"
        );
    }

    Ok(())
}

async fn run_list(client: &RegistryClient, repos: &[String], verbose: bool) -> Result<()> {
    let mut total_tags: usize = 0;

    for repo in repos {
        if verbose {
            eprintln!("[DEBUG] Listing repository: {}", repo);
        }

        let tags = match client.resolve_all_tags(repo).await {
            Ok(tags) => tags,
            Err(e) => {
                eprintln!("[ERROR] Failed to resolve tags for {}: {}", repo, e);
                continue;
            }
        };

        total_tags += tags.len();
        print_repo_tags(repo, &tags);
    }

    println!(
        "\n{} repositories, {} tags total.",
        repos.len(),
        total_tags
    );

    Ok(())
}

async fn run_clean(
    client: &RegistryClient,
    repos: &[String],
    args: &cli::CleanArgs,
    verbose: bool,
) -> Result<()> {
    let strategy = Strategy::from_args(args)?;

    if verbose {
        eprintln!("[DEBUG] Strategy: {:?}", strategy);
        eprintln!("[DEBUG] Dry run: {}", args.dry_run);
    }

    let mut total_deleted: usize = 0;
    let mut total_kept: usize = 0;
    let mut total_errors: usize = 0;
    let mut all_deleted_digests: HashSet<String> = HashSet::new();

    for repo in repos {
        if verbose {
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
            if verbose {
                eprintln!("[DEBUG] No tags found for {}", repo);
            }
            continue;
        }

        // Apply strategy
        let plan = strategy.apply(repo, tags);

        // Print the plan
        print_plan(&plan, args.dry_run);

        total_kept += plan.to_keep.len();

        // Execute deletions (unless dry-run)
        if args.dry_run {
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

            let mut deleted_digests_this_repo: HashSet<String> = HashSet::new();

            for digest in &digests_to_delete {
                match client.delete_manifest(repo, digest).await {
                    Ok(()) => {
                        if verbose {
                            eprintln!("[DEBUG] Deleted digest {}", digest);
                        }
                        all_deleted_digests.insert(digest.clone());
                        deleted_digests_this_repo.insert(digest.clone());
                    }
                    Err(e) => {
                        eprintln!("[ERROR] Failed to delete digest {}: {}", digest, e);
                        total_errors += 1;
                    }
                }
            }

            // Only count tags whose digests were actually deleted
            for tag in &plan.to_delete {
                if deleted_digests_this_repo.contains(&tag.digest) {
                    total_deleted += 1;
                }
            }
        }
    }

    print_summary(
        total_deleted,
        all_deleted_digests.len(),
        total_kept,
        total_errors,
        args.dry_run,
    );

    if total_errors > 0 {
        process::exit(1);
    }

    Ok(())
}
