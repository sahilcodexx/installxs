use gpui::{
    div, img, prelude::*, px, rgb, Context, Entity, ExternalPaths, FontWeight, IntoElement,
    ParentElement, Render, Styled, Window,
};
use gpui_m3::motion::{self, Spring};
use gpui_m3::theme::alpha;
use gpui_m3::{Button, ButtonSize, Icon, IconButton, NavItem, Scheme, TextField, TextFieldEvent, Theme};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::catalog::{self, backup, store, updates};
use crate::db;
use crate::installer::{self, detect_candidate, uninstaller};
use crate::models::{
    AppReleaseSource, AppResiduals, AppSource, AvailableUpdate, InstallCandidate, InstallState,
    InstalledApp, StorePackage,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavTab {
    Applications,
    Managed,
    Updates,
    Discover,
    Pacman,
    Aur,
    AppImage,
    Flatpak,
    Local,
    DropInstaller,
    Settings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterChip {
    All,
    Pacman,
    Aur,
    AppImage,
    Flatpak,
    Local,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortMode {
    Name,
    Source,
    Size,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    Grid,
    List,
}

pub fn desktop_scheme(dark: bool) -> Scheme {
    let mut s = if dark {
        Scheme::baseline_dark()
    } else {
        Scheme::baseline_light()
    };
    let c = |hex| rgb(hex).into();

    s.primary = c(if dark { 0x65a7f5 } else { 0x2677c9 });
    s.on_primary = c(0xffffff);
    s.primary_container = c(if dark { 0x263d57 } else { 0xd6e9ff });
    s.on_primary_container = c(if dark { 0xdcecff } else { 0x173a5e });
    s.secondary = c(if dark { 0xbac7d5 } else { 0x536273 });
    s.on_secondary = c(0xffffff);
    s.secondary_container = c(if dark { 0x2d3742 } else { 0xe2e9f0 });
    s.on_secondary_container = c(if dark { 0xe0e8f0 } else { 0x26323d });
    s.tertiary = c(if dark { 0x9cb9d6 } else { 0x506b86 });
    s.on_tertiary = c(0xffffff);
    s.tertiary_container = c(if dark { 0x2b3d50 } else { 0xe0ebf7 });
    s.on_tertiary_container = c(if dark { 0xe1edf9 } else { 0x24394d });
    s.surface = c(if dark { 0x191c20 } else { 0xf7f8fa });
    s.surface_dim = c(if dark { 0x14171a } else { 0xe7e9ec });
    s.surface_bright = c(if dark { 0x2d3238 } else { 0xffffff });
    s.surface_container_lowest = c(if dark { 0x111417 } else { 0xffffff });
    s.surface_container_low = c(if dark { 0x1d2126 } else { 0xf1f3f5 });
    s.surface_container = c(if dark { 0x23282e } else { 0xecedef });
    s.surface_container_high = c(if dark { 0x2a3036 } else { 0xe4e7ea });
    s.surface_container_highest = c(if dark { 0x333a41 } else { 0xdce0e4 });
    s.on_surface = c(if dark { 0xf0f3f6 } else { 0x202326 });
    s.on_surface_variant = c(if dark { 0xaeb7c0 } else { 0x555b61 });
    s.outline = c(if dark { 0x747f89 } else { 0x727980 });
    s.outline_variant = c(if dark { 0x343b43 } else { 0xd5d9dd });
    s.inverse_surface = c(if dark { 0xe8ebf2 } else { 0x303336 });
    s.inverse_on_surface = c(if dark { 0x1a1e28 } else { 0xf2f3f4 });
    s.inverse_primary = c(if dark { 0x3b49b8 } else { 0x9fcaff });
    s.scrim = c(0x000000);
    if dark {
        // BoardUI dark: neutral grays (no color tint), blue accent ramp,
        // content sits on the darkest ground with cards one step up.
        s.primary = c(0x2b7fff);
        s.primary_container = c(0x1e2f52);
        s.on_primary_container = c(0xc9d8ff);
        s.secondary_container = c(0x2b2b31);
        s.on_secondary_container = c(0xd6d6dd);
        s.tertiary_container = c(0x2b2f3a);
        s.on_tertiary_container = c(0xd8dee9);
        s.surface = c(0x16161a);
        s.surface_dim = c(0x101014);
        s.surface_bright = c(0x232329);
        s.surface_container_lowest = c(0x1b1b20);
        s.surface_container_low = c(0x212127);
        s.surface_container = c(0x28282e);
        s.surface_container_high = c(0x303037);
        s.surface_container_highest = c(0x3a3a42);
        s.on_surface = c(0xf5f5f7);
        s.on_surface_variant = c(0xa1a1aa);
        s.outline = c(0x4c4c56);
        s.outline_variant = c(0x2e2e36);
    }
    s.error = c(if dark { 0xffb4ab } else { 0xba1a1a });
    s.on_error = c(if dark { 0x690005 } else { 0xffffff });
    s.error_container = c(if dark { 0x93000a } else { 0xffdad6 });
    s.on_error_container = c(if dark { 0xffdad6 } else { 0x410002 });
    s.success = c(if dark { 0x8ed49b } else { 0x246b35 });
    s.warning = c(if dark { 0xf2c66d } else { 0x805500 });
    s
}

pub fn format_file_size(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;

    let b = bytes as f64;
    if b >= GB {
        format!("{:.2} GB", b / GB)
    } else if b >= MB {
        format!("{:.1} MB", b / MB)
    } else if b >= KB {
        format!("{:.0} KB", b / KB)
    } else if bytes > 0 {
        format!("{} B", bytes)
    } else {
        "0 B".to_string()
    }
}

pub struct AppView {
    pub active_tab: NavTab,
    pub active_filter: FilterChip,
    pub sort_mode: SortMode,
    pub view_mode: ViewMode,
    pub search_field: Entity<TextField>,
    pub search_query: String,
    pub apps: Vec<InstalledApp>,
    pub is_scanning: bool,
    pub candidate: Option<InstallCandidate>,
    pub downloaded_packages: Vec<InstallCandidate>,
    pub install_state: InstallState,
    pub selected_for_uninstall: Option<InstalledApp>,
    pub selected_app_details: Option<InstalledApp>,
    pub dark_mode: bool,
    pub toast: Option<(String, bool)>,
    // Release Updater & Source State
    pub update_source_field: Entity<TextField>,
    pub update_source_input: String,
    pub editing_update_source: bool,
    pub checking_app_update: bool,
    pub app_update_info: Option<AvailableUpdate>,
    // New Feature State
    pub available_updates: Vec<AvailableUpdate>,
    pub is_checking_updates: bool,
    pub updating_pkg: Option<String>,
    pub store_packages: Vec<StorePackage>,
    pub is_searching_store: bool,
    pub installing_store_pkg: Option<String>,
    pub uninstall_residuals: Option<AppResiduals>,
    pub selected_residual_indices: HashSet<usize>,
    pub backup_status: Option<String>,
}

impl AppView {
    pub fn new(initial_file: Option<String>, cx: &mut Context<Self>) -> Self {
        let mut initial_candidate = None;
        let mut initial_tab = NavTab::Applications;

        if let Some(ref path_str) = initial_file {
            if path_str == "--drop" || path_str == "drop" {
                initial_tab = NavTab::DropInstaller;
            } else {
                let path = PathBuf::from(path_str);
                if let Ok(cand) = detect_candidate(&path) {
                    initial_candidate = Some(cand);
                    initial_tab = NavTab::DropInstaller;
                }
            }
        }

        let search_field = cx.new(|cx| {
            TextField::new(cx)
                .placeholder("Search applications, packages or files...")
                .bare()
        });

        cx.subscribe(
            &search_field,
            |this: &mut Self, _, event: &TextFieldEvent, cx| match event {
                TextFieldEvent::Changed(text) => {
                    this.search_query = text.to_string();
                    cx.notify();
                }
                TextFieldEvent::Submitted(text) if this.active_tab == NavTab::Discover => {
                    this.search_store_packages(text.as_ref(), cx);
                }
                _ => {}
            },
        )
        .detach();

        let update_source_field = cx.new(|cx| {
            TextField::new(cx)
                .placeholder("e.g. imputnet/helium-linux or GitHub/GitLab release URL")
                .bare()
        });

        cx.subscribe(
            &update_source_field,
            |this: &mut Self, _, event: &TextFieldEvent, cx| match event {
                TextFieldEvent::Changed(text) => {
                    this.update_source_input = text.to_string();
                    cx.notify();
                }
                TextFieldEvent::Submitted(_) => {
                    this.save_current_app_update_source(cx);
                }
                _ => {}
            },
        )
        .detach();

        let mut view = Self {
            active_tab: initial_tab,
            active_filter: FilterChip::All,
            sort_mode: SortMode::Name,
            view_mode: ViewMode::Grid,
            search_field,
            search_query: String::new(),
            apps: Vec::new(),
            is_scanning: false,
            candidate: initial_candidate,
            downloaded_packages: Vec::new(),
            install_state: InstallState::Idle,
            selected_for_uninstall: None,
            selected_app_details: None,
            dark_mode: true,
            toast: None,
            update_source_field,
            update_source_input: String::new(),
            editing_update_source: false,
            checking_app_update: false,
            app_update_info: None,
            available_updates: Vec::new(),
            is_checking_updates: false,
            updating_pkg: None,
            store_packages: store::get_featured_store_packages(),
            is_searching_store: false,
            installing_store_pkg: None,
            uninstall_residuals: None,
            selected_residual_indices: HashSet::new(),
            backup_status: None,
        };

        view.refresh_apps(cx);
        view
    }

    pub fn refresh_apps(&mut self, cx: &mut Context<Self>) {
        self.is_scanning = true;
        self.apps = catalog::scan_all_apps();
        self.downloaded_packages = installer::scan_downloaded_packages();
        self.is_scanning = false;
        cx.notify();
    }

    pub fn open_app_details(&mut self, app: InstalledApp, cx: &mut Context<Self>) {
        self.selected_app_details = Some(app.clone());
        self.editing_update_source = false;
        self.app_update_info = None;
        if let Some(src) = db::get_update_source(&app.id, &app.name) {
            self.update_source_input = src.repo_or_url.clone();
            self.check_single_app_update(cx);
        } else {
            self.update_source_input = String::new();
        }
        cx.notify();
    }

    pub fn save_current_app_update_source(&mut self, cx: &mut Context<Self>) {
        let app = match self.selected_app_details.as_ref() {
            Some(a) => a.clone(),
            None => return,
        };

        let input = self.update_source_input.trim();
        if input.is_empty() {
            return;
        }

        if let Some((provider, owner, repo)) = crate::catalog::release_updater::parse_repo_string(input) {
            let repo_or_url = format!("{}/{}", owner, repo);
            let src = AppReleaseSource {
                app_id: app.id.clone(),
                app_name: app.name.clone(),
                provider: provider.clone(),
                repo_or_url: repo_or_url.clone(),
                asset_keyword: None,
                current_version: app.version.clone(),
            };
            if let Err(e) = db::set_update_source(src) {
                self.toast = Some((format!("Failed to save source: {}", e), true));
            } else {
                self.toast = Some((format!("Linked to {} ({})", repo_or_url, provider.label()), false));
                self.editing_update_source = false;
                self.check_single_app_update(cx);
            }
        } else {
            self.toast = Some(("Invalid repository format. Use owner/repo or release URL.".to_string(), true));
        }
        cx.notify();
    }

    pub fn check_single_app_update(&mut self, cx: &mut Context<Self>) {
        let app = match self.selected_app_details.as_ref() {
            Some(a) => a.clone(),
            None => return,
        };

        if let Some(src) = db::get_update_source(&app.id, &app.name) {
            self.checking_app_update = true;
            cx.notify();
            match crate::catalog::release_updater::check_single_release_source(&src) {
                Ok(Some(upd)) => {
                    self.app_update_info = Some(upd);
                }
                Ok(None) => {
                    self.app_update_info = None;
                    self.toast = Some((format!("'{}' is already up to date!", app.name), false));
                }
                Err(err) => {
                    self.toast = Some((format!("Update check failed: {}", err), true));
                }
            }
            self.checking_app_update = false;
            cx.notify();
        }
    }

    pub fn unlink_update_source(&mut self, cx: &mut Context<Self>) {
        let app = match self.selected_app_details.as_ref() {
            Some(a) => a.clone(),
            None => return,
        };
        let _ = db::remove_update_source(&app.id, &app.name);
        self.app_update_info = None;
        self.editing_update_source = false;
        self.toast = Some(("Unlinked release source".to_string(), false));
        cx.notify();
    }

    fn render_filter_chip(
        &self,
        id: &'static str,
        label: &'static str,
        chip: FilterChip,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let is_active = self.active_filter == chip;
        div()
            .id(id)
            .px_3p5()
            .py_1p5()
            .rounded_full()
            .cursor_pointer()
            .text_size(px(12.5))
            .font_weight(if is_active { FontWeight::BOLD } else { FontWeight::MEDIUM })
            .when(is_active, |el| {
                el.bg(rgb(0xcfbcff)).text_color(rgb(0x1e1035))
            })
            .when(!is_active, |el| {
                el.bg(rgb(0x201c28))
                    .border_1()
                    .border_color(rgb(0x322a3d))
                    .text_color(rgb(0xcac4d0))
                    .hover(|h| h.bg(rgb(0x2b2436)).text_color(rgb(0xffffff)))
            })
            .on_click(cx.listener(move |this, _, _, cx| {
                this.active_filter = chip;
                cx.notify();
            }))
            .child(label)
    }

    pub fn check_updates(&mut self, cx: &mut Context<Self>) {
        self.is_checking_updates = true;
        cx.notify();
        self.available_updates = updates::check_system_updates();
        self.is_checking_updates = false;
        cx.notify();
    }

    pub fn update_single_package(&mut self, update: AvailableUpdate, cx: &mut Context<Self>) {
        let name = update.package_name.clone();
        self.updating_pkg = Some(name.clone());
        cx.notify();
        match updates::run_package_update(&update) {
            Ok(_) => {
                self.toast = Some((
                    format!("Updated '{}' to {}", name, update.new_version),
                    false,
                ));
                self.available_updates.retain(|u| u.package_name != name);
            }
            Err(err) => {
                self.toast = Some((err, true));
            }
        }
        self.updating_pkg = None;
        self.refresh_apps(cx);
        cx.notify();
    }

    pub fn update_all_packages(&mut self, cx: &mut Context<Self>) {
        let list = self.available_updates.clone();
        for u in list {
            let _ = updates::run_package_update(&u);
        }
        self.toast = Some(("All available updates processed!".to_string(), false));
        self.check_updates(cx);
        self.refresh_apps(cx);
        cx.notify();
    }

    pub fn search_store_packages(&mut self, query: &str, cx: &mut Context<Self>) {
        self.is_searching_store = true;
        cx.notify();
        self.store_packages = store::search_store(query);
        self.is_searching_store = false;
        cx.notify();
    }

    pub fn install_from_store(&mut self, pkg: StorePackage, cx: &mut Context<Self>) {
        let name = pkg.name.clone();
        self.installing_store_pkg = Some(pkg.id.clone());
        cx.notify();
        match store::install_store_package(&pkg) {
            Ok(_) => {
                self.toast = Some((format!("'{}' installed successfully!", name), false));
                self.refresh_apps(cx);
            }
            Err(err) => {
                self.toast = Some((err, true));
            }
        }
        self.installing_store_pkg = None;
        cx.notify();
    }

    pub fn export_machine_backup(&mut self, cx: &mut Context<Self>) {
        if let Some(home) = dirs::home_dir() {
            let dest = home.join("installxs-backup.json");
            match backup::export_backup_to_file(&dest) {
                Ok(p) => {
                    self.backup_status = Some(format!("Backup saved to {}", p.display()));
                    self.toast = Some((format!("Saved backup bundle to {}", p.display()), false));
                }
                Err(e) => {
                    self.backup_status = Some(format!("Backup failed: {}", e));
                    self.toast = Some((e, true));
                }
            }
        }
        cx.notify();
    }

    pub fn prepare_uninstall(&mut self, app: InstalledApp, cx: &mut Context<Self>) {
        let residuals = uninstaller::detect_residuals(&app);
        let mut selected_indices = HashSet::new();
        for i in 0..residuals.targets.len() {
            selected_indices.insert(i);
        }
        self.uninstall_residuals = Some(residuals);
        self.selected_residual_indices = selected_indices;
        self.selected_for_uninstall = Some(app);
        cx.notify();
    }

    pub fn execute_deep_uninstall(&mut self, purge_residuals: bool, cx: &mut Context<Self>) {
        if let Some(app) = self.selected_for_uninstall.take() {
            let name = app.name.clone();
            let targets_to_purge: Vec<PathBuf> = if purge_residuals {
                if let Some(ref res) = self.uninstall_residuals {
                    res.targets
                        .iter()
                        .enumerate()
                        .filter(|(idx, _)| self.selected_residual_indices.contains(idx))
                        .map(|(_, t)| t.path.clone())
                        .collect()
                } else {
                    Vec::new()
                }
            } else {
                Vec::new()
            };

            match uninstaller::uninstall_with_residuals(&app, &targets_to_purge) {
                Ok(freed_bytes) => {
                    let msg = if freed_bytes > 0 {
                        format!(
                            "Cleaned '{}' and purged {}",
                            name,
                            format_file_size(freed_bytes)
                        )
                    } else {
                        format!("Removed '{}' successfully", name)
                    };
                    self.toast = Some((msg, false));
                    self.selected_app_details = None;
                    self.uninstall_residuals = None;
                    self.refresh_apps(cx);
                }
                Err(err) => {
                    self.toast = Some((format!("Failed to uninstall: {}", err), true));
                }
            }
        }
        cx.notify();
    }

    pub fn pick_file(&mut self, cx: &mut Context<Self>) {
        let picked = rfd::FileDialog::new()
            .set_title("Select Package to Install (.AppImage, .tar, .zip, .deb)")
            .add_filter(
                "Linux App Packages",
                &[
                    "AppImage", "tar.gz", "tgz", "tar.xz", "txz", "zip", "deb", "bin", "desktop",
                ],
            )
            .pick_file();

        if let Some(path) = picked {
            self.stage_file(path, cx);
        }
    }

    pub fn stage_file(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        match detect_candidate(&path) {
            Ok(cand) => {
                self.candidate = Some(cand);
                self.install_state = InstallState::Idle;
                self.active_tab = NavTab::DropInstaller;
                self.toast = None;
            }
            Err(err) => {
                self.toast = Some((err, true));
            }
        }
        cx.notify();
    }

    pub fn install_current_candidate(&mut self, cx: &mut Context<Self>) {
        let Some(candidate) = self.candidate.clone() else {
            return;
        };

        self.install_state = InstallState::Installing {
            app_name: candidate.suggested_name.clone(),
            step: "Extracting archive & setting up FreeDesktop launcher...".to_string(),
        };
        cx.notify();

        match installer::install_candidate(&candidate) {
            Ok(app) => {
                let name = app.name.clone();
                self.install_state = InstallState::Success {
                    app_name: name.clone(),
                    message: format!("'{}' installed successfully! Ready to launch.", name),
                };
                self.refresh_apps(cx);
            }
            Err(err) => {
                self.install_state = InstallState::Error(err);
            }
        }
        cx.notify();
    }

    #[allow(dead_code)]
    pub fn execute_uninstall(&mut self, cx: &mut Context<Self>) {
        if let Some(app) = self.selected_for_uninstall.take() {
            let name = app.name.clone();
            match uninstaller::uninstall(&app) {
                Ok(_) => {
                    self.toast = Some((format!("Removed '{}' successfully", name), false));
                    self.selected_app_details = None;
                    self.refresh_apps(cx);
                }
                Err(err) => {
                    self.toast = Some((format!("Failed to uninstall: {}", err), true));
                }
            }
        }
        cx.notify();
    }

    pub fn toggle_theme(&mut self, cx: &mut Context<Self>) {
        self.dark_mode = !self.dark_mode;
        Theme::set_scheme(desktop_scheme(self.dark_mode), self.dark_mode, cx);
        cx.notify();
    }

    pub fn launch_app(&self, app: &InstalledApp) {
        if let Some(ref exec) = app.exec {
            let parts: Vec<&str> = exec.split_whitespace().collect();
            if let Some((first, rest)) = parts.split_first() {
                let clean_args: Vec<&str> = rest
                    .iter()
                    .copied()
                    .filter(|arg| !arg.starts_with('%'))
                    .collect();

                let _ = Command::new(first).args(&clean_args).spawn();
            }
        } else if let Some(ref desk) = app.desktop_file {
            let _ = Command::new("gtk-launch")
                .arg(desk.file_name().unwrap_or_default())
                .spawn();
        }
    }

    pub fn reveal_folder(&self, app: &InstalledApp) {
        if let Some(ref path) = app.install_path {
            let _ = open::that(path);
        } else if let Some(ref desk) = app.desktop_file {
            if let Some(parent) = desk.parent() {
                let _ = open::that(parent);
            }
        }
    }

    fn filtered_apps(&self) -> Vec<&InstalledApp> {
        let q = self.search_query.trim().to_lowercase();
        let mut list: Vec<&InstalledApp> = self
            .apps
            .iter()
            .filter(|app| {
                let matches_tab = match self.active_tab {
                    NavTab::Applications => true,
                    NavTab::Managed => app.is_managed_by_installxs,
                    NavTab::Updates => false,
                    NavTab::Discover => false,
                    NavTab::Pacman => app.source == AppSource::Pacman,
                    NavTab::Aur => app.source == AppSource::Aur,
                    NavTab::AppImage => app.source == AppSource::AppImage,
                    NavTab::Flatpak => app.source == AppSource::Flatpak,
                    NavTab::Local => matches!(
                        app.source,
                        AppSource::Manual | AppSource::Tarball | AppSource::Deb
                    ),
                    NavTab::DropInstaller => true,
                    NavTab::Settings => false,
                };
                if !matches_tab {
                    return false;
                }

                if self.active_tab == NavTab::Applications {
                    let matches_chip = match self.active_filter {
                        FilterChip::All => true,
                        FilterChip::Pacman => app.source == AppSource::Pacman,
                        FilterChip::Aur => app.source == AppSource::Aur,
                        FilterChip::AppImage => app.source == AppSource::AppImage,
                        FilterChip::Flatpak => app.source == AppSource::Flatpak,
                        FilterChip::Local => matches!(
                            app.source,
                            AppSource::Manual | AppSource::Tarball | AppSource::Deb
                        ),
                    };
                    if !matches_chip {
                        return false;
                    }
                }

                if q.is_empty() {
                    return true;
                }

                app.name.to_lowercase().contains(&q)
                    || app
                        .comment
                        .as_ref()
                        .map(|c| c.to_lowercase().contains(&q))
                        .unwrap_or(false)
                    || app.id.to_lowercase().contains(&q)
            })
            .collect();

        match self.sort_mode {
            SortMode::Name => {
                list.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
            }
            SortMode::Source => list.sort_by(|a, b| a.source.label().cmp(b.source.label())),
            SortMode::Size => list.sort_by(|a, b| {
                b.installed_size
                    .unwrap_or(0)
                    .cmp(&a.installed_size.unwrap_or(0))
            }),
        }

        list
    }
}

impl Render for AppView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let s = *Theme::of(cx);

        let managed_count = self
            .apps
            .iter()
            .filter(|a| a.is_managed_by_installxs)
            .count();
        let flatpak_count = self
            .apps
            .iter()
            .filter(|a| a.source == AppSource::Flatpak)
            .count();
        let pacman_count = self
            .apps
            .iter()
            .filter(|a| a.source == AppSource::Pacman)
            .count();
        let aur_count = self
            .apps
            .iter()
            .filter(|a| a.source == AppSource::Aur)
            .count();
        let appimage_count = self
            .apps
            .iter()
            .filter(|a| a.source == AppSource::AppImage)
            .count();
        let local_count = self
            .apps
            .iter()
            .filter(|a| {
                matches!(
                    a.source,
                    AppSource::Manual | AppSource::Tarball | AppSource::Deb
                )
            })
            .count();
        let total_count = self.apps.len();
        let updates_count = self.available_updates.len();
        let is_search_active = !self.search_query.is_empty();

        let tab_id: &'static str = match self.active_tab {
            NavTab::Applications => "tab_applications",
            NavTab::Managed => "tab_managed",
            NavTab::Updates => "tab_updates",
            NavTab::Discover => "tab_discover",
            NavTab::Pacman => "tab_pacman",
            NavTab::Aur => "tab_aur",
            NavTab::AppImage => "tab_appimage",
            NavTab::Flatpak => "tab_flatpak",
            NavTab::Local => "tab_local",
            NavTab::DropInstaller => "tab_drop",
            NavTab::Settings => "tab_settings",
        };
        let view_p = motion::entrance(tab_id, Spring::SPATIAL, 0., window, cx);

        div()
            .size_full()
            .bg(s.surface_dim)
            .text_color(s.on_surface)
            .font_family(gpui_m3::theme::FONT_FAMILY)
            .flex()
            .flex_row()
            // Global drag & drop support
            .on_drop::<ExternalPaths>(cx.listener(|this, paths: &ExternalPaths, _, cx| {
                if let Some(first) = paths.paths().first() {
                    this.stage_file(first.clone(), cx);
                }
            }))
            // 1. Sidebar Navigation (Left Rail matching reference design)
            .child(
                div()
                    .w(px(248.))
                    .h_full()
                    .m_3()
                    .rounded(px(20.))
                    .bg(glass_bg(&s))
                    .border_1()
                    .border_color(s.outline_variant)
                    .shadow(glass_shadow(&s))
                    .flex()
                    .flex_col()
                    .justify_between()
                    .p_3()
                    // Top Section: Window Controls + Brand + Nav Groups
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_3()
                                    .px_2()
                                    .pt_2()
                                    .pb_4()
                                    .child(
                                        div()
                                            .size(px(38.))
                                            .rounded(px(12.))
                                            .bg(s.primary_container)
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .overflow_hidden()
                                            .child(
                                                if Path::new(
                                                    "/usr/share/pixmaps/archlinux-logo.png",
                                                )
                                                .exists()
                                                {
                                                    img(PathBuf::from(
                                                        "/usr/share/pixmaps/archlinux-logo.png",
                                                    ))
                                                    .size(px(24.))
                                                    .into_any_element()
                                                } else {
                                                    div()
                                                        .text_size(px(18.))
                                                        .text_color(s.on_primary_container)
                                                        .font_weight(FontWeight::BOLD)
                                                        .child("A")
                                                        .into_any_element()
                                                },
                                            ),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .child(
                                                div()
                                                    .font_weight(FontWeight::BOLD)
                                                    .text_size(px(15.5))
                                                    .text_color(s.on_surface)
                                                    .child("InstallXS"),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(11.))
                                                    .text_color(s.on_surface_variant)
                                                    .child("Application manager"),
                                            ),
                                    ),
                            )
                            // Library section label
                            .child(
                                div()
                                    .px_3()
                                    .pt_3()
                                    .pb_1()
                                    .text_size(px(11.))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(s.on_surface_variant)
                                    .child("Library"),
                            )
                            // Main Nav: Applications (Lilac pill when active)
                            .child(self.render_nav_item(
                                "nav_apps",
                                "Applications",
                                "blocks",
                                total_count,
                                self.active_tab == NavTab::Applications,
                                s,
                                cx.listener(|this, _, _, cx| {
                                    this.active_tab = NavTab::Applications;
                                    cx.notify();
                                }),
                            ))
                            // Managed
                            .child(self.render_nav_item(
                                "nav_managed",
                                "Managed",
                                "check",
                                managed_count,
                                self.active_tab == NavTab::Managed,
                                s,
                                cx.listener(|this, _, _, cx| {
                                    this.active_tab = NavTab::Managed;
                                    cx.notify();
                                }),
                            ))
                            // Updates
                            .child(self.render_nav_item(
                                "nav_updates",
                                "Updates",
                                "refresh",
                                updates_count,
                                self.active_tab == NavTab::Updates,
                                s,
                                cx.listener(|this, _, _, cx| {
                                    this.active_tab = NavTab::Updates;
                                    cx.notify();
                                }),
                            ))
                            // Discover
                            .child(self.render_nav_item(
                                "nav_discover",
                                "Discover",
                                "spark",
                                self.store_packages.len(),
                                self.active_tab == NavTab::Discover,
                                s,
                                cx.listener(|this, _, _, cx| {
                                    this.active_tab = NavTab::Discover;
                                    cx.notify();
                                }),
                            ))
                            // Sources Header
                            .child(
                                div()
                                    .px_3()
                                    .pt_4()
                                    .pb_1()
                                    .text_size(px(11.))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(s.on_surface_variant)
                                    .child("Sources"),
                            )
                            // Sources: Pacman
                            .child(self.render_nav_item(
                                "nav_pacman",
                                "Pacman",
                                "box",
                                pacman_count,
                                self.active_tab == NavTab::Pacman,
                                s,
                                cx.listener(|this, _, _, cx| {
                                    this.active_tab = NavTab::Pacman;
                                    cx.notify();
                                }),
                            ))
                            // Sources: AUR
                            .child(self.render_nav_item(
                                "nav_aur",
                                "AUR",
                                "terminal",
                                aur_count,
                                self.active_tab == NavTab::Aur,
                                s,
                                cx.listener(|this, _, _, cx| {
                                    this.active_tab = NavTab::Aur;
                                    cx.notify();
                                }),
                            ))
                            // Sources: AppImage
                            .child(self.render_nav_item(
                                "nav_appimage",
                                "AppImage",
                                "bolt",
                                appimage_count,
                                self.active_tab == NavTab::AppImage,
                                s,
                                cx.listener(|this, _, _, cx| {
                                    this.active_tab = NavTab::AppImage;
                                    cx.notify();
                                }),
                            ))
                            // Sources: Flatpak
                            .child(self.render_nav_item(
                                "nav_flatpak",
                                "Flatpak",
                                "layers",
                                flatpak_count,
                                self.active_tab == NavTab::Flatpak,
                                s,
                                cx.listener(|this, _, _, cx| {
                                    this.active_tab = NavTab::Flatpak;
                                    cx.notify();
                                }),
                            ))
                            // Sources: Local
                            .child(self.render_nav_item(
                                "nav_local",
                                "Local",
                                "folder",
                                local_count,
                                self.active_tab == NavTab::Local,
                                s,
                                cx.listener(|this, _, _, cx| {
                                    this.active_tab = NavTab::Local;
                                    cx.notify();
                                }),
                            )),
                    )
                    // Settings
                    .child(
                        NavItem::new("btn_sidebar_settings", "Settings")
                            .icon("gear")
                            .selected(self.active_tab == NavTab::Settings)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.active_tab = NavTab::Settings;
                                cx.notify();
                            })),
                    ),
            )
            // 2. Main Workspace (Header pinned, content scrolls smoothly)
            .child(
                div()
                    .flex_1()
                    .h_full()
                    .flex()
                    .flex_col()
                    .min_w_0()
                    .min_h_0()
                    .overflow_hidden()
                    .bg(s.surface)
                    // Search and install controls belong to the application pages.
                    .when(self.active_tab != NavTab::DropInstaller, |workspace| {
                        workspace.child(
                            div()
                                .flex_none()
                                .h(px(64.))
                                .px_8()
                                .bg(s.surface)
                                .border_b_1()
                                .border_color(s.outline_variant)
                                .flex()
                                .items_center()
                                .justify_between()
                                .gap_4()
                                // Centered Search Bar
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .max_w(px(680.))
                                        .h(px(42.))
                                        .bg(s.surface_container_high)
                                        .border_1()
                                        .border_color(s.outline_variant)
                                        .rounded(px(21.))
                                        .px_4()
                                        .flex()
                                        .items_center()
                                        .gap_3()
                                        .child(
                                            div()
                                                .text_color(s.on_surface_variant)
                                                .text_size(px(15.))
                                                .child(
                                                    Icon::new("search")
                                                        .size(px(18.))
                                                        .color(s.on_surface_variant),
                                                ),
                                        )
                                        .child(
                                            div()
                                                .flex_1()
                                                .min_w_0()
                                                .child(self.search_field.clone()),
                                        )
                                        .when(is_search_active, |el| {
                                            el.child(
                                                div()
                                                    .id("clear_search_btn")
                                                    .cursor_pointer()
                                                    .text_color(s.on_surface_variant)
                                                    .text_size(px(13.))
                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                        this.search_query.clear();
                                                        this.search_field.update(cx, |f, cx| {
                                                            f.set_text("", cx);
                                                        });
                                                        cx.notify();
                                                    }))
                                                    .child("✕"),
                                            )
                                        }),
                                )
                                // Keep one clear install action in the app bar.
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_3()
                                        .flex_shrink_0()
                                        .child(
                                            div()
                                                .id("header_theme_toggle")
                                                .size(px(36.))
                                                .rounded_full()
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .cursor_pointer()
                                                .hover(|h| h.bg(s.surface_container_high))
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.toggle_theme(cx);
                                                }))
                                                .child(
                                                    Icon::new(if self.dark_mode {
                                                        "sun"
                                                    } else {
                                                        "moon"
                                                    })
                                                    .size(px(17.))
                                                    .color(s.on_surface_variant),
                                                ),
                                        )
                                        .child(
                                        Button::new("header_install", "Install package")
                                            .filled()
                                            .on_click(
                                                cx.listener(|this, _, _, cx| this.pick_file(cx)),
                                            ),
                                    ),
                                ),
                        )
                    })
                    // Scrollable Workspace Content
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .min_h_0()
                            .id("main_scroll")
                            .overflow_y_scroll()
                            .px_8()
                            .py_4()
                            .child(
                                div()
                                    .relative()
                                    .top(px((1. - view_p) * 10.))
                                    .opacity(view_p.clamp(0., 1.))
                                    .child(match self.active_tab {
                                        NavTab::DropInstaller => {
                                            self.render_installer_view(cx, s)
                                                .into_any_element()
                                        }
                                        NavTab::Updates => {
                                            self.render_updates_view(cx, s).into_any_element()
                                        }
                                        NavTab::Discover => {
                                            self.render_discover_view(cx, s).into_any_element()
                                        }
                                        NavTab::Settings => {
                                            self.render_settings_view(cx, s).into_any_element()
                                        }
                                        _ => self
                                            .render_app_hub_view(window, cx, s)
                                            .into_any_element(),
                                    }),
                            ),
                    ),
            )
            // Floating toast pill
            .when_some(self.toast.clone(), |el, (msg, is_err)| {
                let tp = motion::entrance("toast", Spring::EFFECTS_FAST, 0., window, cx);
                el.child(
                    div()
                        .absolute()
                        .bottom(px(28.))
                        .left_0()
                        .right_0()
                        .flex()
                        .justify_center()
                        .child(
                            div()
                                .id("toast_pill")
                                .relative()
                                .top(px((1. - tp) * 10.))
                                .opacity(tp.clamp(0., 1.))
                                .flex()
                                .items_center()
                                .gap_2p5()
                                .pl_3()
                                .pr_1p5()
                                .py_1p5()
                                .rounded_full()
                                .bg(s.inverse_surface)
                                .text_color(s.inverse_on_surface)
                                .shadow(glass_shadow(&s))
                                .child(
                                    Icon::new(if is_err { "x" } else { "check" })
                                        .size(px(15.))
                                        .color(if is_err { s.error } else { s.success }),
                                )
                                .child(
                                    div()
                                        .text_size(px(12.5))
                                        .font_weight(FontWeight::MEDIUM)
                                        .child(msg),
                                )
                                .child(
                                    div()
                                        .id("toast_dismiss")
                                        .size(px(24.))
                                        .rounded_full()
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .cursor_pointer()
                                        .hover(|h| {
                                            h.bg(alpha(s.inverse_on_surface, 0.12))
                                        })
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.toast = None;
                                            cx.notify();
                                        }))
                                        .child(
                                            Icon::new("x")
                                                .size(px(13.))
                                                .color(s.inverse_on_surface),
                                        ),
                                ),
                        ),
                )
            })
            // App Details Inspector Modal
            .when_some(self.selected_app_details.clone(), |el, app| {
                el.child(self.render_app_details_modal(window, app, cx, s))
            })
            // Uninstall Confirmation Modal
            .when_some(self.selected_for_uninstall.clone(), |el, app| {
                el.child(self.render_uninstall_modal(window, app, cx, s))
            })
    }
}

