use tauri::{
    App,
    image::Image,
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};

use crate::window;

pub fn create_tray(app: &App) -> Result<(), String> {
    let icon = Image::from_bytes(include_bytes!("../icons/icon.png"))
        .map_err(|error| format!("failed to load tray icon: {error}"))?;
    let app_handle = app.handle().clone();

    TrayIconBuilder::with_id("main")
        .icon(icon)
        .icon_as_template(true)
        .tooltip("ScaleBridge")
        .show_menu_on_left_click(false)
        .on_tray_icon_event(move |_tray, event| {
            if is_activation_event(&event) {
                if let Err(error) = window::show_main_window(&app_handle) {
                    eprintln!("failed to show main window: {error}");
                }
            }
        })
        .build(app)
        .map_err(|error| format!("failed to create tray icon: {error}"))?;

    Ok(())
}

fn is_activation_event(event: &TrayIconEvent) -> bool {
    matches!(
        event,
        TrayIconEvent::Click {
            button: MouseButton::Left,
            button_state: MouseButtonState::Up,
            ..
        } | TrayIconEvent::DoubleClick {
            button: MouseButton::Left,
            ..
        }
    )
}
