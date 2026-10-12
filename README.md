<div align="center">

# Exospine

**A portable desktop email client for IMAP, Gmail and Outlook accounts.**

</div>

[![License: GPL-3.0-or-later](https://img.shields.io/badge/License-GPL--3.0--or--later-blue.svg)](LICENSE)
[![Colony app](https://img.shields.io/badge/Colony-network-purple)](https://github.com/Project-Colony/Colony)
[![Platforms](https://img.shields.io/badge/platforms-linux%20%7C%20windows%20%7C%20macOS-lightgrey)](#installation)

Reading mail usually means a browser tab per provider or a full mail suite with a
profile tied to one machine. Exospine is one small program for all of your
accounts: a Rust backend (Tauri v2) speaks IMAP, SMTP and OAuth2 and keeps a local
cache, and a plain JavaScript webview renders the mail after the backend has
cleaned it.

> **Status:** early development. CI builds and tests every change on Linux,
> Windows and macOS. Releases are built, signed and published by release-please
> and the Colony signing workflow; the first signed release has not been cut yet.
> Sign-in, sync and the keyring have not yet been re-tested against real Gmail,
> Outlook and IMAP accounts since the 2026-10 security and layout changes, and the
> interface does not yet follow the Colony design system.

## Why Exospine

- **One window for every account.** Gmail and Outlook / Office 365 through their
  OAuth2 sign-in, and any other IMAP/SMTP server with a password or app password,
  side by side instead of one webmail tab each.
- **Mail is cleaned before you see it.** Email HTML is sanitized in the backend,
  so no window ever receives scripts, forms or event handlers from a message, and
  remote images (the usual read-tracking pixels) stay blocked until you ask for
  them.
- **Secrets stay in the operating system.** Passwords and OAuth tokens live in the
  OS keyring, not in a file next to your mail.
- **Portable when you need it.** Drop a `portable.txt` next to the executable and
  the program keeps its settings, accounts and mail cache beside it.

## What it does

- IMAP sync with IDLE push and SMTP sending, for several accounts at once
- Gmail and Outlook / Office 365 sign-in through OAuth2 in your own browser;
  other providers with a password or app password
- Email HTML sanitized in the backend for every view: the reader, reply and
  forward, print and the pop-out window
- Remote images blocked until you click **Show images**
- Warnings for phishing links, look-alike (homograph) sender domains, failed
  SPF/DKIM checks and a Reply-To that differs from the sender
- Passwords and OAuth refresh tokens in the OS keyring on Linux (Secret Service),
  Windows (Credential Manager) and macOS (Keychain)
- Local SQLite mail cache with search
- Calendar, contacts, tasks, notes, templates and RSS feeds alongside the mail
- Portable mode: a `portable.txt` file next to the executable puts the settings,
  the account list, the mail cache and the window state in a `data/` folder
  beside it

## Installation

### Via Colony (recommended)

Search for **Exospine** in [Colony](https://github.com/Project-Colony/Colony) and
install it. Updates arrive through the launcher.

### Direct binary download

Grab the asset for your platform from the
[latest release](../../releases/latest):

| Platform | Asset |
|---|---|
| Linux | `exospine-linux` |
| Windows | `exospine-windows.exe` |
| macOS (Apple Silicon) | `exospine-macos` |
| macOS (Intel) | `exospine-macos-x86` |

```bash
chmod +x exospine-linux && ./exospine-linux
```

The Linux binary needs WebKitGTK 4.1 at runtime (`webkit2gtk-4.1` on Arch,
`libwebkit2gtk-4.1-0` on Debian and Ubuntu). Windows needs the WebView2 runtime,
which Windows 10 and 11 already include.

### Build from source

```bash
git clone https://github.com/Project-Colony/Exospine
cd Exospine
cargo build --release
```

Requires Rust 1.90 or newer and the
[Tauri v2 prerequisites](https://v2.tauri.app/start/prerequisites/) for your
platform (WebKitGTK 4.1 on Linux, WebView2 on Windows). The binary is
`target/release/exospine` (`exospine.exe` on Windows). `cargo run` starts a debug
build, which also has the web inspector.

## Documentation

Full documentation is in [docs/](docs/). Start at
[docs/README.md](docs/README.md).

## Code signing policy

Every release asset is signed by the Project Colony organisation in CI, never on
a developer machine. Next to each asset on the release page:

| File | What it is |
|---|---|
| `<asset>.sig` | an ed25519 signature over the asset, made with the organisation's release key |
| `<asset>.meta` | three lines binding the asset to its file name, its sha256 and the release version |
| `<asset>.meta.sig` | an ed25519 signature over the `.meta` |

The private key is an organisation secret, used only by the shared
[sign-and-publish workflow](https://github.com/Project-Colony/Project-Colony-Resources/blob/main/.github/workflows/sign-and-publish.yml)
in a job that builds nothing; the jobs that compile Exospine never see it.
Colony checks all three files before it installs or updates Exospine, and
refuses a release older than the one installed. To check a download yourself
with OpenSSL 3 (the same commands work for every asset):

```bash
cat > colony-release.pub <<'EOF'
-----BEGIN PUBLIC KEY-----
MCowBQYDK2VwAyEARNjg3Nn8H6/aBg1unwGjkUTcrdTxERNefVaqU8cFu0s=
-----END PUBLIC KEY-----
EOF
a=exospine-linux
openssl pkeyutl -verify -pubin -inkey colony-release.pub -rawin -in "$a" -sigfile "$a.sig"
openssl pkeyutl -verify -pubin -inkey colony-release.pub -rawin -in "$a.meta" -sigfile "$a.meta.sig"
cat "$a.meta"     # version=<tag>, asset=<file name>, sha256=<digest>
sha256sum "$a"    # the digest must equal the sha256 line
```

How releases are built, signed and published:
[design/releases.md](https://github.com/Project-Colony/Project-Colony-Resources/blob/main/design/releases.md#5-signing).

## Privacy

Exospine sends no telemetry, no analytics and no crash reports. It talks only to
the servers below, and only for the action named.

| Connection | When, and what is sent |
|---|---|
| Your IMAP and SMTP servers | syncing, searching and sending mail for the accounts you add, with the credentials of that account; a read receipt only when you click to send it |
| `accounts.google.com`, `oauth2.googleapis.com` | Gmail sign-in in your browser, then the token exchange and token refreshes |
| `www.googleapis.com/oauth2/v2/userinfo` | once at Gmail sign-in, to read the account's address and name |
| `login.microsoftonline.com` | Outlook / Office 365 sign-in in your browser, then the token exchange and token refreshes |
| `graph.microsoft.com/v1.0/me` | once at Outlook sign-in, to read the account's address and name |
| `127.0.0.1:8844` | a listener on your own machine, open only during an OAuth sign-in, that receives the browser's redirect |
| RSS feeds you add | fetching those feeds |
| Image and content hosts named in an email | only after you click **Show images** on that message |

Links you click in a message open in your default browser, not in Exospine.
Gmail sign-in is covered by the
[Google Privacy Policy](https://policies.google.com/privacy), Outlook sign-in by
the [Microsoft Privacy Statement](https://privacy.microsoft.com/privacystatement),
and your mail server and the feeds you add by their operators' own policies.

What Exospine keeps on your machine:

| Data | Where |
|---|---|
| Passwords and OAuth refresh tokens | the OS keyring, under the service name `exospine` |
| Settings (`config.toml`), the account list without any secret (`accounts.json`), the mail cache (`mail.db`, SQLite) and the window position (`window_state.json`) | `exospine/` in the configuration directory: `~/.config/exospine/` on Linux, `~/Library/Application Support/exospine/` on macOS, `%APPDATA%\exospine\` on Windows; in portable mode, `data/` next to the executable |
| Mail filter rules (`rules.json`) and the security log (`security.log`, which records OAuth sign-ins with the account address and failed sign-ins) | `exospine/` in the configuration directory, also in portable mode |
| Attachments you save | `Exospine/` in your Downloads folder |
| The PIN lock hash, calendar events, RSS feed list, templates, labels, the offline action queue and display preferences | the webview's local storage, in the webview profile that Tauri keeps for the app identifier `com.mothersphere.exospine` |
| Start with Windows, when you turn it on | an `Exospine` value under `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` |

## License

GPL-3.0-or-later. See [LICENSE](LICENSE).
