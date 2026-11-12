mod catalog;
mod db;
mod installer;
mod models;
mod ui;

use gpui::{App, AppContext as _, Application, Bounds, WindowBounds, WindowOptions, px, size};

fn ensure_display_env() {
    let has_wayland = std::env::var("WAYLAND_DISPLAY")
        .map(|v| !v.trim().is_empty())
        .unwrap_or(false);
    let has_x11 = std::env::var("DISPLAY")
        .map(|v| !v.trim().is_empty())
        .unwrap_or(false);

    if has_wayland || has_x11 {
        return;
    }

    // Try detecting Wayland socket in XDG_RUNTIME_DIR or /run/user/<uid>
    let runtime_dir = std::env::var("XDG_RUNTIME_DIR")
        .map(std::path::PathBuf::from)
        .or_else(|_| dirs::runtime_dir().ok_or(()));

    if let Ok(dir) = runtime_dir {
        for candidate in &["wayland-1", "wayland-0", "wayland-2"] {
            if dir.join(candidate).exists() {
                unsafe { std::env::set_var("WAYLAND_DISPLAY", candidate); }
                log::info!("Auto-detected WAYLAND_DISPLAY={}", candidate);
                return;
            }
        }
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                if let Some(name) = entry.file_name().to_str() {
                    if name.starts_with("wayland-")
                        && !name.ends_with(".lock")
                        && !name.ends_with(".sock")
                    {
                        unsafe { std::env::set_var("WAYLAND_DISPLAY", name); }
                        log::info!("Auto-detected WAYLAND_DISPLAY={}", name);
                        return;
                    }
                }
            }
        }
    }

    // Try detecting X11 socket in /tmp/.X11-unix
    let x11_dir = std::path::Path::new("/tmp/.X11-unix");
    if x11_dir.is_dir() {
        for num in &["1", "0", "2"] {
            if x11_dir.join(format!("X{}", num)).exists() {
                let disp = format!(":{}", num);
                unsafe { std::env::set_var("DISPLAY", &disp); }
                log::info!("Auto-detected DISPLAY={}", disp);
                return;
            }
        }
    }
}

fn main() {
    env_logger::init();

    // Check if CLI argument was provided: e.g. installxs ./discord.AppImage
    let args: Vec<String> = std::env::args().collect();
    let initial_file = if args.len() > 1 {
        Some(args[1].clone())
    } else {
        None
    };

    if let Some(ref arg) = initial_file {
        match arg.as_str() {
            "--help" | "-h" => {
                println!("InstallXS - Universal Arch Linux Application Hub & Studio");
                println!();
                println!("USAGE:");
                println!("    installxs [OPTIONS] [FILE]");
                println!();
                println!("OPTIONS:");
                println!("    -h, --help       Print this help message");
                println!("    -v, --version    Print version information");
                println!("    --drop           Open directly into the Package Drop & Install Studio");
                println!("    --list           List installed applications");
                println!("    --updates        Check for available system updates");
                println!("    --backup         Export machine migration manifest bundle");
                println!();
                println!("EXAMPLES:");
                println!("    installxs                         # Launch GUI app hub");
                println!("    installxs ./app.AppImage          # Inspect and auto-integrate an AppImage");
                println!("    installxs ./archive.tar.gz        # Extract and install an archive bundle");
                return;
            }
            "--version" | "-v" => {
                println!("installxs {}", env!("CARGO_PKG_VERSION"));
                return;
            }
            "--list" => {
                let apps = catalog::scan_all_apps();
                println!("Installed Applications ({}):", apps.len());
                for a in apps {
                    println!("  • {:<30} [{}]", a.name, a.source.label());
                }
                return;
            }
            "--updates" => {
                println!("Checking for available updates (Pacman, AUR, Flatpak)...");
                let updates = catalog::updates::check_system_updates();
                if updates.is_empty() {
                    println!("All packages are up to date!");
                } else {
                    println!("Found {} available update(s):", updates.len());
                    for u in updates {
                        println!("  • {:<25} {} -> {}", u.package_name, u.current_version, u.new_version);
                    }
                }
                return;
            }
            "--backup" => {
                if let Some(home) = dirs::home_dir() {
                    let dest = home.join("installxs-backup.json");
                    match catalog::backup::export_backup_to_file(&dest) {
                        Ok(p) => println!("Backup exported successfully to {}", p.display()),
                        Err(e) => eprintln!("Failed to export backup: {}", e),
                    }
                }
                return;
            }
            _ => {}
        }
    }

    ensure_display_env();

    if std::env::var("WAYLAND_DISPLAY").is_err() && std::env::var("DISPLAY").is_err() {
        eprintln!("Error: Cannot launch InstallXS GUI because no display server was found.");
        eprintln!("Neither WAYLAND_DISPLAY nor DISPLAY is set in your current environment.");
        eprintln!();
        eprintln!("To launch with a specific display:");
        eprintln!("    WAYLAND_DISPLAY=wayland-1 installxs");
        eprintln!("    DISPLAY=:1 installxs");
        eprintln!();
        eprintln!("To use CLI commands without a display, run: installxs --help");
        std::process::exit(1);
    }

    Application::new()
        .with_assets(gpui_m3::Assets)
        .run(|cx: &mut App| {
            gpui_m3::init(cx);
            // Default to dark theme for modern Arch Linux setup
            cx.set_global(gpui_m3::Theme::new(true, None));

            let bounds = Bounds::centered(None, size(px(1080.), px(720.)), cx);
            let win = cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: Some(gpui::TitlebarOptions {
                        title: Some("InstallXS - Arch Linux App Hub".into()),
                        appears_transparent: false,
                        traffic_light_position: None,
                    }),
                    ..Default::default()
                },
                |_, cx| {
                    cx.new(|cx| ui::AppView::new(initial_file, cx))
                },
            );

            if let Err(err) = win {
                eprintln!("Failed to open InstallXS window: {}", err);
                return;
            }
            cx.activate(true);
        });
}
