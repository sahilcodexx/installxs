use std::path::PathBuf;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AppSource {
    InstallXs,
    AppImage,
    Tarball,
    Deb,
    Flatpak,
    Pacman,
    Aur,
    Manual,
}

impl AppSource {
    pub fn label(&self) -> &'static str {
        match self {
            AppSource::InstallXs => "InstallXS",
            AppSource::AppImage => "AppImage",
            AppSource::Tarball => "Tarball",
            AppSource::Deb => "Debian",
            AppSource::Flatpak => "Flatpak",
            AppSource::Pacman => "Pacman",
            AppSource::Aur => "AUR",
            AppSource::Manual => "Local",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledApp {
    pub id: String,
    pub name: String,
    pub version: Option<String>,
    pub comment: Option<String>,
    pub source: AppSource,
    pub exec: Option<String>,
    pub icon: Option<String>,
    pub desktop_file: Option<PathBuf>,
    pub install_date: Option<DateTime<Utc>>,
    pub installed_size: Option<u64>,
    pub is_managed_by_installxs: bool,
    pub install_path: Option<PathBuf>,
    pub installed_files: Vec<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ManifestDatabase {
    pub apps: Vec<InstalledApp>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileKind {
    AppImage,
    TarGz,
    TarXz,
    TarBz2,
    Zip,
    Deb,
    ArchPkg,
    DesktopFile,
    ExecutableBinary,
    Unknown,
}

#[derive(Debug, Clone)]
pub struct InstallCandidate {
    pub source_path: PathBuf,
    pub file_name: String,
    pub file_size: u64,
    pub kind: FileKind,
    pub suggested_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub enum InstallState {
    Idle,
    Inspecting(String),
    Installing { app_name: String, step: String },
    Success { app_name: String, message: String },
    Error(String),
}
