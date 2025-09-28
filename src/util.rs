use anyhow::{Context, Result};
use std::{fs, os::unix::fs::symlink, path::Path};
use std::{io::Write, os::unix::fs::PermissionsExt};

pub fn timestamp_version() -> String {
    let t = chrono::Utc::now();
    t.format("%Y%m%d%H%M%S").to_string()
}

pub fn atomic_symlink_switch(target: &Path, link: &Path) -> Result<()> {
    let parent = link.parent().unwrap();
    if !parent.exists() {
        fs::create_dir_all(parent)?;
    }
    let tmp = parent.join(format!(".tmp-{}", std::process::id()));
    if tmp.exists() {
        let _ = fs::remove_file(&tmp);
    }
    symlink(target, &tmp).context("creating temp symlink")?;
    if link.exists() {
        fs::remove_file(link).ok();
    }
    fs::rename(&tmp, link).context("renaming temp symlink into place")?;
    Ok(())
}

pub fn write_shim(shim_path: &Path, current_symlink: &Path) -> Result<()> {
    let current_str = current_symlink.display().to_string();

    let script = format!(
        r#"#!/usr/bin/env bash
set -euo pipefail
CURRENT_LINK="{current}"
ROOT="$(readlink -f "$CURRENT_LINK")"
exe="$ROOT/Windsurf/bin/windsurf"
if [ -x "$exe" ]; then
  exec "$exe" "$@"
fi
# very small fallback
if [ -x "$ROOT/windsurf" ]; then
  exec "$ROOT/windsurf" "$@"
fi
echo "windman: could not locate Windsurf executable at: $exe" >&2
exit 127
"#,
        current = current_str
    );

    if let Some(dir) = shim_path.parent() {
        fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }
    let mut f =
        fs::File::create(shim_path).with_context(|| format!("creating {}", shim_path.display()))?;
    f.write_all(script.as_bytes())
        .with_context(|| format!("writing {}", shim_path.display()))?;
    drop(f);

    let mut perms = fs::metadata(shim_path)?.permissions();
    perms.set_mode(0o755);
    fs::set_permissions(shim_path, perms)?;
    Ok(())
}

/// Best-effort extraction of a version-like name from folder path
#[cfg(test)]
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

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn guess_version_from_folder_extracts_semverish() {
        let p = Path::new("/tmp/Windsurf-1.12.9-linux-x64");
        assert_eq!(guess_version_from_folder(p).as_deref(), Some("1.12.9"));

        let p = Path::new("/tmp/ws-20240922");
        assert!(guess_version_from_folder(p).is_none());
    }

    #[test]
    fn timestamp_version_has_expected_length() {
        let v = timestamp_version();
        // e.g. "20250927153322" -> 14 chars
        assert_eq!(v.len(), 14);
        assert!(v.chars().all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn atomic_symlink_switch_works() {
        let td = tempdir().unwrap();
        let base = td.path();
        let v1 = base.join("1.0.0");
        fs::create_dir_all(&v1).unwrap();
        let v2 = base.join("1.0.1");
        fs::create_dir_all(&v2).unwrap();
        let link = base.join("current");

        // switch -> v1
        atomic_symlink_switch(&v1, &link).unwrap();
        assert_eq!(fs::read_link(&link).unwrap(), v1);

        // switch -> v2
        atomic_symlink_switch(&v2, &link).unwrap();
        assert_eq!(fs::read_link(&link).unwrap(), v2);
    }

    #[test]
    fn write_shim_creates_executable_script() {
        let td = tempdir().unwrap();
        let shim = td.path().join("bin").join("windsurf");
        let exec = td.path().join("Windsurf");
        fs::create_dir_all(shim.parent().unwrap()).unwrap();
        fs::write(&exec, b"#!/bin/sh\necho hi\n").unwrap();

        write_shim(&shim, &exec).unwrap();
        let content = fs::read_to_string(&shim).unwrap();
        assert!(content.contains(exec.to_string_lossy().as_ref()));

        #[cfg(unix)]
        {
            let mode = fs::metadata(&shim).unwrap().permissions().mode();
            assert!(mode & 0o111 != 0, "shim should be executable");
        }
    }
}
