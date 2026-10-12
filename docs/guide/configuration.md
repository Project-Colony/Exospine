# Configuration

Most settings are changed in the Settings window and saved for you. This page
covers what the window does not: where the files are, portable mode, and the
OAuth client that Gmail and Outlook sign-in need.

## Where Exospine keeps its files

| File | What it holds | Location |
|---|---|---|
| `config.toml` | settings, including the OAuth client IDs below | data directory |
| `accounts.json` | the account list: names, addresses and server settings, no passwords or tokens | data directory |
| `mail.db` | the local SQLite mail cache | data directory |
| `window_state.json` | the main window's size and position | data directory |
| `rules.json` | mail filter rules | `exospine/` in the configuration directory |
| `security.log` | OAuth sign-ins and failed sign-ins | `exospine/` in the configuration directory |

The data directory is `exospine/` in the platform configuration directory, or
`data/` beside the executable in portable mode:

| Platform | Configuration directory |
|---|---|
| Linux | `~/.config/` (or `$XDG_CONFIG_HOME`) |
| macOS | `~/Library/Application Support/` |
| Windows | `%APPDATA%\` |

Passwords and OAuth refresh tokens are not in any of these files. They are in
the OS keyring under the service name `exospine`, one entry per account: the
Secret Service on Linux (GNOME Keyring, KWallet), the Credential Manager on
Windows and the Keychain on macOS.

Saved attachments go to `Exospine/` in your Downloads folder.

## Portable mode

Create an empty file named `portable.txt` next to the Exospine executable. From
the next start, `config.toml`, `accounts.json`, `mail.db` and
`window_state.json` are read from and written to a `data/` folder beside the
executable instead of the configuration directory.

Not everything moves yet: `rules.json` and `security.log` stay in the
configuration directory, passwords and tokens stay in the OS keyring of the
machine you signed in on, and the webview's own storage (PIN lock, calendar,
RSS feeds, templates) stays in the webview profile.

## Gmail and Outlook sign-in

Exospine does not ship OAuth client credentials. To sign in with Gmail or
Outlook / Office 365, register your own client with the provider and give
Exospine its ID (and secret, where there is one). Accounts on any other IMAP
server sign in with a password or an app password and need none of this.

During sign-in Exospine opens the provider's page in your browser and listens on
`127.0.0.1:8844` for the redirect, so register exactly this redirect URI:

```text
http://127.0.0.1:8844/oauth/callback
```

**Google.** In the Google Cloud console, enable the Gmail API, configure the
OAuth consent screen, and create an OAuth client of type *Desktop app*. Exospine
asks for the `https://mail.google.com/` scope and the `userinfo.email` and
`userinfo.profile` scopes.

**Microsoft.** In the Microsoft Entra admin center, register an application for
personal and work or school accounts, and add the redirect URI above under the
*Mobile and desktop applications* platform. Exospine asks for
`IMAP.AccessAsUser.All`, `SMTP.Send`, `openid`, `profile`, `email` and
`offline_access`.

Then put the values in `config.toml`:

```toml
google_client_id = "..."
google_client_secret = "..."
microsoft_client_id = "..."
microsoft_client_secret = ""
```

Or set them for one run through the environment instead:

| Variable | Overrides |
|---|---|
| `EXOSPINE_GOOGLE_CLIENT_ID` | `google_client_id` |
| `EXOSPINE_GOOGLE_CLIENT_SECRET` | `google_client_secret` |
| `EXOSPINE_MICROSOFT_CLIENT_ID` | `microsoft_client_id` |
| `EXOSPINE_MICROSOFT_CLIENT_SECRET` | `microsoft_client_secret` |

An environment variable takes precedence over `config.toml` while it is set, and
is never written into `config.toml`.
