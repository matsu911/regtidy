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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{DateTime, Duration, Utc};
    use crate::models::TagInfo;

    fn make_tag(repo: &str, tag: &str, digest: &str, created: Option<DateTime<Utc>>) -> TagInfo {
        TagInfo {
            repository: repo.to_string(),
            tag: tag.to_string(),
            digest: digest.to_string(),
            created,
        }
    }

    #[test]
    fn test_keep_recent_basic() {
        let now = Utc::now();
        let tags = vec![
            make_tag("r", "t1", "d1", Some(now - Duration::days(5))),
            make_tag("r", "t2", "d2", Some(now - Duration::days(4))),
            make_tag("r", "t3", "d3", Some(now - Duration::days(3))),
            make_tag("r", "t4", "d4", Some(now - Duration::days(2))),
            make_tag("r", "t5", "d5", Some(now - Duration::days(1))),
        ];

        let strategy = Strategy::KeepRecent(3);
        let plan = strategy.apply("r", tags);

        let kept_tags: Vec<&str> = plan.to_keep.iter().map(|t| t.tag.as_str()).collect();
        let deleted_tags: Vec<&str> = plan.to_delete.iter().map(|t| t.tag.as_str()).collect();

        assert_eq!(plan.to_keep.len(), 3);
        assert_eq!(plan.to_delete.len(), 2);
        assert!(kept_tags.contains(&"t5"));
        assert!(kept_tags.contains(&"t4"));
        assert!(kept_tags.contains(&"t3"));
        assert!(deleted_tags.contains(&"t1"));
        assert!(deleted_tags.contains(&"t2"));
    }

    #[test]
    fn test_keep_recent_more_than_available() {
        let now = Utc::now();
        let tags = vec![
            make_tag("r", "t1", "d1", Some(now - Duration::days(3))),
            make_tag("r", "t2", "d2", Some(now - Duration::days(2))),
            make_tag("r", "t3", "d3", Some(now - Duration::days(1))),
        ];

        let strategy = Strategy::KeepRecent(10);
        let plan = strategy.apply("r", tags);

        assert_eq!(plan.to_keep.len(), 3);
        assert_eq!(plan.to_delete.len(), 0);
    }

    #[test]
    fn test_older_than_basic() {
        let now = Utc::now();
        let tags = vec![
            make_tag("r", "old1", "d1", Some(now - Duration::days(60))),
            make_tag("r", "old2", "d2", Some(now - Duration::days(45))),
            make_tag("r", "new1", "d3", Some(now - Duration::days(10))),
            make_tag("r", "new2", "d4", Some(now - Duration::days(5))),
        ];

        let strategy = Strategy::OlderThan(30);
        let plan = strategy.apply("r", tags);

        let kept_tags: Vec<&str> = plan.to_keep.iter().map(|t| t.tag.as_str()).collect();
        let deleted_tags: Vec<&str> = plan.to_delete.iter().map(|t| t.tag.as_str()).collect();

        assert_eq!(plan.to_delete.len(), 2);
        assert_eq!(plan.to_keep.len(), 2);
        assert!(deleted_tags.contains(&"old1"));
        assert!(deleted_tags.contains(&"old2"));
        assert!(kept_tags.contains(&"new1"));
        assert!(kept_tags.contains(&"new2"));
    }

    #[test]
    fn test_older_than_unknown_date_kept() {
        let now = Utc::now();
        let tags = vec![
            make_tag("r", "old", "d1", Some(now - Duration::days(60))),
            make_tag("r", "unknown", "d2", None),
        ];

        let strategy = Strategy::OlderThan(30);
        let plan = strategy.apply("r", tags);

        let kept_tags: Vec<&str> = plan.to_keep.iter().map(|t| t.tag.as_str()).collect();

        assert_eq!(plan.to_delete.len(), 1);
        assert_eq!(plan.to_delete[0].tag, "old");
        assert!(kept_tags.contains(&"unknown"));
    }

    #[test]
    fn test_pattern_matching() {
        let tags = vec![
            make_tag("r", "v1.0", "d1", None),
            make_tag("r", "dev-abc", "d2", None),
            make_tag("r", "dev-xyz", "d3", None),
            make_tag("r", "v2.0", "d4", None),
        ];

        let strategy = Strategy::Pattern(Regex::new("^dev-").unwrap());
        let plan = strategy.apply("r", tags);

        let kept_tags: Vec<&str> = plan.to_keep.iter().map(|t| t.tag.as_str()).collect();
        let deleted_tags: Vec<&str> = plan.to_delete.iter().map(|t| t.tag.as_str()).collect();

        assert_eq!(plan.to_delete.len(), 2);
        assert_eq!(plan.to_keep.len(), 2);
        assert!(deleted_tags.contains(&"dev-abc"));
        assert!(deleted_tags.contains(&"dev-xyz"));
        assert!(kept_tags.contains(&"v1.0"));
        assert!(kept_tags.contains(&"v2.0"));
    }

    #[test]
    fn test_shared_digest_safety() {
        let now = Utc::now();
        // t1 and t2 share digest "shared-digest"
        // KeepRecent(1) keeps only the newest (t2), so t1 would normally be deleted.
        // But since t1 shares a digest with t2 (which is kept), t1 should also be kept.
        let tags = vec![
            make_tag("r", "t1", "shared-digest", Some(now - Duration::days(2))),
            make_tag("r", "t2", "shared-digest", Some(now - Duration::days(1))),
            make_tag("r", "t3", "other-digest", Some(now - Duration::days(3))),
        ];

        let strategy = Strategy::KeepRecent(1);
        let plan = strategy.apply("r", tags);

        let kept_tags: Vec<&str> = plan.to_keep.iter().map(|t| t.tag.as_str()).collect();
        let deleted_tags: Vec<&str> = plan.to_delete.iter().map(|t| t.tag.as_str()).collect();

        // t2 is kept by strategy (newest), t1 is kept by shared-digest safety
        assert!(kept_tags.contains(&"t2"));
        assert!(kept_tags.contains(&"t1"));
        // t3 has a unique digest and is old, so it's deleted
        assert!(deleted_tags.contains(&"t3"));
        assert_eq!(plan.to_delete.len(), 1);
        assert_eq!(plan.to_keep.len(), 2);
    }
}
