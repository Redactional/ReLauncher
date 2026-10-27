//! What ReLauncher remembers: where programs go and what is installed where.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Where new installs go (None = the platform default).
    pub install_dir: Option<PathBuf>,
    /// Look for a newer ReLauncher at startup.
    pub check_self_update: bool,
    /// By program id.
    pub installed: BTreeMap<String, Installed>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            install_dir: None,
            check_self_update: true,
            installed: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Installed {
    pub version: String,
    /// The program's own folder (inside the install directory at the time).
    pub path: PathBuf,
}

impl Settings {
    fn path() -> PathBuf {
        crate::catalog::config_dir().join("settings.json")
    }

    pub fn load() -> Self {
        std::fs::read(Self::path())
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) -> anyhow::Result<()> {
        std::fs::create_dir_all(crate::catalog::config_dir())?;
        std::fs::write(Self::path(), serde_json::to_vec_pretty(self)?)?;
        Ok(())
    }

    pub fn install_dir(&self) -> PathBuf {
        self.install_dir.clone().unwrap_or_else(default_install_dir)
    }
}

/// Per-user locations that need no administrator rights:
/// `%LOCALAPPDATA%\Programs\Re`, `~/Applications` (Spotlight and Launchpad list
/// apps there), `~/.local/share/re-programs`.
pub fn default_install_dir() -> PathBuf {
    let home = dirs::home_dir().unwrap_or_else(std::env::temp_dir);
    match crate::catalog::os() {
        "windows" => dirs::data_local_dir()
            .unwrap_or(home)
            .join("Programs")
            .join("Re"),
        "macos" => home.join("Applications"),
        _ => dirs::data_local_dir()
            .unwrap_or_else(|| home.join(".local/share"))
            .join("re-programs"),
    }
}
