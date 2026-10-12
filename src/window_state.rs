//! Window state persistence: save and restore window position/size.

/// Window state stored as JSON on disk.
#[derive(serde::Serialize, serde::Deserialize)]
struct WindowState {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    maximized: bool,
}

pub fn window_state_path() -> std::path::PathBuf {
    let dir = crate::config::data_dir();
    let _ = std::fs::create_dir_all(&dir);
    dir.join("window_state.json")
}

pub fn save_window_state(window: &tauri::WebviewWindow) {
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

pub fn restore_window_state(window: &tauri::WebviewWindow) {
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
