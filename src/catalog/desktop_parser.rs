use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
pub struct ParsedDesktopEntry {
    pub name: String,
    pub exec: Option<String>,
    pub icon: Option<String>,
    pub comment: Option<String>,
    pub no_display: bool,
    pub categories: Vec<String>,
    pub terminal: bool,
    pub path: PathBuf,
    pub version: Option<String>,
}

pub fn parse_desktop_file(path: &Path) -> Option<ParsedDesktopEntry> {
    let content = fs::read_to_string(path).ok()?;
    let mut in_desktop_entry = false;
    let mut fields: HashMap<String, String> = HashMap::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') || trimmed.is_empty() {
            continue;
        }

        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            let section = &trimmed[1..trimmed.len() - 1];
            in_desktop_entry = section == "Desktop Entry";
            continue;
        }

        if in_desktop_entry {
            if let Some((key, val)) = trimmed.split_once('=') {
                let key = key.trim();
                let val = val.trim();
                // Store primary locale or raw key
                if !key.contains('[') {
                    fields.insert(key.to_string(), val.to_string());
                } else if key.starts_with("Name[") && !fields.contains_key("Name") {
                    fields.insert("Name".to_string(), val.to_string());
                }
            }
        }
    }

    let name = fields.get("Name")?.to_string();
    let no_display = fields.get("NoDisplay").map(|v| v.eq_ignore_ascii_case("true")).unwrap_or(false);
    let exec = fields.get("Exec").cloned();
    let icon = fields.get("Icon").cloned();
    let comment = fields.get("Comment").cloned();
    let terminal = fields.get("Terminal").map(|v| v.eq_ignore_ascii_case("true")).unwrap_or(false);
    let categories = fields.get("Categories")
        .map(|c| c.split(';').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect())
        .unwrap_or_default();

    let version = fields.get("X-AppImage-Version")
        .cloned()
        .or_else(|| {
            fields.get("Version")
                .filter(|v| v.as_str() != "1.0" && v.as_str() != "1.1" && v.as_str() != "1.5")
                .cloned()
        });

    Some(ParsedDesktopEntry {
        name,
        exec,
        icon,
        comment,
        no_display,
        categories,
        terminal,
        path: path.to_path_buf(),
        version,
    })
}

pub fn generate_desktop_entry(
    name: &str,
    exec_path: &Path,
    icon_path: Option<&Path>,
    comment: Option<&str>,
    categories: Option<&str>,
) -> String {
    let icon_str = icon_path
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| "application-x-executable".to_string());
    
    let exec_str = exec_path.to_string_lossy();
    let comment_str = comment.unwrap_or("Installed via InstallXS");
    let cat_str = categories.unwrap_or("Utility;Application;");

    format!(
r#"[Desktop Entry]
Type=Application
Version=1.0
Name={name}
Comment={comment_str}
Exec="{exec_str}" %U
Icon={icon_str}
Terminal=false
Categories={cat_str}
StartupNotify=true
X-Installed-By=InstallXS
"#
    )
}
