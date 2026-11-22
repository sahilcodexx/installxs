use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::Command;
use serde::Deserialize;
use crate::db;
use crate::models::{AppReleaseSource, AppSource, AvailableUpdate, ReleaseProvider};

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct GitHubRelease {
    pub tag_name: String,
    pub name: Option<String>,
    pub prerelease: bool,
    pub draft: bool,
    pub body: Option<String>,
    pub assets: Vec<GitHubAsset>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct GitHubAsset {
    pub name: String,
    pub size: u64,
    pub browser_download_url: String,
}

pub fn parse_repo_string(input: &str) -> Option<(ReleaseProvider, String, String)> {
    let raw = input.trim();
    if raw.is_empty() {
        return None;
    }

    // Direct owner/repo format e.g. "imputnet/helium-linux"
    if !raw.contains('/') {
        return None;
    }

    if !raw.starts_with("http://") && !raw.starts_with("https://") {
        let parts: Vec<&str> = raw.split('/').collect();
        if parts.len() == 2 && !parts[0].is_empty() && !parts[1].is_empty() {
            let repo = parts[1].trim_end_matches(".git");
            return Some((ReleaseProvider::GitHub, parts[0].to_string(), repo.to_string()));
        }
    }

    // URL format: e.g. "https://github.com/imputnet/helium-linux"
    // or "https://github.com/imputnet/helium-linux/releases/tag/0.22.2.1"
    let clean_url = raw.trim_start_matches("https://").trim_start_matches("http://");
    let parts: Vec<&str> = clean_url.split('/').collect();

    if parts.is_empty() {
        return None;
    }

    let host = parts[0].to_lowercase();
    if host.contains("github.com") && parts.len() >= 3 {
        let owner = parts[1].to_string();
        let repo = parts[2].trim_end_matches(".git").to_string();
        if !owner.is_empty() && !repo.is_empty() {
            return Some((ReleaseProvider::GitHub, owner, repo));
        }
    } else if host.contains("gitlab.com") && parts.len() >= 3 {
        let owner = parts[1].to_string();
        let repo = parts[2].trim_end_matches(".git").to_string();
        if !owner.is_empty() && !repo.is_empty() {
            return Some((ReleaseProvider::GitLab, owner, repo));
        }
    }

    None
}

pub fn fetch_latest_github_release(owner: &str, repo: &str) -> Result<GitHubRelease, String> {
    let api_url = format!("https://api.github.com/repos/{owner}/{repo}/releases/latest");
    let output = Command::new("curl")
        .args([
            "-s",
            "--max-time", "12",
            "-H", "User-Agent: InstallXS-AppManager",
            "-H", "Accept: application/vnd.github.v3+json",
            &api_url,
        ])
        .output()
        .map_err(|e| format!("Failed to connect to GitHub API: {}", e))?;

    if !output.status.success() {
        return Err("curl failed while contacting GitHub API".to_string());
    }

    let release: GitHubRelease = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("Failed to parse release response from GitHub: {}", e))?;

    Ok(release)
}

