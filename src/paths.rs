use crate::config::Config;
use anyhow::Result;
use std::path::PathBuf;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Changelog, Config, Install, Network, Telemetry};
    use std::env;
    use tempfile::tempdir;

    #[test]
    fn resolve_paths_expands_tilde_with_home() {
        let td = tempdir().unwrap();
        // pointer $HOME vers un dossier temporaire
        env::set_var("HOME", td.path());

        let cfg = Config {
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
        };

        let eff = resolve_paths(&cfg).unwrap();
        assert!(eff
            .prefix_dir
            .starts_with(td.path().join(".local/opt/windsurf")));
        assert_eq!(eff.bin_shim, td.path().join(".local/bin/windsurf"));
        assert_eq!(eff.current_symlink, eff.prefix_dir.join("current"));
    }
}
