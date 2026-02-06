use anyhow::{Context, Result};
use reqwest::header::{ACCEPT, LINK};
use reqwest::Client;
use std::sync::Arc;
use tokio::sync::Semaphore;

use crate::models::{Catalog, ImageConfig, Manifest, TagInfo, TagList};

const MANIFEST_V2_MEDIA_TYPE: &str = "application/vnd.docker.distribution.manifest.v2+json";

pub struct RegistryClient {
    client: Client,
    base_url: String,
    verbose: bool,
}

impl RegistryClient {
    pub fn new(base_url: &str, verbose: bool) -> Self {
        let base_url = base_url.trim_end_matches('/').to_string();
        Self {
            client: Client::new(),
            base_url,
            verbose,
        }
    }

    /// GET /v2/_catalog with pagination
    pub async fn list_repositories(&self) -> Result<Vec<String>> {
        let mut repos = Vec::new();
        let mut url = format!("{}/v2/_catalog", self.base_url);

        loop {
            if self.verbose {
                eprintln!("[DEBUG] GET {}", url);
            }
            let resp = self
                .client
                .get(&url)
                .send()
                .await
                .context("Failed to fetch catalog")?;

            let next_link = Self::parse_next_link(&resp);

            let catalog: Catalog = resp.json().await.context("Failed to parse catalog JSON")?;
            repos.extend(catalog.repositories);

            match next_link {
                Some(next) => url = self.resolve_url(&next),
                None => break,
            }
        }

        Ok(repos)
    }

    /// GET /v2/<repo>/tags/list with pagination
    pub async fn list_tags(&self, repo: &str) -> Result<Vec<String>> {
        let mut tags = Vec::new();
        let mut url = format!("{}/v2/{}/tags/list", self.base_url, repo);

        loop {
            if self.verbose {
                eprintln!("[DEBUG] GET {}", url);
            }
            let resp = self
                .client
                .get(&url)
                .send()
                .await
                .with_context(|| format!("Failed to fetch tags for {}", repo))?;

            let next_link = Self::parse_next_link(&resp);

            let tag_list: TagList = resp
                .json()
                .await
                .with_context(|| format!("Failed to parse tag list for {}", repo))?;

            if let Some(t) = tag_list.tags {
                tags.extend(t);
            }

            match next_link {
                Some(next) => url = self.resolve_url(&next),
                None => break,
            }
        }

        Ok(tags)
    }