impl AppView {
    fn render_nav_item(
        &self,
        id: &'static str,
        label: &'static str,
        icon: &'static str,
        count: usize,
        active: bool,
        _s: Scheme,
        on_click: impl Fn(&gpui::ClickEvent, &mut Window, &mut gpui::App) + 'static,
    ) -> impl IntoElement {
        NavItem::new(id, label)
            .icon(icon)
            .meta(count.to_string())
            .selected(active)
            .on_click(on_click)
    }

    fn render_installer_view(&self, cx: &mut Context<Self>, s: Scheme) -> impl IntoElement {
        let downloads = if self.downloaded_packages.is_empty() {
            div()
                .p_4()
                .rounded(px(12.))
                .bg(glass_bg_high(&s))
                .child(
                    div()
                        .text_size(px(12.5))
                        .text_color(s.on_surface_variant)
                        .child("Supported packages in ~/Downloads will appear here."),
                )
                .into_any_element()
        } else {
            let mut list = div().flex().flex_col().gap_2();
            for (index, candidate) in self.downloaded_packages.iter().take(5).enumerate() {
                let path = candidate.source_path.clone();
                let icon = match &candidate.kind {
                    crate::models::FileKind::AppImage => "bolt",
                    crate::models::FileKind::Deb | crate::models::FileKind::ArchPkg => "box",
                    _ => "folder",
                };
                list = list.child(
                    div()
                        .id(("downloaded_package", index))
                        .w_full()
                        .p_2p5()
                        .rounded(px(12.))
                        .hover(|el| el.bg(s.surface_container_high))
                        .cursor_pointer()
                        .flex()
                        .items_center()
                        .gap_3()
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.stage_file(path.clone(), cx);
                        }))
                        .child(
                            div()
                                .size(px(36.))
                                .rounded(px(10.))
                                .bg(s.surface_container_high)
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(Icon::new(icon).size(px(18.)).color(s.on_surface_variant)),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .gap_0p5()
                                .child(
                                    div()
                                        .truncate()
                                        .text_size(px(12.5))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(s.on_surface)
                                        .child(candidate.suggested_name.clone()),
                                )
                                .child(
                                    div()
                                        .truncate()
                                        .text_size(px(11.))
                                        .text_color(s.on_surface_variant)
                                        .child(format!(
                                            "{} · {}",
                                            format_file_size(candidate.file_size),
                                            candidate.file_name
                                        )),
                                ),
                        )
                        .child(
                            Icon::new("chevron-right")
                                .size(px(16.))
                                .color(s.on_surface_variant),
                        ),
                );
            }
            list.into_any_element()
        };

        let main_panel = if let Some(candidate) = self.candidate.clone() {
            let kind_label = match &candidate.kind {
                crate::models::FileKind::AppImage => "AppImage",
                crate::models::FileKind::TarGz
                | crate::models::FileKind::TarXz
                | crate::models::FileKind::TarBz2
                | crate::models::FileKind::Zip => "Archive",
                crate::models::FileKind::Deb => "Debian package",
                crate::models::FileKind::ArchPkg => "Arch package",
                crate::models::FileKind::DesktopFile => "Desktop entry",
                crate::models::FileKind::ExecutableBinary => "Executable",
                crate::models::FileKind::Unknown => "Package",
            };
            let (status, status_color) = match &self.install_state {
                InstallState::Idle => ("Ready to install".to_string(), s.on_surface_variant),
                InstallState::Inspecting(message) => (message.clone(), s.primary),
                InstallState::Installing { step, .. } => (step.clone(), s.primary),
                InstallState::Success { message, .. } => (message.clone(), s.success),
                InstallState::Error(error) => (error.clone(), s.error),
            };
            let can_install = matches!(
                &self.install_state,
                InstallState::Idle | InstallState::Error(_)
            );
            let installed = matches!(&self.install_state, InstallState::Success { .. });

            div()
                .w_full()
                .p_6()
                .rounded(px(20.))
                .bg(glass_bg(&s))
                .border_1()
                .border_color(s.outline_variant)
                .shadow(glass_shadow(&s))
                .flex()
                .flex_col()
                .gap_5()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_4()
                        .child(
                            div()
                                .size(px(52.))
                                .rounded(px(14.))
                                .bg(s.primary_container)
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(
                                    Icon::new("box").size(px(26.)).color(s.on_primary_container),
                                ),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .child(
                                    div()
                                        .text_size(px(19.))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(s.on_surface)
                                        .truncate()
                                        .child(candidate.suggested_name.clone()),
                                )
                                .child(
                                    div()
                                        .text_size(px(12.5))
                                        .text_color(s.on_surface_variant)
                                        .truncate()
                                        .child(candidate.file_name.clone()),
                                ),
                        )
                        .child(
                            div()
                                .px_3()
                                .py_1p5()
                                .rounded_full()
                                .bg(s.surface_container_high)
                                .text_size(px(11.5))
                                .text_color(s.on_surface_variant)
                                .child(kind_label),
                        ),
                )
                .child(div().h(px(1.)).bg(s.outline_variant))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(
                            div()
                                .text_size(px(11.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(s.on_surface_variant)
                                .child("PACKAGE SIZE"),
                        )
                        .child(
                            div()
                                .text_size(px(14.))
                                .text_color(s.on_surface)
                                .child(format_file_size(candidate.file_size)),
                        ),
                )
                .child(
                    div()
                        .p_3()
                        .rounded(px(10.))
                        .bg(glass_bg_high(&s))
                        .text_size(px(12.5))
                        .text_color(status_color)
                        .child(status),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_3()
                        .child(
                            Button::new("cancel_staged_btn", "Remove file")
                                .text()
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.candidate = None;
                                    this.install_state = InstallState::Idle;
                                    cx.notify();
                                })),
                        )
                        .child(if installed {
                            Button::new("done_install_btn", "View installed apps")
                                .tonal()
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.candidate = None;
                                    this.active_tab = NavTab::Managed;
                                    cx.notify();
                                }))
                                .into_any_element()
                        } else if can_install {
                            Button::new("do_install_btn", "Install now")
                                .filled()
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.install_current_candidate(cx);
                                }))
                                .into_any_element()
                        } else {
                            div()
                                .text_size(px(12.))
                                .text_color(s.on_surface_variant)
                                .child("Working…")
                                .into_any_element()
                        }),
                )
                .into_any_element()
        } else {
            div()
                .w_full()
                .min_h(px(400.))
                .p_10()
                .rounded(px(20.))
                .border_2()
                .border_dashed()
                .border_color(alpha(s.primary, 0.4))
                .bg(gpui::linear_gradient(
                    180.,
                    gpui::linear_color_stop(alpha(s.primary_container, 0.55), 0.),
                    gpui::linear_color_stop(alpha(s.primary_container, 0.12), 1.),
                ))
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap_5()
                .child(
                    div()
                        .size(px(72.))
                        .rounded(px(22.))
                        .bg(s.primary_container)
                        .border_1()
                        .border_color(s.outline_variant)
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(Icon::new("import").size(px(34.)).color(s.on_primary_container)),
                )
                .child(
                    div()
                        .text_size(px(21.))
                        .font_weight(FontWeight::BOLD)
                        .text_color(s.on_surface)
                        .child("Drop a package to install"),
                )
                .child(
                    div()
                        .text_size(px(13.))
                        .text_color(s.on_surface_variant)
                        .child("AppImage, .deb, .tar.gz, .zip or a plain executable — drop it anywhere in this window."),
                )
                .child(
                    Button::new("browse_packages_btn", "Browse files")
                        .filled()
                        .on_click(cx.listener(|this, _, _, cx| this.pick_file(cx))),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .children(
                            ["AppImage", ".deb", ".tar.gz", ".zip", "binary"]
                                .iter()
                                .map(|label| {
                                    div()
                                        .px_2p5()
                                        .py_0p5()
                                        .rounded_full()
                                        .bg(glass_bg_high(&s))
                                        .border_1()
                                        .border_color(s.outline_variant)
                                        .text_size(px(11.))
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(s.on_surface_variant)
                                        .child(*label)
                                }),
                        ),
                )
                .into_any_element()
        };

        div().w_full().flex().justify_center().child(
            div()
                .w_full()
                .max_w(px(1240.))
                .flex()
                .flex_col()
                .gap_6()
                .pb_12()
                .child(render_page_heading(
                    "Install package",
                    "Review a package before adding it to your application library.",
                    Some(
                        Button::new("installer_back", "Applications")
                            .tonal()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.active_tab = NavTab::Applications;
                                cx.notify();
                            }))
                            .into_any_element(),
                    ),
                    s,
                ))
                .child(
                    div()
                        .w_full()
                        .flex()
                        .flex_wrap()
                        .items_start()
                        .gap_5()
                        .min_w_0()
                        .child(
                            div()
                                .flex_1()
                                .min_w(px(460.))
                                .max_w(px(900.))
                                .flex()
                                .flex_col()
                                .gap_4()
                                .child(main_panel),
                        )
                        .child(
                            div()
                                .w(px(300.))
                                .flex_shrink_0()
                                .p_4()
                                .rounded(px(20.))
                                .bg(glass_bg(&s))
                                .border_1()
                                .border_color(s.outline_variant)
                                .shadow(glass_shadow(&s))
                                .flex()
                                .flex_col()
                                .gap_3()
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .justify_between()
                                        .child(
                                            div()
                                                .flex()
                                                .items_center()
                                                .gap_2p5()
                                                .child(
                                                    Icon::new("folder")
                                                        .size(px(17.))
                                                        .color(s.on_surface_variant),
                                                )
                                                .child(
                                                    div()
                                                        .font_weight(FontWeight::SEMIBOLD)
                                                        .text_size(px(14.))
                                                        .text_color(s.on_surface)
                                                        .child("From Downloads"),
                                                ),
                                        )
                                        .child(
                                            div()
                                                .text_size(px(11.5))
                                                .text_color(s.on_surface_variant)
                                                .child(self.downloaded_packages.len().to_string()),
                                        ),
                                )
                                .child(
                                    div()
                                        .text_size(px(12.))
                                        .text_color(s.on_surface_variant)
                                        .child("Select a recent package to review it."),
                                )
                                .child(downloads),
                        ),
                ),
        )
    }

    fn render_app_hub_view(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
        s: Scheme,
    ) -> impl IntoElement {
        let apps: Vec<InstalledApp> = self.filtered_apps().into_iter().cloned().collect();
        let current_count = apps.len();

        let tab_title = match self.active_tab {
            NavTab::Applications => "Applications",
            NavTab::Managed => "Managed by InstallXS",
            NavTab::Updates => "Available updates",
            NavTab::Discover => "Discover & App Store",
            NavTab::Pacman => "Pacman native packages",
            NavTab::Aur => "AUR applications",
            NavTab::AppImage => "Standalone AppImages",
            NavTab::Flatpak => "Flatpak applications",
            NavTab::Local => "Local desktop applications",
            NavTab::DropInstaller => "Install application",
            NavTab::Settings => "Settings & Maintenance",
        };

        div()
            .w_full()
            .flex()
            .flex_col()
            .min_w_0()
            .gap_5()
            .pb_10()
            // 1. Hero Banner (displayed when on Applications tab with empty search query)
            .when(self.active_tab == NavTab::Applications && self.search_query.is_empty(), |el| {
                el.child(
                    div()
                        .w_full()
                        .h(px(180.))
                        .rounded(px(24.))
                        .bg(rgb(0x251b38))
                        .border_1()
                        .border_color(rgb(0x3d2c5c))
                        .overflow_hidden()
                        .relative()
                        .flex()
                        .items_center()
                        .justify_between()
                        // Right Side Arch Logo Wave Art
                        .child(
                            div()
                                .absolute()
                                .top_0()
                                .right_0()
                                .bottom_0()
                                .w(px(500.))
                                .overflow_hidden()
                                .flex()
                                .items_center()
                                .justify_end()
                                .child(
                                    img(PathBuf::from(format!("{}/assets/hero_wave.png", env!("CARGO_MANIFEST_DIR"))))
                                        .h_full()
                                        .w_auto()
                                )
                        )
                        // Left Side Content
                        .child(
                            div()
                                .relative()
                                .max_w(px(540.))
                                .py_5()
                                .pl_8()
                                .flex()
                                .flex_col()
                                .gap_2p5()
                                .child(
                                    div()
                                        .font_weight(FontWeight::BOLD)
                                        .text_size(px(26.))
                                        .text_color(rgb(0xffffff))
                                        .child("Your Arch app hub"),
                                )
                                .child(
                                    div()
                                        .text_size(px(13.5))
                                        .text_color(rgb(0xcac4d0))
                                        .child("Discover, install, and manage applications across Pacman, AUR, AppImage and more."),
                                )
                                .child(
                                    div()
                                        .pt_1()
                                        .child(
                                            div()
                                                .id("hero_get_started")
                                                .cursor_pointer()
                                                .px_5()
                                                .py_2()
                                                .rounded_full()
                                                .bg(rgb(0xffffff))
                                                .hover(|h| h.bg(rgb(0xe8def8)))
                                                .text_color(rgb(0x181224))
                                                .font_weight(FontWeight::BOLD)
                                                .text_size(px(13.))
                                                .flex()
                                                .items_center()
                                                .gap_2()
                                                .child("Get started →")
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.active_tab = NavTab::DropInstaller;
                                                    cx.notify();
                                                })),
                                        ),
                                ),
                        ),
                )
            })
            // Section Header: Title, count, and View/Sort Controls
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_4()
                    .pt_1()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .min_w_0()
                            .child(
                                div()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_size(px(24.))
                                    .text_color(s.on_surface)
                                    .child(tab_title),
                            )
                            .child(
                                div()
                                    .px_2p5()
                                    .py_0p5()
                                    .rounded_full()
                                    .bg(s.surface_container)
                                    .text_size(px(11.5))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(s.on_surface_variant)
                                    .child(current_count.to_string()),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            // View Switcher (Grid vs List)
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .p(px(3.))
                                    .rounded(px(12.))
                                    .bg(glass_bg_high(&s))
                                    .border_1()
                                    .border_color(s.outline_variant)
                                    .child(
                                        IconButton::new("view_grid_btn", "blocks")
                                            .selected(self.view_mode == ViewMode::Grid)
                                            .tooltip("Grid view")
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.view_mode = ViewMode::Grid;
                                                cx.notify();
                                            })),
                                    )
                                    .child(
                                        IconButton::new("view_list_btn", "files")
                                            .selected(self.view_mode == ViewMode::List)
                                            .tooltip("List view")
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.view_mode = ViewMode::List;
                                                cx.notify();
                                            })),
                                    ),
                            )
                            // Sort Dropdown Button
                            .child(
                                Button::new(
                                    "sort_toggle_btn",
                                    match self.sort_mode {
                                        SortMode::Name => "Sort by: Name ⌵",
                                        SortMode::Source => "Sort by: Source ⌵",
                                        SortMode::Size => "Sort by: Disk Size ⌵",
                                    },
                                )
                                    .outlined()
                                    .size(ButtonSize::Small)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.sort_mode = match this.sort_mode {
                                            SortMode::Name => SortMode::Source,
                                            SortMode::Source => SortMode::Size,
                                            SortMode::Size => SortMode::Name,
                                        };
                                        cx.notify();
                                    })),
                            ),
                    ),
            )
            // Filter Chips (only when on Applications tab)
            .when(self.active_tab == NavTab::Applications, |el| {
                el.child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(self.render_filter_chip("f_all", "✓ All", FilterChip::All, cx))
                        .child(self.render_filter_chip("f_pacman", "📦 Pacman", FilterChip::Pacman, cx))
                        .child(self.render_filter_chip("f_aur", "A AUR", FilterChip::Aur, cx))
                        .child(self.render_filter_chip("f_appimage", "🚀 AppImage", FilterChip::AppImage, cx))
                        .child(self.render_filter_chip("f_flatpak", "🧊 Flatpak", FilterChip::Flatpak, cx))
                        .child(self.render_filter_chip("f_local", "📁 Local", FilterChip::Local, cx)),
                )
            })
            // 3. Content Cards or Compact List
            .child(if apps.is_empty() {
                div()
                    .w_full()
                    .p_16()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap_3()
                    .child(
                        Icon::new("search")
                            .size(px(32.))
                            .color(s.on_surface_variant),
                    )
                    .child(
                        div()
                            .font_weight(FontWeight::BOLD)
                            .text_size(px(18.))
                            .text_color(s.on_surface)
                            .child("No matching applications found"),
                    )
                    .child(
                        div()
                            .text_size(px(13.))
                            .text_color(s.on_surface_variant)
                            .child("Try another source in the sidebar or clear your search."),
                    )
                    .into_any_element()
            } else if self.view_mode == ViewMode::Grid {
                self.render_grid_cards(window, apps, cx, s).into_any_element()
            } else {
                self.render_compact_list(apps, cx, s).into_any_element()
            })
    }

    fn render_grid_cards(
        &self,
        window: &mut Window,
        apps: Vec<InstalledApp>,
        cx: &mut Context<Self>,
        s: Scheme,
    ) -> impl IntoElement {
        div()
            .w_full()
            .flex()
            .flex_wrap()
            .gap(px(18.))
            .children(apps.into_iter().enumerate().map(|(idx, app)| {
                let app_for_launch = app.clone();
                let app_for_open = app.clone();
                let app_for_details = app.clone();
                let subtitle = clean_app_description(&app);
                let card_group = gpui::SharedString::from(format!("appcard-{}", idx));
                let p = motion::entrance(
                    ("app_card", idx),
                    Spring::SPATIAL,
                    (idx.min(12) as f32) * 0.03,
                    window,
                    cx,
                );

                div()
                    .id(("grid_app_card", idx))
                    .group(card_group.clone())
                    .relative()
                    .top(px((1. - p) * 12.))
                    .opacity(p.clamp(0., 1.))
                    .w(px(300.))
                    .h(px(184.))
                    .p(px(18.))
                    .rounded(px(20.))
                    .bg(glass_bg(&s))
                    .hover(|h| {
                        h.bg(glass_bg_high(&s))
                            .border_color(s.outline)
                            .shadow(glass_shadow_lift(&s))
                    })
                    .border_1()
                    .border_color(s.outline_variant)
                    .shadow(glass_shadow(&s))
                    .flex()
                    .flex_col()
                    .justify_between()
                    .cursor_pointer()
                    .on_click(cx.listener({
                        let a = app_for_launch;
                        move |this, _, _, _| {
                            this.launch_app(&a);
                        }
                    }))
                    // Top row: App Icon (52x52) and overflow menu
                    .child(
                        div()
                            .flex()
                            .items_start()
                            .justify_between()
                            .child(render_app_icon_sized(
                                app.icon.as_ref(),
                                &app.name,
                                &s,
                                52.,
                            ))
                            .child(
                                div()
                                    .id(("kebab_btn", idx))
                                    .size(px(30.))
                                    .rounded_full()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .cursor_pointer()
                                    .hover(|h| h.bg(s.surface_container_highest))
                                    .on_click(cx.listener({
                                        let a = app_for_details;
                                        move |this, _, _, cx| {
                                            this.open_app_details(a.clone(), cx);
                                        }
                                    }))
                                    .child(
                                        Icon::new("more")
                                            .size(px(16.))
                                            .color(s.on_surface_variant),
                                    ),
                            ),
                    )
                    // Middle: App Name & 2-Line Subtitle Description
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_size(px(14.5))
                                    .text_color(s.on_surface)
                                    .truncate()
                                    .child(app.name.clone()),
                            )
                            .child(
                                div()
                                    .h(px(32.))
                                    .text_size(px(12.))
                                    .text_color(s.on_surface_variant)
                                    .overflow_hidden()
                                    .child(subtitle),
                            ),
                    )
                    // Bottom: Source Badge + Size, hover-reveal Open action
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(render_source_badge(&app.source, s))
                                    .when_some(app.installed_size, |row, sz| {
                                        if sz > 0 {
                                            row.child(
                                                div()
                                                    .text_size(px(11.5))
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .text_color(s.on_surface_variant)
                                                    .child(format_file_size(sz)),
                                            )
                                        } else {
                                            row
                                        }
                                    }),
                            )
                            .child(
                                div()
                                    .invisible()
                                    .group_hover(card_group, |h| h.visible())
                                    .child(
                                        Button::new(("open_card", idx), "Open")
                                            .tonal()
                                            .on_click(cx.listener({
                                                let a = app_for_open;
                                                move |this, _, _, _| {
                                                    this.launch_app(&a);
                                                }
                                            })),
                                    ),
                            ),
                    )
            }))
    }

    fn render_compact_list(
        &self,
        apps: Vec<InstalledApp>,
        cx: &mut Context<Self>,
        s: Scheme,
    ) -> impl IntoElement {
        let count = apps.len();
        div()
            .w_full()
            .min_w_0()
            .rounded(px(20.))
            .bg(glass_bg(&s))
            .border_1()
            .border_color(s.outline_variant)
            .shadow(glass_shadow(&s))
            .overflow_hidden()
            .flex()
            .flex_col()
            .child(
                div()
                    .w_full()
                    .px_4()
                    .py_2p5()
                    .flex()
                    .items_center()
                    .gap_4()
                    .text_size(px(10.5))
                    .font_weight(FontWeight::BOLD)
                    .text_color(s.on_surface_variant)
                    .child(div().w(px(44.)).flex_shrink_0())
                    .child(div().flex_1().min_w_0().child("APPLICATION"))
                    .child(div().w(px(88.)).flex_shrink_0().child("SOURCE"))
                    .child(div().w(px(76.)).flex_shrink_0().child("SIZE"))
                    .child(div().w(px(88.)).flex_shrink_0().child("ACTION")),
            )
            .children(apps.into_iter().enumerate().map(|(idx, app)| {
                let details = app.clone();
                let launch = app.clone();
                let mut row =
                    div()
                        .id(("application_row", idx))
                        .w_full()
                        .min_h(px(72.))
                        .px_4()
                        .py_2p5()
                        .hover(|h| h.bg(glass_bg_high(&s)))
                        .cursor_pointer()
                        .flex()
                        .items_center()
                        .gap_4()
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.open_app_details(details.clone(), cx);
                        }))
                        .child(render_app_icon_sized(app.icon.as_ref(), &app.name, &s, 46.))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .child(
                                    div()
                                        .text_size(px(14.5))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(s.on_surface)
                                        .truncate()
                                        .child(app.name.clone()),
                                )
                                .child(
                                    div()
                                        .text_size(px(12.))
                                        .text_color(s.on_surface_variant)
                                        .truncate()
                                        .child(clean_app_description(&app)),
                                ),
                        )
                        .child(
                            div()
                                .w(px(88.))
                                .flex_shrink_0()
                                .child(render_source_badge(&app.source, s)),
                        )
                        .child(
                            div()
                                .w(px(76.))
                                .flex_shrink_0()
                                .text_size(px(12.))
                                .text_color(s.on_surface_variant)
                                .child(
                                    app.installed_size
                                        .filter(|size| *size > 0)
                                        .map(format_file_size)
                                        .unwrap_or_else(|| "—".to_string()),
                                ),
                        )
                        .child(
                            div()
                                .w(px(88.))
                                .flex_shrink_0()
                                .flex()
                                .justify_end()
                                .child(
                                    Button::new(("launch_row", idx), "Launch")
                                        .tonal()
                                        .on_click(cx.listener(move |this, _, _, _| {
                                            this.launch_app(&launch)
                                        })),
                                ),
                        );
                if idx + 1 < count {
                    row = row.border_b_1().border_color(s.outline_variant);
                }
                row
            }))
    }

    fn render_app_details_modal(
        &self,
        window: &mut Window,
        app: InstalledApp,
        cx: &mut Context<Self>,
        s: Scheme,
    ) -> impl IntoElement {
        let app_for_uninstall = app.clone();
        let app_for_launch = app.clone();
        let app_for_folder = app.clone();
        let p = motion::entrance("app_details_modal", Spring::SPATIAL_FAST, 0., window, cx);

        div()
            .absolute()
            .inset_0()
            .bg(alpha(s.scrim, 0.45))
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .relative()
                    .top(px((1. - p) * 16.))
                    .opacity(p.clamp(0., 1.))
                    .w(px(520.))
                    .p_6()
                    .rounded(px(20.))
                    .bg(s.surface_container_high)
                    .border_1()
                    .border_color(s.outline_variant)
                    .shadow(glass_shadow(&s))
                    .flex()
                    .flex_col()
                    .gap_5()
                    // Header: Icon + Name + Source Chip
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_3p5()
                                    .child(render_app_icon(app.icon.as_ref(), &app.name, &s))
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .child(
                                                div()
                                                    .font_weight(FontWeight::BOLD)
                                                    .text_size(px(18.))
                                                    .child(app.name.clone()),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(12.))
                                                    .text_color(s.on_surface_variant)
                                                    .child(app.comment.clone().unwrap_or_else(|| "Installed Application".to_string())),
                                            ),
                                    ),
                            )
                            .child(render_source_badge(&app.source, s)),
                    )
                    // File Paths & Details Box
                    .child(
                        div()
                            .p_4()
                            .rounded(px(14.))
                            .bg(glass_bg_high(&s))
                            .flex()
                            .flex_col()
                            .gap_3()
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(
                                        div()
                                            .text_size(px(11.5))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(s.primary)
                                            .child("Executable / Launch Command:"),
                                    )
                                    .child(
                                        div()
                                            .p_2()
                                            .rounded(px(8.))
                                            .bg(s.surface_container_highest)
                                            .text_size(px(11.))
                                            .text_color(s.on_surface)
                                            .child(app.exec.clone().unwrap_or_else(|| app.id.clone())),
                                    ),
                            )
                            .when_some(app.desktop_file.clone(), |el, desk| {
                                el.child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap_1()
                                        .child(
                                            div()
                                                .text_size(px(11.5))
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .text_color(s.primary)
                                                .child("FreeDesktop Launcher (.desktop):"),
                                        )
                                        .child(
                                            div()
                                                .p_2()
                                                .rounded(px(8.))
                                                .bg(s.surface_container_highest)
                                                .text_size(px(11.))
                                                .text_color(s.on_surface)
                                                .child(desk.to_string_lossy().to_string()),
                                        ),
                                )
                            })
                            .when(app.is_managed_by_installxs, |el| {
                                el.child(
                                    div()
                                        .text_size(px(11.5))
                                        .text_color(s.primary)
                                        .child(format!("✔ Fully managed by InstallXS ({} files tracked in manifest)", app.installed_files.len())),
                                )
                            }),
                    )
                    // Release & Update Channel Box
                    .child({
                        let source_opt = db::get_update_source(&app.id, &app.name);
                        let is_editing = self.editing_update_source;
                        let is_checking = self.checking_app_update;
                        let upd_opt = self.app_update_info.clone();

                        div()
                            .p_4()
                            .rounded(px(14.))
                            .bg(glass_bg_high(&s))
                            .border_1()
                            .border_color(s.outline_variant)
                            .flex()
                            .flex_col()
                            .gap_3()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap_2()
                                            .child(
                                                Icon::new("files")
                                                    .size(px(16.))
                                                    .color(s.primary),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(12.5))
                                                    .font_weight(FontWeight::BOLD)
                                                    .text_color(s.on_surface)
                                                    .child("Release & Update Channel"),
                                            ),
                                    )
                                    .when_some(source_opt.as_ref(), |header, src| {
                                        header.child(
                                            div()
                                                .px_2()
                                                .py_0p5()
                                                .rounded_full()
                                                .bg(s.primary_container)
                                                .text_color(s.on_primary_container)
                                                .text_size(px(10.5))
                                                .font_weight(FontWeight::BOLD)
                                                .child(src.provider.label()),
                                        )
                                    }),
                            )
                            .when(is_editing || source_opt.is_none(), |b| {
                                b.child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap_2()
                                        .child(
                                            div()
                                                .text_size(px(11.5))
                                                .text_color(s.on_surface_variant)
                                                .child("Link GitHub or GitLab repo (e.g. imputnet/helium-linux or release URL):"),
                                        )
                                        .child(
                                            div()
                                                .p_2()
                                                .rounded(px(8.))
                                                .bg(s.surface_container_highest)
                                                .border_1()
                                                .border_color(s.outline_variant)
                                                .child(self.update_source_field.clone()),
                                        )
                                        .child(
                                            div()
                                                .flex()
                                                .items_center()
                                                .gap_2()
                                                .child(
                                                    Button::new("btn_save_source", "Save & Track")
                                                        .filled()
                                                        .size(ButtonSize::Small)
                                                        .on_click(cx.listener(|this, _, _, cx| {
                                                            this.save_current_app_update_source(cx);
                                                        })),
                                                )
                                                .when(source_opt.is_some(), |row| {
                                                    row.child(
                                                        Button::new("btn_cancel_source", "Cancel")
                                                            .text()
                                                            .size(ButtonSize::Small)
                                                            .on_click(cx.listener(|this, _, _, cx| {
                                                                this.editing_update_source = false;
                                                                cx.notify();
                                                            })),
                                                    )
                                                }),
                                        ),
                                )
                            })
                            .when(!is_editing && source_opt.is_some(), |b| {
                                let src = source_opt.unwrap();
                                b.child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap_2p5()
                                        .child(
                                            div()
                                                .flex()
                                                .items_center()
                                                .justify_between()
                                                .child(
                                                    div()
                                                        .flex()
                                                        .items_center()
                                                        .gap_1p5()
                                                        .text_size(px(12.))
                                                        .text_color(s.on_surface_variant)
                                                        .child("Repository:")
                                                        .child(
                                                            div()
                                                                .font_weight(FontWeight::SEMIBOLD)
                                                                .text_color(s.on_surface)
                                                                .child(src.repo_or_url.clone()),
                                                        ),
                                                )
                                                .child(
                                                    div()
                                                        .flex()
                                                        .items_center()
                                                        .gap_2()
                                                        .child(
                                                            Button::new(
                                                                "btn_chk_app",
                                                                if is_checking { "Checking..." } else { "Check Updates" },
                                                            )
                                                            .tonal()
                                                            .size(ButtonSize::Small)
                                                            .on_click(cx.listener(|this, _, _, cx| {
                                                                this.check_single_app_update(cx);
                                                            })),
                                                        )
                                                        .child(
                                                            Button::new("btn_edit_source", "Edit")
                                                                .tonal()
                                                                .size(ButtonSize::Small)
                                                                .on_click(cx.listener({
                                                                    let repo = src.repo_or_url.clone();
                                                                    move |this, _, _, cx| {
                                                                        this.update_source_input = repo.clone();
                                                                        this.editing_update_source = true;
                                                                        cx.notify();
                                                                    }
                                                                })),
                                                        )
                                                        .child(
                                                            Button::new("btn_unlink_source", "Unlink")
                                                                .text()
                                                                .size(ButtonSize::Small)
                                                                .on_click(cx.listener(|this, _, _, cx| {
                                                                    this.unlink_update_source(cx);
                                                                })),
                                                        ),
                                                ),
                                        )
                                        .when_some(upd_opt, |box_el, upd| {
                                            let upd_copy = upd.clone();
                                            box_el.child(
                                                div()
                                                    .p_3()
                                                    .rounded(px(10.))
                                                    .bg(alpha(s.primary_container, 0.4))
                                                    .border_1()
                                                    .border_color(s.primary)
                                                    .flex()
                                                    .flex_col()
                                                    .gap_2()
                                                    .child(
                                                        div()
                                                            .flex()
                                                            .items_center()
                                                            .justify_between()
                                                            .child(
                                                                div()
                                                                    .flex()
                                                                    .items_center()
                                                                    .gap_2()
                                                                    .child(
                                                                        div()
                                                                            .text_size(px(12.5))
                                                                            .font_weight(FontWeight::BOLD)
                                                                            .text_color(s.primary)
                                                                            .child(format!("New Release Available: v{}", upd.new_version)),
                                                                    )
                                                                    .when_some(upd.download_size, |row, sz| {
                                                                        row.child(
                                                                            div()
                                                                                .text_size(px(11.))
                                                                                .text_color(s.on_surface_variant)
                                                                                .child(format!("({})", format_file_size(sz))),
                                                                        )
                                                                    }),
                                                            )
                                                            .child(
                                                                Button::new(
                                                                    "btn_update_modal",
                                                                    format!("Update to {}", upd.new_version),
                                                                )
                                                                .filled()
                                                                .size(ButtonSize::Small)
                                                                .on_click(cx.listener({
                                                                    let u = upd_copy;
                                                                    move |this, _, _, cx| {
                                                                        this.update_single_package(u.clone(), cx);
                                                                    }
                                                                })),
                                                            ),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_size(px(10.5))
                                                            .text_color(s.success)
                                                            .child("🛡 100% Safe In-Place Update: All bookmarks, cookies, tabs, profiles, and settings in ~/.config are preserved."),
                                                    ),
                                            )
                                        })
                                )
                            })
                    })
                    // Action Buttons Row
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .pt_2()
                            .child(
                                Button::new("close_details", "Close")
                                    .tonal()
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.selected_app_details = None;
                                        this.editing_update_source = false;
                                        this.app_update_info = None;
                                        cx.notify();
                                    })),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .when(app_for_folder.install_path.is_some() || app_for_folder.desktop_file.is_some(), |row| {
                                        row.child(
                                            Button::new("reveal_details", "Show in Files")
                                                .tonal()
                                                .on_click(cx.listener({
                                                    let a = app_for_folder;
                                                    move |this, _, _, _| {
                                                        this.reveal_folder(&a);
                                                    }
                                                })),
                                        )
                                    })
                                    .child(
                                        Button::new("launch_details", "Launch App")
                                            .filled()
                                            .on_click(cx.listener({
                                                let a = app_for_launch;
                                                move |this, _, _, _| {
                                                    this.launch_app(&a);
                                                }
                                            })),
                                    )
                                    .child(
                                        Button::new("un_details", "Uninstall")
                                            .text()
                                            .on_click(cx.listener({
                                                let a = app_for_uninstall;
                                                move |this, _, _, cx| {
                                                    this.selected_app_details = None;
                                                    this.prepare_uninstall(a.clone(), cx);
                                                }
                                            })),
                                    ),
                            ),
                    ),
            )
    }

    fn render_updates_view(&self, cx: &mut Context<Self>, s: Scheme) -> impl IntoElement {
        let count = self.available_updates.len();
        let is_checking = self.is_checking_updates;
        let updating_pkg = self.updating_pkg.clone();

        div()
            .w_full()
            .flex()
            .flex_col()
            .gap_6()
            .pb_10()
            .child(render_page_heading(
                "Updates",
                format!("{} package updates are available", count),
                Some(
                    div()
                        .flex()
                        .items_center()
                        .gap_3()
                        .child(
                            Button::new("btn_check_updates", if is_checking { "Checking..." } else { "Check for updates" })
                                .tonal()
                                .on_click(cx.listener(|this, _, _, cx| this.check_updates(cx))),
                        )
                        .when(count > 0, |row| {
                            row.child(
                                Button::new("btn_update_all", format!("Update all ({count})"))
                                    .filled()
                                    .on_click(cx.listener(|this, _, _, cx| this.update_all_packages(cx))),
                            )
                        })
                        .into_any_element(),
                ),
                s,
            ))
            // Body: Updates List or "All Up to Date" Banner
            .child(if count == 0 {
                div()
                    .w_full()
                    .p_12()
                    .rounded(px(20.))
                    .bg(glass_bg(&s))
                    .border_1()
                    .border_color(s.outline_variant)
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap_3()
                    .child(Icon::new("check").size(px(36.)).color(s.success))
                    .child(
                        div()
                            .font_weight(FontWeight::BOLD)
                            .text_size(px(17.))
                            .text_color(s.on_surface)
                            .child("All Packages are Up to Date!"),
                    )
                    .child(
                        div()
                            .text_size(px(13.))
                            .text_color(s.on_surface_variant)
                            .child("Your Arch Linux packages, Flatpaks, and AUR software have no pending updates."),
                    )
                    .into_any_element()
            } else {
                div()
                    .w_full()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .children(self.available_updates.iter().enumerate().map(|(idx, u)| {
                        let update_clone = u.clone();
                        let is_this_updating = updating_pkg.as_deref() == Some(&u.package_name);

                        let source_str = match u.source {
                            AppSource::Pacman => "Pacman",
                            AppSource::Aur => "AUR",
                            AppSource::Flatpak => "Flatpak",
                            _ => "System",
                        };

                        div()
                            .id(("update_row", idx))
                            .w_full()
                            .p_4()
                            .rounded(px(14.))
                            .bg(glass_bg_high(&s))
                            .border_1()
                            .border_color(s.outline_variant)
                            .shadow(glass_shadow(&s))
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_4()
                                    .child(
                                        div()
                                            .px_2p5()
                                            .py_1()
                                            .rounded_full()
                                            .bg(s.secondary_container)
                                            .text_color(s.on_secondary_container)
                                            .text_size(px(11.))
                                            .font_weight(FontWeight::BOLD)
                                            .child(source_str),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap_0p5()
                                            .child(
                                                div()
                                                    .font_weight(FontWeight::BOLD)
                                                    .text_size(px(15.))
                                                    .text_color(s.on_surface)
                                                    .child(u.package_name.clone()),
                                            )
                                            .child(
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .gap_2()
                                                    .text_size(px(12.5))
                                                    .child(
                                                        div()
                                                            .text_color(s.on_surface_variant)
                                                            .child(u.current_version.clone()),
                                                    )
                                                    .child(div().text_color(s.on_surface_variant).child("→"))
                                                    .child(
                                                        div()
                                                            .text_color(s.success)
                                                            .font_weight(FontWeight::MEDIUM)
                                                            .child(u.new_version.clone()),
                                                    ),
                                            ),
                                    ),
                            )
                            .child(
                                Button::new(("btn_update_single", idx), if is_this_updating { "Updating..." } else { "Update" })
                                    .tonal()
                                    .on_click(cx.listener({
                                        let upd = update_clone;
                                        move |this, _, _, cx| {
                                            this.update_single_package(upd.clone(), cx);
                                        }
                                    })),
                            )
                    }))
                    .into_any_element()
            })
    }

    fn render_discover_view(&self, cx: &mut Context<Self>, s: Scheme) -> impl IntoElement {
        let installing_id = self.installing_store_pkg.clone();
        let is_searching = self.is_searching_store;

        div()
            .w_full()
            .flex()
            .flex_col()
            .gap_6()
            .pb_10()
            .child(render_page_heading(
                "Discover",
                "Find applications in Flathub, the AUR, and AppImageHub.",
                Some(
                    Button::new("discover_featured_btn", "Featured apps")
                        .tonal()
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.store_packages = store::get_featured_store_packages();
                            cx.notify();
                        }))
                        .into_any_element(),
                ),
                s,
            ))
            // Quick Search Suggestion Pills
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .flex_wrap()
                    .child(
                        div()
                            .text_size(px(12.5))
                            .text_color(s.on_surface_variant)
                            .child("Popular:"),
                    )
                    .child(
                        div()
                            .id("pop_spotify")
                            .px_3()
                            .py_1()
                            .rounded_full()
                            .bg(glass_bg_high(&s))
                            .border_1()
                            .border_color(s.outline_variant)
                            .hover(|h| h.bg(s.surface_container_high))
                            .cursor_pointer()
                            .text_size(px(12.))
                            .text_color(s.primary)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.search_store_packages("spotify", cx);
                            }))
                            .child("Spotify"),
                    )
                    .child(
                        div()
                            .id("pop_discord")
                            .px_3()
                            .py_1()
                            .rounded_full()
                            .bg(glass_bg_high(&s))
                            .border_1()
                            .border_color(s.outline_variant)
                            .hover(|h| h.bg(s.surface_container_high))
                            .cursor_pointer()
                            .text_size(px(12.))
                            .text_color(s.primary)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.search_store_packages("discord", cx);
                            }))
                            .child("Discord"),
                    )
                    .child(
                        div()
                            .id("pop_code")
                            .px_3()
                            .py_1()
                            .rounded_full()
                            .bg(glass_bg_high(&s))
                            .border_1()
                            .border_color(s.outline_variant)
                            .hover(|h| h.bg(s.surface_container_high))
                            .cursor_pointer()
                            .text_size(px(12.))
                            .text_color(s.primary)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.search_store_packages("code", cx);
                            }))
                            .child("VS Code"),
                    )
                    .child(
                        div()
                            .id("pop_obsidian")
                            .px_3()
                            .py_1()
                            .rounded_full()
                            .bg(glass_bg_high(&s))
                            .border_1()
                            .border_color(s.outline_variant)
                            .hover(|h| h.bg(s.surface_container_high))
                            .cursor_pointer()
                            .text_size(px(12.))
                            .text_color(s.primary)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.search_store_packages("obsidian", cx);
                            }))
                            .child("Obsidian"),
                    )
                    .child(
                        div()
                            .id("pop_steam")
                            .px_3()
                            .py_1()
                            .rounded_full()
                            .bg(glass_bg_high(&s))
                            .border_1()
                            .border_color(s.outline_variant)
                            .hover(|h| h.bg(s.surface_container_high))
                            .cursor_pointer()
                            .text_size(px(12.))
                            .text_color(s.primary)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.search_store_packages("steam", cx);
                            }))
                            .child("Steam"),
                    ),
            )
            // Store Packages Cards Grid
            .child(if is_searching {
                div()
                    .w_full()
                    .p_12()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(s.primary)
                    .text_size(px(15.))
                    .child("Searching AUR & Flathub packages...")
                    .into_any_element()
            } else if self.store_packages.is_empty() {
                div()
                    .w_full()
                    .p_12()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap_3()
                    .child(Icon::new("box").size(px(32.)).color(s.on_surface_variant))
                    .child(
                        div()
                            .font_weight(FontWeight::BOLD)
                            .text_size(px(16.))
                            .text_color(s.on_surface)
                            .child("No matching packages found"),
                    )
                    .child(
                        div()
                            .text_size(px(13.))
                            .text_color(s.on_surface_variant)
                            .child(
                                "Try searching by package name or keyword in the top search bar.",
                            ),
                    )
                    .into_any_element()
            } else {
                div()
                    .w_full()
                    .flex()
                    .flex_wrap()
                    .gap(px(18.))
                    .children(self.store_packages.iter().enumerate().map(|(idx, pkg)| {
                        let pkg_clone = pkg.clone();
                        let is_installing = installing_id.as_deref() == Some(&pkg.id);
                        let is_installed = pkg.is_installed;

                        let source_label = match pkg.source {
                            crate::models::StoreSource::Aur => "AUR",
                            crate::models::StoreSource::Flathub => "Flathub",
                            crate::models::StoreSource::AppImageHub => "AppImage",
                        };

                        div()
                            .id(("store_card", idx))
                            .w(px(272.))
                            .h(px(180.))
                            .p(px(18.))
                            .rounded(px(20.))
                            .bg(glass_bg_high(&s))
                            .border_1()
                            .border_color(s.outline_variant)
                            .shadow(glass_shadow(&s))
                            .hover(|h| {
                                h.bg(s.surface_container_high)
                                    .border_color(s.outline)
                                    .shadow(glass_shadow_lift(&s))
                            })
                            .flex()
                            .flex_col()
                            .justify_between()
                            // Top Row: Source badge & Version
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .child(
                                        div()
                                            .px_2p5()
                                            .py_0p5()
                                            .rounded_full()
                                            .bg(s.secondary_container)
                                            .text_color(s.on_secondary_container)
                                            .text_size(px(10.5))
                                            .font_weight(FontWeight::BOLD)
                                            .child(source_label),
                                    )
                                    .when_some(pkg.version.as_ref(), |el, ver| {
                                        el.child(
                                            div()
                                                .text_size(px(11.))
                                                .text_color(s.on_surface_variant)
                                                .child(ver.clone()),
                                        )
                                    }),
                            )
                            // Middle: Package icon, Title & Summary
                            .child(
                                div()
                                    .flex()
                                    .items_start()
                                    .gap_3()
                                    .child(
                                        div()
                                            .size(px(44.))
                                            .rounded(px(13.))
                                            .bg(s.primary_container)
                                            .border_1()
                                            .border_color(s.outline_variant)
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .flex_shrink_0()
                                            .text_color(s.on_primary_container)
                                            .font_weight(FontWeight::BOLD)
                                            .text_size(px(17.))
                                            .child(
                                                pkg.name
                                                    .chars()
                                                    .next()
                                                    .unwrap_or('A')
                                                    .to_uppercase()
                                                    .to_string(),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap_1()
                                            .min_w_0()
                                            .child(
                                                div()
                                                    .font_weight(FontWeight::BOLD)
                                                    .text_size(px(15.))
                                                    .text_color(s.on_surface)
                                                    .truncate()
                                                    .child(pkg.name.clone()),
                                            )
                                            .child(
                                                div()
                                                    .h(px(36.))
                                                    .text_size(px(12.))
                                                    .text_color(s.on_surface_variant)
                                                    .overflow_hidden()
                                                    .child(pkg.summary.clone()),
                                            ),
                                    ),
                            )
                            // Bottom Action: Install / Installed button
                            .child(div().flex().items_center().justify_end().child(
                                if is_installed {
                                    div()
                                        .px_3()
                                        .py_1()
                                        .rounded_full()
                                        .bg(s.secondary_container)
                                        .text_color(s.on_secondary_container)
                                        .text_size(px(11.5))
                                        .font_weight(FontWeight::BOLD)
                                        .child("✓ Installed")
                                        .into_any_element()
                                } else if is_installing {
                                    div()
                                        .px_3()
                                        .py_1()
                                        .rounded_full()
                                        .bg(s.surface_container_high)
                                        .text_color(s.primary)
                                        .text_size(px(11.5))
                                        .font_weight(FontWeight::BOLD)
                                        .child("Installing...")
                                        .into_any_element()
                                } else {
                                    Button::new(("btn_store_inst", idx), "Install")
                                        .filled()
                                        .on_click(cx.listener({
                                            let p = pkg_clone;
                                            move |this, _, _, cx| {
                                                this.install_from_store(p.clone(), cx);
                                            }
                                        }))
                                        .into_any_element()
                                },
                            ))
                    }))
                    .into_any_element()
            })
    }

    fn render_settings_view(&self, cx: &mut Context<Self>, s: Scheme) -> impl IntoElement {
        let backup_msg = self.backup_status.clone();
        let is_dark = self.dark_mode;

        div()
            .w_full()
            .flex()
            .flex_col()
            .gap_6()
            .pb_10()
            .child(render_page_heading(
                "Settings",
                "Manage backups, application cleanup, system integration, and appearance.",
                None,
                s,
            ))
            // Section 1: Backup & Migration
            .child(
                div()
                    .w_full()
                    .p_6()
                    .rounded(px(20.))
                    .bg(glass_bg(&s))
                    .border_1()
                    .border_color(s.outline_variant)
                    .shadow(glass_shadow(&s))
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        div()
                            .font_weight(FontWeight::BOLD)
                            .text_size(px(16.))
                            .text_color(s.on_surface)
                                            .child(render_setting_section_title("Machine migration & backup", "export", s)),
                    )
                    .child(
                        div()
                            .text_size(px(13.))
                            .text_color(s.on_surface_variant)
                            .child("Export all your installed applications across Pacman, AUR, Flatpak, and AppImages into a portable JSON manifest and automated bash restore script for new Arch machines."),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .pt_2()
                            .child(
                                Button::new("btn_export_backup", "Export Backup Bundle (~/installxs-backup.json)")
                                    .filled()
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.export_machine_backup(cx);
                                    })),
                            )
                            .when_some(backup_msg, |row, msg| {
                                row.child(
                                    div()
                                        .text_size(px(12.5))
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(s.success)
                                        .child(msg),
                                )
                            }),
                    ),
            )
            // Section 2: Deep AppCleaner Configuration
            .child(
                div()
                    .w_full()
                    .p_6()
                    .rounded(px(20.))
                    .bg(glass_bg(&s))
                    .border_1()
                    .border_color(s.outline_variant)
                    .shadow(glass_shadow(&s))
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        div()
                            .font_weight(FontWeight::BOLD)
                            .text_size(px(16.))
                            .text_color(s.on_surface)
                                            .child(render_setting_section_title("Deep app cleanup", "trash", s)),
                    )
                    .child(
                        div()
                            .text_size(px(13.))
                            .text_color(s.on_surface_variant)
                            .child("When uninstalling any application, InstallXS scans for orphaned residual files in ~/.config, ~/.cache, ~/.local/share, and ~/.var/app to free storage and keep your system pristine."),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .pt_1()
                            .child(
                                div()
                                    .px_3()
                                    .py_1()
                                    .rounded_full()
                                    .bg(s.secondary_container)
                                    .text_color(s.on_secondary_container)
                                    .text_size(px(11.5))
                                    .font_weight(FontWeight::BOLD)
                                    .child("✓ Residual Detection Active"),
                            ),
                    ),
            )
            // Section 3: MIME & FreeDesktop Integration
            .child(
                div()
                    .w_full()
                    .p_6()
                    .rounded(px(20.))
                    .bg(glass_bg(&s))
                    .border_1()
                    .border_color(s.outline_variant)
                    .shadow(glass_shadow(&s))
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        div()
                            .font_weight(FontWeight::BOLD)
                            .text_size(px(16.))
                            .text_color(s.on_surface)
                                            .child(render_setting_section_title("Desktop integration", "plug", s)),
                    )
                    .child(
                        div()
                            .text_size(px(13.))
                            .text_color(s.on_surface_variant)
                            .child("InstallXS is registered as the system handler for .AppImage, .tar.gz, .deb, and .zip application archives."),
                    )
                    .child(
                        div()
                            .pt_1()
                            .child(
                                Button::new("btn_relink_mime", "Update Desktop Database")
                                    .tonal()
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        let _ = Command::new("update-desktop-database")
                                            .arg(dirs::home_dir().map(|h| h.join(".local/share/applications")).unwrap_or_default())
                                            .output();
                                        this.toast = Some(("Updated desktop associations database".to_string(), false));
                                        cx.notify();
                                    })),
                            ),
                    ),
            )
            // Section 4: Theme & Appearance
            .child(
                div()
                    .w_full()
                    .p_6()
                    .rounded(px(20.))
                    .bg(glass_bg(&s))
                    .border_1()
                    .border_color(s.outline_variant)
                    .shadow(glass_shadow(&s))
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .font_weight(FontWeight::BOLD)
                                    .text_size(px(16.))
                                    .text_color(s.on_surface)
                                    .child(render_setting_section_title("Appearance", "palette", s)),
                            )
                            .child(
                                div()
                                    .text_size(px(13.))
                                    .text_color(s.on_surface_variant)
                                    .child(if is_dark { "Dark appearance" } else { "Light appearance" }),
                            ),
                    )
                    .child(
                        Button::new("btn_toggle_theme_settings", if is_dark { "Switch to Light Mode" } else { "Switch to Dark Mode" })
                            .tonal()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.toggle_theme(cx);
                            })),
                    ),
            )
    }

    fn render_uninstall_modal(
        &self,
        window: &mut Window,
        app: InstalledApp,
        cx: &mut Context<Self>,
        s: Scheme,
    ) -> impl IntoElement {
        let is_managed = app.is_managed_by_installxs;
        let file_count = app.installed_files.len();

        let residuals_opt = self.uninstall_residuals.as_ref();
        let selected_indices = &self.selected_residual_indices;

        let total_freed_bytes: u64 = residuals_opt
            .map(|res| {
                res.targets
                    .iter()
                    .enumerate()
                    .filter(|(idx, _)| selected_indices.contains(idx))
                    .map(|(_, t)| t.size_bytes)
                    .sum()
            })
            .unwrap_or(0);

        let targets = residuals_opt.map(|r| r.targets.clone()).unwrap_or_default();
        let has_residuals = !targets.is_empty();
        let p = motion::entrance("uninstall_modal", Spring::SPATIAL_FAST, 0., window, cx);

        div()
            .absolute()
            .inset_0()
            .bg(alpha(s.scrim, 0.45))
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .relative()
                    .top(px((1. - p) * 16.))
                    .opacity(p.clamp(0., 1.))
                    .w(px(540.))
                    .max_h(px(600.))
                    .p_6()
                    .rounded(px(20.))
                    .bg(glass_bg_high(&s))
                    .border_1()
                    .border_color(s.outline_variant)
                    .shadow(glass_shadow(&s))
                    .flex()
                    .flex_col()
                    .gap_4()
                    // Header
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_3()
                                    .child(
                                        div()
                                            .size(px(44.))
                                            .rounded(px(14.))
                                            .bg(s.error_container)
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .child(Icon::new("trash").size(px(22.)).color(s.on_error_container)),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap_0p5()
                                            .child(
                                                div()
                                                    .text_size(px(18.))
                                                    .font_weight(FontWeight::BOLD)
                                                    .text_color(s.on_surface)
                                                    .child(format!("Deep Clean: {}", app.name)),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(12.5))
                                                    .text_color(s.on_surface_variant)
                                                    .child(if is_managed {
                                                        format!("Managed app • {} manifest files tracked", file_count)
                                                    } else {
                                                        format!("Source: {}", app.source.label())
                                                    }),
                                            ),
                                    ),
                            )
                            .when(total_freed_bytes > 0, |header| {
                                header.child(
                                    div()
                                        .px_3()
                                        .py_1()
                                        .rounded_full()
                                        .bg(s.error_container)
                                        .border_1()
                                        .border_color(s.outline)
                                        .text_size(px(12.))
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(s.on_error_container)
                                        .child(format!("Reclaim: {}", format_file_size(total_freed_bytes))),
                                )
                            }),
                    )
                    // Description
                    .child(
                        div()
                            .text_size(px(13.))
                            .text_color(s.on_surface_variant)
                            .child(if has_residuals {
                                "InstallXS detected leftover configuration, cache, and state files. Select the items you wish to purge:"
                            } else {
                                "No leftover configuration or cache files detected for this application."
                            }),
                    )
                    // Residual Targets Checklist
                    .when(has_residuals, |el| {
                        el.child(
                            div()
                                .id("residuals_scroll")
                                .w_full()
                                .max_h(px(240.))
                                .overflow_y_scroll()
                                .flex()
                                .flex_col()
                                .gap_2()
                                .p_2()
                                .rounded(px(20.))
                                .bg(glass_bg(&s))
                                .border_1()
                                .border_color(s.outline_variant)
                                .children(targets.into_iter().enumerate().map(|(idx, target)| {
                                    let is_checked = selected_indices.contains(&idx);
                                    let path_display = target.path.to_string_lossy().to_string();
                                    let size_str = format_file_size(target.size_bytes);

                                    div()
                                        .id(("residual_item", idx))
                                        .w_full()
                                        .p_2p5()
                                        .rounded(px(12.))
                                        .bg(if is_checked { s.secondary_container } else { s.surface_container_low })
                                        .border_1()
                                        .border_color(if is_checked { s.outline } else { s.outline_variant })
                                        .hover(|h| h.bg(s.surface_container_high))
                                        .cursor_pointer()
                                        .flex()
                                        .items_center()
                                        .justify_between()
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            if this.selected_residual_indices.contains(&idx) {
                                                this.selected_residual_indices.remove(&idx);
                                            } else {
                                                this.selected_residual_indices.insert(idx);
                                            }
                                            cx.notify();
                                        }))
                                        .child(
                                            div()
                                                .flex()
                                                .items_center()
                                                .gap_2p5()
                                                .min_w_0()
                                                .child(
                                                    div()
                                                        .size(px(18.))
                                                        .rounded(px(5.))
                                                        .border_1()
                                                        .border_color(if is_checked { s.primary } else { s.outline })
                                                        .bg(if is_checked { s.primary } else { s.surface_container })
                                                        .flex()
                                                        .items_center()
                                                        .justify_center()
                                                        .text_size(px(11.))
                                                        .text_color(s.on_primary)
                                                        .child(if is_checked { "✓" } else { "" }),
                                                )
                                                .child(
                                                    div()
                                                        .flex()
                                                        .flex_col()
                                                        .min_w_0()
                                                        .child(
                                                            div()
                                                                .text_size(px(12.5))
                                                                .font_weight(FontWeight::MEDIUM)
                                                                .text_color(s.on_surface)
                                                                .truncate()
                                                                .child(path_display),
                                                        )
                                                        .child(
                                                            div()
                                                                .text_size(px(11.))
                                                                .text_color(s.on_surface_variant)
                                                                .child(target.label),
                                                        ),
                                                ),
                                        )
                                        .child(
                                            div()
                                                .px_2()
                                                .py_0p5()
                                                .rounded_full()
                                                .bg(s.surface_container_high)
                                                .text_size(px(11.))
                                                .text_color(s.primary)
                                                .font_weight(FontWeight::BOLD)
                                                .flex_shrink_0()
                                                .child(size_str),
                                        )
                                })),
                        )
                    })
                    // Action Buttons Row
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .pt_2()
                            .child(
                                Button::new("cancel_un", "Cancel")
                                    .tonal()
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.selected_for_uninstall = None;
                                        this.uninstall_residuals = None;
                                        cx.notify();
                                    })),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        Button::new("standard_un", "Standard Uninstall")
                                            .tonal()
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.execute_deep_uninstall(false, cx);
                                            })),
                                    )
                                    .child(
                                        Button::new("purge_un", if total_freed_bytes > 0 {
                                            format!("Deep clean and purge ({})", format_file_size(total_freed_bytes))
                                        } else {
                                            "Deep clean and purge".to_string()
                                        })
                                        .filled()
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.execute_deep_uninstall(true, cx);
                                        })),
                                    ),
                            ),
                    ),
            )
    }
}

