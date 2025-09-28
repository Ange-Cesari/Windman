use anyhow::{anyhow, bail, Context, Result};
use reqwest::blocking::Client;
use reqwest::header::{ACCEPT, USER_AGENT};
use serde::Deserialize;
use std::{env, time::Duration};

#[derive(Debug, Clone)]
pub struct LatestInfo {
    pub version: String,
    pub url: String, // <- toujours présent en mode API
}

const DEFAULT_TIMEOUT_SECS: u64 = 15;
const LINUX_X64_STABLE_LATEST: &str =
    "https://windsurf-stable.codeium.com/api/update/linux-x64/stable/latest";
const RELEASES_PAGE_URL: &str = "https://windsurf.com/editor/releases";

fn build_client(timeout_secs: Option<u64>) -> Result<Client> {
    let t = timeout_secs.unwrap_or(DEFAULT_TIMEOUT_SECS);
    Client::builder()
        .timeout(Duration::from_secs(t))
        .build()
        .context("building reqwest Client")
}

#[derive(Debug, Deserialize)]
struct ApiLatest {
    version: String,
    url: String,
}

/// Essaie d’abord l’API officielle (surchargée par WINDMAN_LATEST_ENDPOINT si défini),
/// qui renvoie {version, url}. C’est notre chemin standard.
fn try_latest_via_api(client: &Client) -> Result<LatestInfo> {
    let endpoint = env::var("WINDMAN_LATEST_ENDPOINT")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| LINUX_X64_STABLE_LATEST.to_string());

    let resp = client
        .get(&endpoint)
        .header(USER_AGENT, "windman/0.1")
        .header(ACCEPT, "application/json")
        .send()
        .with_context(|| format!("GET {}", endpoint))?;

    if !resp.status().is_success() {
        bail!("unexpected status {} for {}", resp.status(), endpoint);
    }

    let parsed: ApiLatest = resp.json().context("deserializing latest JSON")?;
    if parsed.version.trim().is_empty() || parsed.url.trim().is_empty() {
        bail!("latest API returned empty fields");
    }
    Ok(LatestInfo {
        version: parsed.version,
        url: parsed.url,
    })
}

/// Secours : récupère le HTML et lit juste la version (pas l’URL).
/// On s’en sert pour afficher une info utile si l’API est indispo.
fn latest_version_from_releases_html(html: &str) -> Option<String> {
    let re_h2 = regex::Regex::new(r"(?is)<h2[^>]*>\s*([0-9]+\.[0-9]+\.[0-9]+)\s*</h2>").ok()?;
    let caps = re_h2.captures(html)?;
    Some(caps.get(1)?.as_str().to_string())
}

pub fn fetch_releases_html(timeout_secs: Option<u64>) -> Result<String> {
    let client = build_client(timeout_secs)?;
    let resp = client
        .get(RELEASES_PAGE_URL)
        .header(USER_AGENT, "windman/0.1")
        .header(ACCEPT, "text/html,*/*")
        .send()
        .with_context(|| format!("GET {}", RELEASES_PAGE_URL))?;
    if !resp.status().is_success() {
        bail!(
            "unexpected status {} for {}",
            resp.status(),
            RELEASES_PAGE_URL
        );
    }
    resp.text().context("reading releases HTML")
}

/// API publique : renvoie {version, url} via l’API. Si l’API tombe,
/// on tente d’afficher la version via HTML puis on échoue proprement.
pub fn latest_stable_linux_x64(timeout_secs: Option<u64>) -> Result<LatestInfo> {
    let client = build_client(timeout_secs)?;

    match try_latest_via_api(&client) {
        Ok(mut info) => {
            // Si la "version" n'est pas clairement un semver, on tente de l'extraire depuis l'URL.
            let re = regex::Regex::new(r"(\d+\.\d+\.\d+)").unwrap();
            let looks_like_semver = re.is_match(&info.version);
            if !looks_like_semver {
                if let Some(cap) = re.captures(&info.url) {
                    info.version = cap.get(1).unwrap().as_str().to_string();
                }
            }
            Ok(info)
        }
        Err(api_err) => {
            // fallback “informative” : on trouve au moins la version HTML pour aider au debug
            if let Ok(html) = fetch_releases_html(timeout_secs) {
                if let Some(ver) = latest_version_from_releases_html(&html) {
                    return Err(anyhow!(
                        "failed to fetch latest JSON ({}), but releases page shows version {}.\n\
                         Please try again later or override endpoint via WINDMAN_LATEST_ENDPOINT.",
                        api_err,
                        ver
                    ));
                }
            }
            Err(anyhow!("failed to fetch latest JSON: {}", api_err))
        }
    }
}

#[cfg(test)]
pub(crate) fn semver_from_string(s: &str) -> Option<String> {
    let re = regex::Regex::new(r"(\d+\.\d+\.\d+)").ok()?;
    re.captures(s)
        .map(|c| c.get(1).unwrap().as_str().to_string())
}
#[cfg(test)]
mod tests_semver_pick {
    use super::*;

    #[test]
    fn picks_semver_from_url() {
        let url = "https://windsurf-stable.codeiumdata.com/linux-x64/stable/abcd/Windsurf-linux-x64-1.12.11.tar.gz";
        assert_eq!(semver_from_string(url).as_deref(), Some("1.12.11"));
    }

    #[test]
    fn picks_semver_from_text() {
        let s = "latest = 0.9.4 (build 42)";
        assert_eq!(semver_from_string(s).as_deref(), Some("0.9.4"));
    }
}
