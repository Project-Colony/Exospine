//! The main window: opened at its saved position and size, saved again when
//! it closes.

use tauri::Manager;

use crate::app_state::AppState;

/// Open the main window. Built here rather than in tauri.conf.json, so that
/// it opens on the webview profile of the Colony layout. Closing it closes
/// the message pop-outs and stops the background tasks.
pub fn open_main_window(app: &tauri::App) -> tauri::Result<()> {
    let window = crate::paths::with_webview_profile(
        tauri::WebviewWindowBuilder::new(app, "main", tauri::WebviewUrl::default())
            .title("Exospine")
            .inner_size(1200.0, 800.0)
            .min_inner_size(800.0, 600.0),
    )
    .build()?;
    #[cfg(debug_assertions)]
    window.open_devtools();
    restore_window_state(&window);

    let win = window.clone();
    let shutdown = app.state::<AppState>().shutdown_tx.clone();
    window.on_window_event(move |event| {
        if let tauri::WindowEvent::CloseRequested { .. } = event {
            save_window_state(&win);
            // Pop-out message windows close with the main window,
            // so the app does not keep running without it.
            for (label, popout) in win.webview_windows() {
                if label.starts_with("mail-") {
                    let _ = popout.close();
                }
            }
            // Signal all background tasks to stop
            let _ = shutdown.send(());
            // Brief grace period for background tasks
            std::thread::sleep(std::time::Duration::from_millis(500));
        }
    });
    Ok(())
}

/// Window state stored as JSON on disk.
#[derive(serde::Serialize, serde::Deserialize)]
struct WindowState {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    maximized: bool,
}

fn window_state_path() -> std::path::PathBuf {
    let dir = &crate::paths::layout().data;
    let _ = std::fs::create_dir_all(dir);
    dir.join("window_state.json")
}

fn save_window_state(window: &tauri::WebviewWindow) {
    let Ok(position) = window.outer_position() else { return };
    let Ok(size) = window.outer_size() else { return };
    let maximized = window.is_maximized().unwrap_or(false);

    let scale = window.scale_factor().unwrap_or(1.0);
    let state = WindowState {
        x: position.x as f64 / scale,
        y: position.y as f64 / scale,
        width: size.width as f64 / scale,
        height: size.height as f64 / scale,
        maximized,
    };

    if let Ok(json) = serde_json::to_string_pretty(&state) {
        let _ = std::fs::write(window_state_path(), json);
    }
}

fn restore_window_state(window: &tauri::WebviewWindow) {
    let path = window_state_path();
    let Ok(data) = std::fs::read_to_string(&path) else { return };
    let Ok(state) = serde_json::from_str::<WindowState>(&data) else { return };

    let scale = window.scale_factor().unwrap_or(1.0);
    let _ = window.set_position(tauri::PhysicalPosition::new(
        (state.x * scale) as i32,
        (state.y * scale) as i32,
    ));
    let _ = window.set_size(tauri::PhysicalSize::new(
        (state.width * scale) as u32,
        (state.height * scale) as u32,
    ));
    if state.maximized {
        let _ = window.maximize();
    }
}
