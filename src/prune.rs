use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};

/// Prune with an explicit list of paths to preserve (e.g., current and previous_current).
pub fn prune_old_versions_with_preserve(
    versions_dir: &Path,
    keep: usize,
    preserve: &[PathBuf],
) -> Result<()> {
    // Collect version directories (skip "current")
    let mut dirs: Vec<PathBuf> = Vec::new();
    if versions_dir.exists() {
        for ent in fs::read_dir(versions_dir)? {
            let ent = ent?;
            let p = ent.path();
            if p.file_name().map(|n| n == "current").unwrap_or(false) {
                continue;
            }
            if p.is_dir() {
                dirs.push(p);
            }
        }
    }

    // Sort by mtime desc (newest first)
    dirs.sort_by_key(|p| fs::metadata(p).and_then(|m| m.modified()).ok());
    dirs.reverse();

    // Keep the N newest + any path listed in 'preserve'
    let mut kept: Vec<PathBuf> = Vec::new();
    for d in dirs {
        let is_preserved = preserve.contains(&d);
        if kept.len() < keep || is_preserved {
            kept.push(d);
        } else {
            let _ = fs::remove_dir_all(&d);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;
    use std::{fs, thread, time::Duration};
    use tempfile::tempdir;

    #[test]
    fn prune_keeps_n_newest_and_preserves_current() {
        let td = tempdir().unwrap();
        let versions_dir = td.path().to_path_buf();
        fs::create_dir_all(&versions_dir).unwrap();

        // Crée 3 versions avec mtime différents
        let v1 = versions_dir.join("1.0.0");
        fs::create_dir_all(&v1).unwrap();
        thread::sleep(Duration::from_millis(10));

        let v2 = versions_dir.join("1.0.1");
        fs::create_dir_all(&v2).unwrap();
        thread::sleep(Duration::from_millis(10));

        let v3 = versions_dir.join("1.0.2");
        fs::create_dir_all(&v3).unwrap();

        // current -> v3
        let current = versions_dir.join("current");
        symlink(&v3, &current).unwrap();

        // Préserver la current (v3) et garder N=2 versions au total
        prune_old_versions_with_preserve(&versions_dir, 2, &[v3.clone()]).unwrap();

        // v3 (current) doit exister; v2 doit rester (2 plus récentes); v1 supprimée
        assert!(v3.exists(), "latest (and current) should remain");
        assert!(v2.exists(), "second latest should remain");
        assert!(!v1.exists(), "oldest should be pruned");
    }
}
