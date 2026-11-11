use std::fs;
use std::process::Command;
use crate::db;
use crate::models::{AppSource, InstalledApp};

pub fn uninstall(app: &InstalledApp) -> Result<(), String> {
    if app.is_managed_by_installxs {
        uninstall_installxs_app(app)
    } else {
        match app.source {
            AppSource::Flatpak => {
                let id = app.id.strip_prefix("flatpak:").unwrap_or(&app.id);
                let status = Command::new("flatpak")
                    .args(["uninstall", "-y", id])
                    .status()
                    .map_err(|e| format!("Failed to run flatpak uninstall: {}", e))?;
                if !status.success() {
                    return Err(format!("flatpak uninstall exited with non-zero status"));
                }
                Ok(())
            }
            AppSource::Pacman | AppSource::Aur => {
                let pkg_name = app.id.strip_prefix("arch:").unwrap_or(&app.id);
                let status = Command::new("pkexec")
                    .args(["pacman", "-R", "--noconfirm", pkg_name])
                    .status()
                    .map_err(|e| format!("Failed to run pkexec pacman -R: {}", e))?;
                if !status.success() {
                    return Err(format!("pacman -R failed or was cancelled by user"));
                }
                Ok(())
            }
            AppSource::Manual | AppSource::AppImage => {
                // If there's a desktop file in user's ~/.local/share/applications, remove it
                if let Some(ref desktop) = app.desktop_file {
                    if desktop.starts_with(db::user_applications_dir()) {
                        let _ = fs::remove_file(desktop);
                        let _ = Command::new("update-desktop-database")
                            .arg(db::user_applications_dir())
                            .output();
                    }
                }
                Ok(())
            }
            _ => Err("Uninstall not supported for this source".to_string()),
        }
    }
}

fn uninstall_installxs_app(app: &InstalledApp) -> Result<(), String> {
    // 1. Delete all individual files tracked in manifest
    for file in &app.installed_files {
        if file.is_file() || file.is_symlink() {
            let _ = fs::remove_file(file);
        } else if file.is_dir() {
            let _ = fs::remove_dir_all(file);
        }
    }

    // 2. Delete install folder if exists
    if let Some(ref dir) = app.install_path {
        if dir.exists() {
            let _ = fs::remove_dir_all(dir);
        }
    }

    // 3. Delete desktop file if exists
    if let Some(ref desktop) = app.desktop_file {
        if desktop.exists() {
            let _ = fs::remove_file(desktop);
        }
    }

    // 4. Clean up database
    let _ = db::remove_app(&app.id);

    // 5. Update desktop database
    let _ = Command::new("update-desktop-database")
        .arg(db::user_applications_dir())
        .output();

    Ok(())
}
