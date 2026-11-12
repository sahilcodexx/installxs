pub mod desktop_parser;
pub mod icon_resolver;
pub mod updates;
pub mod store;
pub mod backup;

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
use std::process::Command;
use crate::models::{AppSource, InstalledApp};
use crate::db;
use desktop_parser::parse_desktop_file;
use icon_resolver::resolve_app_icon;

pub fn scan_all_apps() -> Vec<InstalledApp> {
    let mut apps = Vec::new();
    let mut seen_keys: HashSet<String> = HashSet::new();
    let pacman_meta = load_pacman_metadata();

    // 1. InstallXS Managed Apps (Highest priority: exact records)
    let managed_db = db::load_database();
    for mut app in managed_db.apps {
        seen_keys.insert(app.name.trim().to_lowercase());
        seen_keys.insert(app.id.clone());
        if let Some(ref desk) = app.desktop_file {
            if let Some(stem) = desk.file_stem().and_then(|s| s.to_str()) {
                seen_keys.insert(stem.to_string());
            }
        }
        if let Some(ref icon_name) = app.icon {
            if let Some(resolved) = resolve_app_icon(icon_name) {
                app.icon = Some(resolved.to_string_lossy().to_string());
            }
        }
        apps.push(app);
    }

    // 2. Flatpak Apps
    scan_flatpak_apps(&mut apps, &mut seen_keys);

    // 3. User Local Applications (~/.local/share/applications)
    scan_user_local_desktop_apps(&mut apps, &mut seen_keys);

    // 4. Arch Linux Native Packages (Pacman & AUR)
    scan_arch_packages(&mut apps, &mut seen_keys, &pacman_meta);

    // Sort alphabetically by name
    apps.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    apps
}

fn scan_flatpak_apps(apps: &mut Vec<InstalledApp>, seen_keys: &mut HashSet<String>) {
    let output = match Command::new("flatpak")
        .args(["list", "--app", "--columns=application,name,version,origin"])
        .output()
    {
        Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout).to_string(),
        _ => return,
    };

    for line in output.lines() {
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.is_empty() {
            continue;
        }
        let app_id = parts[0].trim();
        if app_id.is_empty() {
            continue;
        }
        let name = parts.get(1).map(|s| s.trim()).unwrap_or(app_id);
        let version = parts.get(2).map(|s| s.trim()).filter(|s| !s.is_empty()).map(String::from);
        let norm_name = name.trim().to_lowercase();

        if seen_keys.contains(&norm_name) || seen_keys.contains(app_id) {
            continue;
        }

        seen_keys.insert(norm_name);
        seen_keys.insert(app_id.to_string());

        let icon_path = resolve_app_icon(app_id).map(|p| p.to_string_lossy().to_string());

        apps.push(InstalledApp {
            id: format!("flatpak:{}", app_id),
            name: name.to_string(),
            version,
            comment: Some("Flatpak Application".to_string()),
            source: AppSource::Flatpak,
            exec: Some(format!("flatpak run {}", app_id)),
            icon: icon_path,
            desktop_file: None,
            install_date: None,
            installed_size: None,
            is_managed_by_installxs: false,
            install_path: None,
            installed_files: Vec::new(),
        });
    }
}

fn scan_user_local_desktop_apps(apps: &mut Vec<InstalledApp>, seen_keys: &mut HashSet<String>) {
    let dir = db::user_applications_dir();
    if !dir.exists() {
        return;
    }

    if let Ok(entries) = fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("desktop") {
                let file_stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                if file_stem.is_empty() || seen_keys.contains(file_stem) {
                    continue;
                }

                if let Some(entry) = parse_desktop_file(&path) {
                    if entry.no_display {
                        continue;
                    }

                    let norm_name = entry.name.trim().to_lowercase();
                    if seen_keys.contains(&norm_name) {
                        continue;
                    }

                    seen_keys.insert(norm_name);
                    seen_keys.insert(file_stem.to_string());

                    let is_appimage = entry.exec.as_ref().map(|e| e.to_lowercase().contains("appimage")).unwrap_or(false);
                    let resolved_icon = entry.icon.as_deref().and_then(resolve_app_icon).map(|p| p.to_string_lossy().to_string());

                    apps.push(InstalledApp {
                        id: format!("local:{}", file_stem),
                        name: entry.name,
                        version: None,
                        comment: entry.comment,
                        source: if is_appimage { AppSource::AppImage } else { AppSource::Manual },
                        exec: entry.exec,
                        icon: resolved_icon,
                        desktop_file: Some(path),
                        install_date: None,
                        installed_size: None,
                        is_managed_by_installxs: false,
                        install_path: None,
                        installed_files: Vec::new(),
                    });
                }
            }
        }
    }
}