fn render_page_heading(
    title: impl Into<gpui::SharedString>,
    subtitle: impl Into<gpui::SharedString>,
    action: Option<gpui::AnyElement>,
    s: Scheme,
) -> impl IntoElement {
    div()
        .w_full()
        .flex()
        .flex_wrap()
        .items_center()
        .justify_between()
        .gap_6()
        .pb_2()
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .font_weight(FontWeight::BOLD)
                        .text_size(px(24.))
                        .text_color(s.on_surface)
                        .child(title.into().to_string()),
                )
                .child(
                    div()
                        .text_size(px(13.5))
                        .text_color(s.on_surface_variant)
                        .child(subtitle.into().to_string()),
                ),
        )
        .when_some(action, |el, action| el.child(action))
}

fn render_setting_section_title(title: &str, icon: &'static str, s: Scheme) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap_3()
        .child(Icon::new(icon).size(px(18.)).color(s.primary))
        .child(title.to_string())
}

fn render_source_badge(source: &AppSource, s: Scheme) -> impl IntoElement {
    let dot = match source {
        AppSource::Pacman => s.primary,
        AppSource::Aur => s.tertiary,
        AppSource::Flatpak => s.secondary,
        AppSource::AppImage => s.warning,
        AppSource::InstallXs => s.success,
        AppSource::Manual | AppSource::Tarball | AppSource::Deb => s.on_surface_variant,
    };
    div()
        .px_2p5()
        .py_1()
        .rounded(px(8.))
        .bg(s.secondary_container)
        .flex()
        .items_center()
        .gap_1p5()
        .child(div().size(px(6.)).rounded_full().bg(dot))
        .child(
            div()
                .text_color(s.on_secondary_container)
                .text_size(px(11.))
                .font_weight(FontWeight::MEDIUM)
                .child(source.label()),
        )
}

