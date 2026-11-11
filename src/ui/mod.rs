use std::path::{Path, PathBuf};
use std::process::Command;
use gpui::{
    div, img, prelude::*, px, rgb, Context, Entity, ExternalPaths, FontWeight,
    IntoElement, ParentElement, Render, Styled, Window,
};
use gpui_m3::{
    Button, Chip, IconButton, Scheme, TextField, TextFieldEvent, Theme,
};

use crate::catalog;
use crate::installer::{self, detect_candidate, uninstaller};
use crate::models::{AppSource, InstallCandidate, InstallState, InstalledApp};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavTab {
    Applications,
    Managed,
    Updates,
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    Grid,
    List,
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
            |this: &mut Self, _, event: &TextFieldEvent, cx| {
                if let TextFieldEvent::Changed(text) = event {
                    this.search_query = text.to_string();
                    cx.notify();
                }
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

    pub fn pick_file(&mut self, cx: &mut Context<Self>) {
        let picked = rfd::FileDialog::new()
            .set_title("Select Package to Install (.AppImage, .tar, .zip, .deb)")
            .add_filter(
                "Linux App Packages",
                &["AppImage", "tar.gz", "tgz", "tar.xz", "txz", "zip", "deb", "bin", "desktop"],
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
        cx.set_global(Theme::new(self.dark_mode, None));
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
        let mut list: Vec<&InstalledApp> = self.apps
            .iter()
            .filter(|app| {
                let matches_tab = match self.active_tab {
                    NavTab::Applications => true,
                    NavTab::Managed => app.is_managed_by_installxs,
                    NavTab::Updates => false,
                    NavTab::Pacman => app.source == AppSource::Pacman,
                    NavTab::Aur => app.source == AppSource::Aur,
                    NavTab::AppImage => app.source == AppSource::AppImage,
                    NavTab::Flatpak => app.source == AppSource::Flatpak,
                    NavTab::Local => matches!(app.source, AppSource::Manual | AppSource::Tarball | AppSource::Deb),
                    NavTab::DropInstaller => true,
                    NavTab::Settings => true,
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
                        FilterChip::Local => matches!(app.source, AppSource::Manual | AppSource::Tarball | AppSource::Deb),
                    };
                    if !matches_chip {
                        return false;
                    }
                }

                if q.is_empty() {
                    return true;
                }

                app.name.to_lowercase().contains(&q)
                    || app.comment.as_ref().map(|c| c.to_lowercase().contains(&q)).unwrap_or(false)
                    || app.id.to_lowercase().contains(&q)
            })
            .collect();

        match self.sort_mode {
            SortMode::Name => list.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase())),
            SortMode::Source => list.sort_by(|a, b| a.source.label().cmp(b.source.label())),
        }

        list
    }
}

