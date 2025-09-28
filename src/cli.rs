use anyhow::{bail, Result};
use clap::{Args, Parser, Subcommand};

use crate::config::{Config, ConfigPaths};
use crate::paths::resolve_paths;
use crate::{desktop, install, prune, version};

#[derive(Parser, Debug)]
#[command(
    name = "windman",
    version,
    about = "Windsurf Manager (userland, standalone)"
)]
pub struct Cli {
    /// Override config path
    #[arg(long, global = true, env = "WINDMAN_CONFIG_PATH")]
    pub config: Option<String>,

    /// Override install prefix directory for this run (e.g., ~/.local/opt/windsurf)
    #[arg(long, global = true, value_name = "DIR")]
    pub prefix: Option<String>,

    /// Override bin dir for this run (where the shim 'windsurf' is written)
    #[arg(long, global = true, value_name = "DIR")]
    pub bin_dir: Option<String>,

    /// Verbose output
    #[arg(short, long, global = true)]
    pub verbose: bool,

    #[command(subcommand)]
    pub cmd: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Download + install latest stable (next step). For now: install from a provided tar.gz.
    Install(InstallArgs),
    /// Compare local vs remote and update if needed
    Update(UpdateArgs),
    /// Show local version and paths
    Status,
    /// Print install and shim paths
    Where,
    /// List installed versions and show current
    List,
    /// Show changelog delta (coming soon)
    Changelog,
    /// Remove installs and shims (keeps user data)
    Uninstall {
        #[arg(long)]
        purge: bool,
    },
    /// Switch back to previous kept version
    Rollback,
    /// Manage configuration
    #[command(subcommand)]
    Config(ConfigCmd),

    /// Internal helper to test latest endpoint (hidden in help)
    #[command(hide = true)]
    DevLatest(DevLatestArgs),

    /// Internal helper to test the downloader (hidden in help)
    #[command(hide = true)]
    DevDownload(DevDownloadArgs),
}

#[derive(Args, Debug)]
pub struct DevLatestArgs {
    #[arg(long)]
    pub timeout: Option<u64>,
    #[arg(long)]
    pub dump_html: Option<String>,
}

#[derive(Args, Debug)]
pub struct InstallArgs {
    /// Path to a local Windsurf tar.gz (temporary until network fetch is added)
    #[arg(long, value_name = "FILE")]
    pub tar: Option<String>,

    /// Force desktop integration even if disabled in config
    #[arg(long)]
    pub desktop: bool,

    /// Do not create/update desktop integration for this run
    #[arg(long)]
    pub no_desktop: bool,

    /// Keep N previous versions (overrides config)
    #[arg(long, value_name = "N")]
    pub keep: Option<usize>,

    /// Dry-run: print actions without changing the system
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Args, Debug)]
pub struct UpdateArgs {
    /// Force desktop integration for this run
    #[arg(long)]
    pub desktop: bool,

    /// Do not create/update desktop integration for this run
    #[arg(long)]
    pub no_desktop: bool,

    /// Dry-run
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Subcommand, Debug)]
pub enum ConfigCmd {
    /// Create default config file if missing
    Init,
    /// Show effective config (after env overrides)
    Show,
}

#[derive(Args, Debug)]
pub struct DevDownloadArgs {
    /// URL to download
    #[arg(long)]
    pub url: String,

    /// Output file path
    #[arg(long)]
    pub out: String,

    /// Request timeout in seconds (overrides default 30s)
    #[arg(long)]
    pub timeout: Option<u64>,
}

pub(crate) fn collect_installed(eff: &crate::paths::EffectivePaths) -> Vec<(String, bool)> {
    use std::fs;
    use std::path::Path;

    let mut entries = Vec::new();
    let current_target = fs::read_link(&eff.current_symlink).ok();

    if eff.versions_dir.exists() {
        if let Ok(rd) = fs::read_dir(&eff.versions_dir) {
            for ent in rd.flatten() {
                let path = ent.path();
                let file_name = ent.file_name();
                let name = file_name.to_string_lossy().to_string();

                if name == "current" { continue; }
                if path.is_dir() {
                    let is_current = match &current_target {
                        Some(ct) => Path::new(ct).file_name() == Some(file_name.as_ref()),
                        None => false,
                    };
                    entries.push((name, is_current));
                }
            }
        }
    }

    // tri (semver desc > lexico desc)
    entries.sort_by(|a, b| {
        let asv = semver::Version::parse(&a.0);
        let bsv = semver::Version::parse(&b.0);
        match (asv, bsv) {
            (Ok(av), Ok(bv)) => bv.cmp(&av),
            _ => b.0.cmp(&a.0),
        }
    });
    entries
}



