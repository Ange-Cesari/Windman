
use anyhow::{Result, Context};
use std::fs;
use serde::Deserialize;

#[derive(Deserialize)]
struct ProductJson { #[serde(rename = "windsurfVersion")] version: Option<String> }

pub fn detect_local_version(eff: &crate::paths::EffectivePaths) -> Result<Option<String>> {
    let curr = &eff.current_symlink;
    if !curr.exists() { return Ok(None); }
    let pj = curr.join("resources/app/product.json");
    if !pj.exists() { return Ok(None); }
    let s = fs::read_to_string(&pj)
        .with_context(|| format!("reading {}", pj.display()))?;
    let p: ProductJson = serde_json::from_str(&s)?;
    Ok(p.version)
}