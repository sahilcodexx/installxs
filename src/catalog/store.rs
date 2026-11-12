use std::process::Command;
use serde::Deserialize;
use crate::models::{StorePackage, StoreSource};

#[derive(Debug, Deserialize)]
struct AurResponse {
    results: Option<Vec<AurItem>>,
}

#[derive(Debug, Deserialize)]
struct AurItem {
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "Description")]
    description: Option<String>,
    #[serde(rename = "Version")]
    version: Option<String>,
}

fn url_encode(input: &str) -> String {
    let mut encoded = String::new();
    for b in input.bytes() {
        match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(b as char);
            }
            _ => {
                encoded.push_str(&format!("%{:02X}", b));
            }
        }
    }
    encoded
}

pub fn search_store(query: &str) -> Vec<StorePackage> {
    let q = query.trim();
    let mut packages = Vec::new();

    if q.is_empty() {
        return get_featured_store_packages();
    }

    // 1. Query AUR via RPC
    let aur_url = format!("https://aur.archlinux.org/rpc/v5/search/{}", url_encode(q));
    if let Ok(output) = Command::new("curl").args(["-s", "--max-time", "4", &aur_url]).output() {
        if output.status.success() {
            if let Ok(res) = serde_json::from_slice::<AurResponse>(&output.stdout) {
                if let Some(items) = res.results {
                    for item in items.into_iter().take(12) {
                        packages.push(StorePackage {
                            id: format!("aur:{}", item.name),
                            name: item.name.clone(),
                            summary: item.description.unwrap_or_else(|| "Arch User Repository package".to_string()),
                            icon_url: None,
                            version: item.version,
                            source: StoreSource::Aur,
                            install_cmd: format!("yay -S --noconfirm {}", item.name),
                            is_installed: is_package_installed(&item.name),
                        });
                    }
                }
            }
        }
    }

    // 2. Query Flatpak search
    if let Ok(output) = Command::new("flatpak")
        .args(["search", q, "--columns=name,application,version,description"])
        .output()
    {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines().take(8) {
                let parts: Vec<&str> = line.split('\t').collect();
                if parts.len() >= 2 {
                    let name = parts[0].trim();
                    let app_id = parts[1].trim();
                    let version = if parts.len() > 2 { Some(parts[2].trim().to_string()) } else { None };
                    let desc = if parts.len() > 3 { parts[3].trim().to_string() } else { "Flathub application".to_string() };

                    if !app_id.is_empty() {
                        packages.push(StorePackage {
                            id: format!("flatpak:{}", app_id),
                            name: if name.is_empty() { app_id.to_string() } else { name.to_string() },
                            summary: desc,
                            icon_url: None,
                            version,
                            source: StoreSource::Flathub,
                            install_cmd: format!("flatpak install -y flathub {}", app_id),
                            is_installed: is_flatpak_installed(app_id),
                        });
                    }
                }
            }
        }
    }

    packages
}

pub fn get_featured_store_packages() -> Vec<StorePackage> {
    vec![
        StorePackage {
            id: "aur:visual-studio-code-bin".to_string(),
            name: "Visual Studio Code".to_string(),
            summary: "Code editing. Redefined. Official binary package from Microsoft.".to_string(),
            icon_url: None,
            version: Some("latest".to_string()),
            source: StoreSource::Aur,
            install_cmd: "yay -S --noconfirm visual-studio-code-bin".to_string(),
            is_installed: is_package_installed("visual-studio-code-bin"),
        },
        StorePackage {
            id: "flatpak:md.obsidian.Obsidian".to_string(),
            name: "Obsidian".to_string(),
            summary: "Extensible markdown knowledge base and local personal wiki.".to_string(),
            icon_url: None,
            version: Some("latest".to_string()),
            source: StoreSource::Flathub,
            install_cmd: "flatpak install -y flathub md.obsidian.Obsidian".to_string(),
            is_installed: is_flatpak_installed("md.obsidian.Obsidian"),
        },
        StorePackage {
            id: "flatpak:com.discordapp.Discord".to_string(),
            name: "Discord".to_string(),
            summary: "All-in-one voice and text chat for gamers and communities.".to_string(),
            icon_url: None,
            version: Some("latest".to_string()),
            source: StoreSource::Flathub,
            install_cmd: "flatpak install -y flathub com.discordapp.Discord".to_string(),
            is_installed: is_flatpak_installed("com.discordapp.Discord"),
        },
        StorePackage {
            id: "aur:spotify".to_string(),
            name: "Spotify".to_string(),
            summary: "Proprietary music streaming service client.".to_string(),
            icon_url: None,
            version: Some("latest".to_string()),
            source: StoreSource::Aur,
            install_cmd: "yay -S --noconfirm spotify".to_string(),
            is_installed: is_package_installed("spotify"),
        },
        StorePackage {
            id: "flatpak:com.valvesoftware.Steam".to_string(),
            name: "Steam".to_string(),
            summary: "Digital software and video game distribution platform.".to_string(),
            icon_url: None,
            version: Some("latest".to_string()),
            source: StoreSource::Flathub,
            install_cmd: "flatpak install -y flathub com.valvesoftware.Steam".to_string(),
            is_installed: is_flatpak_installed("com.valvesoftware.Steam"),
        },
        StorePackage {
            id: "flatpak:org.videolan.VLC".to_string(),
            name: "VLC Media Player".to_string(),
            summary: "Cross-platform multimedia player and framework for almost all codecs.".to_string(),
            icon_url: None,
            version: Some("latest".to_string()),
            source: StoreSource::Flathub,
            install_cmd: "flatpak install -y flathub org.videolan.VLC".to_string(),
            is_installed: is_flatpak_installed("org.videolan.VLC"),
        },
    ]
}

pub fn install_store_package(pkg: &StorePackage) -> Result<(), String> {
    match pkg.source {
        StoreSource::Flathub => {
            let app_id = pkg.id.strip_prefix("flatpak:").unwrap_or(&pkg.id);
            let status = Command::new("flatpak")
                .args(["install", "-y", "flathub", app_id])
                .status()
                .map_err(|e| format!("Failed to run flatpak install: {}", e))?;
            if !status.success() {
                return Err(format!("Flatpak install failed for '{}'", app_id));
            }
        }
        StoreSource::Aur => {
            let pkg_name = pkg.id.strip_prefix("aur:").unwrap_or(&pkg.id);
            let status = Command::new("yay")
                .args(["-S", "--noconfirm", pkg_name])
                .status()
                .map_err(|e| format!("Failed to run yay: {}", e))?;
            if !status.success() {
                return Err(format!("AUR installation failed for '{}'", pkg_name));
            }
        }
        StoreSource::AppImageHub => {
            return Err("AppImageHub direct install requires a direct download link".to_string());
        }
    }
    Ok(())
}

fn is_package_installed(pkg_name: &str) -> bool {
    Command::new("pacman")
        .args(["-Qq", pkg_name])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn is_flatpak_installed(app_id: &str) -> bool {
    Command::new("flatpak")
        .args(["info", app_id])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}
