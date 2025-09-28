use anyhow::{bail, Context, Result};
use flate2::read::GzDecoder;
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use tar::Archive;

use crate::paths::EffectivePaths;
use crate::util::{atomic_symlink_switch, timestamp_version, write_shim};

/// Install from a .tar.gz path. Returns the resolved version string used.
pub fn install_from_tar(tar_path: &str, eff: &EffectivePaths) -> Result<String> {
    fs::create_dir_all(&eff.versions_dir)
        .with_context(|| format!("creating {}", eff.versions_dir.display()))?;

    // Staging dir (atomic move later)
    let staging = eff
        .versions_dir
        .join(format!(".staging-{}", timestamp_version()));
    if staging.exists() {
        fs::remove_dir_all(&staging).ok();
    }
    fs::create_dir_all(&staging)?;

    // Extract tar.gz
    extract_tar_to_dir(tar_path, &staging)?;

    // Determine version:
    // 1) from tar filename
    let ver_from_filename = extract_version_from_filename(tar_path);

    // 2) from product.json if present
    let ver_from_product = detect_version_from_product_json(&staging).ok();

    // 3) choose dir name
    let version = ver_from_filename
    .or(ver_from_product)
    .unwrap_or_else(timestamp_version);

    let final_dir = eff.versions_dir.join(&version);

    // If target exists already, remove it before rename (overwrite)
    if final_dir.exists() {
        fs::remove_dir_all(&final_dir)
            .with_context(|| format!("removing pre-existing {}", final_dir.display()))?;
    }

    // Move staging -> final
    fs::rename(&staging, &final_dir)
        .with_context(|| format!("moving {} -> {}", staging.display(), final_dir.display()))?;

    // Update 'current' symlink atomically
    atomic_symlink_switch(&final_dir, &eff.current_symlink)?;

    // Ensure bin dir exists and write shim
    fs::create_dir_all(&eff.bin_dir)?;
    write_shim(&eff.bin_shim, &eff.current_symlink)?;

    Ok(version)
}

pub fn rollback(eff: &EffectivePaths) -> Result<()> {
    use std::fs;
    // List versions
    let mut dirs = list_version_dirs(&eff.versions_dir)?;
    if dirs.len() < 2 {
        bail!("no previous version to roll back to");
    }
    // current target
    let cur_target = fs::read_link(&eff.current_symlink)
        .with_context(|| format!("reading {}", eff.current_symlink.display()))?;
    // Remove current from list
    dirs.retain(|p| p != &cur_target);
    // Pick the most recent by mtime
    dirs.sort_by_key(|p| fs::metadata(p).and_then(|m| m.modified()).ok());
    let prev = dirs.pop().unwrap();
    atomic_symlink_switch(&prev, &eff.current_symlink)?;
    println!(
        "Rolled back to {}",
        prev.file_name().unwrap().to_string_lossy()
    );
    Ok(())
}

pub fn uninstall_all(eff: &EffectivePaths, purge: bool) -> Result<()> {
    // Remove symlink & shim
    let _ = fs::remove_file(&eff.current_symlink);
    let _ = fs::remove_file(&eff.bin_shim);

    // Remove versions dir
    if eff.versions_dir.exists() {
        fs::remove_dir_all(&eff.versions_dir)?;
    }

    if purge {
        // Remove desktop files, icons etc. (best-effort)
        let _ = fs::remove_file(&eff.desktop_file);
        if eff.icons_dir.exists() {
            let _ = fs::remove_dir_all(&eff.icons_dir);
        }
    }
    Ok(())
}

// ---------------- helpers ----------------

fn extract_tar_to_dir(tar_path: &str, dest: &Path) -> Result<()> {
    let file = File::open(tar_path).with_context(|| format!("opening {}", tar_path))?;
    let dec = GzDecoder::new(file);
    let mut ar = Archive::new(dec);
    ar.unpack(dest)
        .with_context(|| format!("extracting to {}", dest.display()))?;
    Ok(())
}

fn list_version_dirs(base: &Path) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    if base.exists() {
        for ent in fs::read_dir(base)? {
            let ent = ent?;
            let p = ent.path();
            if p.file_name().map(|n| n == "current").unwrap_or(false) {
                continue;
            }
            if p.is_dir() {
                out.push(p);
            }
        }
    }
    Ok(out)
}

fn extract_version_from_filename(path: &str) -> Option<String> {
    let name = std::path::Path::new(path).file_name()?.to_string_lossy();
    let re = regex::Regex::new(r"(\d+\.\d+\.\d+)").ok()?;
    let caps = re.captures(&name)?;
    Some(caps.get(1)?.as_str().to_string())
}