pub fn pick_best_asset<'a>(
    assets: &'a [GitHubAsset],
    preferred_keyword: Option<&str>,
    preferred_format: Option<&str>,
) -> Option<&'a GitHubAsset> {
    let candidates: Vec<&'a GitHubAsset> = assets.iter().filter(|a| {
        let n = a.name.to_lowercase();
        // Skip metadata / signatures
        if n.ends_with(".zsync")
            || n.ends_with(".asc")
            || n.ends_with(".sha256")
            || n.ends_with(".sha512")
            || n.ends_with(".sig")
            || n.ends_with(".md5")
            || n.ends_with(".txt")
        {
            return false;
        }

        // Exclude foreign architectures
        if n.contains("arm64")
            || n.contains("aarch64")
            || n.contains("armv7")
            || n.contains("i386")
            || n.contains("i686")
            || n.contains("win32")
            || n.contains("win64")
            || n.ends_with(".exe")
            || n.ends_with(".dmg")
            || n.ends_with(".pkg")
            || n.ends_with(".rpm")
        {
            return false;
        }

        true
    }).collect();

    if candidates.is_empty() {
        return None;
    }

    // 1. If preferred keyword specified (e.g. "x86_64.AppImage")
    if let Some(kw) = preferred_keyword {
        let kw_lower = kw.to_lowercase();
        if let Some(found) = candidates.iter().find(|a| a.name.to_lowercase().contains(&kw_lower)) {
            return Some(found);
        }
    }

    // 2. If app format preferred (e.g. AppImage)
    if let Some(fmt) = preferred_format {
        let fmt_lower = fmt.to_lowercase();
        if let Some(found) = candidates.iter().find(|a| a.name.to_lowercase().ends_with(&fmt_lower)) {
            return Some(found);
        }
    }

    // 3. Fallback ranking: AppImage (matching x86_64 / amd64) -> tar.xz -> tar.gz -> zip -> deb
    if let Some(found) = candidates.iter().find(|a| {
        let n = a.name.to_lowercase();
        n.ends_with(".appimage") && (n.contains("x86_64") || n.contains("amd64") || n.contains("x64"))
    }) {
        return Some(found);
    }

    if let Some(found) = candidates.iter().find(|a| a.name.to_lowercase().ends_with(".appimage")) {
        return Some(found);
    }

    if let Some(found) = candidates.iter().find(|a| {
        let n = a.name.to_lowercase();
        (n.ends_with(".tar.xz") || n.ends_with(".tar.gz")) && (n.contains("x86_64") || n.contains("linux"))
    }) {
        return Some(found);
    }

    candidates.into_iter().next()
}

pub fn clean_version_tag(tag: &str) -> String {
    let t = tag.trim();
    if t.starts_with('v') || t.starts_with('V') {
        t[1..].to_string()
    } else {
        t.to_string()
    }
}

pub fn is_newer_version(current: &str, candidate: &str) -> bool {
    let c_clean = clean_version_tag(current);
    let cand_clean = clean_version_tag(candidate);

    if c_clean == cand_clean {
        return false;
    }

    let c_parts: Vec<u32> = c_clean
        .split(|c: char| !c.is_ascii_digit())
        .filter_map(|s| s.parse::<u32>().ok())
        .collect();

    let cand_parts: Vec<u32> = cand_clean
        .split(|c: char| !c.is_ascii_digit())
        .filter_map(|s| s.parse::<u32>().ok())
        .collect();

    for (c_num, cand_num) in c_parts.iter().zip(cand_parts.iter()) {
        if cand_num > c_num {
            return true;
        } else if cand_num < c_num {
            return false;
        }
    }

    if cand_parts.len() > c_parts.len() {
        return true;
    }

    // Fallback string inequality if non-numeric parts exist
    c_clean != cand_clean
}

pub fn check_single_release_source(source: &AppReleaseSource) -> Result<Option<AvailableUpdate>, String> {
    let (provider, owner, repo) = parse_repo_string(&source.repo_or_url)
        .ok_or_else(|| format!("Invalid repository source string: {}", source.repo_or_url))?;

    match provider {
        ReleaseProvider::GitHub => {
            let release = fetch_latest_github_release(&owner, &repo)?;
            let new_ver = clean_version_tag(&release.tag_name);
            let cur_ver = source.current_version.as_deref().unwrap_or("0.0.0");

            if is_newer_version(cur_ver, &new_ver) {
                let asset = pick_best_asset(
                    &release.assets,
                    source.asset_keyword.as_deref(),
                    Some(".AppImage"),
                ).ok_or_else(|| "No compatible Linux x86_64 release asset found in release".to_string())?;

                Ok(Some(AvailableUpdate {
                    package_name: source.app_name.clone(),
                    current_version: cur_ver.to_string(),
                    new_version: new_ver,
                    source: AppSource::AppImage,
                    repository: Some(format!("GitHub: {}/{}", owner, repo)),
                    download_size: Some(asset.size),
                    download_url: Some(asset.browser_download_url.clone()),
                    target_app_id: Some(source.app_id.clone()),
                    release_notes: release.body,
                }))
            } else {
                Ok(None)
            }
        }
        ReleaseProvider::GitLab | ReleaseProvider::DirectUrl => {
            Err("Only GitHub release tracking is currently supported".to_string())
        }
    }
}

