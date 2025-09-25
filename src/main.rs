
use anyhow::Result;

mod cli;
mod config;
mod paths;
mod version;
mod install;
mod download;
mod desktop;
mod prune;
mod util;

use cli::Cli;

fn main() -> Result<()> {
    let cli = Cli::parse();
    cli.run()
}