use anyhow::{Context, Result};
use serde_json::Value;
use std::{fs, path::PathBuf};

use crate::paths::EffectivePaths;

/// Detect the local Windsurf version by resolving the 'current' symlink
/// and scanning for a 'product.json' (up to a few levels deep).
/// Returns Some("x.y.z") or Ok(None) if not installed.
pub fn detect_local_version(eff: &EffectivePaths) -> Result<Option<String>> {
    // 1) Ensure the 'current' symlink exists
    if !eff.current_symlink.exists() {
        return Ok(None);
    }

    // 2) Resolve the symlink target (folder of the active install)
    let current_target =
        fs::read_link(&eff.current_symlink).unwrap_or_else(|_| eff.current_symlink.clone());

    // 3) Search for product.json within the active folder
    if let Some(product_path) = find_product_json(&current_target) {
        let data = fs::read_to_string(&product_path)
            .with_context(|| format!("reading {}", product_path.display()))?;

        // Try JSON parse first
        if let Ok(json) = serde_json::from_str::<Value>(&data) {
            // Prefer "windsurfVersion", else "version"
            if let Some(v) = json.get("windsurfVersion").and_then(|x| x.as_str()) {
                return Ok(Some(v.to_string()));
            }
            if let Some(v) = json.get("version").and_then(|x| x.as_str()) {
                return Ok(Some(v.to_string()));
            }
        }

        // Fallback: regex fish-out if JSON had comments or odd format
        if let Some(v) = regex_fish_version(&data) {
            return Ok(Some(v));
        }

        // If product.json exists but we couldn't parse a version, treat as not installed
        return Ok(None);
    }

    // No product.json found → not installed (or layout unexpected)
    Ok(None)
}

/// Walk a few levels to find .../resources/app/product.json under current target.
/// Typical paths:
///   <root>/Windsurf/resources/app/product.json
///   <root>/resources/app/product.json
fn find_product_json(root: &PathBuf) -> Option<PathBuf> {
    // Minimal scanning to avoid heavy WalkDir; we know common patterns.
    let candidates = [
        root.join("resources/app/product.json"),
        root.join("Windsurf/resources/app/product.json"),
        root.join("app/resources/product.json"), // just-in-case variants
        root.join("resources/product.json"),
    ];

    for p in candidates {
        if p.is_file() {
            return Some(p);
        }
    }

    // As a last resort, do a shallow walk (depth ≤ 4)
    // Requires walkdir in Cargo.toml (already present in your project)
    for entry in walkdir::WalkDir::new(root).max_depth(4) {
        let entry = entry.ok()?;
        let p = entry.path();
        if p.file_name().map(|n| n == "product.json").unwrap_or(false) && p.is_file() {
            return Some(p.to_path_buf());
        }
    }

    None
}

fn regex_fish_version(s: &str) -> Option<String> {
    // look for "windsurfVersion":"x.y.z" or "version":"x.y.z"
    let re = regex::Regex::new(r#""(?:windsurfVersion|version)"\s*:\s*"(\d+\.\d+\.\d+)""#).ok()?;
    let caps = re.captures(s)?;
    Some(caps.get(1)?.as_str().to_string())
}

#[cfg(test)]
mod tests_detect_version_layout_linux {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn detects_version_under_windsurf_resources_app() {
        let tmp = tempdir().unwrap();
        // Simule: <prefix>/versions/<ver>/Windsurf/resources/app/product.json
        let root = tmp.path().join("versions");
        let ver_dir = root.join("1.2.3");
        let prod = ver_dir.join("Windsurf/resources/app");
        fs::create_dir_all(&prod).unwrap();
        fs::write(prod.join("product.json"), r#"{ "windsurfVersion":"1.2.3" }"#).unwrap();

        // current -> <ver_dir>
        let current = tmp.path().join("current");
        std::os::unix::fs::symlink(&ver_dir, &current).unwrap();

        // EffectivePaths minimal
        let eff = crate::paths::EffectivePaths {
            prefix_dir: tmp.path().to_path_buf(),
            versions_dir: root.clone(),
            current_symlink: current.clone(),
            bin_dir: tmp.path().join("bin"),
            bin_shim: tmp.path().join("bin/windsurf"),
            desktop_file: tmp.path().join("share/applications/windsurf.desktop"),
            icons_dir: tmp.path().join("share/icons"),
        };

        let v = detect_local_version(&eff).unwrap();
        assert_eq!(v.as_deref(), Some("1.2.3"));
    }
}
