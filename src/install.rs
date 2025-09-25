
use anyhow::{Result, Context, bail};
use std::{fs, path::{Path, PathBuf}};
use crate::paths::EffectivePaths;
use crate::util;
use flate2::read::GzDecoder;
use tar::Archive;

/// Install from a local tar.gz (temporary helper until network fetch is added).
/// Returns detected version string (directory name under prefix).
pub fn install_from_tar(tar_path: &str, eff: &EffectivePaths) -> Result<String> {
    let tar_path = Path::new(tar_path);
    if !tar_path.exists() { bail!("tar file not found: {}", tar_path.display()); }

    fs::create_dir_all(&eff.prefix_dir)?;
    fs::create_dir_all(&eff.bin_dir)?;

    // Extract to a temp dir inside prefix
    let temp_dir = tempfile::Builder::new().prefix("windman-extract-").tempdir_in(&eff.prefix_dir)?;
    let tar_gz = fs::File::open(&tar_path)?;
    let dec = GzDecoder::new(tar_gz);
    let mut ar = Archive::new(dec);
    ar.unpack(temp_dir.path()).context("extracting tar.gz")?;

    // Heuristic: most releases unpack to a top-level directory. Find it.
    let mut top = None;
    for entry in fs::read_dir(temp_dir.path())? { let e = entry?; if e.file_type()?.is_dir() { top = Some(e.path()); break; } }
    let top = top.unwrap_or_else(|| temp_dir.path().to_path_buf());

    // Determine version directory name. If the folder name contains a version-like token, use it; else timestamp.
    let version = util::guess_version_from_folder(&top).unwrap_or_else(|| util::timestamp_version());
    let target = eff.versions_dir.join(&version);
    if target.exists() { fs::remove_dir_all(&target)?; }
    fs::rename(&top, &target).context("moving extracted folder into versions dir")?;

    // Atomically switch current -> new version
    util::atomic_symlink_switch(&target, &eff.current_symlink)?;

    // Create/update shim
    util::write_shim(&eff.bin_shim, &eff.current_symlink.join("Windsurf"))?;

    Ok(version)
}

pub fn uninstall_all(eff: &EffectivePaths, purge: bool) -> Result<()> {
    // remove shim
    if eff.bin_shim.exists() { fs::remove_file(&eff.bin_shim)?; }
    // remove desktop entries
    if eff.desktop_file.exists() { fs::remove_file(&eff.desktop_file)?; }
    let icon_png = eff.icons_dir.join("windsurf.png");
    if icon_png.exists() { fs::remove_file(icon_png)?; }
    // remove versions dir
    if eff.prefix_dir.exists() { fs::remove_dir_all(&eff.prefix_dir)?; }

    if purge {
        // we deliberately do NOT remove user config/cache/data of the app here unless specified in future
    }
    Ok(())
}

pub fn rollback(eff: &EffectivePaths) -> Result<()> {
    // Find all version dirs, sort by mtime descending, select the second newest, switch.
    let mut dirs: Vec<(std::time::SystemTime, PathBuf)> = vec![];
    if !eff.versions_dir.exists() { bail!("no installs found"); }
    for entry in fs::read_dir(&eff.versions_dir)? {
        let e = entry?;
        if e.file_name() == "current" { continue; }
        if e.file_type()?.is_dir() {
            let meta = e.metadata()?;
            let mtime = meta.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH);
            dirs.push((mtime, e.path()));
        }
    }
    if dirs.len() < 2 { bail!("not enough versions to rollback"); }
    dirs.sort_by_key(|(t, _)| std::cmp::Reverse(*t));
    let target = &dirs[1].1;
    util::atomic_symlink_switch(target, &eff.current_symlink)?;
    println!("Rolled back to {}", target.file_name().unwrap().to_string_lossy());
    Ok(())
}