
use anyhow::Result;
use std::path::{PathBuf};
use crate::config::Config;

#[derive(Debug, Clone)]
pub struct EffectivePaths {
    pub prefix_dir: PathBuf,
    pub versions_dir: PathBuf,
    pub current_symlink: PathBuf,
    pub bin_dir: PathBuf,
    pub bin_shim: PathBuf,
    pub desktop_file: PathBuf,
    pub icons_dir: PathBuf,
}

pub fn expand_tilde(s: &str) -> PathBuf {
    let expanded = shellexpand::tilde(s).to_string();
    PathBuf::from(expanded)
}

pub fn resolve_paths(cfg: &Config) -> Result<EffectivePaths> {
    let prefix_dir = expand_tilde(&cfg.install.prefix_dir);
    let versions_dir = prefix_dir.clone();
    let current_symlink = prefix_dir.join("current");
    let bin_dir = expand_tilde(&cfg.install.bin_dir);
    let bin_shim = bin_dir.join("windsurf");
    let desktop_file = expand_tilde("~/.local/share/applications").join("windsurf.desktop");
    let icons_dir = expand_tilde("~/.local/share/icons/hicolor/512x512/apps");
    Ok(EffectivePaths { prefix_dir, versions_dir, current_symlink, bin_dir, bin_shim, desktop_file, icons_dir })
}
