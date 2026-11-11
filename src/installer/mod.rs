pub mod appimage;
pub mod tarball;
pub mod deb;
pub mod uninstaller;

use std::fs;
use std::path::Path;
use crate::models::{FileKind, InstallCandidate, InstalledApp};

pub fn detect_candidate(path: &Path) -> Result<InstallCandidate, String> {
    if !path.exists() {
        return Err(format!("File does not exist: {:?}", path));
    }

    let file_name = path.file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| "Invalid file name".to_string())?
        .to_string();

    let meta = fs::metadata(path).map_err(|e| e.to_string())?;
    let file_size = meta.len();

    let lower = file_name.to_lowercase();
    let kind = if lower.ends_with(".appimage") {
        FileKind::AppImage
    } else if lower.ends_with(".tar.gz") || lower.ends_with(".tgz") {
        FileKind::TarGz
    } else if lower.ends_with(".tar.xz") || lower.ends_with(".txz") {
        FileKind::TarXz
    } else if lower.ends_with(".tar.bz2") || lower.ends_with(".tbz2") {
        FileKind::TarBz2
    } else if lower.ends_with(".zip") {
        FileKind::Zip
    } else if lower.ends_with(".deb") {
        FileKind::Deb
    } else if lower.ends_with(".pkg.tar.zst") || lower.ends_with(".pkg.tar.xz") {
        FileKind::ArchPkg
    } else if lower.ends_with(".desktop") {
        FileKind::DesktopFile
    } else {
        // Check if ELF binary
        if is_elf_executable(path) {
            FileKind::ExecutableBinary
        } else {
            FileKind::Unknown
        }
    };

    if kind == FileKind::Unknown {
        return Err(format!("Unsupported file format for '{}'. Supported: .AppImage, .tar.gz, .tar.xz, .zip, .deb, .pkg.tar.zst, executable binary", file_name));
    }

    let suggested_name = sanitize_app_name(&file_name);

    Ok(InstallCandidate {
        source_path: path.to_path_buf(),
        file_name,
        file_size,
        kind,
        suggested_name,
    })
}

pub fn is_elf_executable(path: &Path) -> bool {
    use std::io::Read;
    if let Ok(mut file) = fs::File::open(path) {
        let mut magic = [0u8; 4];
        if file.read_exact(&mut magic).is_ok() {
            return magic == [0x7f, b'E', b'L', b'F'];
        }
    }
    false
}

pub fn sanitize_app_name(file_name: &str) -> String {
    let mut name = file_name.to_string();
    let extensions = [
        ".appimage", ".tar.gz", ".tar.xz", ".tar.bz2", ".tgz", ".txz",
        ".zip", ".deb", ".pkg.tar.zst", ".pkg.tar.xz", ".bin", ".desktop"
    ];

    for ext in extensions {
        if name.to_lowercase().ends_with(ext) {
            name = name[..name.len() - ext.len()].to_string();
            break;
        }
    }

    // Strip common arch/version suffixes: e.g. -x86_64, -linux, -amd64, _amd64
    let suffixes = ["-x86_64", "_x86_64", "-amd64", "_amd64", "-linux", "_linux", "-x64", "_x64"];
    for suf in suffixes {
        if name.to_lowercase().ends_with(suf) {
            name = name[..name.len() - suf.len()].to_string();
        }
    }

    // Capitalize words or format cleanly
    name.replace(['-', '_'], " ").trim().to_string()
}

pub fn install_candidate(candidate: &InstallCandidate) -> Result<InstalledApp, String> {
    match candidate.kind {
        FileKind::AppImage => appimage::install(candidate),
        FileKind::TarGz | FileKind::TarXz | FileKind::TarBz2 | FileKind::Zip => {
            tarball::install(candidate)
        }
        FileKind::Deb => deb::install(candidate),
        FileKind::ExecutableBinary => install_single_binary(candidate),
        FileKind::DesktopFile => install_single_desktop(candidate),
        FileKind::ArchPkg => {
            Err("Arch packages (.pkg.tar.zst) should be installed via 'sudo pacman -U <file>'".to_string())
        }
        FileKind::Unknown => Err("Unknown or unsupported file format".to_string()),
    }
}

