# Configuration

Most settings are changed in the Settings window and saved for you. This page
covers what the window does not: where the files are, portable mode, and the
OAuth client that Gmail and Outlook sign-in need.

## Where Exospine keeps its files

Exospine keeps its files in `Colony/Exospine/`, in the platform folder for each
kind of file, like every Project Colony program:

| Folder | Linux | macOS | Windows |
|---|---|---|---|
| configuration | `~/.config/Colony/Exospine/` | `~/Library/Application Support/Colony/Exospine/` | `%LOCALAPPDATA%\Colony\Exospine\` |
| data | `~/.local/share/Colony/Exospine/` | `~/Library/Application Support/Colony/Exospine/` | `%LOCALAPPDATA%\Colony\Exospine\` |

On Linux, `$XDG_CONFIG_HOME` and `$XDG_DATA_HOME` move the two folders as usual.

| File | What it holds | Location |
|---|---|---|
| `config.toml` | settings, including the OAuth client IDs below | `preferences/` in the configuration folder |
| `accounts.json` | the account list: names, addresses and server settings, no passwords or tokens | `preferences/` in the configuration folder |
| `rules.json` | mail filter rules | `preferences/` in the configuration folder |
| `mail.db` | the local SQLite mail cache | data folder |
| `window_state.json` | the main window's size and position | data folder |
| `security.log` | OAuth sign-ins and failed sign-ins | data folder |
| `webview/` | the webview's own storage: PIN lock, calendar, RSS feeds, templates, labels, the offline action queue and display preferences | data folder, on Linux and Windows; on macOS the system keeps it in `~/Library/WebKit/` |

Passwords and OAuth refresh tokens are not in any of these files. They are in
the OS keyring under the service name `exospine`: the Secret Service on Linux
(GNOME Keyring, KWallet), the Credential Manager on Windows and the Keychain on
macOS. A password account has one entry named after its account ID, an OAuth
account one named `<account ID>:refresh-token`. OAuth access tokens are only
ever kept in memory.

Older versions encrypted `accounts.json` and kept OAuth refresh tokens in it.
On the first start after an upgrade, Exospine moves those tokens to the keyring,
rewrites the file as plain JSON and deletes the old `encryption.salt`. If
`accounts.json` cannot be read, it is renamed to
`accounts.json.unreadable-<time>` instead of being overwritten, and Exospine
says so at startup. If a token cannot be moved because the keyring is not
reachable, or an unreadable file cannot be renamed, the file is left as it is
and adding or removing an account fails with an error for that session, until a
later start can reach the keyring or the file is moved away. **Wipe All Data**
in Settings deletes these copies too, with the keyring entries of the accounts
they list; a copy still in the old encrypted format cannot be read, so the
entries of its accounts stay in the keyring.

Saved attachments go to `Exospine/` in your Downloads folder.

### Upgrading from 0.2 or earlier

Versions up to 0.2 kept every file in `exospine/` in the roaming configuration
folder (`~/.config/exospine/` on Linux, `~/Library/Application Support/exospine/`
on macOS, `%APPDATA%\exospine\` on Windows) and the webview storage in
`com.mothersphere.exospine` (`~/.local/share/` on Linux, `%LOCALAPPDATA%\` on
Windows). On its first start, this version copies them to the folders above and
writes a `.migrated` file in the data folder so it does not copy them again.

The old folders are left as they are, so nothing is lost if the copy turns out
wrong; you can delete them yourself once Exospine works from the new ones. If a
file cannot be copied, Exospine removes what it copied in that run, keeps
using the old folders and tries again at the next start. **Wipe All Data**
also deletes the mail cache, the account list and the webview storage left in
the old folders.

## Portable mode

Create an empty file named `portable.txt` next to the Exospine executable. From
the next start, the configuration and data folders are `data/config/` and
`data/data/` beside the executable, so the settings, the account list, the
rules, the mail cache, the security log and, on Linux and Windows, the webview
storage all travel with it. Passwords and tokens stay in the OS keyring of the
machine you signed in on.

A portable copy of version 0.2 or earlier kept its files directly in `data/`
(and its rules, security log and webview storage outside it). They are copied
into the new folders on the first start, as above.

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
