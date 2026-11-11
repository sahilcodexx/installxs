use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use chrono::Utc;
use walkdir::WalkDir;
use crate::catalog::desktop_parser::{generate_desktop_entry, parse_desktop_file};
use crate::db;
use crate::installer::slugify;
use crate::models::{AppSource, InstallCandidate, InstalledApp};

pub fn install(candidate: &InstallCandidate) -> Result<InstalledApp, String> {
    db::ensure_directories().map_err(|e| e.to_string())?;

    let slug = slugify(&candidate.suggested_name);
    let app_dir = db::apps_dir().join(&slug);
    fs::create_dir_all(&app_dir).map_err(|e| format!("Failed to create app directory: {}", e))?;

    let target_appimage = app_dir.join(format!("{}.AppImage", &slug));
    
    // Copy the AppImage
    fs::copy(&candidate.source_path, &target_appimage)
        .map_err(|e| format!("Failed to copy AppImage: {}", e))?;

    // Make executable (chmod +x)
    let mut perms = fs::metadata(&target_appimage)
        .map_err(|e| e.to_string())?
        .permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&target_appimage, perms).map_err(|e| e.to_string())?;

    let mut installed_files = vec![target_appimage.clone()];

    // Try extracting desktop and icon from the AppImage
    let (mut app_name, mut comment, icon_path) = extract_metadata_and_icon(&target_appimage, &app_dir);
    if app_name.is_empty() {
        app_name = candidate.suggested_name.clone();
    }
    if comment.is_empty() {
        comment = format!("{} (AppImage installed via InstallXS)", app_name);
    }

    if let Some(ref icon) = icon_path {
        installed_files.push(icon.clone());
    }

    // Generate .desktop file in ~/.local/share/applications
    let desktop_filename = format!("installxs-{}.desktop", &slug);
    let desktop_file_path = db::user_applications_dir().join(&desktop_filename);
    let desktop_content = generate_desktop_entry(
        &app_name,
        &target_appimage,
        icon_path.as_deref(),
        Some(&comment),
        Some("Utility;Application;"),
    );

    fs::write(&desktop_file_path, desktop_content)
        .map_err(|e| format!("Failed to write desktop file: {}", e))?;
    installed_files.push(desktop_file_path.clone());

    // Create symlink in ~/.local/bin/<slug>
    let bin_link = db::bin_dir().join(&slug);
    let _ = fs::remove_file(&bin_link); // remove existing if any
    #[cfg(unix)]
    {
        let _ = std::os::unix::fs::symlink(&target_appimage, &bin_link);
        if bin_link.exists() {
            installed_files.push(bin_link.clone());
        }
    }

    // Update desktop database if available
    let _ = Command::new("update-desktop-database")
        .arg(db::user_applications_dir())
        .output();

    let app = InstalledApp {
        id: format!("installxs:{}", slug),
        name: app_name,
        version: None,
        comment: Some(comment),
        source: AppSource::AppImage,
        exec: Some(target_appimage.to_string_lossy().to_string()),
        icon: icon_path.map(|p| p.to_string_lossy().to_string()),
        desktop_file: Some(desktop_file_path),
        install_date: Some(Utc::now()),
        installed_size: Some(candidate.file_size),
        is_managed_by_installxs: true,
        install_path: Some(app_dir),
        installed_files,
    };

    db::add_or_update_app(app.clone()).map_err(|e| e.to_string())?;

    Ok(app)
}

fn extract_metadata_and_icon(appimage: &Path, app_dir: &Path) -> (String, String, Option<PathBuf>) {
    let temp_extract_dir = app_dir.join(".extract_tmp");
    let _ = fs::create_dir_all(&temp_extract_dir);

    // Try running --appimage-extract
    let _ = Command::new(appimage)
        .current_dir(&temp_extract_dir)
        .arg("--appimage-extract")
        .output();

    let squash_root = temp_extract_dir.join("squashfs-root");
    let mut detected_name = String::new();
    let mut detected_comment = String::new();
    let mut detected_icon_path: Option<PathBuf> = None;

    if squash_root.exists() {
        // Find .desktop file
        for entry in WalkDir::new(&squash_root).max_depth(3).into_iter().flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("desktop") {
                if let Some(parsed) = parse_desktop_file(path) {
                    detected_name = parsed.name;
                    detected_comment = parsed.comment.unwrap_or_default();
                    break;
                }
            }
        }

        // Find icon (.DirIcon, or *.png, *.svg)
        let dir_icon = squash_root.join(".DirIcon");
        if dir_icon.exists() {
            let dest_icon = app_dir.join("icon.png");
            if fs::copy(&dir_icon, &dest_icon).is_ok() {
                detected_icon_path = Some(dest_icon);
            }
        }

        if detected_icon_path.is_none() {
            for entry in WalkDir::new(&squash_root).max_depth(4).into_iter().flatten() {
                let path = entry.path();
                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                if ext == "png" || ext == "svg" {
                    let dest_icon = app_dir.join(format!("icon.{}", ext));
                    if fs::copy(path, &dest_icon).is_ok() {
                        detected_icon_path = Some(dest_icon);
                        break;
                    }
                }
            }
        }

        let _ = fs::remove_dir_all(&temp_extract_dir);
    }

    (detected_name, detected_comment, detected_icon_path)
}