fn render_app_icon(icon_path: Option<&String>, name: &str, s: &Scheme) -> impl IntoElement {
    render_app_icon_sized(icon_path, name, s, 44.)
}

fn render_app_icon_sized(
    icon_path: Option<&String>,
    name: &str,
    s: &Scheme,
    size: f32,
) -> impl IntoElement {
    let inner = size - 10.;
    if let Some(path_str) = icon_path {
        let p = Path::new(path_str);
        if p.exists() {
            let ext = p
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();
            if ext == "png" || ext == "svg" || ext == "webp" || ext == "jpg" || ext == "jpeg" {
                return div()
                    .size(px(size))
                    .rounded(px(size * 0.27))
                    .overflow_hidden()
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(s.surface_container_high)
                    .border_1()
                    .border_color(s.outline_variant)
                    .p(px(4.))
                    .child(img(PathBuf::from(path_str)).size(px(inner)))
                    .into_any_element();
            }
        }
    }

    let initial = name
        .chars()
        .next()
        .unwrap_or('A')
        .to_uppercase()
        .to_string();
    div()
        .size(px(size))
        .rounded(px(size * 0.27))
        .bg(s.primary_container)
        .border_1()
        .border_color(s.outline_variant)
        .text_color(s.on_primary_container)
        .flex()
        .items_center()
        .justify_center()
        .font_weight(FontWeight::BOLD)
        .text_size(px(size * 0.4))
        .child(initial)
        .into_any_element()
}

