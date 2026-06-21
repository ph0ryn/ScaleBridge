use tauri::{
    App, Emitter, Manager,
    image::Image,
    menu::{Menu, MenuItem},
    tray::{TrayIconBuilder, TrayIconEvent},
};

use crate::services;
use crate::state::AppState;
use crate::window;

const OPEN_MENU_ID: &str = "open";
const REFRESH_MENU_ID: &str = "refresh";
const TOGGLE_WATCHER_MENU_ID: &str = "toggle-watcher";
const QUIT_MENU_ID: &str = "quit";
const REFRESH_DASHBOARD_EVENT: &str = "dashboard://refresh-requested";
const RESUME_WATCHER_LABEL: &str = "Resume";
const STOP_WATCHER_LABEL: &str = "Stop";

pub fn create_tray(app: &App) -> Result<(), String> {
    let icon = Image::from_bytes(include_bytes!("../icons/icon.png"))
        .map_err(|error| format!("failed to load tray icon: {error}"))?;
    let open_item = MenuItem::with_id(app, OPEN_MENU_ID, "Open", true, None::<&str>)
        .map_err(|error| format!("failed to create open menu item: {error}"))?;
    let refresh_item = MenuItem::with_id(app, REFRESH_MENU_ID, "Refresh", true, None::<&str>)
        .map_err(|error| format!("failed to create refresh menu item: {error}"))?;
    let watcher_item = MenuItem::with_id(
        app,
        TOGGLE_WATCHER_MENU_ID,
        watcher_menu_label(app.handle()),
        true,
        None::<&str>,
    )
    .map_err(|error| format!("failed to create watcher menu item: {error}"))?;
    let quit_item = MenuItem::with_id(app, QUIT_MENU_ID, "Quit", true, None::<&str>)
        .map_err(|error| format!("failed to create quit menu item: {error}"))?;
    let menu = Menu::with_items(app, &[&open_item, &refresh_item, &watcher_item, &quit_item])
        .map_err(|error| format!("failed to create tray menu: {error}"))?;
    let app_handle = app.handle().clone();
    let watcher_item_for_menu = watcher_item.clone();
    let watcher_item_for_tray = watcher_item.clone();

    TrayIconBuilder::with_id("main")
        .icon(icon)
        .icon_as_template(true)
        .menu(&menu)
        .tooltip("ScaleBridge")
        .show_menu_on_left_click(true)
        .on_menu_event(move |app, event| match event.id().as_ref() {
            OPEN_MENU_ID => {
                if let Err(error) = window::show_main_window(app) {
                    eprintln!("failed to show main window: {error}");
                }
            }
            REFRESH_MENU_ID => {
                request_dashboard_refresh(app);
            }
            TOGGLE_WATCHER_MENU_ID => {
                toggle_watcher(app);
                update_watcher_menu_label(&watcher_item_for_menu, app);
            }
            QUIT_MENU_ID => {
                quit_app(app);
            }
            _ => {}
        })
        .on_tray_icon_event(move |_tray, event| {
            if should_refresh_menu_label(&event) {
                update_watcher_menu_label(&watcher_item_for_tray, &app_handle);
            }
        })
        .build(app)
        .map_err(|error| format!("failed to create tray icon: {error}"))?;

    Ok(())
}

fn request_dashboard_refresh(app: &tauri::AppHandle) {
    if let Err(error) = app.emit(REFRESH_DASHBOARD_EVENT, ()) {
        eprintln!("failed to request dashboard refresh: {error}");
    }
}

fn toggle_watcher(app: &tauri::AppHandle) {
    let Some(state) = app.try_state::<AppState>() else {
        eprintln!("failed to toggle watcher: app state is not available");
        return;
    };
    let state = state.inner().clone();

    let result = if watcher_running(app) {
        services::stop_watcher(state)
    } else {
        services::start_watcher(app.clone(), state)
    };

    if let Err(error) = result {
        eprintln!("failed to toggle watcher: {error}");
    }
}

fn quit_app(app: &tauri::AppHandle) {
    if let Some(state) = app.try_state::<AppState>() {
        if let Err(error) = services::stop_watcher(state.inner().clone()) {
            eprintln!("failed to stop watcher before quit: {error}");
        }
    }

    app.exit(0);
}

fn update_watcher_menu_label(item: &MenuItem<tauri::Wry>, app: &tauri::AppHandle) {
    if let Err(error) = item.set_text(watcher_menu_label(app)) {
        eprintln!("failed to update watcher menu item: {error}");
    }
}

fn watcher_menu_label(app: &tauri::AppHandle) -> &'static str {
    if watcher_running(app) {
        STOP_WATCHER_LABEL
    } else {
        RESUME_WATCHER_LABEL
    }
}

fn watcher_running(app: &tauri::AppHandle) -> bool {
    app.try_state::<AppState>()
        .and_then(|state| {
            state
                .inner()
                .with_lock(|state| Ok(state.status_snapshot().watcher_running))
                .ok()
        })
        .unwrap_or(false)
}

fn should_refresh_menu_label(event: &TrayIconEvent) -> bool {
    matches!(
        event,
        TrayIconEvent::Click { .. } | TrayIconEvent::DoubleClick { .. }
    )
}