fn install_single_binary(candidate: &InstallCandidate) -> Result<InstalledApp, String> {
    use std::os::unix::fs::PermissionsExt;
    use chrono::Utc;
    use crate::catalog::desktop_parser::generate_desktop_entry;
    use crate::db;

    db::ensure_directories().map_err(|e| e.to_string())?;
    let slug = slugify(&candidate.suggested_name);
    let target_bin = db::bin_dir().join(&slug);

    fs::copy(&candidate.source_path, &target_bin).map_err(|e| e.to_string())?;

    let mut perms = fs::metadata(&target_bin).map_err(|e| e.to_string())?.permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&target_bin, perms).map_err(|e| e.to_string())?;

    let desktop_path = db::user_applications_dir().join(format!("installxs-{}.desktop", &slug));
    let desktop_content = generate_desktop_entry(
        &candidate.suggested_name,
        &target_bin,
        None,
        Some("Installed binary via InstallXS"),
        Some("Utility;Application;"),
    );
    fs::write(&desktop_path, desktop_content).map_err(|e| e.to_string())?;

    let app = InstalledApp {
        id: format!("installxs:{}", slug),
        name: candidate.suggested_name.clone(),
        version: None,
        comment: Some("Standalone binary installed via InstallXS".to_string()),
        source: crate::models::AppSource::InstallXs,
        exec: Some(target_bin.to_string_lossy().to_string()),
        icon: None,
        desktop_file: Some(desktop_path.clone()),
        install_date: Some(Utc::now()),
        installed_size: Some(candidate.file_size),
        is_managed_by_installxs: true,
        install_path: None,
        installed_files: vec![target_bin, desktop_path],
    };

    db::add_or_update_app(app.clone()).map_err(|e| e.to_string())?;
    Ok(app)
}

fn install_single_desktop(candidate: &InstallCandidate) -> Result<InstalledApp, String> {
    use chrono::Utc;
    use crate::catalog::desktop_parser::parse_desktop_file;
    use crate::db;

    db::ensure_directories().map_err(|e| e.to_string())?;
    let slug = slugify(&candidate.suggested_name);
    let target_desktop = db::user_applications_dir().join(format!("installxs-{}.desktop", &slug));

    fs::copy(&candidate.source_path, &target_desktop).map_err(|e| e.to_string())?;
    let parsed = parse_desktop_file(&target_desktop);

    let app = InstalledApp {
        id: format!("installxs:{}", slug),
        name: parsed.as_ref().map(|p| p.name.clone()).unwrap_or_else(|| candidate.suggested_name.clone()),
        version: None,
        comment: parsed.as_ref().and_then(|p| p.comment.clone()),
        source: crate::models::AppSource::InstallXs,
        exec: parsed.as_ref().and_then(|p| p.exec.clone()),
        icon: parsed.as_ref().and_then(|p| p.icon.clone()),
        desktop_file: Some(target_desktop.clone()),
        install_date: Some(Utc::now()),
        installed_size: Some(candidate.file_size),
        is_managed_by_installxs: true,
        install_path: None,
        installed_files: vec![target_desktop],
    };

    db::add_or_update_app(app.clone()).map_err(|e| e.to_string())?;
    Ok(app)
}

pub fn slugify(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_alphanumeric() { c.to_ascii_lowercase() } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

pub fn scan_downloaded_packages() -> Vec<InstallCandidate> {
    let mut candidates = Vec::new();
    let Some(home) = dirs::home_dir() else { return candidates; };
    let downloads = home.join("Downloads");
    if !downloads.is_dir() { return candidates; }

    let extensions = [
        ".appimage", ".tar.gz", ".tar.xz", ".tar.bz2", ".tgz", ".txz",
        ".zip", ".deb", ".pkg.tar.zst", ".pkg.tar.xz"
    ];

    if let Ok(entries) = fs::read_dir(&downloads) {
        let mut files: Vec<std::path::PathBuf> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
                    let lower = name.to_lowercase();
                    extensions.iter().any(|ext| lower.ends_with(ext))
                } else {
                    false
                }
            })
            .collect();

        // Sort newest first
        files.sort_by(|a, b| {
            let meta_a = a.metadata().and_then(|m| m.modified()).ok();
            let meta_b = b.metadata().and_then(|m| m.modified()).ok();
            meta_b.cmp(&meta_a)
        });

        for path in files.into_iter().take(24) {
            if let Ok(cand) = detect_candidate(&path) {
                candidates.push(cand);
            }
        }
    }

    candidates
}
