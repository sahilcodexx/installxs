use std::path::{Path, PathBuf};

pub fn resolve_app_icon(icon_name_or_path: &str) -> Option<PathBuf> {
    let p = Path::new(icon_name_or_path);
    if p.is_absolute() && p.exists() {
        return Some(p.to_path_buf());
    }

    // Try common Linux icon directories in order of best resolution
    let dirs = [
        "/usr/share/pixmaps",
        "/usr/share/icons/hicolor/128x128/apps",
        "/usr/share/icons/hicolor/256x256/apps",
        "/usr/share/icons/hicolor/512x512/apps",
        "/usr/share/icons/hicolor/64x64/apps",
        "/usr/share/icons/hicolor/48x48/apps",
        "/usr/share/icons/hicolor/32x32/apps",
        "/usr/share/icons/hicolor/scalable/apps",
        "/usr/share/icons/breeze/apps/48",
        "/usr/share/icons/breeze-dark/apps/48",
        "/usr/share/icons/breeze/apps/64",
        "/usr/share/icons/breeze/apps/32",
        "/usr/share/icons/breeze/apps/scalable",
        "/usr/share/icons/Adwaita/48x48/apps",
        "/usr/share/icons/Adwaita/scalable/apps",
        "/usr/share/icons/Adwaita/symbolic/apps",
    ];

    let exts = ["png", "svg", "xpm", "webp", "jpg"];

    // 1. Direct match with extensions
    for d in &dirs {
        for ext in &exts {
            let candidate = Path::new(d).join(format!("{}.{}", icon_name_or_path, ext));
            if candidate.exists() {
                return Some(candidate);
            }
        }
        let direct = Path::new(d).join(icon_name_or_path);
        if direct.exists() && direct.is_file() {
            return Some(direct);
        }
    }

    // 2. Also check in user's ~/.local/share/icons
    if let Some(home) = dirs::home_dir() {
        let user_icon_dirs = [
            home.join(".local/share/icons/hicolor/128x128/apps"),
            home.join(".local/share/icons/hicolor/256x256/apps"),
            home.join(".local/share/icons/hicolor/64x64/apps"),
            home.join(".local/share/icons/hicolor/scalable/apps"),
            home.join(".local/share/icons"),
            home.join(".local/share/pixmaps"),
        ];

        for d in &user_icon_dirs {
            for ext in &exts {
                let candidate = d.join(format!("{}.{}", icon_name_or_path, ext));
                if candidate.exists() {
                    return Some(candidate);
                }
            }
            let direct = d.join(icon_name_or_path);
            if direct.exists() && direct.is_file() {
                return Some(direct);
            }
        }
    }

    None
}
