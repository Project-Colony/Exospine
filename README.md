# Exospine

A modern, portable email client built with **Rust + Tauri v2**.
Exospine aims to be a fast, privacy-respecting alternative to mainstream email clients, with a lightweight vanilla JS frontend and a Rust backend handling all mail protocols.

![Screenshot placeholder](docs/screenshot.png)

---

## Features

### Mail
- IMAP sync with IDLE push notifications (real-time new mail)
- SMTP sending with draft versioning
- Multi-account support (unified inbox + per-account folder trees)
- Threaded conversation view (In-Reply-To / References)
- Full-text search across cached messages
- Pin, snooze, schedule send, flag for follow-up
- Star, archive, move, sweep sender
- Read receipts (send/request)
- Spam scoring and reporting
- Categories / labels
- Drag-and-drop `.eml` file import

### Providers
- **Gmail** — OAuth2 (XOAUTH2 SASL)
- **Outlook / Office 365 / Exchange Online** — OAuth2 (Microsoft endpoints, `outlook.office365.com`)
- **ProtonMail** — via Proton Bridge (localhost IMAP/SMTP, app password)
- **Yahoo, iCloud, Fastmail** — app password
- **Custom IMAP/SMTP** — any provider with standard ports

### Security
- Passwords and tokens stored in OS keyring
- Accounts file encrypted at rest (AES-256-GCM)
- TLS enforced on all IMAP/SMTP connections (rustls)
- Phishing / sender-spoofing warnings
- Anti-tracking for HTML emails
- Authentication-Results header analysis (SPF, DKIM, DMARC)

### Compose
- Rich text and plain text editor
- Per-account signatures (text + HTML)
- Draft auto-save with version history
- CC / BCC support
- Attachment handling

### UI
- Dark and light themes
- Compact / normal / comfortable density
- Right or bottom reading pane
- Resizable panels
- Keyboard shortcuts (customizable)
- Offline mode with action queue (sync on reconnect)
- Focus mode
- i18n (English, French, and more)
- Calendar and contacts views

### Portable Mode
- Drop a `portable.txt` file next to the executable
- All config and data stored in a `data/` folder next to the exe (USB-friendly)

---

## Installation / Build

### Prerequisites
- [Rust](https://rustup.rs/) (stable toolchain)
- [Node.js](https://nodejs.org/) (for Tauri CLI)
- [WebView2 Runtime](https://developer.microsoft.com/en-us/microsoft-edge/webview2/) (Windows; usually pre-installed on Windows 10+)

### Build
```bash
cd src-tauri
cargo build --release
```

The binary will be at `target/release/exospine-tauri.exe` (Windows).

### Run
```bash
cargo tauri dev
```

If WebView2 is installed in a non-standard location, set the environment variable:
```bash
set WEBVIEW2_BROWSER_EXECUTABLE_FOLDER=C:\path\to\WebView2
```

---

## Architecture

```
Exospine
 |- src-tauri/         Rust backend (Tauri v2)
 |   |- src/
 |   |   |- main.rs           App entry, Tauri setup, IDLE loops
 |   |   |- config.rs         Config loading/saving, account persistence (encrypted)
 |   |   |- app_state.rs      Shared state types (Account, MailEntry, etc.)
 |   |   |- accounts/         Provider detection, OAuth2, keyring, account setup
 |   |   |- mail/             IMAP, SMTP, parser, attachments, security, spam, IDLE
 |   |   |- commands/         Tauri IPC commands (accounts, mail, compose, settings, ...)
 |   |   |- storage/          SQLite database, audit log
 |   |   |- search.rs         Full-text search
 |   |   |- notifications.rs  Desktop notifications
 |   |- Cargo.toml
 |
 |- frontend/          Vanilla JS frontend
 |   |- js/
 |   |   |- app.js            Main entry, global state, init
 |   |   |- api.js            Tauri invoke wrappers
 |   |   |- views/            Sidebar, mail list, mail view, compose, onboarding, settings, ...
 |   |   |- components/       Toast, dialog, context menu
 |   |   |- i18n.js           Internationalization
 |   |- css/
 |   |- index.html
```

**Backend:** Rust with Tauri v2. All IMAP/SMTP operations, OAuth2 flows, encryption, and SQLite storage happen in the backend. The frontend communicates via Tauri's IPC invoke mechanism.

**Frontend:** Vanilla JavaScript (no framework). Views are rendered imperatively into DOM elements. State is managed in a simple global object in `app.js`.

---

## Configuration

Configuration is stored at:
- **Normal mode:** `%APPDATA%/exospine/config.toml` (Windows) or `~/.config/exospine/config.toml` (Linux/macOS)
- **Portable mode:** `data/config.toml` next to the executable (when `portable.txt` exists)

### OAuth2 Setup (Gmail / Outlook)

Set credentials in `config.toml`:
```toml
google_client_id = "your-google-client-id"
google_client_secret = "your-google-client-secret"
microsoft_client_id = "your-microsoft-client-id"
microsoft_client_secret = "your-microsoft-client-secret"
```

Or via environment variables:
```
EXOSPINE_GOOGLE_CLIENT_ID
EXOSPINE_GOOGLE_CLIENT_SECRET
EXOSPINE_MICROSOFT_CLIENT_ID
EXOSPINE_MICROSOFT_CLIENT_SECRET
```

---

## License

MIT

---

## Contributing

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/my-feature`)
3. Make your changes and ensure `cargo check` passes in `src-tauri/`
4. Test manually (no CI yet)
5. Commit with a clear message describing what changed and why
6. Open a pull request

Please keep the codebase lean: no heavy frameworks, no unnecessary dependencies. Vanilla JS on the frontend, idiomatic Rust on the backend.
