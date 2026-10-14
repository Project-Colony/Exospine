// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::ffi::OsStr;

mod accounts;
mod app_state;
pub mod auth;
mod background;
mod commands;
mod config;
mod mail;
pub mod notifications;
mod paths;
mod search;
mod storage;
pub mod url_handlers;
pub mod window_state;

use url_handlers::{parse_exospine_url, parse_mailto, PENDING_MAILTO};

/// Whether the first command-line argument asks for the version only.
fn is_version_request(first_arg: Option<&OsStr>) -> bool {
    first_arg.is_some_and(|arg| arg == OsStr::new("--version") || arg == OsStr::new("-V"))
}

/// Release builds on Windows are GUI-subsystem programs with no console of
/// their own, so `--version` would print nowhere. Borrow the console of the
/// shell that started us, when there is one.
#[cfg(all(windows, not(debug_assertions)))]
fn attach_parent_console() {
    #[link(name = "kernel32")]
    extern "system" {
        fn AttachConsole(process_id: u32) -> i32;
    }
    const ATTACH_PARENT_PROCESS: u32 = u32::MAX;
    // SAFETY: AttachConsole takes a plain integer and has no preconditions;
    // failure (no parent console) is harmless and ignored.
    unsafe {
        AttachConsole(ATTACH_PARENT_PROCESS);
    }
}

fn main() {
    // Answer `--version` before Tauri or logging start, so it never opens a
    // window: the release workflow smoke-tests every binary this way.
    if is_version_request(std::env::args_os().nth(1).as_deref()) {
        #[cfg(all(windows, not(debug_assertions)))]
        attach_parent_console();
        println!("exospine {}", env!("CARGO_PKG_VERSION"));
        return;
    }

    // Install rustls CryptoProvider for IMAP TLS connections
    let _ = rustls::crypto::ring::default_provider().install_default();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    // Check if launched with a mailto: or exospine:// argument
    for arg in std::env::args().skip(1) {
        let data = if arg.starts_with("mailto:") {
            parse_mailto(&arg)
        } else if arg.starts_with("exospine://") {
            parse_exospine_url(&arg)
        } else {
            continue;
        };
        tracing::info!("Launched to compose to={}", data.to);
        *PENDING_MAILTO.lock().unwrap_or_else(|e| e.into_inner()) = Some(data);
        break;
    }

    // Resolve where the files are, carrying those of older versions over
    // first, before anything reads one.
    paths::layout();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(app_state::build())
        .setup(|app| {
            window_state::open_main_window(app)?;
            auth::refresh_all_in_background(app.handle());
            background::start(app.handle());
            mail::idle::start_all(app.handle());
            Ok(())
        })
        .invoke_handler(commands::handler())
        .run(tauri::generate_context!())
        .unwrap_or_else(|e| startup_failed(&e));
}

/// Tell the user why Exospine could not start, then exit.
fn startup_failed(e: &tauri::Error) -> ! {
    let msg = format!(
        "Exospine failed to start.\n\n\
         Error: {e}\n\n\
         This is often caused by a missing or outdated WebView2 runtime.\n\
         Please install WebView2 from:\n\
         https://developer.microsoft.com/en-us/microsoft-edge/webview2/\n\n\
         If the problem persists, try running Exospine as administrator.",
    );
    tracing::error!("{}", msg);
    // Show a native message box so the user sees the error even without a console
    #[cfg(target_os = "windows")]
    {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;
        let wide_msg: Vec<u16> = OsStr::new(&msg).encode_wide().chain(Some(0)).collect();
        let wide_title: Vec<u16> = OsStr::new("Exospine — Startup Error")
            .encode_wide()
            .chain(Some(0))
            .collect();
        unsafe {
            #[link(name = "user32")]
            extern "system" {
                fn MessageBoxW(
                    hwnd: *mut std::ffi::c_void,
                    text: *const u16,
                    caption: *const u16,
                    utype: u32,
                ) -> i32;
            }
            // MB_OK | MB_ICONERROR = 0x10
            MessageBoxW(
                std::ptr::null_mut(),
                wide_msg.as_ptr(),
                wide_title.as_ptr(),
                0x10,
            );
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        eprintln!("{}", msg);
    }
    std::process::exit(1);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_flags_are_recognised() {
        assert!(is_version_request(Some(OsStr::new("--version"))));
        assert!(is_version_request(Some(OsStr::new("-V"))));
    }

    #[test]
    fn other_arguments_start_the_app() {
        assert!(!is_version_request(None));
        for arg in ["-v", "--Version", "version", "mailto:a@example.com", "exospine://compose"] {
            assert!(!is_version_request(Some(OsStr::new(arg))), "{arg}");
        }
    }
}
