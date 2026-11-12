use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use walkdir::WalkDir;
use crate::db;
use crate::installer::slugify;
use crate::models::{AppResiduals, AppSource, InstalledApp, ResidualTarget};

#[allow(dead_code)]
pub fn uninstall(app: &InstalledApp) -> Result<(), String> {
    uninstall_with_residuals(app, &[]).map(|_| ())
}

pub fn uninstall_with_residuals(app: &InstalledApp, selected_residuals: &[PathBuf]) -> Result<u64, String> {
    let mut freed_bytes = 0;

    // 1. Uninstall the core application
    if app.is_managed_by_installxs {
        uninstall_installxs_app(app)?;
    } else {
        match app.source {
            AppSource::Flatpak => {
                let id = app.id.strip_prefix("flatpak:").unwrap_or(&app.id);
                let status = Command::new("flatpak")
                    .args(["uninstall", "-y", id])
                    .status()
                    .map_err(|e| format!("Failed to run flatpak uninstall: {}", e))?;
                if !status.success() {
                    return Err("flatpak uninstall exited with non-zero status".to_string());
                }
            }
            AppSource::Pacman | AppSource::Aur => {
                let pkg_name = app.id.strip_prefix("arch:").unwrap_or(&app.id);
                let status = Command::new("pkexec")
                    .args(["pacman", "-R", "--noconfirm", pkg_name])
                    .status()
                    .map_err(|e| format!("Failed to run pkexec pacman -R: {}", e))?;
                if !status.success() {
                    return Err("pacman -R failed or was cancelled by user".to_string());
                }
            }
            AppSource::Manual | AppSource::AppImage => {
                if let Some(ref desktop) = app.desktop_file {
                    if desktop.starts_with(db::user_applications_dir()) {
                        let _ = fs::remove_file(desktop);
                        let _ = Command::new("update-desktop-database")
                            .arg(db::user_applications_dir())
                            .output();
                    }
                }
            }
            _ => return Err("Uninstall not supported for this source".to_string()),
        }
    }

    // 2. Purge user-confirmed residual files and directories
    if !selected_residuals.is_empty() {
        freed_bytes += purge_residuals(selected_residuals)?;
    }

    Ok(freed_bytes)
}

pub fn calculate_path_size(path: &Path) -> u64 {
    if !path.exists() {
        return 0;
    }
    if path.is_file() || path.is_symlink() {
        return path.metadata().map(|m| m.len()).unwrap_or(0);
    }
    WalkDir::new(path)
        .into_iter()
        .flatten()
        .filter_map(|e| e.metadata().ok())
        .map(|m| m.len())
        .sum()
}

pub fn detect_residuals(app: &InstalledApp) -> AppResiduals {
    let mut targets = Vec::new();
    let Some(home) = dirs::home_dir() else {
        return AppResiduals::default();
    };

    let mut candidate_slugs = HashSet::new();
    let app_slug = slugify(&app.name);
    candidate_slugs.insert(app_slug.clone());
    candidate_slugs.insert(app.name.to_lowercase());
    candidate_slugs.insert(app.name.to_lowercase().replace(' ', ""));
    candidate_slugs.insert(app.name.to_lowercase().replace(' ', "-"));

    if let Some(ref exec) = app.exec {
        if let Some(first_word) = exec.split_whitespace().next() {
            let p = Path::new(first_word);
            if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
                candidate_slugs.insert(slugify(stem));
                candidate_slugs.insert(stem.to_lowercase());
            }
        }
    }

    let flatpak_id = if app.source == AppSource::Flatpak {
        Some(app.id.strip_prefix("flatpak:").unwrap_or(&app.id).to_string())
    } else {
        None
    };

    let search_roots = [
        (home.join(".config"), "Configuration", false, true),
        (home.join(".cache"), "App Cache & Temp", true, false),
        (home.join(".local/share"), "Local Application Data", false, false),
        (home.join(".local/state"), "Application State Logs", false, false),
    ];

    let mut seen_paths = HashSet::new();

    for (root, label_type, is_cache, is_config) in &search_roots {
        if !root.is_dir() {
            continue;
        }

        for slug in &candidate_slugs {
            if slug.len() < 2 || slug == "bin" || slug == "share" || slug == "local" || slug == "config" {
                continue;
            }
            let target_path = root.join(slug);
            if target_path.exists() && seen_paths.insert(target_path.clone()) {
                let size = calculate_path_size(&target_path);
                if size > 0 {
                    targets.push(ResidualTarget {
                        path: target_path.clone(),
                        label: format!("{} (~/{})", label_type, target_path.strip_prefix(&home).unwrap_or(&target_path).display()),
                        size_bytes: size,
                        is_cache: *is_cache,
                        is_config: *is_config,
                        selected: true,
                    });
                }
            }
        }
    }

    // Flatpak specific sandbox dir
    if let Some(ref fid) = flatpak_id {
        let var_app = home.join(".var/app").join(fid);
        if var_app.exists() && seen_paths.insert(var_app.clone()) {
            let size = calculate_path_size(&var_app);
            targets.push(ResidualTarget {
                path: var_app,
                label: format!("Flatpak Sandbox Data (~/.var/app/{})", fid),
                size_bytes: size,
                is_cache: true,
                is_config: true,
                selected: true,
            });
        }
    }

    let total_bytes = targets.iter().map(|t| t.size_bytes).sum();
    AppResiduals { targets, total_bytes }
}

pub fn purge_residuals(paths: &[PathBuf]) -> Result<u64, String> {
    let mut total_freed = 0;
    for path in paths {
        if path.exists() {
            let size = calculate_path_size(path);
            if path.is_file() || path.is_symlink() {
                let _ = fs::remove_file(path);
            } else if path.is_dir() {
                let _ = fs::remove_dir_all(path);
            }
            total_freed += size;
        }
    }
    Ok(total_freed)
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
