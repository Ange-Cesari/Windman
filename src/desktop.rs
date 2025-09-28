use crate::paths::EffectivePaths;
use anyhow::Result;
use std::fs;

pub fn ensure_desktop_files(eff: &EffectivePaths) -> Result<()> {
    if !eff.icons_dir.exists() {
        fs::create_dir_all(&eff.icons_dir)?;
    }

    // icon is optional; users may add their own. We just ensure the dir exists.
    // If you want to install an icon file, write it to eff.icons_dir.join("windsurf.png").

    let desktop_dir = eff.desktop_file.parent().unwrap();
    if !desktop_dir.exists() {
        fs::create_dir_all(desktop_dir)?;
    }

    let exec_path = eff.current_symlink.join("Windsurf");
    let content = format!(
        "[Desktop Entry]\nName=Windsurf\nComment=AI IDE by Codeium\nExec={} %U\nTerminal=false\nType=Application\nIcon=windsurf\nCategories=Development;IDE;\nStartupWMClass=Windsurf\n",
        exec_path.display()
    );
    fs::write(&eff.desktop_file, content)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::paths::EffectivePaths;
    use std::{fs, os::unix::fs::symlink};
    use tempfile::tempdir;

    #[test]
    fn ensure_desktop_files_writes_entry_pointing_to_current() {
        let td = tempdir().unwrap();
        let base = td.path();

        // Fake version dir with a fake "Windsurf" executable
        let vdir = base.join("1.2.3");
        fs::create_dir_all(&vdir).unwrap();
        fs::write(vdir.join("Windsurf"), b"#!/bin/sh\necho ws\n").unwrap();

        // current -> vdir
        let current = base.join("current");
        symlink(&vdir, &current).unwrap();

        // EffectivePaths au sein du tempdir
        let eff = EffectivePaths {
            prefix_dir: base.to_path_buf(),
            versions_dir: base.to_path_buf(),
            current_symlink: current.clone(),
            bin_dir: base.join("bin"),
            bin_shim: base.join("bin/windsurf"),
            desktop_file: base.join("share/applications/windsurf.desktop"),
            icons_dir: base.join("share/icons/hicolor/512x512/apps"),
        };

        super::ensure_desktop_files(&eff).unwrap();
        let desktop = fs::read_to_string(&eff.desktop_file).unwrap();
        assert!(desktop.contains("Name=Windsurf"));
        assert!(desktop.contains("Exec="));
        assert!(desktop.contains("Icon=windsurf"));
        // L'Exec doit pointer vers current/Windsurf
        let exec_line = desktop.lines().find(|l| l.starts_with("Exec=")).unwrap();
        assert!(exec_line.contains("current/Windsurf"));
    }
}