impl Render for AppView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let s = *Theme::of(cx);

        let managed_count = self.apps.iter().filter(|a| a.is_managed_by_installxs).count();
        let flatpak_count = self.apps.iter().filter(|a| a.source == AppSource::Flatpak).count();
        let pacman_count = self.apps.iter().filter(|a| a.source == AppSource::Pacman).count();
        let aur_count = self.apps.iter().filter(|a| a.source == AppSource::Aur).count();
        let appimage_count = self.apps.iter().filter(|a| a.source == AppSource::AppImage).count();
        let local_count = self.apps.iter().filter(|a| matches!(a.source, AppSource::Manual | AppSource::Tarball | AppSource::Deb)).count();
        let total_count = self.apps.len();
        let updates_count = 5;
        let is_search_active = !self.search_query.is_empty();

        div()
            .size_full()
            .bg(rgb(0x131118))
            .text_color(rgb(0xffffff))
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
                    .w(px(234.))
                    .h_full()
                    .bg(rgb(0x16131c))
                    .border_r_1()
                    .border_color(rgb(0x252030))
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
                            // macOS Traffic Light dots
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .px_2()
                                    .pt_1()
                                    .pb_4()
                                    .child(div().size(px(11.)).rounded_full().bg(rgb(0xed6a5e)))
                                    .child(div().size(px(11.)).rounded_full().bg(rgb(0xf5bf4f)))
                                    .child(div().size(px(11.)).rounded_full().bg(rgb(0x62c554))),
                            )
                            // Brand Header: Arch Logo + InstallXS
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_3()
                                    .px_2()
                                    .pb_4()
                                    .child(
                                        div()
                                            .size(px(38.))
                                            .rounded(px(12.))
                                            .bg(rgb(0x1e283d))
                                            .border_1()
                                            .border_color(rgb(0x324468))
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .overflow_hidden()
                                            .child(
                                                if Path::new("/usr/share/pixmaps/archlinux-logo.png").exists() {
                                                    img(PathBuf::from("/usr/share/pixmaps/archlinux-logo.png")).size(px(24.)).into_any_element()
                                                } else {
                                                    div().text_size(px(18.)).text_color(rgb(0x38bdf8)).font_weight(FontWeight::BOLD).child("A").into_any_element()
                                                }
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
                                                    .text_color(rgb(0xffffff))
                                                    .child("InstallXS"),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(11.))
                                                    .text_color(rgb(0x9a91a8))
                                                    .child("Arch Universal Hub"),
                                            ),
                                    ),
                            )
                            // Main Nav: Applications (Lilac pill when active)
                            .child(self.render_nav_item(
                                "nav_apps",
                                "Applications",
                                "⊞",
                                total_count,
                                self.active_tab == NavTab::Applications,
                                cx.listener(|this, _, _, cx| {
                                    this.active_tab = NavTab::Applications;
                                    cx.notify();
                                }),
                            ))
                            // Managed
                            .child(self.render_nav_item(
                                "nav_managed",
                                "Managed",
                                "✓",
                                managed_count,
                                self.active_tab == NavTab::Managed,
                                cx.listener(|this, _, _, cx| {
                                    this.active_tab = NavTab::Managed;
                                    cx.notify();
                                }),
                            ))
                            // Updates
                            .child(self.render_nav_item(
                                "nav_updates",
                                "Updates",
                                "↻",
                                updates_count,
                                self.active_tab == NavTab::Updates,
                                cx.listener(|this, _, _, cx| {
                                    this.active_tab = NavTab::Updates;
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
                                    .text_color(rgb(0x7c758a))
                                    .child("Sources"),
                            )
                            // Sources: Pacman
                            .child(self.render_nav_item(
                                "nav_pacman",
                                "Pacman",
                                "📦",
                                pacman_count,
                                self.active_tab == NavTab::Pacman,
                                cx.listener(|this, _, _, cx| {
                                    this.active_tab = NavTab::Pacman;
                                    cx.notify();
                                }),
                            ))
                            // Sources: AUR
                            .child(self.render_nav_item(
                                "nav_aur",
                                "AUR",
                                "A",
                                aur_count,
                                self.active_tab == NavTab::Aur,
                                cx.listener(|this, _, _, cx| {
                                    this.active_tab = NavTab::Aur;
                                    cx.notify();
                                }),
                            ))
                            // Sources: AppImage
                            .child(self.render_nav_item(
                                "nav_appimage",
                                "AppImage",
                                "🚀",
                                appimage_count,
                                self.active_tab == NavTab::AppImage,
                                cx.listener(|this, _, _, cx| {
                                    this.active_tab = NavTab::AppImage;
                                    cx.notify();
                                }),
                            ))
                            // Sources: Flatpak
                            .child(self.render_nav_item(
                                "nav_flatpak",
                                "Flatpak",
                                "🧊",
                                flatpak_count,
                                self.active_tab == NavTab::Flatpak,
                                cx.listener(|this, _, _, cx| {
                                    this.active_tab = NavTab::Flatpak;
                                    cx.notify();
                                }),
                            ))
                            // Sources: Local
                            .child(self.render_nav_item(
                                "nav_local",
                                "Local",
                                "📁",
                                local_count,
                                self.active_tab == NavTab::Local,
                                cx.listener(|this, _, _, cx| {
                                    this.active_tab = NavTab::Local;
                                    cx.notify();
                                }),
                            )),
                    )
                    // Bottom Section: Drop & Install + Settings
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .pt_3()
                            // Drop & Install CTA Pill Button
                            .child(
                                div()
                                    .id("btn_sidebar_drop")
                                    .w_full()
                                    .py_2p5()
                                    .px_4()
                                    .rounded_full()
                                    .bg(if self.active_tab == NavTab::DropInstaller { rgb(0xcfbcff) } else { rgb(0x2d243c) })
                                    .border_1()
                                    .border_color(if self.active_tab == NavTab::DropInstaller { rgb(0xcfbcff) } else { rgb(0x483a5e) })
                                    .text_color(if self.active_tab == NavTab::DropInstaller { rgb(0x1e1035) } else { rgb(0xe8def8) })
                                    .hover(|h| h.bg(rgb(0x372c49)))
                                    .cursor_pointer()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .gap_2()
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.active_tab = NavTab::DropInstaller;
                                        cx.notify();
                                    }))
                                    .child(div().text_size(px(15.)).child("⤓"))
                                    .child(
                                        div()
                                            .font_weight(FontWeight::BOLD)
                                            .text_size(px(13.5))
                                            .child("Drop & Install"),
                                    ),
                            )
                            // Settings Button
                            .child(
                                div()
                                    .id("btn_sidebar_settings")
                                    .w_full()
                                    .py_2()
                                    .px_3p5()
                                    .rounded_full()
                                    .text_color(rgb(0xcac4d0))
                                    .hover(|h| h.bg(rgb(0x231e2d)).text_color(rgb(0xffffff)))
                                    .cursor_pointer()
                                    .flex()
                                    .items_center()
                                    .gap_2p5()
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.active_tab = NavTab::Settings;
                                        this.toggle_theme(cx);
                                    }))
                                    .child(div().text_size(px(15.)).child("⚙"))
                                    .child(div().text_size(px(13.5)).font_weight(FontWeight::MEDIUM).child("Settings")),
                            ),
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
                    .bg(rgb(0x131118))
                    // Top App Bar with Search & Action Controls
                    .child(
                        div()
                            .flex_none()
                            .h(px(64.))
                            .px_8()
                            .bg(rgb(0x131118))
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap_4()
                            // Centered Search Bar
                            .child(
                                div()
                                    .flex_1()
                                    .max_w(px(520.))
                                    .h(px(42.))
                                    .bg(rgb(0x201c28))
                                    .border_1()
                                    .border_color(rgb(0x322a3d))
                                    .rounded(px(21.))
                                    .px_4()
                                    .flex()
                                    .items_center()
                                    .gap_3()
                                    .child(
                                        div()
                                            .text_color(rgb(0x9a91a8))
                                            .text_size(px(15.))
                                            .child("🔍"),
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
                                                .text_color(rgb(0x9a91a8))
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
                            // Right Top Bar Controls: 🔔 + ⋮ + Window controls
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2p5()
                                    .flex_shrink_0()
                                    // Notification Circle Button
                                    .child(
                                        div()
                                            .id("bell_circle_btn")
                                            .size(px(38.))
                                            .rounded_full()
                                            .bg(rgb(0x201c28))
                                            .border_1()
                                            .border_color(rgb(0x322a3d))
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .text_size(px(14.))
                                            .text_color(rgb(0xcac4d0))
                                            .hover(|h| h.bg(rgb(0x2b2436)))
                                            .cursor_pointer()
                                            .child("🔔"),
                                    )
                                    // Menu Circle Button
                                    .child(
                                        div()
                                            .id("menu_circle_btn")
                                            .size(px(38.))
                                            .rounded_full()
                                            .bg(rgb(0x201c28))
                                            .border_1()
                                            .border_color(rgb(0x322a3d))
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .text_size(px(16.))
                                            .text_color(rgb(0xcac4d0))
                                            .hover(|h| h.bg(rgb(0x2b2436)))
                                            .cursor_pointer()
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.pick_file(cx);
                                            }))
                                            .child("⋮"),
                                    )
                                    // Window Control Buttons (—, □, ✕)
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap_1()
                                            .pl_2()
                                            .child(
                                                div()
                                                    .size(px(28.))
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .text_color(rgb(0x9a91a8))
                                                    .hover(|h| h.text_color(rgb(0xffffff)))
                                                    .cursor_pointer()
                                                    .child("—"),
                                            )
                                            .child(
                                                div()
                                                    .size(px(28.))
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .text_color(rgb(0x9a91a8))
                                                    .hover(|h| h.text_color(rgb(0xffffff)))
                                                    .cursor_pointer()
                                                    .child("□"),
                                            )
                                            .child(
                                                div()
                                                    .size(px(28.))
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .text_color(rgb(0x9a91a8))
                                                    .hover(|h| h.text_color(rgb(0xed6a5e)))
                                                    .cursor_pointer()
                                                    .child("✕"),
                                            ),
                                    ),
                            ),
                    )
                    // Notification Toast
                    .when_some(self.toast.clone(), |el, (msg, is_err)| {
                        el.child(
                            div()
                                .flex_none()
                                .mx_8()
                                .mt_2()
                                .p_3()
                                .rounded(px(14.))
                                .bg(if is_err { rgb(0x421818) } else { rgb(0x24382c) })
                                .border_1()
                                .border_color(if is_err { rgb(0x662424) } else { rgb(0x385c46) })
                                .text_color(if is_err { rgb(0xf2b8b5) } else { rgb(0xa8dab5) })
                                .flex()
                                .items_center()
                                .justify_between()
                                .child(div().text_size(px(13.)).child(msg))
                                .child(
                                    Button::new("toast_dismiss", "Dismiss")
                                        .text()
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.toast = None;
                                            cx.notify();
                                        })),
                                ),
                        )
                    })
                    // Scrollable Workspace Content
                    .child(
                        div()
                            .flex_1()
                            .min_h_0()
                            .id("main_scroll")
                            .overflow_y_scroll()
                            .px_8()
                            .py_4()
                            .child(if self.active_tab == NavTab::DropInstaller || self.candidate.is_some() {
                                self.render_installer_view(cx, s).into_any_element()
                            } else {
                                self.render_app_hub_view(cx, s).into_any_element()
                            }),
                    ),
            )
            // App Details Inspector Modal
            .when_some(self.selected_app_details.clone(), |el, app| {
                el.child(self.render_app_details_modal(app, cx, s))
            })
            // Uninstall Confirmation Modal
            .when_some(self.selected_for_uninstall.clone(), |el, app| {
                el.child(self.render_uninstall_modal(app, cx, s))
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
        on_click: impl Fn(&gpui::ClickEvent, &mut Window, &mut gpui::App) + 'static,
    ) -> impl IntoElement {
        let is_active = active;
        div()
            .id(id)
            .w_full()
            .px_3p5()
            .py_2p5()
            .rounded_full()
            .cursor_pointer()
            .when(is_active, |el| {
                el.bg(rgb(0xcfbcff)).text_color(rgb(0x1e1035))
            })
            .when(!is_active, |el| {
                el.text_color(rgb(0xcac4d0))
                    .hover(|h| h.bg(rgb(0x231e2d)).text_color(rgb(0xffffff)))
            })
            .on_click(on_click)
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
                            .text_size(px(15.))
                            .child(icon),
                    )
                    .child(
                        div()
                            .text_size(px(13.5))
                            .font_weight(if is_active { FontWeight::BOLD } else { FontWeight::NORMAL })
                            .child(label),
                    ),
            )
            .child(
                div()
                    .px_2()
                    .py_0p5()
                    .rounded_full()
                    .text_size(px(11.))
                    .font_weight(FontWeight::BOLD)
                    .when(is_active, |el| {
                        el.bg(rgb(0xba9eff)).text_color(rgb(0x1e1035))
                    })
                    .when(!is_active, |el| {
                        el.text_color(rgb(0x8f869e))
                    })
                    .child(count.to_string()),
            )
    }

    fn render_installer_view(&self, cx: &mut Context<Self>, s: Scheme) -> impl IntoElement {
        let managed_apps: Vec<InstalledApp> = self.apps.iter().filter(|a| a.is_managed_by_installxs).cloned().collect();

        div()
            .w_full()
            .max_w(px(1440.))
            .mx_auto()
            .flex()
            .flex_col()
            .gap_6()
            .pb_12()
            // 1. Top Navigation & Header
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .pt_2()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_4()
                            .child(
                                div()
                                    .id("back_to_apps_btn")
                                    .cursor_pointer()
                                    .px_4()
                                    .py_2()
                                    .rounded_full()
                                    .bg(rgb(0x201c28))
                                    .border_1()
                                    .border_color(rgb(0x322a3d))
                                    .hover(|h| h.bg(rgb(0x282136)).text_color(rgb(0xffffff)))
                                    .text_color(rgb(0xcac4d0))
                                    .text_size(px(13.))
                                    .font_weight(FontWeight::MEDIUM)
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.active_tab = NavTab::Applications;
                                        cx.notify();
                                    }))
                                    .child("← Back to Applications"),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .child(
                                        div()
                                            .font_weight(FontWeight::BOLD)
                                            .text_size(px(20.))
                                            .text_color(rgb(0xffffff))
                                            .child("Package Installer & Integration"),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(12.5))
                                            .text_color(rgb(0x9a91a8))
                                            .child("Install, configure, and register standalone packages to your Arch system"),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(div().px_2p5().py_1().rounded_full().bg(rgb(0x351f4c)).text_color(rgb(0xdfb2fe)).text_size(px(11.)).font_weight(FontWeight::BOLD).child(".AppImage"))
                            .child(div().px_2p5().py_1().rounded_full().bg(rgb(0x182840)).text_color(rgb(0xa2cafe)).text_size(px(11.)).font_weight(FontWeight::BOLD).child(".tar.gz / .xz"))
                            .child(div().px_2p5().py_1().rounded_full().bg(rgb(0x193322)).text_color(rgb(0xa2e8bd)).text_size(px(11.)).font_weight(FontWeight::BOLD).child(".zip"))
                            .child(div().px_2p5().py_1().rounded_full().bg(rgb(0x3d2c18)).text_color(rgb(0xf5d098)).text_size(px(11.)).font_weight(FontWeight::BOLD).child(".deb"))
                            .child(div().px_2p5().py_1().rounded_full().bg(rgb(0x262132)).text_color(rgb(0xcac4d0)).text_size(px(11.)).font_weight(FontWeight::BOLD).child("binary")),
                    ),
            )
            // 2. Studio Main Row (Left: Drop Target or Staged Card + Architecture Cards, Right: Detected in Downloads)
            .child(
                div()
                    .flex()
                    .items_start()
                    .gap_6()
                    .w_full()
                    // Left Column (Drop Target / Staged Card + Architecture Cards)
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .gap_5()
                            // If NO candidate staged: Large Interactive Drop Target Card
                            .when(self.candidate.is_none(), |col| {
                                col.child(
                                    div()
                                        .id("dropzone_interactive_card")
                                        .p_8()
                                        .min_h(px(280.))
                                        .rounded(px(24.))
                                        .bg(rgb(0x1e182c))
                                        .border_2()
                                        .border_color(rgb(0x4a3666))
                                        .hover(|h| h.bg(rgb(0x241d34)).border_color(rgb(0xcfbcff)))
                                        .cursor_pointer()
                                        .flex()
                                        .flex_col()
                                        .items_center()
                                        .justify_center()
                                        .gap_3()
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.pick_file(cx);
                                        }))
                                        .child(
                                            div()
                                                .size(px(68.))
                                                .rounded(px(22.))
                                                .bg(rgb(0x33224d))
                                                .border_1()
                                                .border_color(rgb(0x56387d))
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .text_size(px(34.))
                                                .child("📦"),
                                        )
                                        .child(
                                            div()
                                                .font_weight(FontWeight::BOLD)
                                                .text_size(px(22.))
                                                .text_color(rgb(0xffffff))
                                                .child("Drop App Package to Install"),
                                        )
                                        .child(
                                            div()
                                                .text_size(px(13.5))
                                                .text_color(rgb(0x9a91a8))
                                                .child("Drag and drop any .AppImage, .tar.gz, .tar.xz, .zip, .deb, or binary here"),
                                        )
                                        .child(
                                            div()
                                                .mt_2()
                                                .px_6()
                                                .py_2p5()
                                                .rounded_full()
                                                .bg(rgb(0xcfbcff))
                                                .hover(|h| h.bg(rgb(0xe8def8)))
                                                .text_color(rgb(0x1e1035))
                                                .font_weight(FontWeight::BOLD)
                                                .text_size(px(13.5))
                                                .flex()
                                                .items_center()
                                                .gap_2()
                                                .child("📂 Browse Files from Computer"),
                                        )
                                        .child(
                                            div()
                                                .mt_1()
                                                .text_size(px(11.5))
                                                .text_color(rgb(0x7c738c))
                                                .child("Auto-extracts icons • FreeDesktop launcher • ~/.local/bin/ symlink • Clean manifest"),
                                        ),
                                )
                            })
                            // If candidate IS staged: Staged Candidate Card
                            .when_some(self.candidate.clone(), |col, cand| {
                                col.child(
                                    div()
                                        .p_6()
                                        .rounded(px(24.))
                                        .bg(rgb(0x1f192d))
                                        .border_2()
                                        .border_color(rgb(0xcfbcff))
                                        .flex()
                                        .flex_col()
                                        .gap_4()
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
                                                        .child(
                                                            div()
                                                                .size(px(52.))
                                                                .rounded(px(16.))
                                                                .bg(rgb(0x33224d))
                                                                .border_1()
                                                                .border_color(rgb(0x56387d))
                                                                .flex()
                                                                .items_center()
                                                                .justify_center()
                                                                .text_size(px(26.))
                                                                .child("🚀"),
                                                        )
                                                        .child(
                                                            div()
                                                                .flex()
                                                                .flex_col()
                                                                .child(
                                                                    div()
                                                                        .font_weight(FontWeight::BOLD)
                                                                        .text_size(px(18.))
                                                                        .text_color(rgb(0xffffff))
                                                                        .child(cand.suggested_name.clone()),
                                                                )
                                                                .child(
                                                                    div()
                                                                        .text_size(px(12.5))
                                                                        .text_color(rgb(0x9a91a8))
                                                                        .child(format!(
                                                                            "{} • {:.1} MB",
                                                                            cand.file_name,
                                                                            cand.file_size as f64 / 1_048_576.0
                                                                        )),
                                                                ),
                                                        ),
                                                )
                                                .child(
                                                    div()
                                                        .px_3()
                                                        .py_1()
                                                        .rounded_full()
                                                        .bg(rgb(0x351f4c))
                                                        .text_color(rgb(0xdfb2fe))
                                                        .text_size(px(11.5))
                                                        .font_weight(FontWeight::BOLD)
                                                        .child(format!("{:?}", cand.kind)),
                                                ),
                                        )
                                        .child(
                                            div()
                                                .p_4()
                                                .rounded(px(16.))
                                                .bg(rgb(0x161220))
                                                .border_1()
                                                .border_color(rgb(0x2d243c))
                                                .flex()
                                                .flex_col()
                                                .gap_2()
                                                .child(
                                                    div()
                                                        .text_size(px(12.))
                                                        .font_weight(FontWeight::BOLD)
                                                        .text_color(rgb(0xcfbcff))
                                                        .child("Automated System Integration Steps:"),
                                                )
                                                .child(
                                                    div()
                                                        .text_size(px(12.))
                                                        .text_color(rgb(0xcac4d0))
                                                        .child("✔ Installs cleanly into ~/.local/share/installxs/apps/"),
                                                )
                                                .child(
                                                    div()
                                                        .text_size(px(12.))
                                                        .text_color(rgb(0xcac4d0))
                                                        .child("✔ Generates FreeDesktop launcher in ~/.local/share/applications/"),
                                                )
                                                .child(
                                                    div()
                                                        .text_size(px(12.))
                                                        .text_color(rgb(0xcac4d0))
                                                        .child("✔ Creates terminal executable symlink in ~/.local/bin/"),
                                                )
                                                .child(
                                                    div()
                                                        .text_size(px(12.))
                                                        .text_color(rgb(0xcac4d0))
                                                        .child("✔ Records exact file manifest for complete 1-click clean uninstall"),
                                                ),
                                        )
                                        .child(
                                            div()
                                                .flex()
                                                .items_center()
                                                .justify_between()
                                                .pt_2()
                                                .child(
                                                    div()
                                                        .text_size(px(13.))
                                                        .text_color(rgb(0xcac4d0))
                                                        .child(match &self.install_state {
                                                            InstallState::Idle => "Ready to install to system.".to_string(),
                                                            InstallState::Inspecting(_) => "Inspecting archive contents...".to_string(),
                                                            InstallState::Installing { step, .. } => format!("⏳ {}", step),
                                                            InstallState::Success { message, .. } => format!("✅ {}", message),
                                                            InstallState::Error(err) => format!("❌ {}", err),
                                                        }),
                                                )
                                                .child(
                                                    div()
                                                        .flex()
                                                        .items_center()
                                                        .gap_3()
                                                        .child(
                                                            div()
                                                                .id("cancel_staged_btn")
                                                                .px_4()
                                                                .py_2()
                                                                .rounded_full()
                                                                .bg(rgb(0x282236))
                                                                .hover(|h| h.bg(rgb(0x352b45)))
                                                                .text_color(rgb(0xcac4d0))
                                                                .text_size(px(13.))
                                                                .cursor_pointer()
                                                                .on_click(cx.listener(|this, _, _, cx| {
                                                                    this.candidate = None;
                                                                    this.install_state = InstallState::Idle;
                                                                    cx.notify();
                                                                }))
                                                                .child("Cancel"),
                                                        )
                                                        .when(
                                                            matches!(self.install_state, InstallState::Idle | InstallState::Error(_)),
                                                            |row| {
                                                                row.child(
                                                                    div()
                                                                        .id("do_install_btn")
                                                                        .px_5()
                                                                        .py_2()
                                                                        .rounded_full()
                                                                        .bg(rgb(0xcfbcff))
                                                                        .hover(|h| h.bg(rgb(0xe8def8)))
                                                                        .text_color(rgb(0x1e1035))
                                                                        .font_weight(FontWeight::BOLD)
                                                                        .text_size(px(13.))
                                                                        .cursor_pointer()
                                                                        .on_click(cx.listener(|this, _, _, cx| {
                                                                            this.install_current_candidate(cx);
                                                                        }))
                                                                        .child("Install to System Now"),
                                                                )
                                                            },
                                                        )
                                                        .when(
                                                            matches!(self.install_state, InstallState::Success { .. }),
                                                            |row| {
                                                                row.child(
                                                                    div()
                                                                        .id("done_install_btn")
                                                                        .px_5()
                                                                        .py_2()
                                                                        .rounded_full()
                                                                        .bg(rgb(0xcfbcff))
                                                                        .hover(|h| h.bg(rgb(0xe8def8)))
                                                                        .text_color(rgb(0x1e1035))
                                                                        .font_weight(FontWeight::BOLD)
                                                                        .text_size(px(13.))
                                                                        .cursor_pointer()
                                                                        .on_click(cx.listener(|this, _, _, cx| {
                                                                            this.candidate = None;
                                                                            this.active_tab = NavTab::Managed;
                                                                            cx.notify();
                                                                        }))
                                                                        .child("View in App List"),
                                                                )
                                                            },
                                                        ),
                                                ),
                                        ),
                                )
                            })
                            // 3 Feature Architecture Cards (Row)
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_3p5()
                                    .child(
                                        div()
                                            .flex_1()
                                            .p_4()
                                            .rounded(px(18.))
                                            .bg(rgb(0x1e192a))
                                            .border_1()
                                            .border_color(rgb(0x31273d))
                                            .flex()
                                            .flex_col()
                                            .gap_1p5()
                                            .child(
                                                div()
                                                    .size(px(36.))
                                                    .rounded(px(10.))
                                                    .bg(rgb(0x2f2146))
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .text_size(px(18.))
                                                    .child("⚡"),
                                            )
                                            .child(
                                                div()
                                                    .font_weight(FontWeight::BOLD)
                                                    .text_size(px(13.5))
                                                    .text_color(rgb(0xffffff))
                                                    .child("Automated Isolation"),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(11.5))
                                                    .text_color(rgb(0x9a91a8))
                                                    .child("Installs into dedicated sandboxes in ~/.local/share/installxs/apps/."),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .flex_1()
                                            .p_4()
                                            .rounded(px(18.))
                                            .bg(rgb(0x1e192a))
                                            .border_1()
                                            .border_color(rgb(0x31273d))
                                            .flex()
                                            .flex_col()
                                            .gap_1p5()
                                            .child(
                                                div()
                                                    .size(px(36.))
                                                    .rounded(px(10.))
                                                    .bg(rgb(0x19283f))
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .text_size(px(18.))
                                                    .child("🖥️"),
                                            )
                                            .child(
                                                div()
                                                    .font_weight(FontWeight::BOLD)
                                                    .text_size(px(13.5))
                                                    .text_color(rgb(0xffffff))
                                                    .child("FreeDesktop Launcher"),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(11.5))
                                                    .text_color(rgb(0x9a91a8))
                                                    .child("Generates .desktop entries and ~/.local/bin/ symlinks for instant launch."),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .flex_1()
                                            .p_4()
                                            .rounded(px(18.))
                                            .bg(rgb(0x1e192a))
                                            .border_1()
                                            .border_color(rgb(0x31273d))
                                            .flex()
                                            .flex_col()
                                            .gap_1p5()
                                            .child(
                                                div()
                                                    .size(px(36.))
                                                    .rounded(px(10.))
                                                    .bg(rgb(0x1a3324))
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .text_size(px(18.))
                                                    .child("🛡️"),
                                            )
                                            .child(
                                                div()
                                                    .font_weight(FontWeight::BOLD)
                                                    .text_size(px(13.5))
                                                    .text_color(rgb(0xffffff))
                                                    .child("100% Clean Uninstall"),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(11.5))
                                                    .text_color(rgb(0x9a91a8))
                                                    .child("Tracks every installed file in SQLite for complete orphan-free removal."),
                                            ),
                                    ),
                            ),
                    )
                    // Right Column: "Detected in ~/Downloads" (Quick Install Box)
                    .child(
                        div()
                            .w(px(390.))
                            .p_5()
                            .rounded(px(24.))
                            .bg(rgb(0x1a1527))
                            .border_1()
                            .border_color(rgb(0x31273d))
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
                                            .child(div().text_size(px(18.)).child("📥"))
                                            .child(
                                                div()
                                                    .font_weight(FontWeight::BOLD)
                                                    .text_size(px(15.5))
                                                    .text_color(rgb(0xffffff))
                                                    .child("Detected in Downloads"),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .px_2p5()
                                            .py_0p5()
                                            .rounded_full()
                                            .bg(rgb(0x351f4c))
                                            .text_color(rgb(0xdfb2fe))
                                            .text_size(px(11.))
                                            .font_weight(FontWeight::BOLD)
                                            .child(format!("{} found", self.downloaded_packages.len())),
                                    ),
                            )
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(rgb(0x9a91a8))
                                    .child("Packages found in your ~/Downloads folder. Click any to stage and install immediately:"),
                            )
                            .child(if self.downloaded_packages.is_empty() {
                                div()
                                    .p_4()
                                    .rounded(px(14.))
                                    .bg(rgb(0x221a30))
                                    .border_1()
                                    .border_color(rgb(0x36274a))
                                    .flex()
                                    .flex_col()
                                    .gap_1p5()
                                    .child(
                                        div()
                                            .font_weight(FontWeight::BOLD)
                                            .text_size(px(13.))
                                            .text_color(rgb(0xffffff))
                                            .child("No packages in ~/Downloads"),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(11.5))
                                            .text_color(rgb(0x9a91a8))
                                            .child("Downloaded .AppImage or archive files will be detected and shown here automatically."),
                                    )
                                    .into_any_element()
                            } else {
                                let mut list = div().flex().flex_col().gap_2();
                                for (idx, cand) in self.downloaded_packages.iter().take(5).enumerate() {
                                    let path = cand.source_path.clone();
                                    let icon = match cand.kind {
                                        crate::models::FileKind::AppImage => "🚀",
                                        crate::models::FileKind::Deb => "📦",
                                        _ => "📁",
                                    };
                                    list = list.child(
                                        div()
                                            .p_2p5()
                                            .rounded(px(14.))
                                            .bg(rgb(0x221a30))
                                            .border_1()
                                            .border_color(rgb(0x36274a))
                                            .hover(|h| h.bg(rgb(0x2a1f3d)).border_color(rgb(0x56387d)))
                                            .flex()
                                            .items_center()
                                            .justify_between()
                                            .child(
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .gap_2p5()
                                                    .flex_1()
                                                    .child(
                                                        div()
                                                            .size(px(34.))
                                                            .rounded(px(10.))
                                                            .bg(rgb(0x33224d))
                                                            .flex()
                                                            .items_center()
                                                            .justify_center()
                                                            .text_size(px(16.))
                                                            .child(icon),
                                                    )
                                                    .child(
                                                        div()
                                                            .flex()
                                                            .flex_col()
                                                            .child(
                                                                div()
                                                                    .font_weight(FontWeight::BOLD)
                                                                    .text_size(px(12.5))
                                                                    .text_color(rgb(0xffffff))
                                                                    .child(cand.suggested_name.clone()),
                                                            )
                                                            .child(
                                                                div()
                                                                    .text_size(px(10.5))
                                                                    .text_color(rgb(0x9a91a8))
                                                                    .child(format!(
                                                                        "{:.1} MB • {:?}",
                                                                        cand.file_size as f64 / 1_048_576.0,
                                                                        cand.kind
                                                                    )),
                                                            ),
                                                    ),
                                            )
                                            .child(
                                                div()
                                                    .id(("dl_install_btn", idx as u32))
                                                    .cursor_pointer()
                                                    .px_3()
                                                    .py_1p5()
                                                    .rounded_full()
                                                    .bg(rgb(0x3c2758))
                                                    .hover(|h| h.bg(rgb(0xcfbcff)).text_color(rgb(0x1e1035)))
                                                    .text_color(rgb(0xdfb2fe))
                                                    .font_weight(FontWeight::BOLD)
                                                    .text_size(px(11.5))
                                                    .on_click(cx.listener({
                                                        let p = path.clone();
                                                        move |this, _, _, cx| {
                                                            this.stage_file(p.clone(), cx);
                                                        }
                                                    }))
                                                    .child("Install"),
                                            ),
                                    );
                                }
                                list.into_any_element()
                            }),
                    ),
            )
            // 3. Managed Apps Section (Bottom)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .pt_2()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2p5()
                            .child(
                                div()
                                    .font_weight(FontWeight::BOLD)
                                    .text_size(px(18.))
                                    .text_color(rgb(0xffffff))
                                    .child("Managed by InstallXS"),
                            )
                            .child(
                                div()
                                    .px_2p5()
                                    .py_0p5()
                                    .rounded_full()
                                    .bg(rgb(0x282035))
                                    .text_color(rgb(0xcac4d0))
                                    .text_size(px(11.5))
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(format!("{} apps", managed_apps.len())),
                            ),
                    )
                    .child(if managed_apps.is_empty() {
                        div()
                            .p_6()
                            .rounded(px(20.))
                            .bg(rgb(0x1a1527))
                            .border_1()
                            .border_color(rgb(0x31273d))
                            .flex()
                            .items_center()
                            .gap_4()
                            .child(div().text_size(px(32.)).child("📁"))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .child(
                                        div()
                                            .font_weight(FontWeight::BOLD)
                                            .text_size(px(14.5))
                                            .text_color(rgb(0xffffff))
                                            .child("No standalone packages installed yet"),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(12.))
                                            .text_color(rgb(0x9a91a8))
                                            .child("Packages you drop or install with InstallXS will be managed and tracked here with 1-click clean uninstall."),
                                    ),
                            )
                            .into_any_element()
                    } else {
                        self.render_grid_cards(managed_apps, cx, s).into_any_element()
                    }),
            )
    }

    fn render_app_hub_view(&self, cx: &mut Context<Self>, s: Scheme) -> impl IntoElement {
        let apps: Vec<InstalledApp> = self.filtered_apps().into_iter().cloned().collect();
        let current_count = apps.len();

        let tab_title = match self.active_tab {
            NavTab::Applications => "All applications",
            NavTab::Managed => "Managed by InstallXS",
            NavTab::Updates => "Available updates",
            NavTab::Pacman => "Pacman native packages",
            NavTab::Aur => "AUR applications",
            NavTab::AppImage => "Standalone AppImages",
            NavTab::Flatpak => "Flatpak applications",
            NavTab::Local => "Local desktop applications",
            NavTab::DropInstaller => "Install application",
            NavTab::Settings => "Settings",
        };

        div()
            .w_full()
            .flex()
            .flex_col()
            .gap_5()
            .pb_10()
            // 1. Hero Banner (displayed when on Applications tab with empty search query)
            .when(self.active_tab == NavTab::Applications && self.search_query.is_empty(), |el| {
                el.child(
                    div()
                        .w_full()
                        .h(px(196.))
                        .rounded(px(28.))
                        .bg(rgb(0x251b38))
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
                                .w(px(520.))
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
                                .max_w(px(560.))
                                .py_6()
                                .pl_8()
                                .flex()
                                .flex_col()
                                .gap_2p5()
                                .child(
                                    div()
                                        .font_weight(FontWeight::BOLD)
                                        .text_size(px(28.))
                                        .text_color(rgb(0xffffff))
                                        .child("Your Arch app hub"),
                                )
                                .child(
                                    div()
                                        .text_size(px(14.))
                                        .text_color(rgb(0xcac4d0))
                                        .child("Discover, install, and manage applications across Pacman, AUR, AppImage and more."),
                                )
                                .child(
                                    div()
                                        .pt_2()
                                        .child(
                                            div()
                                                .id("hero_get_started")
                                                .cursor_pointer()
                                                .px_6()
                                                .py_2p5()
                                                .rounded_full()
                                                .bg(rgb(0xffffff))
                                                .hover(|h| h.bg(rgb(0xe8def8)))
                                                .text_color(rgb(0x181224))
                                                .font_weight(FontWeight::BOLD)
                                                .text_size(px(13.5))
                                                .flex()
                                                .items_center()
                                                .gap_2()
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.active_tab = NavTab::DropInstaller;
                                                    cx.notify();
                                                }))
                                                .child("Get started →"),
                                        ),
                                ),
                        ),
                )
            })
            // 2. Section Header: Title, count, and View/Sort Controls
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .pt_1()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_0p5()
                            .child(
                                div()
                                    .font_weight(FontWeight::BOLD)
                                    .text_size(px(20.))
                                    .text_color(rgb(0xffffff))
                                    .child(tab_title),
                            )
                            .child(
                                div()
                                    .text_size(px(13.))
                                    .text_color(rgb(0x9a91a8))
                                    .child(format!("{} applications", current_count)),
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
                                    .bg(rgb(0x201c28))
                                    .border_1()
                                    .border_color(rgb(0x322a3d))
                                    .child(
                                        div()
                                            .id("view_grid_btn")
                                            .size(px(28.))
                                            .rounded(px(8.))
                                            .cursor_pointer()
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .when(self.view_mode == ViewMode::Grid, |el| {
                                                el.bg(rgb(0xcfbcff)).text_color(rgb(0x1e1035))
                                            })
                                            .when(self.view_mode != ViewMode::Grid, |el| {
                                                el.text_color(rgb(0x9a91a8)).hover(|h| h.text_color(rgb(0xffffff)))
                                            })
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.view_mode = ViewMode::Grid;
                                                cx.notify();
                                            }))
                                            .child("⊞"),
                                    )
                                    .child(
                                        div()
                                            .id("view_list_btn")
                                            .size(px(28.))
                                            .rounded(px(8.))
                                            .cursor_pointer()
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .when(self.view_mode == ViewMode::List, |el| {
                                                el.bg(rgb(0xcfbcff)).text_color(rgb(0x1e1035))
                                            })
                                            .when(self.view_mode != ViewMode::List, |el| {
                                                el.text_color(rgb(0x9a91a8)).hover(|h| h.text_color(rgb(0xffffff)))
                                            })
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.view_mode = ViewMode::List;
                                                cx.notify();
                                            }))
                                            .child("≡"),
                                    ),
                            )
                            // Sort Dropdown Button
                            .child(
                                div()
                                    .id("sort_toggle_btn")
                                    .px_3p5()
                                    .py_1p5()
                                    .rounded(px(14.))
                                    .bg(rgb(0x201c28))
                                    .border_1()
                                    .border_color(rgb(0x322a3d))
                                    .cursor_pointer()
                                    .hover(|h| h.bg(rgb(0x282234)))
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .text_size(px(12.5))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(rgb(0xe8def8))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.sort_mode = match this.sort_mode {
                                            SortMode::Name => SortMode::Source,
                                            SortMode::Source => SortMode::Name,
                                        };
                                        cx.notify();
                                    }))
                                    .child(match self.sort_mode {
                                        SortMode::Name => "Sort by: Name ⌵",
                                        SortMode::Source => "Sort by: Source ⌵",
                                    }),
                            ),
                    ),
            )
            // 3. Filter Chips (only when on Applications tab)
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
            // 4. Content Cards or Compact List
            .child(if apps.is_empty() {
                div()
                    .w_full()
                    .p_16()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap_3()
                    .child(div().text_size(px(40.)).child("🔍"))
                    .child(
                        div()
                            .font_weight(FontWeight::BOLD)
                            .text_size(px(18.))
                            .text_color(rgb(0xffffff))
                            .child("No matching applications found"),
                    )
                    .child(
                        div()
                            .text_size(px(13.))
                            .text_color(rgb(0x9a91a8))
                            .child("Try clearing your search query or switching source filter."),
                    )
                    .into_any_element()
            } else if self.view_mode == ViewMode::Grid {
                self.render_grid_cards(apps, cx, s).into_any_element()
            } else {
                self.render_compact_list(apps, cx, s).into_any_element()
            })
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

    fn render_grid_cards(
        &self,
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
                let app_for_details = app.clone();
                let subtitle = clean_app_description(&app);

                div()
                    .id(("grid_app_card", idx))
                    .w(px(272.))
                    .h(px(176.))
                    .p(px(18.))
                    .rounded(px(22.))
                    .bg(rgb(0x201c2a))
                    .hover(|h| h.bg(rgb(0x282136)).border_color(rgb(0x4d3d62)))
                    .border_1()
                    .border_color(rgb(0x31273d))
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
                    // Top row: App Icon (44x44) and Kebab Menu (⋮)
                    .child(
                        div()
                            .flex()
                            .items_start()
                            .justify_between()
                            .child(render_app_icon(app.icon.as_ref(), &app.name, &s))
                            .child(
                                div()
                                    .id(("kebab_btn", idx))
                                    .size(px(32.))
                                    .rounded_full()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_size(px(16.))
                                    .text_color(rgb(0x8e859b))
                                    .hover(|h| h.bg(rgb(0x352a45)).text_color(rgb(0xffffff)))
                                    .cursor_pointer()
                                    .on_click(cx.listener({
                                        let a = app_for_details;
                                        move |this, _, _, cx| {
                                            this.selected_app_details = Some(a.clone());
                                            cx.notify();
                                        }
                                    }))
                                    .child("⋮"),
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
                                    .font_weight(FontWeight::BOLD)
                                    .text_size(px(15.))
                                    .text_color(rgb(0xffffff))
                                    .truncate()
                                    .child(app.name.clone()),
                            )
                            .child(
                                div()
                                    .h(px(34.))
                                    .text_size(px(12.))
                                    .text_color(rgb(0x9a91a8))
                                    .overflow_hidden()
                                    .child(subtitle),
                            ),
                    )
                    // Bottom: Source Badge Pill
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .child(render_source_badge(&app.source)),
                    )
            }))
    }

    fn render_compact_list(
        &self,
        apps: Vec<InstalledApp>,
        cx: &mut Context<Self>,
        s: Scheme,
    ) -> impl IntoElement {
        div()
            .w_full()
            .flex()
            .flex_col()
            .gap_2p5()
            .children(apps.into_iter().enumerate().map(|(idx, app)| {
                let app_clone = app.clone();
                let app_details = app.clone();
                let subtitle = clean_app_description(&app);

                div()
                    .id(("list_app_row", idx))
                    .py_3()
                    .px_4()
                    .rounded(px(18.))
                    .bg(rgb(0x201c2a))
                    .hover(|h| h.bg(rgb(0x282136)).border_color(rgb(0x4d3d62)))
                    .border_1()
                    .border_color(rgb(0x31273d))
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3p5()
                            .min_w_0()
                            .child(render_app_icon(app.icon.as_ref(), &app.name, &s))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .min_w_0()
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap_2p5()
                                            .child(
                                                div()
                                                    .font_weight(FontWeight::BOLD)
                                                    .text_size(px(14.5))
                                                    .text_color(rgb(0xffffff))
                                                    .child(app.name.clone()),
                                            )
                                            .child(render_source_badge(&app.source)),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(12.))
                                            .text_color(rgb(0x9a91a8))
                                            .truncate()
                                            .child(subtitle),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .flex_shrink_0()
                            .child(
                                Button::new(("open_list", idx), "Launch")
                                    .tonal()
                                    .on_click(cx.listener({
                                        let a = app.clone();
                                        move |this, _, _, _| {
                                            this.launch_app(&a);
                                        }
                                    })),
                            )
                            .child(
                                IconButton::new(("info_list", idx), "brain")
                                    .size(px(32.))
                                    .tooltip("App Details & Paths")
                                    .on_click(cx.listener({
                                        let a = app_details;
                                        move |this, _, _, cx| {
                                            this.selected_app_details = Some(a.clone());
                                            cx.notify();
                                        }
                                    })),
                            )
                            .when(app.install_path.is_some() || app.desktop_file.is_some(), |row| {
                                row.child(
                                    IconButton::new(("fol_list", idx), "folder")
                                        .size(px(32.))
                                        .tooltip("Reveal in Files")
                                        .on_click(cx.listener({
                                            let a = app.clone();
                                            move |this, _, _, _| {
                                                this.reveal_folder(&a);
                                            }
                                        })),
                                )
                            })
                            .child(
                                IconButton::new(("un_list", idx), "trash")
                                    .size(px(32.))
                                    .tooltip("Uninstall Application")
                                    .on_click(cx.listener({
                                        let a = app_clone;
                                        move |this, _, _, cx| {
                                            this.selected_for_uninstall = Some(a.clone());
                                            cx.notify();
                                        }
                                    })),
                            ),
                    )
            }))
    }

    fn render_app_details_modal(
        &self,
        app: InstalledApp,
        cx: &mut Context<Self>,
        s: Scheme,
    ) -> impl IntoElement {
        let app_for_uninstall = app.clone();
        let app_for_launch = app.clone();
        let app_for_folder = app.clone();

        div()
            .absolute()
            .inset_0()
            .bg(s.scrim)
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .w(px(520.))
                    .p_6()
                    .rounded(px(24.))
                    .bg(s.surface_container_high)
                    .border_1()
                    .border_color(s.outline_variant)
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
                            .child(
                                Chip::new("detail_src", app.source.label())
                                    .tone(match app.source {
                                        AppSource::InstallXs => gpui_m3::Tone::Primary,
                                        AppSource::AppImage => gpui_m3::Tone::Primary,
                                        AppSource::Flatpak => gpui_m3::Tone::Secondary,
                                        AppSource::Aur => gpui_m3::Tone::Tertiary,
                                        _ => gpui_m3::Tone::Secondary,
                                    }),
                            ),
                    )
                    // File Paths & Details Box
                    .child(
                        div()
                            .p_4()
                            .rounded(px(14.))
                            .bg(s.surface_container)
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
                                                    this.selected_for_uninstall = Some(a.clone());
                                                    cx.notify();
                                                }
                                            })),
                                    ),
                            ),
                    ),
            )
    }

    fn render_uninstall_modal(
        &self,
        app: InstalledApp,
        cx: &mut Context<Self>,
        s: Scheme,
    ) -> impl IntoElement {
        let is_managed = app.is_managed_by_installxs;
        let file_count = app.installed_files.len();

        div()
            .absolute()
            .inset_0()
            .bg(s.scrim)
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .w(px(460.))
                    .p_6()
                    .rounded(px(24.))
                    .bg(s.surface_container_high)
                    .border_1()
                    .border_color(s.outline_variant)
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(
                                div()
                                    .size(px(42.))
                                    .rounded(px(12.))
                                    .bg(s.error_container)
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_color(s.on_error_container)
                                    .text_size(px(20.))
                                    .child("🗑️"),
                            )
                            .child(
                                div()
                                    .text_size(px(18.))
                                    .font_weight(FontWeight::BOLD)
                                    .child(format!("Uninstall {}?", app.name)),
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(13.))
                            .text_color(s.on_surface_variant)
                            .child(if is_managed {
                                format!(
                                    "InstallXS will cleanly remove this app and all its {} installed files, desktop launcher, and symlinks without leaving orphan files.",
                                    file_count
                                )
                            } else {
                                format!("This will invoke the uninstaller for {}.", app.name)
                            }),
                    )
                    .child(
                        div()
                            .flex()
                            .justify_end()
                            .gap_3()
                            .pt_2()
                            .child(
                                Button::new("cancel_un", "Cancel")
                                    .tonal()
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.selected_for_uninstall = None;
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("confirm_un", "Uninstall App")
                                    .filled()
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.execute_uninstall(cx);
                                    })),
                            ),
                    ),
            )
    }
}

