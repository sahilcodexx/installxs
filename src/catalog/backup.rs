use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use chrono::Utc;
use crate::db;
use crate::models::BackupBundle;

pub fn create_backup_bundle() -> Result<BackupBundle, String> {
    // 1. Pacman explicitly installed packages from official repos
    let mut arch_packages = Vec::new();
    if let Ok(output) = Command::new("pacman").args(["-Qne"]).output() {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                if let Some(pkg) = line.split_whitespace().next() {
                    arch_packages.push(pkg.to_string());
                }
            }
        }
    }

    // 2. AUR foreign explicitly installed packages
    let mut aur_packages = Vec::new();
    if let Ok(output) = Command::new("pacman").args(["-Qme"]).output() {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                if let Some(pkg) = line.split_whitespace().next() {
                    aur_packages.push(pkg.to_string());
                }
            }
        }
    }

    // 3. Flatpak installed applications
    let mut flatpaks = Vec::new();
    if let Ok(output) = Command::new("flatpak").args(["list", "--app", "--columns=application"]).output() {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                let id = line.trim();
                if !id.is_empty() {
                    flatpaks.push(id.to_string());
                }
            }
        }
    }

    // 4. InstallXS managed standalone apps
    let managed_db = db::load_database();

    Ok(BackupBundle {
        export_date: Utc::now(),
        arch_packages,
        aur_packages,
        flatpaks,
        managed_apps: managed_db.apps,
    })
}

pub fn export_backup_to_file(destination: &Path) -> Result<PathBuf, String> {
    let bundle = create_backup_bundle()?;
    let json = serde_json::to_string_pretty(&bundle)
        .map_err(|e| format!("Failed to serialize backup bundle: {}", e))?;
    fs::write(destination, json).map_err(|e| format!("Failed to write backup file: {}", e))?;

    // Also write executable restore bash script next to it
    let restore_script_path = destination.with_file_name("installxs-restore.sh");
    let script = generate_restore_script(&bundle);
    let _ = fs::write(&restore_script_path, script);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(metadata) = fs::metadata(&restore_script_path) {
            let mut perms = metadata.permissions();
            perms.set_mode(0o755);
            let _ = fs::set_permissions(&restore_script_path, perms);
        }
    }

    Ok(destination.to_path_buf())
}

#[allow(dead_code)]
pub fn import_backup_from_file(path: &Path) -> Result<BackupBundle, String> {
    let content = fs::read_to_string(path).map_err(|e| format!("Failed to read backup file: {}", e))?;
    let bundle: BackupBundle = serde_json::from_str(&content)
        .map_err(|e| format!("Invalid InstallXS backup file format: {}", e))?;
    Ok(bundle)
}

pub fn generate_restore_script(bundle: &BackupBundle) -> String {
    let mut script = String::new();
    script.push_str("#!/usr/bin/env bash\n");
    script.push_str("# InstallXS Auto-Restore Script\n");
    script.push_str("set -e\n\n");

    if !bundle.arch_packages.is_empty() {
        script.push_str("# 1. Install Pacman Packages\n");
        script.push_str(&format!("sudo pacman -S --needed --noconfirm {}\n\n", bundle.arch_packages.join(" ")));
    }

    if !bundle.aur_packages.is_empty() {
        script.push_str("# 2. Install AUR Packages\n");
        script.push_str(&format!("yay -S --needed --noconfirm {}\n\n", bundle.aur_packages.join(" ")));
    }

    if !bundle.flatpaks.is_empty() {
        script.push_str("# 3. Install Flatpaks\n");
        for fp in &bundle.flatpaks {
            script.push_str(&format!("flatpak install -y flathub {}\n", fp));
        }
    }

    script
}
