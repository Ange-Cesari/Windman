use anyhow::Result;

mod cli;
mod config;
mod desktop;
mod download;
mod install;
mod paths;
mod prune;
mod remote;
mod util;
mod version;

use cli::Cli;

fn main() -> Result<()> {
    let cli = Cli::parse();
    cli.run()
}
