mod catalog;
mod db;
mod installer;
mod models;
mod ui;

use gpui::{App, AppContext as _, Application, Bounds, WindowBounds, WindowOptions, px, size};

fn main() {
    env_logger::init();

    // Check if CLI argument was provided: e.g. installxs ./discord.AppImage
    let args: Vec<String> = std::env::args().collect();
    let initial_file = if args.len() > 1 {
        Some(args[1].clone())
    } else {
        None
    };

    Application::new()
        .with_assets(gpui_m3::Assets)
        .run(|cx: &mut App| {
            gpui_m3::init(cx);
            // Default to dark theme for modern Arch Linux setup
            cx.set_global(gpui_m3::Theme::new(true, None));

            let bounds = Bounds::centered(None, size(px(1080.), px(720.)), cx);
            cx.open_window(
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
            )
            .unwrap();
            cx.activate(true);
        });
}
