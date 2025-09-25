
use anyhow::Result;
use std::{fs, path::PathBuf};
use crate::paths::EffectivePaths;

pub fn prune_old_versions(eff: &EffectivePaths, keep: usize) -> Result<()> {
    if keep == 0 { return Ok(()); }
    if !eff.versions_dir.exists() { return Ok(()); }

    let mut entries: Vec<(std::time::SystemTime, PathBuf)> = vec![];
    for e in fs::read_dir(&eff.versions_dir)? {
        let e = e?;
        if e.file_name() == "current" { continue; }
        if e.file_type()?.is_dir() {
            let meta = e.metadata()?;
            let m = meta.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH);
            entries.push((m, e.path()));
        }
    }
    entries.sort_by_key(|(t, _)| std::cmp::Reverse(*t));
    if entries.len() <= keep { return Ok(()); }

    for (_, path) in entries.into_iter().skip(keep) {
        // avoid deleting the target of current symlink
        if let Ok(target) = std::fs::read_link(&eff.current_symlink) {
            if target == path { continue; }
        }
        std::fs::remove_dir_all(&path)?;
    }
    Ok(())
}