    /// HEAD /v2/<repo>/manifests/<tag> — extract Docker-Content-Digest header
    pub async fn get_digest(&self, repo: &str, tag: &str) -> Result<String> {
        let url = format!("{}/v2/{}/manifests/{}", self.base_url, repo, tag);
        if self.verbose {
            eprintln!("[DEBUG] HEAD {}", url);
        }
        let resp = self
            .client
            .head(&url)
            .header(ACCEPT, MANIFEST_V2_MEDIA_TYPE)
            .send()
            .await
            .with_context(|| format!("Failed to HEAD manifest for {}:{}", repo, tag))?;

        let status = resp.status();
        if !status.is_success() {
            anyhow::bail!(
                "HEAD manifest for {}:{} returned status {}",
                repo,
                tag,
                status
            );
        }

        resp.headers()
            .get("Docker-Content-Digest")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string())
            .with_context(|| {
                format!(
                    "Missing Docker-Content-Digest header for {}:{}",
                    repo, tag
                )
            })
    }

    /// GET /v2/<repo>/manifests/<tag> — parse manifest JSON
    pub async fn get_manifest(&self, repo: &str, tag: &str) -> Result<Manifest> {
        let url = format!("{}/v2/{}/manifests/{}", self.base_url, repo, tag);
        if self.verbose {
            eprintln!("[DEBUG] GET {}", url);
        }
        let resp = self
            .client
            .get(&url)
            .header(ACCEPT, MANIFEST_V2_MEDIA_TYPE)
            .send()
            .await
            .with_context(|| format!("Failed to GET manifest for {}:{}", repo, tag))?;

        let status = resp.status();
        if !status.is_success() {
            anyhow::bail!(
                "GET manifest for {}:{} returned status {}",
                repo,
                tag,
                status
            );
        }

        resp.json()
            .await
            .with_context(|| format!("Failed to parse manifest for {}:{}", repo, tag))
    }

    /// GET /v2/<repo>/blobs/<config_digest> — parse created timestamp
    pub async fn get_image_config(&self, repo: &str, config_digest: &str) -> Result<ImageConfig> {
        let url = format!("{}/v2/{}/blobs/{}", self.base_url, repo, config_digest);
        if self.verbose {
            eprintln!("[DEBUG] GET {}", url);
        }
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .with_context(|| format!("Failed to GET blob {} for {}", config_digest, repo))?;

        let status = resp.status();
        if !status.is_success() {
            anyhow::bail!(
                "GET blob {} for {} returned status {}",
                config_digest,
                repo,
                status
            );
        }

        resp.json()
            .await
            .with_context(|| format!("Failed to parse image config for {}", repo))
    }

    /// DELETE /v2/<repo>/manifests/<digest>
    pub async fn delete_manifest(&self, repo: &str, digest: &str) -> Result<()> {
        let url = format!("{}/v2/{}/manifests/{}", self.base_url, repo, digest);
        if self.verbose {
            eprintln!("[DEBUG] DELETE {}", url);
        }
        let resp = self
            .client
            .delete(&url)
            .header(ACCEPT, MANIFEST_V2_MEDIA_TYPE)
            .send()
            .await
            .with_context(|| format!("Failed to DELETE manifest {} for {}", digest, repo))?;

        let status = resp.status();
        if !status.is_success() {
            anyhow::bail!(
                "DELETE manifest {} for {} returned status {}",
                digest,
                repo,
                status
            );
        }

        Ok(())
    }

    /// Resolve a single tag into TagInfo (digest + created timestamp)
    pub async fn resolve_tag_info(&self, repo: &str, tag: &str) -> Result<TagInfo> {
        let digest = self.get_digest(repo, tag).await?;
        let manifest = self.get_manifest(repo, tag).await?;

        let created = if let Some(config) = &manifest.config {
            match self.get_image_config(repo, &config.digest).await {
                Ok(img_config) => img_config.created,
                Err(e) => {
                    if self.verbose {
                        eprintln!(
                            "[WARN] Could not fetch image config for {}:{}: {}",
                            repo, tag, e
                        );
                    }
                    None
                }
            }
        } else {
            None
        };

        Ok(TagInfo {
            repository: repo.to_string(),
            tag: tag.to_string(),
            digest,
            created,
        })
    }

    /// Resolve all tags in a repo with bounded concurrency
    pub async fn resolve_all_tags(&self, repo: &str) -> Result<Vec<TagInfo>> {
        let tags = self.list_tags(repo).await?;
        if tags.is_empty() {
            return Ok(Vec::new());
        }

        let semaphore = Arc::new(Semaphore::new(10));
        let mut handles = Vec::with_capacity(tags.len());

        for tag in &tags {
            let permit = semaphore.clone().acquire_owned().await.unwrap();
            let repo = repo.to_string();
            let tag = tag.clone();
            let client = self.client.clone();
            let base_url = self.base_url.clone();
            let verbose = self.verbose;

            handles.push(tokio::spawn(async move {
                let rc = RegistryClient {
                    client,
                    base_url,
                    verbose,
                };
                let result = rc.resolve_tag_info(&repo, &tag).await;
                drop(permit);
                (tag, result)
            }));
        }

        let mut infos = Vec::new();
        for handle in handles {
            let (tag, result) = handle.await.context("Task join error")?;
            match result {
                Ok(info) => infos.push(info),
                Err(e) => {
                    eprintln!("[ERROR] Failed to resolve {}:{}: {}", repo, tag, e);
                }
            }
        }

        Ok(infos)
    }

    /// Parse the Link header for pagination (next URL)
    fn parse_next_link(resp: &reqwest::Response) -> Option<String> {
        let link = resp.headers().get(LINK)?.to_str().ok()?;
        // Link: </v2/_catalog?n=100&last=xxx>; rel="next"
        if link.contains("rel=\"next\"") {
            let start = link.find('<')? + 1;
            let end = link.find('>')?;
            Some(link[start..end].to_string())
        } else {
            None
        }
    }

    /// Resolve a relative URL path against the base URL
    fn resolve_url(&self, path: &str) -> String {
        if path.starts_with("http://") || path.starts_with("https://") {
            path.to_string()
        } else {
            format!("{}{}", self.base_url, path)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_url_relative() {
        let client = RegistryClient::new("http://localhost:5000", false);
        let resolved = client.resolve_url("/v2/_catalog?n=100&last=foo");
        assert_eq!(resolved, "http://localhost:5000/v2/_catalog?n=100&last=foo");
    }

    #[test]
    fn test_resolve_url_absolute() {
        let client = RegistryClient::new("http://localhost:5000", false);
        let resolved = client.resolve_url("http://other:5000/v2/_catalog?n=100");
        assert_eq!(resolved, "http://other:5000/v2/_catalog?n=100");
    }

    #[test]
    fn test_resolve_url_strips_trailing_slash() {
        let client = RegistryClient::new("http://localhost:5000/", false);
        let resolved = client.resolve_url("/v2/_catalog");
        assert_eq!(resolved, "http://localhost:5000/v2/_catalog");
    }
}
