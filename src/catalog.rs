//! The list of Re programs (`programs.json` in the ReLauncher repository), so new
//! programs show up without a ReLauncher update.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const CATALOG_URL: &str =
    "https://raw.githubusercontent.com/Redactional/ReLauncher/main/programs.json";

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Catalog {
    pub programs: Vec<Program>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Program {
    /// Stable key ("reetched"): install records and shortcuts use it.
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    /// GitHub repository; its latest release is the version to install.
    pub repo: String,
    /// Raw URL of the app icon (PNG).
    #[serde(default)]
    pub icon: Option<String>,
    /// What to run, relative to the install folder, per OS.
    #[serde(default)]
    pub launch: Launch,
    #[serde(default)]
    pub requirements: Vec<Requirement>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Launch {
    pub windows: Option<String>,
    pub macos: Option<String>,
    pub linux: Option<String>,
}

impl Launch {
    pub fn current(&self) -> Option<&str> {
        match os() {
            "windows" => self.windows.as_deref(),
            "macos" => self.macos.as_deref(),
            _ => self.linux.as_deref(),
        }
    }
}

/// Something a program needs on this system (a runtime, a driver library, ...).
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Requirement {
    pub name: String,
    /// Nice to have; installs go ahead without it.
    #[serde(default)]
    pub optional: bool,
    /// "windows", "macos", "linux"; empty = all.
    #[serde(default)]
    pub platforms: Vec<String>,
    /// Shown next to the requirement (what it's for, how to get it by hand).
    #[serde(default)]
    pub note: String,
    pub check: Check,
    #[serde(default)]
    pub install: InstallStep,
}

impl Requirement {
    pub fn applies(&self) -> bool {
        self.platforms.is_empty() || self.platforms.iter().any(|p| p == os())
    }
}

/// Met when any file exists or any command runs successfully.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Check {
    /// Paths; `%VAR%`, `$VAR` and a leading `~` are expanded.
    #[serde(default)]
    pub files: Vec<String>,
    #[serde(default)]
    pub commands: Vec<Vec<String>>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct InstallStep {
    /// URL of an installer to download and run.
    #[serde(default)]
    pub run: Option<String>,
    #[serde(default)]
    pub args: Vec<String>,
    /// Needs administrator rights (Windows asks for them).
    #[serde(default)]
    pub elevated: bool,
    /// A page to open instead (where the user installs it themselves).
    #[serde(default)]
    pub open: Option<String>,
}

pub fn os() -> &'static str {
    match std::env::consts::OS {
        "windows" => "windows",
        "macos" => "macos",
        _ => "linux",
    }
}

/// "windows-x64", "macos-arm64", "linux-x64": release archives end with `-<platform>.zip`.
pub fn platform() -> String {
    let arch = match std::env::consts::ARCH {
        "x86_64" => "x64",
        "aarch64" => "arm64",
        other => other,
    };
    format!("{}-{arch}", os())
}

pub fn config_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("RELAUNCHER_CONFIG_DIR") {
        return dir.into();
    }
    dirs::config_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("ReLauncher")
}

/// The catalog from `RELAUNCHER_CATALOG` (a file or URL, for testing new entries)
/// or the repository. The last good copy is kept for when GitHub can't be reached.
pub fn load() -> Result<Catalog> {
    let cache = config_dir().join("programs.json");
    let source = std::env::var("RELAUNCHER_CATALOG").unwrap_or_else(|_| CATALOG_URL.into());
    let fetched = if source.starts_with("http") {
        crate::http::get_bytes(&source)
    } else {
        std::fs::read(&source).with_context(|| format!("reading {source}"))
    };
    match fetched.and_then(|b| Ok((serde_json::from_slice::<Catalog>(&b)?, b))) {
        Ok((catalog, bytes)) => {
            let _ = std::fs::create_dir_all(config_dir());
            let _ = std::fs::write(&cache, bytes);
            Ok(catalog)
        }
        Err(e) => {
            let bytes = std::fs::read(&cache).map_err(|_| e)?;
            Ok(serde_json::from_slice(&bytes)?)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shipped_catalog_parses() {
        let c: Catalog = serde_json::from_str(include_str!("../programs.json")).unwrap();
        let p = &c.programs[0];
        assert_eq!(p.id, "reetched");
        assert!(p.repo.starts_with("https://github.com/"));
        assert!(p.launch.current().is_some());
        assert!(p.requirements.iter().any(|r| r.optional));
    }
}
