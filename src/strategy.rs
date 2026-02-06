use std::collections::HashSet;

use chrono::Utc;
use regex::Regex;

use crate::cli::Cli;
use crate::error::AppError;
use crate::models::{CleanupPlan, TagInfo};

#[derive(Debug)]
pub enum Strategy {
    KeepRecent(usize),
    OlderThan(u64),
    Pattern(Regex),
}

impl Strategy {
    /// Build a Strategy from CLI arguments
    pub fn from_cli(cli: &Cli) -> Result<Self, AppError> {
        if let Some(n) = cli.keep {
            Ok(Strategy::KeepRecent(n))
        } else if let Some(days) = cli.older_than {
            Ok(Strategy::OlderThan(days))
        } else if let Some(ref pat) = cli.pattern {
            let re = Regex::new(pat)?;
            Ok(Strategy::Pattern(re))
        } else {
            Err(AppError::NoStrategy)
        }
    }

    /// Apply the strategy to a list of tags and produce a CleanupPlan
    pub fn apply(&self, repo: &str, mut tags: Vec<TagInfo>) -> CleanupPlan {
        let (mut to_delete, mut to_keep) = match self {
            Strategy::KeepRecent(n) => {
                // Sort by created descending; None sorts to end (deleted first)
                tags.sort_by(|a, b| {
                    let a_time = a.created.map(|t| t.timestamp()).unwrap_or(i64::MIN);
                    let b_time = b.created.map(|t| t.timestamp()).unwrap_or(i64::MIN);
                    b_time.cmp(&a_time)
                });

                let keep_count = (*n).min(tags.len());
                let to_keep: Vec<TagInfo> = tags.drain(..keep_count).collect();
                let to_delete = tags;
                (to_delete, to_keep)
            }
            Strategy::OlderThan(days) => {
                let cutoff = Utc::now() - chrono::Duration::days(*days as i64);
                let mut to_delete = Vec::new();
                let mut to_keep = Vec::new();

                for tag in tags {
                    match tag.created {
                        Some(created) if created < cutoff => to_delete.push(tag),
                        _ => to_keep.push(tag), // None → conservative: keep
                    }
                }

                (to_delete, to_keep)
            }
            Strategy::Pattern(re) => {
                let mut to_delete = Vec::new();
                let mut to_keep = Vec::new();

                for tag in tags {
                    if re.is_match(&tag.tag) {
                        to_delete.push(tag);
                    } else {
                        to_keep.push(tag);
                    }
                }

                (to_delete, to_keep)
            }
        };

        // Shared-digest safety: if multiple tags point to the same digest
        // and one is in to_keep, do not delete that digest
        let keep_digests: HashSet<String> = to_keep.iter().map(|t| t.digest.clone()).collect();

        let mut warned_digests: HashSet<String> = HashSet::new();
        let mut safe_delete = Vec::new();

        for tag in to_delete.drain(..) {
            if keep_digests.contains(&tag.digest) {
                if warned_digests.insert(tag.digest.clone()) {
                    eprintln!(
                        "[WARN] Digest {} is shared with a kept tag; skipping deletion of tag '{}'",
                        truncate_digest(&tag.digest),
                        tag.tag
                    );
                } else {
                    eprintln!(
                        "[WARN] Skipping deletion of tag '{}' (shared digest {})",
                        tag.tag,
                        truncate_digest(&tag.digest)
                    );
                }
                to_keep.push(tag);
            } else {
                safe_delete.push(tag);
            }
        }

        CleanupPlan {
            repository: repo.to_string(),
            to_delete: safe_delete,
            to_keep,
        }
    }
}

/// Count unique digests in a list of TagInfos
#[allow(dead_code)]
pub fn count_unique_digests(tags: &[TagInfo]) -> usize {
    let digests: HashSet<&str> = tags.iter().map(|t| t.digest.as_str()).collect();
    digests.len()
}

fn truncate_digest(digest: &str) -> &str {
    if digest.len() > 19 {
        &digest[..19]
    } else {
        digest
    }
}
