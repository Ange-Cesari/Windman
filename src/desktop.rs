
use anyhow::Result;
use std::fs;
use crate::paths::EffectivePaths;

pub fn ensure_desktop_files(eff: &EffectivePaths) -> Result<()> {
    if !eff.icons_dir.exists() { fs::create_dir_all(&eff.icons_dir)?; }

    // icon is optional; users may add their own. We just ensure the dir exists.
    // If you want to install an icon file, write it to eff.icons_dir.join("windsurf.png").

    let desktop_dir = eff.desktop_file.parent().unwrap();
    if !desktop_dir.exists() { fs::create_dir_all(desktop_dir)?; }

    let exec_path = eff.current_symlink.join("Windsurf");
    let content = format!(
        "[Desktop Entry]\nName=Windsurf\nComment=AI IDE by Codeium\nExec={} %U\nTerminal=false\nType=Application\nIcon=windsurf\nCategories=Development;IDE;\nStartupWMClass=Windsurf\n",
        exec_path.display()
    );
    fs::write(&eff.desktop_file, content)?;
    Ok(())
}
