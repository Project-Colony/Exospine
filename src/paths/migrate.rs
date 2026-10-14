//! Carries the files of older versions over to the Colony layout.
//!
//! Up to 0.2, Exospine kept everything in `exospine/` under the roaming
//! configuration directory (or `data/` beside the executable in portable
//! mode), and the webview profile in a directory named after the old app
//! identifier, `com.mothersphere.exospine`. These are live on users'
//! machines, so the protocol of `design/filesystem.md` applies:
//!
//! 1. if the marker says the move already happened, use the new layout;
//! 2. copy every old file that is not at its new path yet;
//! 3. if a copy fails, remove what this run copied, write no marker and keep
//!    the old layout for this run, so the next start retries from the old
//!    files rather than from a half-copied profile;
//! 4. leave the old files alone: deleting them belongs to a later release,
//!    once this one has proven itself. Only Wipe All Data removes them
//!    ([`remove_legacy_copies`]), because the user asked for exactly that.
//!
//! Copying rather than renaming is deliberate: the two locations can be on
//! different filesystems, and a half-finished rename has no safe recovery.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use super::Layout;
use crate::config::{ACCOUNTS_FILE, CONFIG_FILE, LEGACY_SALT_FILE};

/// Written in the new data root once every file is across.
pub(super) const MARKER: &str = ".migrated";

/// The database and the SQLite files that can hold part of it.
const MAIL_DB: [&str; 3] = ["mail.db", "mail.db-wal", "mail.db-shm"];
const WINDOW_STATE: &str = "window_state.json";

/// The layout to use this run: `new` once everything in `old` is copied
/// across, `old` while that is not possible. Never fails: a migration that
/// cannot complete degrades to the old location, not to an empty profile.
pub(super) fn run(new: &Layout, old: &Layout, marker: &Path) -> Layout {
    if marker.exists() {
        return new.clone();
    }
    let mut copied: Vec<PathBuf> = Vec::new();
    for (from, to) in copies(old, new) {
        if let Err(e) = copy_into_place(&from, &to) {
            tracing::warn!(
                "Could not copy {} to {}: {e}. Keeping the old location for now.",
                from.display(),
                to.display()
            );
            for path in copied.iter().rev() {
                let _ = remove_any(path);
            }
            return old.clone();
        }
        tracing::info!("Copied {} to {}", from.display(), to.display());
        copied.push(to);
    }
    // Harmless if it fails: each copy above is skipped once its destination
    // exists, so the next run copies nothing again.
    let written = marker.parent().map_or(Ok(()), fs::create_dir_all);
    if let Err(e) = written.and_then(|()| fs::write(marker, "1")) {
        tracing::warn!("Could not write {}: {e}", marker.display());
    }
    new.clone()
}

/// Each (old, new) pair that still has to be copied.
fn copies(old: &Layout, new: &Layout) -> Vec<(PathBuf, PathBuf)> {
    // accounts.json, the copies set aside as unreadable and the key salt of
    // the encrypted format stay together: the account loader reads them from
    // one directory.
    let mut pairs: Vec<(PathBuf, PathBuf)> = fs::read_dir(&old.preferences)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.file_name())
        .filter(|name| {
            let name = name.to_string_lossy();
            name == CONFIG_FILE || name == LEGACY_SALT_FILE || name.starts_with(ACCOUNTS_FILE)
        })
        .map(|name| (old.preferences.join(&name), new.preferences.join(&name)))
        .collect();
    for name in MAIL_DB.into_iter().chain([WINDOW_STATE]) {
        pairs.push((old.data.join(name), new.data.join(name)));
    }
    pairs.push((old.rules.clone(), new.rules.clone()));
    pairs.push((old.security_log.clone(), new.security_log.clone()));
    if let (Some(from), Some(to)) = (&old.webview, &new.webview) {
        pairs.push((from.clone(), to.clone()));
    }
    pairs.retain(|(from, to)| from.exists() && !to.exists());
    pairs
}

