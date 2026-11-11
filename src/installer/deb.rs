use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::Command;
use chrono::Utc;
use walkdir::WalkDir;
use crate::catalog::desktop_parser::{generate_desktop_entry, parse_desktop_file};
use crate::db;
use crate::installer::{is_elf_executable, slugify};
use crate::models::{AppSource, InstallCandidate, InstalledApp};

pub fn install(candidate: &InstallCandidate) -> Result<InstalledApp, String> {
    db::ensure_directories().map_err(|e| e.to_string())?;

    let slug = slugify(&candidate.suggested_name);
    let app_dir = db::apps_dir().join(&slug);

    if app_dir.exists() {
        let _ = fs::remove_dir_all(&app_dir);
    }
    fs::create_dir_all(&app_dir).map_err(|e| e.to_string())?;

    let temp_ar = app_dir.join(".deb_extract");
    fs::create_dir_all(&temp_ar).map_err(|e| e.to_string())?;

    // Extract .deb using `ar -x`
    let ar_status = Command::new("ar")
        .args(["-x", &candidate.source_path.to_string_lossy()])
        .current_dir(&temp_ar)
        .status()
        .map_err(|e| format!("Failed to run 'ar' tool (is binutils installed?): {}", e))?;

    if !ar_status.success() {
        return Err("Failed to unpack deb archive".to_string());
    }

    // Find data.tar.*
    let mut data_tar: Option<PathBuf> = None;
    if let Ok(entries) = fs::read_dir(&temp_ar) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with("data.tar") {
                data_tar = Some(entry.path());
                break;
            }
        }
    }

    let data_archive = data_tar.ok_or_else(|| "No data.tar found in deb archive".to_string())?;
    let content_dir = app_dir.join("content");
    fs::create_dir_all(&content_dir).map_err(|e| e.to_string())?;

    // Untar data.tar.* into content_dir
    let tar_status = Command::new("tar")
        .args(["-xf", &data_archive.to_string_lossy(), "-C", &content_dir.to_string_lossy()])
        .status()
        .map_err(|e| format!("Failed to unpack data.tar: {}", e))?;

    if !tar_status.success() {
        return Err("Failed to unpack data.tar contents".to_string());
    }
    let _ = fs::remove_dir_all(&temp_ar);

    // Look for desktop file inside content/usr/share/applications/
    let mut detected_name = String::new();
    let mut detected_comment = String::new();
    let mut detected_icon: Option<PathBuf> = None;
    let mut binary_path: Option<PathBuf> = None;

    for entry in WalkDir::new(&content_dir).into_iter().flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("desktop") {
            if let Some(parsed) = parse_desktop_file(path) {
                detected_name = parsed.name;
                detected_comment = parsed.comment.unwrap_or_default();
                break;
            }
        }
    }

    // Look for executable in usr/bin or opt/
    let usr_bin = content_dir.join("usr/bin");
    if usr_bin.exists() {
        if let Ok(entries) = fs::read_dir(&usr_bin) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_file() {
                    binary_path = Some(p);
                    break;
                }
            }
        }
    }

    if binary_path.is_none() {
        // Search opt/
        let opt_dir = content_dir.join("opt");
        if opt_dir.exists() {
            for entry in WalkDir::new(&opt_dir).max_depth(3).into_iter().flatten() {
                let p = entry.path();
                if p.is_file() && (is_elf_executable(p) || p.file_name().and_then(|n| n.to_str()).map(|n| n.contains(&slug)).unwrap_or(false)) {
                    binary_path = Some(p.to_path_buf());
                    break;
                }
            }
        }
    }

    let main_exec = binary_path.ok_or_else(|| "Could not locate binary inside deb package".to_string())?;

    // Make executable
    if let Ok(meta) = fs::metadata(&main_exec) {
        let mut perms = meta.permissions();
        perms.set_mode(0o755);
        let _ = fs::set_permissions(&main_exec, perms);
    }

    // Look for icon
    for entry in WalkDir::new(&content_dir).max_depth(6).into_iter().flatten() {
        let p = entry.path();
        let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext == "png" || ext == "svg" {
            detected_icon = Some(p.to_path_buf());
            break;
        }
    }

    let app_name = if detected_name.is_empty() { candidate.suggested_name.clone() } else { detected_name };
    let comment = if detected_comment.is_empty() { format!("{} (Debian package installed via InstallXS)", app_name) } else { detected_comment };

    let mut installed_files = Vec::new();
    for entry in WalkDir::new(&app_dir).into_iter().flatten() {
        if entry.file_type().is_file() {
            installed_files.push(entry.path().to_path_buf());
        }
    }

    // Generate .desktop launcher
    let desktop_filename = format!("installxs-{}.desktop", &slug);
    let desktop_file_path = db::user_applications_dir().join(&desktop_filename);
    let desktop_content = generate_desktop_entry(
        &app_name,
        &main_exec,
        detected_icon.as_deref(),
        Some(&comment),
        Some("Utility;Application;"),
    );

    fs::write(&desktop_file_path, desktop_content).map_err(|e| e.to_string())?;
    installed_files.push(desktop_file_path.clone());

    // Symlink into ~/.local/bin
    let bin_link = db::bin_dir().join(&slug);
    let _ = fs::remove_file(&bin_link);
    #[cfg(unix)]
    {
        let _ = std::os::unix::fs::symlink(&main_exec, &bin_link);
        if bin_link.exists() {
            installed_files.push(bin_link.clone());
        }
    }

    let _ = Command::new("update-desktop-database")
        .arg(db::user_applications_dir())
        .output();

    let app = InstalledApp {
        id: format!("installxs:{}", slug),
        name: app_name,
        version: None,
        comment: Some(comment),
        source: AppSource::Deb,
        exec: Some(main_exec.to_string_lossy().to_string()),
        icon: detected_icon.map(|p| p.to_string_lossy().to_string()),
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
