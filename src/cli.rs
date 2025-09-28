use crate::config::{Config, ConfigPaths};
use crate::paths::resolve_paths;
use crate::{desktop, install, prune, version};
use anyhow::{bail, Result};
use clap::{Args, Parser, Subcommand};
use std::fs;
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

    /// Switch current to a specific installed version (e.g., windman use 1.12.11)
    Use(UseArgs),

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
pub struct UseArgs {
    /// Version folder name to activate (e.g., 1.12.11)
    #[arg(value_name = "VERSION")]
    pub version: String,

    /// Dry-run: show what would change without touching the system
    #[arg(long)]
    pub dry_run: bool,
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

                if name == "current" {
                    continue;
                }
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

pub(crate) fn switch_to_version(
    eff: &crate::paths::EffectivePaths,
    version: &str,
) -> anyhow::Result<()> {
    let target = eff.versions_dir.join(version);
    if !target.is_dir() {
        // Préparer un message d’erreur utile avec les versions dispo
        let mut available = Vec::new();
        if eff.versions_dir.exists() {
            if let Ok(rd) = fs::read_dir(&eff.versions_dir) {
                for ent in rd.flatten() {
                    let p = ent.path();
                    if p.is_dir() {
                        if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
                            if name != "current" {
                                available.push(name.to_string());
                            }
                        }
                    }
                }
            }
        }
        available.sort();
        anyhow::bail!(
            "version '{}' not found under {}.\nAvailable: {}",
            version,
            eff.versions_dir.display(),
            if available.is_empty() {
                "<none>".to_string()
            } else {
                available.join(", ")
            }
        );
    }

    // Si current pointe déjà sur cette version, rien à faire
    if let Ok(cur) = fs::read_link(&eff.current_symlink) {
        if cur == target {
            println!("Already using {}.", version);
            return Ok(());
        }
    }

    crate::util::atomic_symlink_switch(&target, &eff.current_symlink)?;
    println!("Now using {}.", version);
    Ok(())
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
                use std::path::PathBuf;

                let keep = args.keep.unwrap_or(cfg.install.keep);
                if args.dry_run {
                    println!("[dry-run] would install to {:?}", eff.prefix_dir);
                    println!("[dry-run] would maintain shim at {:?}", eff.bin_shim);
                    return Ok(());
                }

                if let Some(tar) = &args.tar {
                    // mémoriser la current avant bascule
                    let previous_current: Option<PathBuf> =
                        std::fs::read_link(&eff.current_symlink).ok();

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

                    // Prune: préserver la nouvelle current + l'ancienne current
                    let mut preserve: Vec<PathBuf> = Vec::new();
                    if let Ok(cur) = std::fs::read_link(&eff.current_symlink) {
                        preserve.push(cur);
                    }
                    if let Some(prev) = previous_current {
                        preserve.push(prev);
                    }
                    prune::prune_old_versions_with_preserve(&eff.versions_dir, keep, &preserve)?;
                    Ok(())
                } else {
                    bail!("--tar <FILE> is required for now. Network download will be added next.")
                }
            }

            Commands::Use(args) => {
                if args.dry_run {
                    let target = eff.versions_dir.join(&args.version);
                    println!("[dry-run] would switch current -> {}", target.display());
                    return Ok(());
                }
                switch_to_version(&eff, &args.version)?;
                // le shim pointe déjà vers 'current', donc rien à régénérer
                Ok(())
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

                // 6) Install (avec capture de l'ancienne current pour prune)
                let previous_current: Option<PathBuf> =
                    std::fs::read_link(&eff.current_symlink).ok();

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

                // 8) Prune: préserver la nouvelle current + l'ancienne current
                let mut preserve: Vec<PathBuf> = Vec::new();
                if let Ok(cur) = std::fs::read_link(&eff.current_symlink) {
                    preserve.push(cur);
                }
                if let Some(prev) = previous_current {
                    preserve.push(prev);
                }
                prune::prune_old_versions_with_preserve(
                    &eff.versions_dir,
                    cfg.install.keep,
                    &preserve,
                )?;
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
                    println!(
                        "No installed versions found in {}.",
                        eff.versions_dir.display()
                    );
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
    use std::fs;
    use tempfile::tempdir;

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

#[cfg(test)]
mod tests_use_switch {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn switches_current_symlink_to_requested_version() {
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
        fs::create_dir_all(eff.versions_dir.join("1.12.10")).unwrap();
        fs::create_dir_all(eff.versions_dir.join("1.12.11")).unwrap();

        // current -> 1.12.10
        std::os::unix::fs::symlink(eff.versions_dir.join("1.12.10"), &eff.current_symlink).unwrap();

        // switch
        switch_to_version(&eff, "1.12.11").unwrap();
        let cur = fs::read_link(&eff.current_symlink).unwrap();
        assert_eq!(cur.file_name().unwrap().to_string_lossy(), "1.12.11");
    }

    #[test]
    fn errors_if_version_missing_with_available_list() {
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
        fs::create_dir_all(eff.versions_dir.join("1.12.11")).unwrap();

        let err = switch_to_version(&eff, "1.12.9").unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("not found"));
        assert!(msg.contains("1.12.11"));
    }
}
