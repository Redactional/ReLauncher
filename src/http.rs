//! HTTP: GitHub's API, raw files and release downloads.

use anyhow::{Context, Result};
use std::io::{Read, Write};
use std::path::Path;
use std::time::Duration;

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        // GitHub's API refuses requests without a User-Agent.
        .user_agent(concat!("ReLauncher/", env!("CARGO_PKG_VERSION")))
        .timeout_connect(Some(Duration::from_secs(15)))
        .timeout_recv_body(Some(Duration::from_secs(60)))
        .build()
        .into()
}

pub fn get_bytes(url: &str) -> Result<Vec<u8>> {
    let mut res = agent()
        .get(url)
        .call()
        .with_context(|| format!("fetching {url}"))?;
    Ok(res.body_mut().with_config().limit(64 << 20).read_to_vec()?)
}

pub fn get_json(url: &str) -> Result<serde_json::Value> {
    let mut res = agent()
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .call()
        .with_context(|| format!("fetching {url}"))?;
    Ok(res.body_mut().read_json()?)
}

/// Downloads `url` to `dest`, reporting (bytes so far, total if known).
pub fn download(url: &str, dest: &Path, mut progress: impl FnMut(u64, Option<u64>)) -> Result<()> {
    let res = agent()
        .get(url)
        .call()
        .with_context(|| format!("downloading {url}"))?;
    let (_, body) = res.into_parts();
    let total = body.content_length();
    let mut reader = body.into_reader();
    let tmp = dest.with_extension("part");
    let mut file =
        std::fs::File::create(&tmp).with_context(|| format!("creating {}", tmp.display()))?;
    let mut buf = vec![0u8; 1 << 16];
    let mut done = 0u64;
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n])?;
        done += n as u64;
        progress(done, total);
    }
    file.flush()?;
    drop(file);
    std::fs::rename(&tmp, dest)?;
    Ok(())
}
