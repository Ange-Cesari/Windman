use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

#[derive(Debug, Clone)]
pub struct ConfigPaths {
    pub dir: PathBuf,
    pub file: PathBuf,
}

impl ConfigPaths {
    pub fn from_override(path: Option<&str>) -> Self {
        if let Some(p) = path {
            let file = shellexpand::tilde(p).to_string();
            let file = PathBuf::from(file);
            let dir = file
                .parent()
                .unwrap_or_else(|| std::path::Path::new("."))
                .to_path_buf();
            return Self { dir, file };
        }
        let proj = ProjectDirs::from("dev", "Windman", "windman").expect("project dirs");
        let dir = proj.config_dir().to_path_buf();
        let file = dir.join("windman.toml");
        Self { dir, file }
    }

    pub fn config_display(&self) -> String {
        self.file.display().to_string()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub install: Install,
    pub changelog: Changelog,
    pub network: Network,
    pub telemetry: Telemetry,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Install {
    /// Intentional, userland-only install to avoid requiring sudo and to keep environments portable.
    pub prefix_dir: String, // e.g. ~/.local/opt/windsurf
    pub bin_dir: String, // e.g. ~/.local/bin
    pub channel: String, // "stable" for now
    pub keep: usize,     // number of previous versions to keep
    pub desktop_integration: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Changelog {
    pub _reserved: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Network {
    pub proxy_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Telemetry {
    pub enabled: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            install: Install {
                prefix_dir: "~/.local/opt/windsurf".into(),
                bin_dir: "~/.local/bin".into(),
                channel: "stable".into(),
                keep: 2,
                desktop_integration: true,
            },
            changelog: Changelog { _reserved: None },
            network: Network {
                proxy_enabled: false,
            },
            telemetry: Telemetry { enabled: false },
        }
    }
}

impl Config {
    pub fn load_or_default(paths: &ConfigPaths) -> Result<Self> {
        if paths.file.exists() {
            let s = fs::read_to_string(&paths.file)
                .with_context(|| format!("reading config at {}", paths.config_display()))?;
            let mut cfg: Config = toml::from_str(&s).with_context(|| "parsing TOML")?;
            // Enforce userland choice explicitly (documented design decision)
            cfg.install.channel = "stable".into();
            Ok(cfg)
        } else {
            Ok(Config::default())
        }
    }

    pub fn save_if_missing(&self, paths: &ConfigPaths) -> Result<()> {
        if !paths.dir.exists() {
            fs::create_dir_all(&paths.dir)?;
        }
        if !paths.file.exists() {
            let s = toml::to_string_pretty(self)?;
            fs::write(&paths.file, s)?;
        }
        Ok(())
    }
}