impl Cli {
    pub fn parse() -> Self {
        <Cli as Parser>::parse()
    }

    pub fn run(&self) -> Result<()> {
        let cfg_paths = ConfigPaths::from_override(self.config.as_deref());
        let mut cfg = Config::load_or_default(&cfg_paths)?;

        // Appliquer les overrides globaux (courte vie, n’écrit pas la config)
        if let Some(p) = &self.prefix {
            cfg.install.prefix_dir = p.clone();
        }
        if let Some(b) = &self.bin_dir {
            cfg.install.bin_dir = b.clone();
        }

        let eff = resolve_paths(&cfg)?;

        if self.verbose {
            eprintln!("[windman] Using config at {}", cfg_paths.config_display());
            eprintln!("[windman] Effective prefix: {}", eff.prefix_dir.display());
            eprintln!("[windman] Effective bin   : {}", eff.bin_dir.display());
        }

        match &self.cmd {
            Commands::Install(args) => {
                let keep = args.keep.unwrap_or(cfg.install.keep);
                if args.dry_run {
                    println!("[dry-run] would install to {:?}", eff.prefix_dir);
                    println!("[dry-run] would maintain shim at {:?}", eff.bin_shim);
                    return Ok(());
                }

                if let Some(tar) = &args.tar {
                    let ver = install::install_from_tar(tar, &eff)?;
                    println!("Installed Windsurf {} to {:?}", ver, eff.prefix_dir);
                    let want_desktop = if args.no_desktop {
                        false
                    } else {
                        args.desktop || cfg.install.desktop_integration
                    };
                    if want_desktop {
                        desktop::ensure_desktop_files(&eff)?;
                        println!("Desktop entry installed");
                    }
                    prune::prune_old_versions(&eff, keep)?;
                    Ok(())
                } else {
                    bail!("--tar <FILE> is required for now. Network download will be added next.")
                }
            }
            Commands::Update(args) => {
                use directories::ProjectDirs;
                use semver::Version;
                use std::path::PathBuf;

                // 1) Local version
                let local = version::detect_local_version(&eff)?;

                // 2) Remote via API (version + url)
                let latest = crate::remote::latest_stable_linux_x64(None)?;
                let latest_ver = Version::parse(&latest.version).map_err(|e| {
                    anyhow::anyhow!("cannot parse remote version {}: {}", latest.version, e)
                })?;

                // 3) Compare
                if let Some(local_s) = &local {
                    if let Ok(local_ver) = Version::parse(local_s) {
                        if local_ver >= latest_ver {
                            println!(
                                "Already up to date (local: {}, latest: {}).",
                                local_ver, latest_ver
                            );
                            return Ok(());
                        }
                    }
                }

                // 4) Dry-run?
                if args.dry_run {
                    println!("[dry-run] local : {}", local.as_deref().unwrap_or("<none>"));
                    println!("[dry-run] latest: {}", latest_ver);
                    println!("[dry-run] url   : {}", latest.url);
                    return Ok(());
                }

                // 5) Download to cache
                let proj = ProjectDirs::from("dev", "Windman", "windman")
                    .ok_or_else(|| anyhow::anyhow!("cannot determine project dirs"))?;
                let dl_dir = proj.cache_dir().join("downloads").join(&latest.version);
                std::fs::create_dir_all(&dl_dir)?;
                let filename = latest
                .url
                .rsplit('/')
                .next()
                .unwrap_or("windsurf-linux-x64.tar.gz");
                let tar_path: PathBuf = dl_dir.join(filename);

                crate::download::download_to_file_with_timeout(&latest.url, &tar_path, None)
                    .map_err(|e| anyhow::anyhow!("downloading {}: {}", latest.url, e))?;
                println!("Downloaded {}", tar_path.display());

                // 6) Install
                let ver = install::install_from_tar(tar_path.to_string_lossy().as_ref(), &eff)?;
                println!("Installed Windsurf {} to {:?}", ver, eff.prefix_dir);

                // 7) Desktop
                let want_desktop = if args.no_desktop {
                    false
                } else {
                    args.desktop || cfg.install.desktop_integration
                };
                if want_desktop {
                    desktop::ensure_desktop_files(&eff)?;
                    println!("Desktop entry installed");
                }

                // 8) Prune
                prune::prune_old_versions(&eff, cfg.install.keep)?;
                Ok(())
            }

            Commands::Status => {
                let local = version::detect_local_version(&eff)?;
                println!("Install prefix : {}", eff.prefix_dir.display());
                println!("Current link   : {}", eff.current_symlink.display());
                println!("Shim           : {}", eff.bin_shim.display());
                match local {
                    Some(v) => println!("Local version  : {}", v),
                    None => println!("Local version  : <not installed>"),
                }
                Ok(())
            }

            Commands::Where => {
                println!("prefix : {}", eff.prefix_dir.display());
                println!("current: {}", eff.current_symlink.display());
                println!("shim   : {}", eff.bin_shim.display());
                Ok(())
            }

            Commands::List => {
                let entries = collect_installed(&eff);
            
                if entries.is_empty() {
                    println!("No installed versions found in {}.", eff.versions_dir.display());
                } else {
                    println!("Installed versions in {}:", eff.versions_dir.display());
                    for (name, is_current) in entries {
                        if is_current {
                            println!("* {}   (current)", name);
                        } else {
                            println!("  {}", name);
                        }
                    }
                }
                Ok(())
            }
            

            Commands::Changelog => {
                println!("Changelog delta not implemented yet (coming next).");
                Ok(())
            }

            Commands::Uninstall { purge } => {
                install::uninstall_all(&eff, *purge)?;
                println!("Windman userland install removed.");
                Ok(())
            }

            Commands::Rollback => {
                install::rollback(&eff)?;
                Ok(())
            }

            Commands::Config(sub) => match sub {
                ConfigCmd::Init => {
                    cfg.save_if_missing(&cfg_paths)?;
                    println!("Config written to {}", cfg_paths.config_display());
                    Ok(())
                }
                ConfigCmd::Show => {
                    println!("{}", toml::to_string_pretty(&cfg)?);
                    Ok(())
                }
            },

            Commands::DevLatest(args) => {
                use anyhow::Context;
                let timeout = args.timeout;

                if let Some(path) = &args.dump_html {
                    let html = crate::remote::fetch_releases_html(timeout)?;
                    std::fs::write(path, &html).with_context(|| format!("writing {}", path))?;
                    println!("Dumped releases HTML to {}", path);
                    return Ok(());
                }

                let info = crate::remote::latest_stable_linux_x64(timeout)?;
                println!("latest.version = {}", info.version);
                println!("latest.url     = {}", info.url);
                Ok(())
            }

            Commands::DevDownload(args) => {
                use std::path::Path;
                crate::download::download_to_file_with_timeout(
                    &args.url,
                    Path::new(&args.out),
                    args.timeout,
                )?;
                println!("Downloaded to {}", args.out);
                Ok(())
            }
        }
    }
}