fn scan_arch_packages(
    apps: &mut Vec<InstalledApp>,
    seen_keys: &mut HashSet<String>,
    pacman_meta: &HashMap<String, (String, u64)>,
) {
    // Explicit pacman packages
    let explicit_pkgs: HashSet<String> = Command::new("pacman")
        .arg("-Qqe")
        .output()
        .ok()
        .and_then(|out| if out.status.success() { Some(String::from_utf8_lossy(&out.stdout).to_string()) } else { None })
        .map(|out| out.lines().map(|l| l.trim().to_string()).collect())
        .unwrap_or_default();

    // Foreign / AUR packages
    let aur_pkgs: HashSet<String> = Command::new("pacman")
        .arg("-Qqm")
        .output()
        .ok()
        .and_then(|out| if out.status.success() { Some(String::from_utf8_lossy(&out.stdout).to_string()) } else { None })
        .map(|out| out.lines().map(|l| l.trim().to_string()).collect())
        .unwrap_or_default();

    let app_dir = Path::new("/usr/share/applications");
    if !app_dir.exists() {
        return;
    }

    let entries = match fs::read_dir(app_dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("desktop") {
            continue;
        }

        let file_stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        if file_stem.is_empty() || seen_keys.contains(file_stem) {
            continue;
        }

        if let Some(desktop) = parse_desktop_file(&path) {
            if desktop.no_display {
                continue;
            }

            let norm_name = desktop.name.trim().to_lowercase();
            if seen_keys.contains(&norm_name) {
                continue;
            }

            let mut is_aur = false;
            let mut is_explicit = false;
            let mut matched_pkg = file_stem.to_string();

            if explicit_pkgs.contains(file_stem) {
                is_explicit = true;
            } else if let Some(ref exec) = desktop.exec {
                let first = exec.split_whitespace().next().unwrap_or("");
                let binary_name = Path::new(first).file_name().and_then(|n| n.to_str()).unwrap_or("");
                if explicit_pkgs.contains(binary_name) {
                    is_explicit = true;
                    matched_pkg = binary_name.to_string();
                }
            }

            if aur_pkgs.contains(&matched_pkg) {
                is_aur = true;
            }

            let (version, installed_size) = pacman_meta
                .get(&matched_pkg)
                .or_else(|| pacman_meta.get(file_stem))
                .map(|(v, s)| (Some(v.clone()), Some(*s)))
                .unwrap_or((None, None));

            seen_keys.insert(norm_name);
            seen_keys.insert(file_stem.to_string());

            let resolved_icon = desktop.icon.as_deref().and_then(resolve_app_icon).map(|p| p.to_string_lossy().to_string());

            apps.push(InstalledApp {
                id: format!("arch:{}", file_stem),
                name: desktop.name,
                version,
                comment: desktop.comment,
                source: if is_aur {
                    AppSource::Aur
                } else if is_explicit {
                    AppSource::Pacman
                } else {
                    AppSource::Pacman
                },
                exec: desktop.exec,
                icon: resolved_icon,
                desktop_file: Some(path),
                install_date: None,
                installed_size,
                is_managed_by_installxs: false,
                install_path: None,
                installed_files: Vec::new(),
            });
        }
    }
}

fn load_pacman_metadata() -> HashMap<String, (String, u64)> {
    let mut map = HashMap::new();
    if let Ok(output) = Command::new("pacman").arg("-Qi").output() {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let mut current_name = String::new();
            let mut current_version = String::new();

            for line in stdout.lines() {
                if let Some(rest) = line.strip_prefix("Name            : ") {
                    current_name = rest.trim().to_string();
                    current_version.clear();
                } else if let Some(rest) = line.strip_prefix("Version         : ") {
                    current_version = rest.trim().to_string();
                } else if let Some(rest) = line.strip_prefix("Installed Size  : ") {
                    let size = parse_pacman_size(rest.trim());
                    if !current_name.is_empty() {
                        map.insert(current_name.clone(), (current_version.clone(), size));
                    }
                }
            }
        }
    }
    map
}

fn parse_pacman_size(s: &str) -> u64 {
    let parts: Vec<&str> = s.split_whitespace().collect();
    if parts.len() < 2 {
        return 0;
    }
    let num: f64 = parts[0].parse().unwrap_or(0.0);
    match parts[1].to_lowercase().as_str() {
        "b" | "bytes" => num as u64,
        "kib" | "kb" => (num * 1024.0) as u64,
        "mib" | "mb" => (num * 1024.0 * 1024.0) as u64,
        "gib" | "gb" => (num * 1024.0 * 1024.0 * 1024.0) as u64,
        _ => num as u64,
    }
}
