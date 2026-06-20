use tauri::{AppHandle, Manager, WebviewWindow, WindowEvent};

const MAIN_WINDOW_LABEL: &str = "main";

pub fn show_main_window(app: &AppHandle) -> Result<(), String> {
    let window = match app.get_webview_window(MAIN_WINDOW_LABEL) {
        Some(window) => window,
        None => create_main_window(app)?,
    };

    show_and_focus(&window)
}

fn create_main_window(app: &AppHandle) -> Result<WebviewWindow, String> {
    let window_config = app
        .config()
        .app
        .windows
        .iter()
        .find(|window| window.label == MAIN_WINDOW_LABEL)
        .ok_or_else(|| format!("window config not found: {MAIN_WINDOW_LABEL}"))?;

    let window = tauri::WebviewWindowBuilder::from_config(app, window_config)
        .map_err(|error| format!("failed to create window builder: {error}"))?
        .build()
        .map_err(|error| format!("failed to create window: {error}"))?;

    destroy_on_close(&window);

    Ok(window)
}

fn show_and_focus(window: &WebviewWindow) -> Result<(), String> {
    window
        .show()
        .map_err(|error| format!("failed to show main window: {error}"))?;
    window
        .set_focus()
        .map_err(|error| format!("failed to focus main window: {error}"))?;
    Ok(())
}

fn destroy_on_close(window: &WebviewWindow) {
    let window_to_destroy = window.clone();
    window.on_window_event(move |event| {
        if let WindowEvent::CloseRequested { api, .. } = event {
            api.prevent_close();
            if let Err(error) = window_to_destroy.destroy() {
                eprintln!("failed to destroy main window: {error}");
            }
        }
    });
}
