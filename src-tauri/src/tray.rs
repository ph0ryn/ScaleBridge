use tauri::{
    App, Manager,
    image::Image,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};

use crate::services;
use crate::state::AppState;
use crate::window;

const OPEN_MENU_ID: &str = "open";
const QUIT_MENU_ID: &str = "quit";

pub fn create_tray(app: &App) -> Result<(), String> {
    let icon = Image::from_bytes(include_bytes!("../icons/icon.png"))
        .map_err(|error| format!("failed to load tray icon: {error}"))?;
    let open_item = MenuItem::with_id(app, OPEN_MENU_ID, "Open ScaleBridge", true, None::<&str>)
        .map_err(|error| format!("failed to create open menu item: {error}"))?;
    let quit_item = MenuItem::with_id(app, QUIT_MENU_ID, "Quit ScaleBridge", true, None::<&str>)
        .map_err(|error| format!("failed to create quit menu item: {error}"))?;
    let menu = Menu::with_items(app, &[&open_item, &quit_item])
        .map_err(|error| format!("failed to create tray menu: {error}"))?;
    let app_handle = app.handle().clone();

    TrayIconBuilder::with_id("main")
        .icon(icon)
        .icon_as_template(true)
        .menu(&menu)
        .tooltip("ScaleBridge")
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            OPEN_MENU_ID => {
                if let Err(error) = window::show_main_window(app) {
                    eprintln!("failed to show main window: {error}");
                }
            }
            QUIT_MENU_ID => {
                quit_app(app);
            }
            _ => {}
        })
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

fn quit_app(app: &tauri::AppHandle) {
    if let Some(state) = app.try_state::<AppState>() {
        if let Err(error) = services::stop_watcher(state.inner().clone()) {
            eprintln!("failed to stop watcher before quit: {error}");
        }
    }

    app.exit(0);
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
