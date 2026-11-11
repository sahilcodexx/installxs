use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use chrono::Utc;
use walkdir::WalkDir;
use crate::catalog::desktop_parser::{generate_desktop_entry, parse_desktop_file};
use crate::db;
use crate::installer::{is_elf_executable, slugify};
use crate::models::{AppSource, FileKind, InstallCandidate, InstalledApp};

pub fn install(candidate: &InstallCandidate) -> Result<InstalledApp, String> {
    db::ensure_directories().map_err(|e| e.to_string())?;

    let slug = slugify(&candidate.suggested_name);
    let app_dir = db::apps_dir().join(&slug);

    if app_dir.exists() {
        let _ = fs::remove_dir_all(&app_dir);
    }
    fs::create_dir_all(&app_dir).map_err(|e| format!("Failed to create app directory: {}", e))?;

    let temp_extract = app_dir.join(".extract");
    fs::create_dir_all(&temp_extract).map_err(|e| e.to_string())?;

    // Extract archive
    extract_archive(&candidate.source_path, &candidate.kind, &temp_extract)?;

    // If extract folder has only 1 child directory, un-nest it
    let extracted_root = resolve_extracted_root(&temp_extract);

    // Move contents into app_dir/content
    let content_dir = app_dir.join("content");
    fs::rename(&extracted_root, &content_dir)
        .map_err(|e| format!("Failed to organize extracted app: {}", e))?;
    let _ = fs::remove_dir_all(&temp_extract);

    // Find main executable and metadata
    let (mut app_name, mut comment, icon_path, main_exec) = find_app_metadata(&content_dir, &slug)?;
    if app_name.is_empty() {
        app_name = candidate.suggested_name.clone();
    }
    if comment.is_empty() {
        comment = format!("{} (Installed via InstallXS)", app_name);
    }

    // Ensure executable permission
    if let Ok(meta) = fs::metadata(&main_exec) {
        let mut perms = meta.permissions();
        perms.set_mode(0o755);
        let _ = fs::set_permissions(&main_exec, perms);
    }

    let mut installed_files = Vec::new();
    // Record all files in content_dir
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
        icon_path.as_deref(),
        Some(&comment),
        Some("Utility;Application;"),
    );

    fs::write(&desktop_file_path, desktop_content)
        .map_err(|e| format!("Failed to write desktop launcher: {}", e))?;
    installed_files.push(desktop_file_path.clone());

    // Create symlink in ~/.local/bin/<slug>
    let bin_link = db::bin_dir().join(&slug);
    let _ = fs::remove_file(&bin_link);
    #[cfg(unix)]
    {
        let _ = std::os::unix::fs::symlink(&main_exec, &bin_link);
        if bin_link.exists() {
            installed_files.push(bin_link.clone());
        }
    }

    // Refresh desktop DB
    let _ = Command::new("update-desktop-database")
        .arg(db::user_applications_dir())
        .output();

    let app = InstalledApp {
        id: format!("installxs:{}", slug),
        name: app_name,
        version: None,
        comment: Some(comment),
        source: AppSource::Tarball,
        exec: Some(main_exec.to_string_lossy().to_string()),
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

fn extract_archive(source: &Path, kind: &FileKind, dest: &Path) -> Result<(), String> {
    match kind {
        FileKind::Zip => {
            let file = fs::File::open(source).map_err(|e| e.to_string())?;
            let mut archive = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
            archive.extract(dest).map_err(|e| format!("Failed to extract zip: {}", e))?;
            Ok(())
        }
        _ => {
            // Use system tar for robust handling of tar.gz, tar.xz, tar.bz2, etc.
            let status = Command::new("tar")
                .args(["-xf", &source.to_string_lossy(), "-C", &dest.to_string_lossy()])
                .status()
                .map_err(|e| format!("Failed to run tar: {}", e))?;

            if status.success() {
                Ok(())
            } else {
                Err("tar extraction failed".to_string())
            }
        }
    }
}

fn resolve_extracted_root(dir: &Path) -> PathBuf {
    if let Ok(entries) = fs::read_dir(dir) {
        let valid_entries: Vec<_> = entries.flatten().collect();
        if valid_entries.len() == 1 && valid_entries[0].file_type().map(|t| t.is_dir()).unwrap_or(false) {
            return valid_entries[0].path();
        }
    }
    dir.to_path_buf()
}

fn find_app_metadata(content_dir: &Path, slug: &str) -> Result<(String, String, Option<PathBuf>, PathBuf), String> {
    let mut detected_name = String::new();
    let mut detected_comment = String::new();
    let mut detected_icon: Option<PathBuf> = None;
    let mut exec_candidate: Option<PathBuf> = None;

    // 1. Search for existing .desktop file
    for entry in WalkDir::new(content_dir).max_depth(3).into_iter().flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("desktop") {
            if let Some(parsed) = parse_desktop_file(path) {
                detected_name = parsed.name;
                detected_comment = parsed.comment.unwrap_or_default();
                if let Some(ref icon) = parsed.icon {
                    // Check if icon is a relative path or file inside directory
                    let icon_cand = content_dir.join(icon);
                    if icon_cand.exists() {
                        detected_icon = Some(icon_cand);
                    }
                }
                break;
            }
        }
    }

    // 2. Search for icons (png / svg)
    if detected_icon.is_none() {
        for entry in WalkDir::new(content_dir).max_depth(4).into_iter().flatten() {
            let path = entry.path();
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if ext == "png" || ext == "svg" {
                detected_icon = Some(path.to_path_buf());
                break;
            }
        }
    }

    // 3. Find main executable
    // Priority:
    // a) Executable named exact match or similar to slug
    // b) Any ELF binary in root or bin/
    // c) Any script with +x
    let mut candidates: Vec<(i32, PathBuf)> = Vec::new();

    for entry in WalkDir::new(content_dir).max_depth(3).into_iter().flatten() {
        let path = entry.path();
        if entry.file_type().is_file() {
            let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            let is_elf = is_elf_executable(path);
            let is_exec_mode = fs::metadata(path).map(|m| (m.permissions().mode() & 0o111) != 0).unwrap_or(false);

            if is_elf || is_exec_mode {
                let mut score = 0;
                if is_elf {
                    score += 100;
                }
                if file_name.eq_ignore_ascii_case(slug) {
                    score += 200;
                } else if file_name.to_lowercase().contains(slug) {
                    score += 50;
                }
                // Depth penalty: favor top-level or bin/
                let depth = entry.depth() as i32;
                score -= depth * 10;

                candidates.push((score, path.to_path_buf()));
            }
        }
    }

    candidates.sort_by(|a, b| b.0.cmp(&a.0));
    if let Some((_, best_path)) = candidates.into_iter().next() {
        exec_candidate = Some(best_path);
    }

    let main_exec = exec_candidate.ok_or_else(|| {
        format!("Could not locate main executable binary inside extracted files")
    })?;

    Ok((detected_name, detected_comment, detected_icon, main_exec))
}