fn detect_version_from_product_json(extracted_root: &Path) -> Result<String> {
    // Find product.json under **/resources/app/product.json (common layout)
    // We'll scan one level or two deep to be safe.
    let mut candidate: Option<PathBuf> = None;

    // Typical layout: extracted_root/<some-root>/resources/app/product.json
    for entry in walkdir::WalkDir::new(extracted_root)
        .min_depth(1)
        .max_depth(4)
    {
        let entry = entry?;
        let p = entry.path();
        if p.file_name().map(|n| n == "product.json").unwrap_or(false) {
            candidate = Some(p.to_path_buf());
            break;
        }
    }

    let p = candidate.ok_or_else(|| anyhow::anyhow!("product.json not found"))?;
    let data = std::fs::read_to_string(&p).with_context(|| format!("reading {}", p.display()))?;
    let v = parse_windsurf_version_from_product(&data)
        .ok_or_else(|| anyhow::anyhow!("windsurfVersion not found in product.json"))?;
    Ok(v)
}

fn parse_windsurf_version_from_product(s: &str) -> Option<String> {
    let v = json_extract_field(s, "windsurfVersion")?;
    Some(v)
}

/// extract a top-level string field from a JSON blob without full serde (lenient)
fn json_extract_field(s: &str, field: &str) -> Option<String> {
    // very lenient regex just to fish out "field":"value"
    let re = regex::Regex::new(&format!(r#""{}\s*"\s*:\s*"(.*?)""#, regex::escape(field))).ok()?;
    let caps = re.captures(s)?;
    Some(caps.get(1)?.as_str().to_string())
}

#[cfg(test)]
mod tests_install_from_tar_names_dir_by_version {
    use super::*;
    use tempfile::tempdir;
    use std::path::Path;

    // Crée un tar.gz minimal: Windsurf/bin/windsurf + Windsurf/resources/app/product.json
    fn make_fake_windsurf_tar(path: &Path, version: &str) {
        let tarfile = File::create(path).unwrap();
        let enc = flate2::write::GzEncoder::new(tarfile, flate2::Compression::default());
        let mut builder = tar::Builder::new(enc);

        let bin_path = "Windsurf/bin/windsurf";
        let prod_path = "Windsurf/resources/app/product.json";

        let mut bin_hdr = tar::Header::new_gnu();
        bin_hdr.set_path(bin_path).unwrap();
        bin_hdr.set_mode(0o755);
        bin_hdr.set_size(2);
        bin_hdr.set_cksum();
        builder.append(&bin_hdr, &b"#!"[..]).unwrap();

        let prod_json = format!(r#"{{ "windsurfVersion":"{}" }}"#, version);
        let mut prod_hdr = tar::Header::new_gnu();
        prod_hdr.set_path(prod_path).unwrap();
        prod_hdr.set_mode(0o644);
        prod_hdr.set_size(prod_json.len() as u64);
        prod_hdr.set_cksum();
        builder.append(&prod_hdr, prod_json.as_bytes()).unwrap();

        builder.finish().unwrap();
    }

    #[test]
    fn install_uses_semver_dir_and_updates_current() {
        let tmp = tempdir().unwrap();
        let prefix = tmp.path().to_path_buf();

        // EffectivePaths minimal vers tmp
        let eff = crate::paths::EffectivePaths {
            prefix_dir: prefix.clone(),
            versions_dir: prefix.join("versions"),
            current_symlink: prefix.join("current"),
            bin_dir: prefix.join("bin"),
            bin_shim: prefix.join("bin/windsurf"),
            desktop_file: prefix.join("share/applications/windsurf.desktop"),
            icons_dir: prefix.join("share/icons"),
        };

        std::fs::create_dir_all(&eff.versions_dir).unwrap();

        let tar_path = tmp.path().join("Windsurf-linux-x64-2.3.4.tar.gz");
        make_fake_windsurf_tar(&tar_path, "2.3.4");

        let ver = super::install_from_tar(tar_path.to_string_lossy().as_ref(), &eff).unwrap();
        assert_eq!(ver, "2.3.4");

        // current -> .../2.3.4
        let cur = std::fs::read_link(&eff.current_symlink).unwrap();
        assert_eq!(cur.file_name().unwrap().to_string_lossy(), "2.3.4");

        // Shim créé
        assert!(eff.bin_shim.is_file());
    }
}