/// Wipe All Data: delete what the migration left in the old location that
/// holds mail, accounts or the webview's storage. Nothing to do when this run
/// uses the old location itself, which the regular wipe covers.
pub fn remove_legacy_copies() -> io::Result<()> {
    let Some(old) = &super::resolved().legacy else {
        return Ok(());
    };
    for name in MAIL_DB {
        remove_any(&old.data.join(name))?;
    }
    if let Some(webview) = &old.webview {
        remove_any(webview)?;
    }
    Ok(())
}

/// Copy a file or a directory tree into place through a staging sibling, so
/// an interrupted copy never leaves a partial destination that a later run
/// would take for a finished one.
fn copy_into_place(from: &Path, to: &Path) -> io::Result<()> {
    let parent = to
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "no parent directory"))?;
    fs::create_dir_all(parent)?;
    let name = to.file_name().map(|n| n.to_string_lossy()).unwrap_or_default();
    let staging = parent.join(format!(".{name}.migrating"));
    remove_any(&staging)?;

    let copied = if from.is_dir() {
        copy_dir(from, &staging)
    } else {
        fs::copy(from, &staging).map(drop)
    };
    match copied.and_then(|()| fs::rename(&staging, to)) {
        Ok(()) => Ok(()),
        Err(e) => {
            let _ = remove_any(&staging);
            Err(e)
        }
    }
}

fn copy_dir(from: &Path, to: &Path) -> io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        let kind = entry.file_type()?;
        if kind.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else if kind.is_file() {
            fs::copy(entry.path(), &target)?;
        }
        // Symbolic links and sockets in a webview profile are runtime state
        // that the webview recreates; they are not copied.
    }
    Ok(())
}

