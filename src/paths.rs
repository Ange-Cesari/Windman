use anyhow::Result;
use directories::BaseDirs;
use shellexpand::tilde;
use std::path::{PathBuf};

use crate::config::Config;

/// All resolved (expanded) paths Windman uses at runtime.
#[derive(Debug, Clone)]
pub struct EffectivePaths {
    /// Versioned installs live here (e.g. ~/.local/opt/windsurf/1.12.11)
    pub prefix_dir: PathBuf,
    /// Alias to the same place where version dirs live (kept for clarity/tests)
    pub versions_dir: PathBuf,
    /// Symlink pointing to the active version dir (e.g. ~/.local/opt/windsurf/current)
    pub current_symlink: PathBuf,
    /// Where the shim script is written (dir only, e.g. ~/.local/bin)
    pub bin_dir: PathBuf,
    /// Full path to the shim (e.g. ~/.local/bin/windsurf)
    pub bin_shim: PathBuf,
    /// Desktop entry path (e.g. ~/.local/share/applications/windsurf.desktop)
    pub desktop_file: PathBuf,
    /// Icons base dir (e.g. ~/.local/share/icons)
    pub icons_dir: PathBuf,
}

/// Expand a path that may contain ~
fn expand(p: &str) -> PathBuf {
    PathBuf::from(tilde(p).into_owned())
}

/// Compute effective paths from config (expands ~, fills XDG locations).
pub fn resolve_paths(cfg: &Config) -> Result<EffectivePaths> {
    let prefix_dir = expand(&cfg.install.prefix_dir);
    let versions_dir = prefix_dir.clone();
    let current_symlink = prefix_dir.join("current");

    let bin_dir = expand(&cfg.install.bin_dir);
    let bin_shim = bin_dir.join("windsurf");

    // XDG data (for desktop file + icons)
    let base = BaseDirs::new();
    // Fallback to ~/.local/share if XDG can't be resolved (very rare)
    let data_dir = base
        .map(|b| b.data_local_dir().to_path_buf())
        .unwrap_or_else(|| {
            let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("~"));
            home.join(".local/share")
        });

    let desktop_file = data_dir.join("applications/windsurf.desktop");
    let icons_dir = data_dir.join("icons");

    Ok(EffectivePaths {
        prefix_dir,
        versions_dir,
        current_symlink,
        bin_dir,
        bin_shim,
        desktop_file,
        icons_dir,
    })
}