fn render_source_badge(source: &AppSource) -> impl IntoElement {
    let (bg_color, text_color, label) = match source {
        AppSource::Pacman => (rgb(0x193322), rgb(0xa2e8bd), "Pacman"),
        AppSource::Aur => (rgb(0x182840), rgb(0xa2cafe), "AUR"),
        AppSource::AppImage => (rgb(0x351f4c), rgb(0xdfb2fe), "AppImage"),
        AppSource::Flatpak => (rgb(0x2d2140), rgb(0xd0bdff), "Flatpak"),
        AppSource::InstallXs => (rgb(0x351f4c), rgb(0xdfb2fe), "InstallXS"),
        AppSource::Manual | AppSource::Tarball | AppSource::Deb => (rgb(0x262132), rgb(0xcac4d0), "Local"),
    };

    div()
        .px_3()
        .py_1()
        .rounded_full()
        .bg(bg_color)
        .text_color(text_color)
        .text_size(px(11.))
        .font_weight(FontWeight::BOLD)
        .child(label)
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
            AppSource::Manual | AppSource::Tarball | AppSource::Deb => "Locally installed application".to_string(),
        }
    }
}

fn render_app_icon(icon_path: Option<&String>, name: &str, _s: &Scheme) -> impl IntoElement {
    if let Some(path_str) = icon_path {
        let p = Path::new(path_str);
        if p.exists() {
            let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
            if ext == "png" || ext == "svg" || ext == "webp" || ext == "jpg" || ext == "jpeg" {
                return div()
                    .size(px(44.))
                    .rounded(px(12.))
                    .overflow_hidden()
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(rgb(0x282133))
                    .border_1()
                    .border_color(rgb(0x3d3250))
                    .p(px(4.))
                    .child(img(PathBuf::from(path_str)).size(px(34.)))
                    .into_any_element();
            }
        }
    }

    let initial = name.chars().next().unwrap_or('A').to_uppercase().to_string();
    div()
        .size(px(44.))
        .rounded(px(12.))
        .bg(rgb(0x2e253c))
        .border_1()
        .border_color(rgb(0x45375c))
        .text_color(rgb(0xcfbcff))
        .flex()
        .items_center()
        .justify_center()
        .font_weight(FontWeight::BOLD)
        .text_size(px(18.))
        .child(initial)
        .into_any_element()
}
