use crate::paths::EffectivePaths;
use anyhow::Result;
use std::{fs, path::PathBuf};

pub fn prune_old_versions(eff: &EffectivePaths, keep: usize) -> Result<()> {
    if keep == 0 {
        return Ok(());
    }
    if !eff.versions_dir.exists() {
        return Ok(());
    }

    let mut entries: Vec<(std::time::SystemTime, PathBuf)> = vec![];
    for e in fs::read_dir(&eff.versions_dir)? {
        let e = e?;
        if e.file_name() == "current" {
            continue;
        }
        if e.file_type()?.is_dir() {
            let meta = e.metadata()?;
            let m = meta.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH);
            entries.push((m, e.path()));
        }
    }
    entries.sort_by_key(|(t, _)| std::cmp::Reverse(*t));
    if entries.len() <= keep {
        return Ok(());
    }

    for (_, path) in entries.into_iter().skip(keep) {
        // avoid deleting the target of current symlink
        if let Ok(target) = std::fs::read_link(&eff.current_symlink) {
            if target == path {
                continue;
            }
        }
        std::fs::remove_dir_all(&path)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::EffectivePaths;
    use std::os::unix::fs::symlink;
    use std::{fs, thread, time::Duration};
    use tempfile::tempdir;

    #[test]
    fn prune_keeps_n_newest_and_preserves_current() {
        let td = tempdir().unwrap();
        let root = td.path().to_path_buf();
        fs::create_dir_all(&root).unwrap();

        // Crée 3 versions avec mtime différents (petites pauses)
        let v1 = root.join("1.0.0");
        fs::create_dir_all(&v1).unwrap();
        thread::sleep(Duration::from_millis(10));
        let v2 = root.join("1.0.1");
        fs::create_dir_all(&v2).unwrap();
        thread::sleep(Duration::from_millis(10));
        let v3 = root.join("1.0.2");
        fs::create_dir_all(&v3).unwrap();

        let current = root.join("current");
        symlink(&v3, &current).unwrap();

        let eff = EffectivePaths {
            prefix_dir: root.clone(),
            versions_dir: root.clone(),
            current_symlink: current.clone(),
            bin_dir: root.join("bin"),
            bin_shim: root.join("bin/windsurf"),
            desktop_file: root.join("windsurf.desktop"),
            icons_dir: root.join("icons"),
        };

        // Garder 2 versions
        prune_old_versions(&eff, 2).unwrap();

        // v3 (current) doit exister; v2 doit rester (2 plus récentes); v1 supprimée
        assert!(v3.exists(), "latest should remain");
        assert!(v2.exists(), "second latest should remain");
        assert!(!v1.exists(), "oldest should be pruned");
    }
}
