use anyhow::Result;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

const CURRENT_VERSION: &str = "1.0.2";
const VERSION_CHECK_URL: &str = "https://api.github.com/repos/wyeeeee/hajimi/releases/latest";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionInfo {
    pub current_version: String,
    pub latest_version: Option<String>,
    pub has_update_available: bool,
    pub release_notes: Option<String>,
    pub release_date: Option<String>,
}

impl Default for VersionInfo {
    fn default() -> Self {
        Self {
            current_version: CURRENT_VERSION.to_string(),
            latest_version: None,
            has_update_available: false,
            release_notes: None,
            release_date: None,
        }
    }
}

impl VersionInfo {
    pub fn current() -> Self {
        Self {
            current_version: CURRENT_VERSION.to_string(),
            latest_version: None,
            has_update_available: false,
            release_notes: None,
            release_date: None,
        }
    }
}

#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    name: String,
    body: String,
    published_at: String,
    draft: bool,
    prerelease: bool,
}

pub async fn check_for_updates() -> Result<VersionInfo> {
    info!("Checking for updates...");

    let client = reqwest::Client::new();
    let response = client
        .get(VERSION_CHECK_URL)
        .header("User-Agent", "rujimi/1.0.2")
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await?;

    if !response.status().is_success() {
        warn!("Failed to check for updates: HTTP {}", response.status());
        return Ok(VersionInfo::current());
    }

    let release: GitHubRelease = response.json().await?;

    // Skip draft or prerelease versions
    if release.draft || release.prerelease {
        return Ok(VersionInfo::current());
    }

    let latest_version = clean_version_string(&release.tag_name);
    let current_version = clean_version_string(CURRENT_VERSION);

    let has_update = is_newer_version(&latest_version, &current_version);

    let version_info = VersionInfo {
        current_version: CURRENT_VERSION.to_string(),
        latest_version: Some(latest_version),
        has_update_available: has_update,
        release_notes: Some(release.body),
        release_date: Some(release.published_at),
    };

    if has_update {
        info!("Update available: {} -> {}", CURRENT_VERSION, release.tag_name);
    } else {
        info!("Running latest version: {}", CURRENT_VERSION);
    }

    Ok(version_info)
}

fn clean_version_string(version: &str) -> String {
    // Remove 'v' prefix and any other non-semantic version characters
    version
        .trim_start_matches('v')
        .trim_start_matches('V')
        .to_string()
}

fn is_newer_version(latest: &str, current: &str) -> bool {
    match (parse_semantic_version(latest), parse_semantic_version(current)) {
        (Some(latest_parts), Some(current_parts)) => {
            // Compare major.minor.patch
            for i in 0..3 {
                let latest_part = latest_parts.get(i).copied().unwrap_or(0);
                let current_part = current_parts.get(i).copied().unwrap_or(0);

                if latest_part > current_part {
                    return true;
                } else if latest_part < current_part {
                    return false;
                }
            }
            false // Versions are equal
        }
        _ => {
            // Fallback to string comparison if semantic parsing fails
            latest > current
        }
    }
}

fn parse_semantic_version(version: &str) -> Option<Vec<u32>> {
    let parts: Result<Vec<u32>, _> = version
        .split('.')
        .take(3) // Only take major.minor.patch
        .map(|part| part.parse())
        .collect();

    parts.ok()
}

pub fn get_current_version() -> String {
    CURRENT_VERSION.to_string()
}

pub fn format_version_for_display(version_info: &VersionInfo) -> String {
    if version_info.has_update_available {
        if let Some(latest) = &version_info.latest_version {
            format!(
                "v{} (update available: v{})",
                version_info.current_version, latest
            )
        } else {
            format!("v{}", version_info.current_version)
        }
    } else {
        format!("v{} (latest)", version_info.current_version)
    }
}

pub fn get_build_info() -> serde_json::Value {
    serde_json::json!({
        "version": CURRENT_VERSION,
        "build_date": env!("CARGO_PKG_VERSION"), // This will be the Cargo version
        "rust_version": option_env!("CARGO_PKG_RUST_VERSION").unwrap_or("unknown"),
        "target": option_env!("TARGET").unwrap_or("unknown"),
        "profile": if cfg!(debug_assertions) { "debug" } else { "release" },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clean_version_string() {
        assert_eq!(clean_version_string("v1.2.3"), "1.2.3");
        assert_eq!(clean_version_string("V1.2.3"), "1.2.3");
        assert_eq!(clean_version_string("1.2.3"), "1.2.3");
    }

    #[test]
    fn test_parse_semantic_version() {
        assert_eq!(parse_semantic_version("1.2.3"), Some(vec![1, 2, 3]));
        assert_eq!(parse_semantic_version("10.0.1"), Some(vec![10, 0, 1]));
        assert_eq!(parse_semantic_version("1.2"), Some(vec![1, 2]));
        assert_eq!(parse_semantic_version("invalid"), None);
    }

    #[test]
    fn test_is_newer_version() {
        assert!(is_newer_version("1.2.3", "1.2.2"));
        assert!(is_newer_version("1.3.0", "1.2.9"));
        assert!(is_newer_version("2.0.0", "1.9.9"));
        assert!(!is_newer_version("1.2.2", "1.2.3"));
        assert!(!is_newer_version("1.2.3", "1.2.3"));
    }

    #[test]
    fn test_format_version_for_display() {
        let mut version_info = VersionInfo::current();
        assert_eq!(format_version_for_display(&version_info), "v1.0.2 (latest)");

        version_info.has_update_available = true;
        version_info.latest_version = Some("1.0.3".to_string());
        assert_eq!(
            format_version_for_display(&version_info),
            "v1.0.2 (update available: v1.0.3)"
        );
    }
}