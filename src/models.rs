use chrono::{DateTime, Utc};
use serde::Deserialize;

/// GET /v2/_catalog response
#[derive(Debug, Deserialize)]
pub struct Catalog {
    pub repositories: Vec<String>,
}

/// GET /v2/<repo>/tags/list response
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct TagList {
    pub name: String,
    pub tags: Option<Vec<String>>,
}

/// GET /v2/<repo>/manifests/<tag> (schema v2)
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct Manifest {
    #[serde(rename = "schemaVersion")]
    pub schema_version: u32,
    pub config: Option<ManifestConfig>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct ManifestConfig {
    #[serde(rename = "mediaType")]
    pub media_type: String,
    pub size: u64,
    pub digest: String,
}

/// GET /v2/<repo>/blobs/<config-digest> — image config containing the created timestamp
#[derive(Debug, Deserialize)]
pub struct ImageConfig {
    pub created: Option<DateTime<Utc>>,
}

/// Internal struct combining tag metadata
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct TagInfo {
    pub repository: String,
    pub tag: String,
    pub digest: String,
    pub created: Option<DateTime<Utc>>,
}

/// Result of applying a cleanup strategy to a repository
#[derive(Debug)]
pub struct CleanupPlan {
    pub repository: String,
    pub to_delete: Vec<TagInfo>,
    pub to_keep: Vec<TagInfo>,
}
