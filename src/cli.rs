
use clap::{Parser, Subcommand, Args};
use anyhow::{Result, bail};

use crate::config::{Config, ConfigPaths};
use crate::paths::{resolve_paths};
use crate::install;
use crate::version;
use crate::desktop;
use crate::prune;

#[derive(Parser, Debug)]
#[command(name = "windman", version, about = "Windsurf Manager (userland, standalone)")]
pub struct Cli {
    /// Override config path
    #[arg(long, global = true, env = "WINDMAN_CONFIG_PATH")]
    pub config: Option<String>,

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
    /// Compare local vs remote and update if needed (coming soon)
    Update(UpdateArgs),
    /// Show local version and paths
    Status,
    /// Print install and shim paths
    Where,
    /// Show changelog delta (coming soon)
    Changelog,
    /// Remove installs and shims (keeps user data)
    Uninstall { #[arg(long)] purge: bool },
    /// Switch back to previous kept version
    Rollback,
    /// Manage configuration
    
    #[command(subcommand)]
    Config(ConfigCmd),
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

impl Cli {
    pub fn parse() -> Self { <Cli as Parser>::parse() }

    pub fn run(&self) -> Result<()> {
        let cfg_paths = ConfigPaths::from_override(self.config.as_deref());
        let cfg = Config::load_or_default(&cfg_paths)?;
        let eff = resolve_paths(&cfg)?;

        if self.verbose { eprintln!("[windman] Using config at {}", cfg_paths.config_display()); }

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
                    let want_desktop = if args.no_desktop { false } else { args.desktop || cfg.install.desktop_integration };
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
            Commands::Update(_args) => {
                // Placeholder: will implement remote query + delta changelog + install
                println!("Update not implemented yet (coming next).");
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
        }
    }
}