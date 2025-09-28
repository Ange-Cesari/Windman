use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct ConfigPaths {
    pub dir: PathBuf,
    pub file: PathBuf,
}

impl ConfigPaths {
    pub fn from_override(override_path: Option<&str>) -> Self {
        if let Some(p) = override_path {
            let file = shellexpand::tilde(p).into_owned();
            let file = PathBuf::from(file);
            let dir = file.parent().unwrap_or_else(|| Path::new(".")).to_path_buf();
            return Self { dir, file };
        }
        let proj = ProjectDirs::from("dev", "Windman", "windman")
            .expect("cannot determine config dir");
        let dir = proj.config_dir().to_path_buf();
        let file = dir.join("windman.toml");
        Self { dir, file }
    }

    pub fn config_display(&self) -> String {
        self.file.display().to_string()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallConfig {
    /// Default userland prefix for versioned installs
    pub prefix_dir: String, // e.g. "~/.local/opt/windsurf"
    /// Where the shim is written
    pub bin_dir: String,    // e.g. "~/.local/bin"
    /// Only "stable" for now
    pub channel: String,
    /// Keep N newest versions (prune policy)
    pub keep: usize,
    /// Create/update desktop integration by default
    pub desktop_integration: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    /// Reserved for future proxy support
    pub proxy_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub install: InstallConfig,
    #[serde(default)]
    pub changelog: ChangelogConfig,
    pub network: NetworkConfig,
    // NOTE: telemetry removed (standalone, no tracking).
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ChangelogConfig {
    // reserved for future (e.g., show delta)
}

impl Default for Config {
    fn default() -> Self {
        Self {
            install: InstallConfig {
                prefix_dir: "~/.local/opt/windsurf".to_string(),
                bin_dir: "~/.local/bin".to_string(),
                channel: "stable".to_string(),
                keep: 2,
                desktop_integration: true,
            },
            changelog: ChangelogConfig::default(),
            network: NetworkConfig {
                proxy_enabled: false,
            },
        }
    }
}

impl Config {
    pub fn load_or_default(paths: &ConfigPaths) -> Result<Self> {
        if paths.file.exists() {
            let s = fs::read_to_string(&paths.file)
                .with_context(|| format!("reading {}", paths.config_display()))?;
            // Unknown fields (e.g., legacy [telemetry]) are ignored by default.
            let cfg: Config = toml::from_str(&s)
                .with_context(|| format!("parsing {}", paths.config_display()))?;
            Ok(cfg)
        } else {
            Ok(Self::default())
        }
    }

    pub fn save_if_missing(&self, paths: &ConfigPaths) -> Result<()> {
        if !paths.dir.exists() {
            fs::create_dir_all(&paths.dir)
                .with_context(|| format!("creating {}", paths.dir.display()))?;
        }
        if !paths.file.exists() {
            let mut out = String::new();
            out.push_str("[install]\n");
            out.push_str(&format!("prefix_dir = \"{}\"\n", self.install.prefix_dir));
            out.push_str(&format!("bin_dir = \"{}\"\n", self.install.bin_dir));
            out.push_str(&format!("channel = \"{}\"\n", self.install.channel));
            out.push_str(&format!("keep = {}\n", self.install.keep));
            out.push_str(&format!(
                "desktop_integration = {}\n\n",
                self.install.desktop_integration
            ));

            out.push_str("[changelog]\n\n");

            out.push_str("[network]\n");
            out.push_str(&format!(
                "proxy_enabled = {}\n",
                self.network.proxy_enabled
            ));

            fs::write(&paths.file, out)
                .with_context(|| format!("writing {}", paths.config_display()))?;
        }
        Ok(())
    }
}
