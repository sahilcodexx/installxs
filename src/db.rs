use std::fs;
use std::path::PathBuf;
use crate::models::{AppReleaseSource, InstalledApp, ManifestDatabase, ReleaseProvider};

pub fn data_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp")).join(".local/share"))
        .join("installxs")
}

pub fn apps_dir() -> PathBuf {
    data_dir().join("apps")
}

pub fn bin_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(".local/bin")
}

pub fn user_applications_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp")).join(".local/share"))
        .join("applications")
}

pub fn database_path() -> PathBuf {
    data_dir().join("apps.json")
}

pub fn update_sources_path() -> PathBuf {
    data_dir().join("update_sources.json")
}

pub fn ensure_directories() -> std::io::Result<()> {
    fs::create_dir_all(data_dir())?;
    fs::create_dir_all(apps_dir())?;
    fs::create_dir_all(bin_dir())?;
    fs::create_dir_all(user_applications_dir())?;
    Ok(())
}

pub fn load_database() -> ManifestDatabase {
    let path = database_path();
    if !path.exists() {
        return ManifestDatabase::default();
    }

    match fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(err) => {
            log::warn!("Failed to read database at {:?}: {}", path, err);
            ManifestDatabase::default()
        }
    }
}

pub fn save_database(db: &ManifestDatabase) -> std::io::Result<()> {
    ensure_directories()?;
    let path = database_path();
    let json = serde_json::to_string_pretty(db)?;
    fs::write(path, json)
}

pub fn add_or_update_app(app: InstalledApp) -> std::io::Result<()> {
    let mut db = load_database();
    db.apps.retain(|a| a.id != app.id);
    db.apps.push(app);
    save_database(&db)
}

pub fn remove_app(app_id: &str) -> std::io::Result<Option<InstalledApp>> {
    let mut db = load_database();
    let removed = if let Some(pos) = db.apps.iter().position(|a| a.id == app_id) {
        Some(db.apps.remove(pos))
    } else {
        None
    };
    save_database(&db)?;
    Ok(removed)
}

#[allow(dead_code)]
pub fn find_app(app_id: &str) -> Option<InstalledApp> {
    let db = load_database();
    db.apps.into_iter().find(|a| a.id == app_id)
}

pub fn load_update_sources() -> Vec<AppReleaseSource> {
    ensure_default_sources();
    let path = update_sources_path();
    if !path.exists() {
        return Vec::new();
    }
    match fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}

pub fn save_update_sources(sources: &[AppReleaseSource]) -> std::io::Result<()> {
    ensure_directories()?;
    let path = update_sources_path();
    let json = serde_json::to_string_pretty(sources)?;
    fs::write(path, json)
}

pub fn get_update_source(app_id: &str, app_name: &str) -> Option<AppReleaseSource> {
    let sources = load_update_sources();
    let norm_name = app_name.trim().to_lowercase();
    sources.into_iter().find(|s| s.app_id == app_id || s.app_name.trim().to_lowercase() == norm_name)
}

pub fn set_update_source(source: AppReleaseSource) -> std::io::Result<()> {
    let mut sources = load_update_sources();
    let norm_name = source.app_name.trim().to_lowercase();
    sources.retain(|s| s.app_id != source.app_id && s.app_name.trim().to_lowercase() != norm_name);
    sources.push(source);
    save_update_sources(&sources)
}

pub fn remove_update_source(app_id: &str, app_name: &str) -> std::io::Result<()> {
    let mut sources = load_update_sources();
    let norm_name = app_name.trim().to_lowercase();
    sources.retain(|s| s.app_id != app_id && s.app_name.trim().to_lowercase() != norm_name);
    save_update_sources(&sources)
}

pub fn ensure_default_sources() {
    let path = update_sources_path();
    let mut sources: Vec<AppReleaseSource> = if path.exists() {
        fs::read_to_string(&path)
            .ok()
            .and_then(|c| serde_json::from_str(&c).ok())
            .unwrap_or_default()
    } else {
        Vec::new()
    };

    // Auto-seed Helium if installed on user system and not yet tracked
    let has_helium = sources.iter().any(|s| s.app_name.eq_ignore_ascii_case("helium"));
    if !has_helium {
        let helium_exists = bin_dir().join("helium").exists()
            || user_applications_dir().read_dir().ok().map(|mut rd| {
                rd.any(|e| e.ok().map(|d| d.file_name().to_string_lossy().to_lowercase().contains("helium")).unwrap_or(false))
            }).unwrap_or(false);

        if helium_exists {
            sources.push(AppReleaseSource {
                app_id: "local:helium".to_string(),
                app_name: "Helium".to_string(),
                provider: ReleaseProvider::GitHub,
                repo_or_url: "imputnet/helium-linux".to_string(),
                asset_keyword: Some("x86_64.AppImage".to_string()),
                current_version: Some("0.17.2.1".to_string()),
            });
            let _ = save_update_sources(&sources);
        }
    }
}