fn remove_any(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.is_dir() => fs::remove_dir_all(path),
        Ok(_) => fs::remove_file(path),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            let dir = std::env::temp_dir().join(format!("exospine-paths-{}", uuid::Uuid::new_v4()));
            fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn write(path: &Path, body: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, body).unwrap();
    }

    fn read(path: &Path) -> String {
        fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
    }

    /// The old layout as a non-portable 0.2 left it, and the new one, both
    /// under `tmp`.
    fn layouts(tmp: &Path) -> (Layout, Layout) {
        let legacy = tmp.join("roaming/exospine");
        let old = Layout {
            preferences: legacy.clone(),
            rules: legacy.join("rules.json"),
            data: legacy.clone(),
            security_log: legacy.join("security.log"),
            webview: Some(tmp.join("local/com.mothersphere.exospine")),
        };
        let new = Layout::colony(&super::super::Roots {
            config: tmp.join("config/Colony/Exospine"),
            data: tmp.join("data/Colony/Exospine"),
            cache: tmp.join("cache/Colony/Exospine"),
        });
        (old, new)
    }

    fn populate(old: &Layout) {
        write(&old.preferences.join("config.toml"), "theme = \"dark\"");
        write(&old.preferences.join("accounts.json"), "[]");
        write(&old.preferences.join("accounts.json.unreadable-1"), "kept");
        write(&old.preferences.join("encryption.salt"), "salt");
        write(&old.data.join("mail.db"), "db");
        write(&old.data.join("mail.db-wal"), "wal");
        write(&old.data.join("window_state.json"), "{}");
        write(&old.rules, "[{}]");
        write(&old.security_log, "[t] OAUTH_LOGIN | a");
        write(&old.webview.as_ref().unwrap().join("localstorage/app.localstorage"), "pin");
    }

    #[test]
    fn a_fresh_install_uses_the_new_layout() {
        let tmp = TempDir::new();
        let (old, new) = layouts(&tmp.0);
        let marker = new.data.join(MARKER);
        assert_eq!(run(&new, &old, &marker), new);
        assert!(marker.exists());
    }

    #[test]
    fn old_files_are_copied_and_kept() {
        let tmp = TempDir::new();
        let (old, new) = layouts(&tmp.0);
        populate(&old);
        let marker = new.data.join(MARKER);

        assert_eq!(run(&new, &old, &marker), new);

        assert_eq!(read(&new.preferences.join("config.toml")), "theme = \"dark\"");
        assert_eq!(read(&new.preferences.join("accounts.json")), "[]");
        assert_eq!(read(&new.preferences.join("accounts.json.unreadable-1")), "kept");
        assert_eq!(read(&new.preferences.join("encryption.salt")), "salt");
        assert_eq!(read(&new.data.join("mail.db")), "db");
        assert_eq!(read(&new.data.join("mail.db-wal")), "wal");
        assert!(!new.data.join("mail.db-shm").exists());
        assert_eq!(read(&new.data.join("window_state.json")), "{}");
        assert_eq!(read(&new.rules), "[{}]");
        assert_eq!(read(&new.security_log), "[t] OAUTH_LOGIN | a");
        let webview = new.webview.as_ref().unwrap();
        assert_eq!(read(&webview.join("localstorage/app.localstorage")), "pin");
        assert!(marker.exists());
        // The originals stay for this release.
        assert_eq!(read(&old.data.join("mail.db")), "db");
        assert!(old.webview.as_ref().unwrap().join("localstorage").is_dir());
    }

    #[test]
    fn a_failed_copy_keeps_the_old_layout_and_retries_later() {
        let tmp = TempDir::new();
        let (old, new) = layouts(&tmp.0);
        populate(&old);
        let marker = new.data.join(MARKER);
        // A file where the data root should go: the first copy into it
        // fails, after the preferences are already across.
        write(&new.data, "in the way");

        assert_eq!(run(&new, &old, &marker), old);
        assert!(!marker.exists());
        assert!(!new.preferences.join("config.toml").exists(), "this run's copies are undone");
        assert_eq!(read(&old.preferences.join("config.toml")), "theme = \"dark\"");

        // Next start: the obstacle is gone and everything copies.
        fs::remove_file(&new.data).unwrap();
        assert_eq!(run(&new, &old, &marker), new);
        assert_eq!(read(&new.data.join("mail.db")), "db");
    }

    #[test]
    fn existing_new_files_are_never_overwritten() {
        let tmp = TempDir::new();
        let (old, new) = layouts(&tmp.0);
        populate(&old);
        write(&new.preferences.join("config.toml"), "newer");

        run(&new, &old, &new.data.join(MARKER));

        assert_eq!(read(&new.preferences.join("config.toml")), "newer");
    }

    #[test]
    fn the_marker_skips_the_copy() {
        let tmp = TempDir::new();
        let (old, new) = layouts(&tmp.0);
        populate(&old);
        let marker = new.data.join(MARKER);
        write(&marker, "1");

        assert_eq!(run(&new, &old, &marker), new);
        assert!(!new.preferences.join("config.toml").exists());
    }

    #[test]
    fn portable_files_move_into_the_portable_roots() {
        // 0.2 portable mode kept its files in data/, which now holds the
        // three roots: the copies land inside the old directory.
        let tmp = TempDir::new();
        let base = tmp.0.join("data");
        let (mut old, _) = layouts(&tmp.0);
        old.preferences = base.clone();
        old.data = base.clone();
        populate(&old);
        let new = Layout::colony(&super::super::Roots::portable(&base));

        assert_eq!(run(&new, &old, &new.data.join(MARKER)), new);
        assert_eq!(read(&base.join("config/preferences/accounts.json")), "[]");
        assert_eq!(read(&base.join("data/mail.db")), "db");
        assert_eq!(read(&base.join("accounts.json")), "[]");
    }

    #[test]
    fn staging_leaves_nothing_behind() {
        let tmp = TempDir::new();
        let from = tmp.0.join("tree");
        write(&from.join("inner/deep.txt"), "line");
        let to = tmp.0.join("out/tree");

        copy_into_place(&from, &to).unwrap();

        assert_eq!(read(&to.join("inner/deep.txt")), "line");
        let names: Vec<_> =
            fs::read_dir(tmp.0.join("out")).unwrap().flatten().map(|e| e.file_name()).collect();
        assert_eq!(names, ["tree"]);
    }
}
