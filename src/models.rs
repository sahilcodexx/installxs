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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResidualTarget {
    pub path: PathBuf,
    pub label: String,
    pub size_bytes: u64,
    pub is_cache: bool,
    pub is_config: bool,
    pub selected: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppResiduals {
    pub targets: Vec<ResidualTarget>,
    pub total_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AvailableUpdate {
    pub package_name: String,
    pub current_version: String,
    pub new_version: String,
    pub source: AppSource,
    pub repository: Option<String>,
    pub download_size: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StoreSource {
    Flathub,
    Aur,
    AppImageHub,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StorePackage {
    pub id: String,
    pub name: String,
    pub summary: String,
    pub icon_url: Option<String>,
    pub version: Option<String>,
    pub source: StoreSource,
    pub install_cmd: String,
    pub is_installed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupBundle {
    pub export_date: DateTime<Utc>,
    pub arch_packages: Vec<String>,
    pub aur_packages: Vec<String>,
    pub flatpaks: Vec<String>,
    pub managed_apps: Vec<InstalledApp>,
}
