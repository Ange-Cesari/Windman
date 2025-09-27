use anyhow::{Context, Result};
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, HeaderValue, USER_AGENT};
use std::{
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
    time::Duration,
};

const DEFAULT_TIMEOUT_SECS: u64 = 30;

/// Build a blocking reqwest client with default headers and a timeout.
fn build_client(timeout_secs: u64) -> Result<Client> {
    let mut headers = HeaderMap::new();
    headers.insert(
        USER_AGENT,
        HeaderValue::from_static("Windman/0.1 (+https://github.com/your-org/windman)"),
    );

    let client = Client::builder()
        .default_headers(headers)
        .timeout(Duration::from_secs(timeout_secs))
        .build()
        .context("building HTTP client")?;
    Ok(client)
}

/// Backward-compatible wrapper with default timeout.
pub fn download_to_file(url: &str, dest: &Path) -> Result<()> {
    download_to_file_with_timeout(url, dest, None)
}

/// Download `url` to `dest`, with optional timeout override (in seconds).
/// Writes atomically: to `dest.part` then renames to `dest` at the end.
pub fn download_to_file_with_timeout(
    url: &str,
    dest: &Path,
    timeout_override: Option<u64>,
) -> Result<()> {
    let timeout = timeout_override.unwrap_or(DEFAULT_TIMEOUT_SECS);

    // Ensure parent directory exists
    let parent: PathBuf = dest
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));
    if !parent.exists() {
        fs::create_dir_all(&parent)
            .with_context(|| format!("creating parent directory {}", parent.display()))?;
    }

    // Temp file in same directory for atomic rename at the end
    let temp_path = dest.with_extension("part");
    let client = build_client(timeout)?;

    let resp = client
        .get(url)
        .send()
        .with_context(|| format!("GET {}", url))?;

    let status = resp.status();
    if !status.is_success() {
        anyhow::bail!("unexpected status {} for {}", status, url);
    }

    // Progress (bar when Content-Length is known, spinner otherwise)
    let len = resp.content_length();
    let pb = match len {
        Some(total) => {
            let pb = ProgressBar::new(total);
            pb.set_style(
                ProgressStyle::with_template("{bar} {bytes}/{total_bytes} {eta}")?
                    .progress_chars("#>-"),
            );
            pb
        }
        None => {
            let pb = ProgressBar::new_spinner();
            pb.set_style(ProgressStyle::with_template("{spinner} {bytes} downloaded")?);
            pb.enable_steady_tick(Duration::from_millis(120));
            pb
        }
    };

    let mut reader = resp;
    let mut out = File::create(&temp_path)
        .with_context(|| format!("creating temp file {}", temp_path.display()))?;

    let mut buf = [0u8; 64 * 1024];
    let mut downloaded: u64 = 0;

    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        out.write_all(&buf[..n])?;
        downloaded += n as u64;
        pb.set_position(downloaded);
    }

    pb.finish_and_clear();

    // Atomic rename to final destination
    fs::rename(&temp_path, dest).with_context(|| {
        format!(
            "renaming {} -> {}",
            temp_path.display(),
            dest.display()
        )
    })?;

    Ok(())
}
