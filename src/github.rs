//! Latest releases, read straight from each program's GitHub repository.

use anyhow::{Result, bail};

#[derive(Clone, Debug)]
pub struct Release {
    pub tag: String,
    pub notes: String,
    /// "2026-10-25"
    pub date: String,
    /// The release page, for the user's browser.
    pub page: String,
    /// The archive for this platform, if the release has one.
    pub asset: Option<Asset>,
}

#[derive(Clone, Debug)]
pub struct Asset {
    pub url: String,
    pub size: u64,
}

impl Release {
    /// The tag without a leading "v".
    pub fn version(&self) -> &str {
        self.tag.trim_start_matches('v')
    }
}

/// The latest (non-draft, non-prerelease) release of `repo`, with the asset for `platform`.
pub fn latest(repo: &str, platform: &str) -> Result<Release> {
    let Some(path) = repo
        .trim_end_matches('/')
        .strip_prefix("https://github.com/")
    else {
        bail!("{repo} is not a GitHub repository URL");
    };
    let json = crate::http::get_json(&format!(
        "https://api.github.com/repos/{path}/releases/latest"
    ))
    .map_err(|e| {
        let text = format!("{e:#}");
        if text.contains("404") {
            anyhow::anyhow!("No releases published yet")
        } else if text.contains("403") || text.contains("429") {
            anyhow::anyhow!("GitHub's request limit was reached; try again in a while")
        } else {
            e
        }
    })?;
    Ok(parse(&json, platform))
}

fn parse(json: &serde_json::Value, platform: &str) -> Release {
    let s = |k: &str| json[k].as_str().unwrap_or_default().to_owned();
    let suffix = format!("-{platform}.zip");
    let asset = json["assets"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|a| a["name"].as_str().is_some_and(|n| n.ends_with(&suffix)))
        .map(|a| Asset {
            url: a["browser_download_url"]
                .as_str()
                .unwrap_or_default()
                .into(),
            size: a["size"].as_u64().unwrap_or(0),
        });
    Release {
        tag: s("tag_name"),
        notes: s("body"),
        date: s("published_at").chars().take(10).collect(),
        page: s("html_url"),
        asset,
    }
}

/// Is `version` ("0.10.0") newer than `current` ("0.9.5")?
pub fn newer(version: &str, current: &str) -> bool {
    let parts = |v: &str| -> Vec<u64> {
        v.trim_start_matches('v')
            .split(['.', '-', '+'])
            .map_while(|p| p.parse().ok())
            .collect()
    };
    parts(version) > parts(current)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_this_platforms_archive() {
        let json = serde_json::json!({
            "tag_name": "v0.2.0",
            "body": "Fixes",
            "published_at": "2026-10-25T12:00:00Z",
            "html_url": "https://github.com/x/y/releases/tag/v0.2.0",
            "assets": [
                {"name": "ReEtched-0.2.0-setup.exe", "browser_download_url": "a", "size": 1},
                {"name": "ReEtched-0.2.0-linux-x64.zip", "browser_download_url": "b", "size": 2},
                {"name": "ReEtched-0.2.0-windows-x64.zip", "browser_download_url": "c", "size": 3}
            ]
        });
        let r = parse(&json, "windows-x64");
        assert_eq!(r.version(), "0.2.0");
        assert_eq!(r.date, "2026-10-25");
        assert_eq!(r.asset.unwrap().url, "c");
        assert!(parse(&json, "macos-arm64").asset.is_none());
    }

    #[test]
    fn versions_compare_numerically() {
        assert!(newer("v0.10.0", "0.9.5"));
        assert!(!newer("0.1.0", "v0.1.0"));
        assert!(!newer("garbage", "0.1.0"));
    }
}