// ---------------------------------------------------------------------------
// Glass surfaces — zeron-style "Plate": a vertical sheen gradient, a hairline
// rim, a soft drop stack, and an inset top highlight so edges catch light.
// ---------------------------------------------------------------------------

fn glass_bg(s: &Scheme) -> gpui::Background {
    // BoardUI cards are flat surfaces; keep a whisper of sheen only.
    let _ = s;
    gpui::linear_gradient(
        180.,
        gpui::linear_color_stop(s.surface_container_low, 0.),
        gpui::linear_color_stop(s.surface_container_low, 1.),
    )
}

fn glass_bg_high(s: &Scheme) -> gpui::Background {
    let _ = s;
    gpui::linear_gradient(
        180.,
        gpui::linear_color_stop(s.surface_container, 0.),
        gpui::linear_color_stop(s.surface_container, 1.),
    )
}

fn glass_shadow(s: &Scheme) -> Vec<gpui::BoxShadow> {
    let _ = s;
    vec![
        gpui::BoxShadow {
            color: gpui::hsla(0., 0., 0., 0.35),
            offset: gpui::point(px(0.), px(1.)),
            blur_radius: px(3.),
            spread_radius: px(0.),
        },
        gpui::BoxShadow {
            color: gpui::hsla(0., 0., 0., 0.2),
            offset: gpui::point(px(0.), px(8.)),
            blur_radius: px(24.),
            spread_radius: px(-8.),
        },
    ]
}

