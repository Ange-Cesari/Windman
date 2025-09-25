
use anyhow::{Result, Context};
use std::{fs, os::unix::fs::symlink, path::Path};

/// Best-effort extraction of a version-like name from folder path
pub fn guess_version_from_folder(path: &Path) -> Option<String> {
    let name = path.file_name()?.to_string_lossy();
    // naive: pick first token that looks like digits and dots
    for chunk in name.split(|c: char| !c.is_ascii_alphanumeric() && c != '.') {
        if chunk.chars().all(|c| c.is_ascii_digit() || c == '.') && chunk.contains('.') {
            return Some(chunk.to_string());
        }
    }
    None
}

pub fn timestamp_version() -> String {
    let t = chrono::Utc::now();
    t.format("%Y%m%d%H%M%S").to_string()
}

pub fn atomic_symlink_switch(target: &Path, link: &Path) -> Result<()> {
    let parent = link.parent().unwrap();
    if !parent.exists() { fs::create_dir_all(parent)?; }
    let tmp = parent.join(format!(".tmp-{}", std::process::id()));
    if tmp.exists() { let _ = fs::remove_file(&tmp); }
    symlink(target, &tmp).context("creating temp symlink")?;
    if link.exists() { fs::remove_file(link).ok(); }
    fs::rename(&tmp, link).context("renaming temp symlink into place")?;
    Ok(())
}

pub fn write_shim(shim_path: &Path, exec_path: &Path) -> Result<()> {
    if let Some(parent) = shim_path.parent() { if !parent.exists() { fs::create_dir_all(parent)?; } }
    let content = format!("#!/bin/sh\nexec \"{}\" \"$@\"\n", exec_path.display());
    fs::write(shim_path, content)?;
    let mut perms = fs::metadata(shim_path)?.permissions();
    #[cfg(unix)] {
        use std::os::unix::fs::PermissionsExt;
        perms.set_mode(0o755);
        fs::set_permissions(shim_path, perms)?;
    }
    Ok(())
}