pub fn check_all_release_sources() -> Vec<AvailableUpdate> {
    let sources = db::load_update_sources();
    let mut updates = Vec::new();

    for src in sources {
        match check_single_release_source(&src) {
            Ok(Some(upd)) => updates.push(upd),
            Ok(None) => {}
            Err(e) => log::warn!("Failed to check release updates for {}: {}", src.app_name, e),
        }
    }

    updates
}

pub fn execute_release_update(update: &AvailableUpdate) -> Result<String, String> {
    let url = update.download_url.as_ref().ok_or("No download URL provided for release update")?;
    let bin_dir = db::bin_dir();
    db::ensure_directories().map_err(|e| e.to_string())?;

    // Determine target binary name from URL or app name
    let filename = url.rsplit('/').next().unwrap_or("update.AppImage");
    let temp_download = bin_dir.join(format!("{}.download.part", filename));

    log::info!("Downloading release update from {} to {:?}", url, temp_download);

    // Download using curl with follow redirects and max timeout
    let status = Command::new("curl")
        .args([
            "-L",
            "--fail",
            "--max-time", "600",
            "-H", "User-Agent: InstallXS-AppManager",
            "-o", temp_download.to_str().unwrap(),
            url,
        ])
        .status()
        .map_err(|e| format!("Failed to download update: {}", e))?;

    if !status.success() || !temp_download.exists() {
        let _ = fs::remove_file(&temp_download);
        return Err(format!("Download failed for '{}'", update.package_name));
    }

    let meta = fs::metadata(&temp_download).map_err(|e| e.to_string())?;
    if meta.len() < 1024 * 1024 {
        let _ = fs::remove_file(&temp_download);
        return Err("Downloaded file is unexpectedly small, download likely failed".to_string());
    }

    let final_dest = bin_dir.join(filename);
    fs::rename(&temp_download, &final_dest).map_err(|e| format!("Failed to stage binary: {}", e))?;

    // Set chmod +x (0o755)
    let mut perms = fs::metadata(&final_dest).map_err(|e| e.to_string())?.permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&final_dest, perms).map_err(|e| e.to_string())?;

    // In-place upgrade symlink and launcher
    let app_slug = update.package_name.to_lowercase().replace(' ', "-");
    let main_symlink = bin_dir.join(&app_slug);

    // Update symlink
    let _ = fs::remove_file(&main_symlink);
    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        let _ = symlink(&final_dest, &main_symlink);
    }

    // Find and update associated .desktop file
    let desktop_dir = db::user_applications_dir();
    if let Ok(entries) = fs::read_dir(&desktop_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
            if stem.contains(&app_slug) || stem.contains("helium") {
                if let Ok(content) = fs::read_to_string(&path) {
                    let mut updated_lines = Vec::new();
                    let final_str = final_dest.to_string_lossy();
                    for line in content.lines() {
                        if line.starts_with("Exec=") {
                            updated_lines.push(format!("Exec=\"{}\" %U", final_str));
                        } else if line.starts_with("TryExec=") {
                            updated_lines.push(format!("TryExec={}", final_str));
                        } else if line.starts_with("X-AppImage-Version=") {
                            updated_lines.push(format!("X-AppImage-Version={}", update.new_version));
                        } else {
                            updated_lines.push(line.to_string());
                        }
                    }
                    let _ = fs::write(&path, updated_lines.join("\n"));
                }
            }
        }
    }

    let _ = Command::new("update-desktop-database").arg(&desktop_dir).output();

    // Clean up older AppImage if different file
    if let Ok(entries) = fs::read_dir(&bin_dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p != final_dest && p.is_file() {
                let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
                if (name.contains(&app_slug) || name.contains("helium"))
                    && name.ends_with(".appimage")
                    && !name.contains(&update.new_version)
                {
                    log::info!("Removing previous release AppImage: {:?}", p);
                    let _ = fs::remove_file(p);
                }
            }
        }
    }

    // Update persistent records in update_sources.json
    if let Some(mut src) = db::get_update_source(update.target_app_id.as_deref().unwrap_or(""), &update.package_name) {
        src.current_version = Some(update.new_version.clone());
        let _ = db::set_update_source(src);
    }

    Ok(format!(
        "Updated '{}' to version {}! All user settings, cookies, tabs, and profiles in ~/.config are preserved.",
        update.package_name, update.new_version
    ))
}
