use std::process::Command;
use crate::models::{AppSource, AvailableUpdate};

pub fn check_system_updates() -> Vec<AvailableUpdate> {
    let mut updates = Vec::new();

    // 1. Pacman updates
    if let Ok(output) = Command::new("checkupdates").output() {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 4 && parts[2] == "->" {
                    updates.push(AvailableUpdate {
                        package_name: parts[0].to_string(),
                        current_version: parts[1].to_string(),
                        new_version: parts[3].to_string(),
                        source: AppSource::Pacman,
                        repository: Some("Official Arch".to_string()),
                        download_size: None,
                        download_url: None,
                        target_app_id: None,
                        release_notes: None,
                    });
                }
            }
        }
    } else if let Ok(output) = Command::new("pacman").args(["-Qu"]).output() {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 4 && parts[2] == "->" {
                    updates.push(AvailableUpdate {
                        package_name: parts[0].to_string(),
                        current_version: parts[1].to_string(),
                        new_version: parts[3].to_string(),
                        source: AppSource::Pacman,
                        repository: Some("Official Arch".to_string()),
                        download_size: None,
                        download_url: None,
                        target_app_id: None,
                        release_notes: None,
                    });
                }
            }
        }
    }

    // 2. Flatpak updates
    if let Ok(output) = Command::new("flatpak").args(["remote-ls", "--updates", "--columns=name,application,version"]).output() {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                let parts: Vec<&str> = line.split('\t').collect();
                if parts.len() >= 2 {
                    let name = parts[0].trim().to_string();
                    let app_id = parts[1].trim().to_string();
                    let version = if parts.len() > 2 { parts[2].trim().to_string() } else { "latest".to_string() };
                    updates.push(AvailableUpdate {
                        package_name: if name.is_empty() { app_id } else { name },
                        current_version: "current".to_string(),
                        new_version: version,
                        source: AppSource::Flatpak,
                        repository: Some("Flathub".to_string()),
                        download_size: None,
                        download_url: None,
                        target_app_id: None,
                        release_notes: None,
                    });
                }
            }
        }
    }

    // 3. AUR updates via yay
    if let Ok(output) = Command::new("yay").args(["-Qua"]).output() {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 4 && parts[2] == "->" {
                    updates.push(AvailableUpdate {
                        package_name: parts[0].to_string(),
                        current_version: parts[1].to_string(),
                        new_version: parts[3].to_string(),
                        source: AppSource::Aur,
                        repository: Some("AUR".to_string()),
                        download_size: None,
                        download_url: None,
                        target_app_id: None,
                        release_notes: None,
                    });
                }
            }
        }
    }

    // 4. Standalone App Releases (GitHub, GitLab, etc.)
    let release_updates = crate::catalog::release_updater::check_all_release_sources();
    updates.extend(release_updates);

    updates
}

pub fn run_package_update(update: &AvailableUpdate) -> Result<(), String> {
    if update.download_url.is_some() {
        return crate::catalog::release_updater::execute_release_update(update).map(|_| ());
    }
    match update.source {
        AppSource::Pacman => {
            let status = Command::new("pkexec")
                .args(["pacman", "-S", "--noconfirm", &update.package_name])
                .status()
                .map_err(|e| e.to_string())?;
            if !status.success() {
                return Err(format!("Failed to update package '{}'", update.package_name));
            }
        }
        AppSource::Flatpak => {
            let status = Command::new("flatpak")
                .args(["update", "-y", &update.package_name])
                .status()
                .map_err(|e| e.to_string())?;
            if !status.success() {
                return Err(format!("Failed to update Flatpak '{}'", update.package_name));
            }
        }
        AppSource::Aur => {
            let status = Command::new("yay")
                .args(["-S", "--noconfirm", &update.package_name])
                .status()
                .map_err(|e| e.to_string())?;
            if !status.success() {
                return Err(format!("Failed to update AUR package '{}'", update.package_name));
            }
        }
        _ => return Err("Updates not supported for this source".to_string()),
    }
    Ok(())
}
