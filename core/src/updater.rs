use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use parking_lot::Mutex;
use serde::Deserialize;

const UPDATE_URL: &str = "https://neoshell.wwwneo.com/updates/update.json";
#[allow(dead_code)]
const CHECK_INTERVAL_SECS: u64 = 3600; // 1 hour

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateInfo {
    pub version: String,
    pub changelog: String,
    pub date: String,
    pub downloads: std::collections::HashMap<String, PlatformDownload>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PlatformDownload {
    pub url: String,
    pub md5: String,
    pub size: u64,
}

#[derive(Debug, Clone)]
pub struct UpdateState {
    pub available: bool,
    pub version: String,
    pub changelog: String,
    pub download_progress: f64, // 0.0 - 1.0
    pub ready: bool,            // Downloaded and verified
    pub error: Option<String>,
}

impl Default for UpdateState {
    fn default() -> Self {
        Self {
            available: false,
            version: String::new(),
            changelog: String::new(),
            download_progress: 0.0,
            ready: false,
            error: None,
        }
    }
}

pub struct Updater {
    pub state: Arc<Mutex<UpdateState>>,
    checking: Arc<AtomicBool>,
}

impl Updater {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(UpdateState::default())),
            checking: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Get the current platform key for update.json
    fn platform_key() -> &'static str {
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        {
            "macos-aarch64"
        }
        #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
        {
            "macos-x86_64"
        }
        #[cfg(target_os = "windows")]
        {
            "windows-x64"
        }
        #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
        {
            "linux-x86_64"
        }
        #[cfg(not(any(
            all(target_os = "macos", target_arch = "aarch64"),
            all(target_os = "macos", target_arch = "x86_64"),
            target_os = "windows",
            all(target_os = "linux", target_arch = "x86_64"),
        )))]
        {
            "unknown"
        }
    }

    /// Get the staging directory for downloaded updates
    fn staging_dir() -> PathBuf {
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("neoshell")
            .join("updates")
    }

    /// Check for updates in background. Non-blocking.
    pub fn check_async(&self) {
        if self.checking.load(Ordering::Relaxed) {
            return; // Already checking
        }
        self.checking.store(true, Ordering::Relaxed);

        let state = self.state.clone();
        let checking = self.checking.clone();
        let current_version = env!("CARGO_PKG_VERSION").to_string();

        std::thread::spawn(move || {
            match check_for_update(&current_version) {
                Ok(Some(info)) => {
                    let mut s = state.lock();
                    s.available = true;
                    s.version = info.version;
                    s.changelog = info.changelog;
                }
                Ok(None) => {
                    // No update available
                }
                Err(e) => {
                    let mut s = state.lock();
                    s.error = Some(format!("Update check failed: {}", e));
                }
            }
            checking.store(false, Ordering::Relaxed);
        });
    }

    /// Download the update in background. Non-blocking.
    pub fn download_async(&self) {
        let state = self.state.clone();

        std::thread::spawn(move || {
            match download_update(&state) {
                Ok(()) => {
                    let mut s = state.lock();
                    s.ready = true;
                }
                Err(e) => {
                    let mut s = state.lock();
                    s.error = Some(format!("Download failed: {}", e));
                }
            }
        });
    }
}

fn check_for_update(current_version: &str) -> Result<Option<UpdateInfo>, String> {
    let resp = ureq::get(UPDATE_URL)
        .call()
        .map_err(|e| format!("HTTP error: {}", e))?;

    let info: UpdateInfo = resp
        .into_json()
        .map_err(|e| format!("JSON parse error: {}", e))?;

    // Simple version comparison (assumes semver x.y.z)
    if info.version != current_version && info.version > current_version.to_string() {
        Ok(Some(info))
    } else {
        Ok(None)
    }
}

fn download_update(state: &Arc<Mutex<UpdateState>>) -> Result<(), String> {
    // Fetch update info
    let resp = ureq::get(UPDATE_URL)
        .call()
        .map_err(|e| format!("HTTP: {}", e))?;
    let info: UpdateInfo = resp.into_json().map_err(|e| format!("JSON: {}", e))?;

    let platform = Updater::platform_key();
    let download = info
        .downloads
        .get(platform)
        .ok_or_else(|| format!("No download for platform: {}", platform))?;

    // Create staging directory
    let staging_dir = Updater::staging_dir();
    std::fs::create_dir_all(&staging_dir).map_err(|e| format!("Create dir: {}", e))?;

    // Determine library filename
    #[cfg(target_os = "macos")]
    let lib_name = "libneoshell_core.dylib";
    #[cfg(target_os = "windows")]
    let lib_name = "neoshell_core.dll";
    #[cfg(target_os = "linux")]
    let lib_name = "libneoshell_core.so";
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    let lib_name = "libneoshell_core.so";

    let staged_path = staging_dir.join(lib_name);

    // Download with progress
    let resp = ureq::get(&download.url)
        .call()
        .map_err(|e| format!("Download: {}", e))?;

    let total = download.size;
    let mut file =
        std::fs::File::create(&staged_path).map_err(|e| format!("Create file: {}", e))?;

    let mut reader = resp.into_reader();
    let mut buf = [0u8; 32768];
    let mut downloaded: u64 = 0;

    use std::io::{Read, Write};
    loop {
        let n = reader.read(&mut buf).map_err(|e| format!("Read: {}", e))?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n])
            .map_err(|e| format!("Write: {}", e))?;
        downloaded += n as u64;

        let mut s = state.lock();
        s.download_progress = if total > 0 {
            downloaded as f64 / total as f64
        } else {
            0.0
        };
    }

    // Verify MD5
    use md5::{Digest, Md5};
    let file_bytes = std::fs::read(&staged_path).map_err(|e| format!("Read for MD5: {}", e))?;
    let hash = format!("{:x}", Md5::digest(&file_bytes));

    if hash != download.md5 {
        let _ = std::fs::remove_file(&staged_path);
        return Err(format!(
            "MD5 mismatch: expected {}, got {}",
            download.md5, hash
        ));
    }

    Ok(())
}