#[cfg(test)]
mod tests_list_collect {
    use super::*;
    use tempfile::tempdir;
    use std::fs;

    #[test]
    fn collects_and_marks_current() {
        let tmp = tempdir().unwrap();
        let eff = crate::paths::EffectivePaths {
            prefix_dir: tmp.path().to_path_buf(),
            versions_dir: tmp.path().join("versions"),
            current_symlink: tmp.path().join("current"),
            bin_dir: tmp.path().join("bin"),
            bin_shim: tmp.path().join("bin/windsurf"),
            desktop_file: tmp.path().join("share/applications/windsurf.desktop"),
            icons_dir: tmp.path().join("share/icons"),
        };
        fs::create_dir_all(&eff.versions_dir.join("1.12.9")).unwrap();
        fs::create_dir_all(&eff.versions_dir.join("1.12.11")).unwrap();
        std::os::unix::fs::symlink(&eff.versions_dir.join("1.12.9"), &eff.current_symlink).unwrap();

        let got = collect_installed(&eff);
        // tri semver desc => 1.12.11, 1.12.9
        assert_eq!(got[0].0, "1.12.11");
        assert_eq!(got[1].0, "1.12.9");
        assert!(got.iter().find(|(n, _)| n == "1.12.9").unwrap().1);
    }
}
