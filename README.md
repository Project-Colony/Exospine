# Exospine

Exospine is a portable desktop email client. A Rust backend (Tauri v2) handles IMAP, SMTP, OAuth2 and local storage, and a vanilla JavaScript webview renders the interface.

> **Status:** early development. There is no release yet. CI builds and tests every change on Windows and is green, and the interface does not yet follow the Colony design system.

## What it does

- IMAP sync with IDLE push and SMTP sending, for several accounts at once
- Gmail and Outlook / Office 365 sign-in through OAuth2; other IMAP/SMTP providers with a password or app password
- Passwords and tokens kept in the OS keyring, the accounts file encrypted at rest, TLS through rustls
- Local SQLite mail cache with search
- Portable mode: put a `portable.txt` file next to the executable and all configuration and data go to a `data/` folder beside it

## Build from source

You need Rust 1.90 or newer and the [Tauri v2 prerequisites](https://v2.tauri.app/start/prerequisites/) for your platform (WebView2 on Windows, WebKitGTK on Linux).

```bash
cd src-tauri
cargo build --release
```

The binary is `src-tauri/target/release/exospine-tauri` (`exospine-tauri.exe` on Windows).

For development, install the Tauri CLI (`cargo install tauri-cli --version "^2"`) and run `cargo tauri dev`.

## Configuration

Settings live in `config.toml` inside the platform configuration directory: `~/.config/exospine/` on Linux, `~/Library/Application Support/exospine/` on macOS, `%APPDATA%\exospine\` on Windows, or `data/` in portable mode.

Gmail and Outlook sign-in need your own OAuth2 client credentials. Set them in `config.toml` (`google_client_id`, `google_client_secret`, `microsoft_client_id`, `microsoft_client_secret`) or through the environment variables `EXOSPINE_GOOGLE_CLIENT_ID`, `EXOSPINE_GOOGLE_CLIENT_SECRET`, `EXOSPINE_MICROSOFT_CLIENT_ID` and `EXOSPINE_MICROSOFT_CLIENT_SECRET`, which take precedence.

## License

Exospine is free software, licensed under the GNU General Public License v3.0 or later. See [LICENSE](LICENSE).