fn glass_shadow_lift(s: &Scheme) -> Vec<gpui::BoxShadow> {
    let _ = s;
    vec![
        gpui::BoxShadow {
            color: gpui::hsla(0., 0., 0., 0.4),
            offset: gpui::point(px(0.), px(2.)),
            blur_radius: px(6.),
            spread_radius: px(0.),
        },
        gpui::BoxShadow {
            color: gpui::hsla(0., 0., 0., 0.28),
            offset: gpui::point(px(0.), px(16.)),
            blur_radius: px(36.),
            spread_radius: px(-10.),
        },
    ]
}

fn clean_app_description(app: &InstalledApp) -> String {
    if let Some(ref comment) = app.comment {
        let trimmed = comment.trim();
        if !trimmed.is_empty()
            && !trimmed.starts_with("local:brave-")
            && !trimmed.starts_with("local:chrome-")
            && !trimmed.starts_with("local:msedge-")
            && trimmed != &app.id
        {
            return trimmed.to_string();
        }
    }

    if app.id.starts_with("brave-") || app.id.contains("Default") || app.id.contains("Profile") {
        "Web Application (PWA)".to_string()
    } else {
        match app.source {
            AppSource::Pacman => "Arch Linux system package".to_string(),
            AppSource::Aur => "Arch User Repository package".to_string(),
            AppSource::AppImage => "Standalone AppImage bundle".to_string(),
            AppSource::Flatpak => "Sandboxed Flatpak application".to_string(),
            AppSource::InstallXs => "Managed by InstallXS".to_string(),
            AppSource::Manual | AppSource::Tarball | AppSource::Deb => {
                "Locally installed application".to_string()
            }
        }
    }
}
