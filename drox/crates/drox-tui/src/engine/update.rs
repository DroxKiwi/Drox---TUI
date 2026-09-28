//! Vérification MAJ TUI via `latest.json` (dépôt sources / Releases).

use std::time::Duration;

use anyhow::Context;
use semver::Version;
use serde::{Deserialize, Serialize};

use crate::i18n::{self, keys_update as u};

/// URL raw GitHub du manifeste (surchargeable en tests via `DROX_UPDATE_JSON_URL`).
pub fn latest_json_url() -> String {
    std::env::var("DROX_UPDATE_JSON_URL").unwrap_or_else(|_| {
        "https://raw.githubusercontent.com/DroxKiwi/Drox---TUI/main/releases/latest.json"
            .into()
    })
}

pub const LOCAL_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlatformAsset {
    pub url: String,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LatestRelease {
    pub version: String,
    pub published_at: String,
    pub product: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub engine_baseline: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub windows_x64: Option<PlatformAsset>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub linux_x64: Option<PlatformAsset>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub release_notes: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateComparison {
    UpToDate,
    UpdateAvailable { remote: String },
    NewerLocal { remote: String },
}

pub fn parse_latest_json(raw: &str) -> anyhow::Result<LatestRelease> {
    let release: LatestRelease =
        serde_json::from_str(raw).context("parse latest.json")?;
    if release.product != "drox-tui" {
        anyhow::bail!("produit inattendu : {}", release.product);
    }
    Ok(release)
}

pub fn compare_versions(local: &str, remote: &str) -> anyhow::Result<UpdateComparison> {
    let local_v = Version::parse(local)
        .with_context(|| format!("version locale invalide ({local})"))?;
    let remote_v = Version::parse(remote)
        .with_context(|| format!("version distante invalide ({remote})"))?;
    Ok(if remote_v > local_v {
        UpdateComparison::UpdateAvailable {
            remote: remote.to_string(),
        }
    } else if remote_v < local_v {
        UpdateComparison::NewerLocal {
            remote: remote.to_string(),
        }
    } else {
        UpdateComparison::UpToDate
    })
}

pub async fn fetch_latest() -> anyhow::Result<LatestRelease> {
    let client = reqwest::Client::builder()
        .user_agent(format!("drox-tui/{LOCAL_VERSION} update-check"))
        .timeout(Duration::from_secs(15))
        .build()
        .context("client HTTP MAJ")?;
    let url = latest_json_url();
    let resp = client
        .get(&url)
        .send()
        .await
        .with_context(|| format!("GET {url}"))?;
    if !resp.status().is_success() {
        anyhow::bail!("HTTP {} — {url}", resp.status());
    }
    let body = resp.text().await.context("corps latest.json")?;
    parse_latest_json(&body)
}

pub async fn check_for_update() -> anyhow::Result<(LatestRelease, UpdateComparison)> {
    let release = fetch_latest().await?;
    let cmp = compare_versions(LOCAL_VERSION, &release.version)?;
    Ok((release, cmp))
}

/// Messages fil système après `/update check`.
#[must_use]
pub fn format_check_lines(release: &LatestRelease, cmp: &UpdateComparison) -> Vec<String> {
    let mut lines = vec![
        i18n::tf(u::UPDATE_CHECK_REMOTE, &release.version),
        i18n::tf(u::UPDATE_STATUS_VERSION, LOCAL_VERSION),
    ];
    if let Some(ref notes) = release.release_notes {
        lines.push(i18n::tf(u::UPDATE_CHECK_RELEASE_NOTES, notes));
    }
    lines.push(match cmp {
        UpdateComparison::UpToDate => i18n::t(u::UPDATE_CHECK_UP_TO_DATE).into(),
        UpdateComparison::UpdateAvailable { remote } => {
            i18n::tf(u::UPDATE_CHECK_AVAILABLE, remote)
        }
        UpdateComparison::NewerLocal { remote } => {
            i18n::tf2(u::UPDATE_CHECK_NEWER_LOCAL, LOCAL_VERSION, remote)
        }
    });
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = r#"{
  "version": "9.9.9",
  "published_at": "2026-06-20T12:00:00Z",
  "product": "drox-tui",
  "engine_baseline": "1.5.0",
  "windows_x64": {
    "url": "https://example.com/setup.exe",
    "sha256": "abc"
  },
  "release_notes": "https://example.com/notes"
}"#;

    #[test]
    fn parse_latest_json_fixture() {
        let release = parse_latest_json(FIXTURE).expect("parse");
        assert_eq!(release.version, "9.9.9");
        assert!(release.windows_x64.is_some());
    }

    #[test]
    fn compare_semver() {
        assert!(matches!(
            compare_versions("2.0.3", "2.0.4").unwrap(),
            UpdateComparison::UpdateAvailable { .. }
        ));
        assert_eq!(
            compare_versions("2.0.3", "2.0.3").unwrap(),
            UpdateComparison::UpToDate
        );
        assert!(matches!(
            compare_versions("2.0.3", "2.0.2").unwrap(),
            UpdateComparison::NewerLocal { .. }
        ));
    }

    #[test]
    fn rejects_wrong_product() {
        let raw = r#"{"version":"1.0.0","published_at":"x","product":"other"}"#;
        assert!(parse_latest_json(raw).is_err());
    }
}
