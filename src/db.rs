use std::fs;
use std::path::PathBuf;
use crate::models::{InstalledApp, ManifestDatabase};

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
