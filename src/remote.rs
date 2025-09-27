use anyhow::{Result, Context};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct LatestInfo {
    pub version: String,
    pub url: String, // download URL for linux-x64 tar.gz
}

pub fn latest_stable_linux_x64() -> Result<LatestInfo> {
    // 1) Appel GET vers l’endpoint JSON (stable)
    // 2) Désérialisation en LatestInfo
    // 3) Retourne { version, url }
    let endpoint = "https://windsurf-stable.codeium.com/api/update/linux-x64/stable/latest";
    let resp = reqwest::blocking::get(endpoint)
        .with_context(|| "requesting latest stable info")?;
    let status = resp.status();
    if !status.is_success() {
        anyhow::bail!("unexpected status {} from latest endpoint", status);
    }
    let info: LatestInfo = resp.json().with_context(|| "parsing latest info json")?;
    Ok(info)
}
