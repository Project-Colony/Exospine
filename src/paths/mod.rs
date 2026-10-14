//! Where Exospine keeps its files.
//!
//! Every Colony program keeps its files under `Colony/<Program>/` in three
//! platform roots (Project-Colony-Resources, `design/filesystem.md`). Exospine
//! is a Tauri program and does not link `colony-ui`, so [`locate`] reproduces
//! `colony_ui::paths` here, name checks included:
//!
//! | Root | Linux | Windows | macOS |
//! |---|---|---|---|
//! | config | `~/.config/Colony/Exospine/` | `%LOCALAPPDATA%\Colony\Exospine\` | `~/Library/Application Support/Colony/Exospine/` |
//! | data | `~/.local/share/Colony/Exospine/` | `%LOCALAPPDATA%\Colony\Exospine\` | `~/Library/Application Support/Colony/Exospine/` |
//! | cache | `~/.cache/Colony/Exospine/` | `%LOCALAPPDATA%\Colony\Exospine\cache\` | `~/Library/Caches/Colony/Exospine/` |
//!
//! Config and data are the same directory on Windows and macOS, so what the
//! user chose sits in `preferences/` and never next to what Exospine produced:
//!
//! - config: `preferences/config.toml`, `preferences/accounts.json`,
//!   `preferences/rules.json`
//! - data: `mail.db`, `security.log`, `window_state.json`, `webview/` (the
//!   webview profile, on Linux and Windows)
//! - cache: nothing yet
//!
//! In portable mode (a `portable.txt` beside the executable) the three roots
//! are `data/config`, `data/data` and `data/cache` beside the executable.
//!
//! [`layout`] resolves all of this once per run, after carrying the files of
//! older versions over (see [`migrate`]).

mod migrate;

use std::ffi::OsStr;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

pub use migrate::remove_legacy_copies;

/// The program directory under the organisation directory.
pub const PROGRAM: &str = "Exospine";
/// The organisation directory every Colony program nests under.
const VENDOR: &str = "Colony";
/// The shared install root of the Colony launcher, which no program may use.
const APPS: &str = "apps";
/// Keeps the cache apart on Windows, where the cache root is the data root.
const CACHE: &str = "cache";
/// The sub-directory of config that holds what the user chose.
const PREFERENCES: &str = "preferences";
/// A file of this name beside the executable turns portable mode on.
const PORTABLE_MARKER: &str = "portable.txt";

/// Reject anything that is not one safe path component: `..` or a separator
/// would escape the Colony tree, and Windows strips a trailing dot or space,
/// so `Exospine.` would open the same folder as `Exospine`.
fn component(name: &str) -> io::Result<&str> {
    let invalid = name.is_empty()
        || name == "."
        || name == ".."
        || name.ends_with(['.', ' '])
        || name.contains(['/', '\\', '\0'])
        || Path::new(name).components().count() != 1
        || Path::new(name).file_name() != Some(OsStr::new(name));
    if invalid {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{name:?} is not a safe single path component"),
        ));
    }
    Ok(name)
}

/// [`component`], minus the name of the shared install root in any case
/// (Windows and default macOS filesystems ignore case).
fn program_component(name: &str) -> io::Result<&str> {
    let name = component(name)?;
    if name.to_uppercase().eq_ignore_ascii_case(APPS) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{name:?} is reserved for the shared install root"),
        ));
    }
    Ok(name)
}

fn missing(what: &str) -> io::Error {
    io::Error::new(
        io::ErrorKind::NotFound,
        format!("cannot determine the platform {what} directory"),
    )
}

/// The platform roots of a program, as `colony_ui::paths::locate` computes
/// them. Nothing is created.
pub mod locate {
    use super::*;

    pub fn config_dir(program: &str) -> io::Result<PathBuf> {
        Ok(dirs::config_local_dir()
            .ok_or_else(|| missing("config"))?
            .join(VENDOR)
            .join(program_component(program)?))
    }

    pub fn data_dir(program: &str) -> io::Result<PathBuf> {
        Ok(dirs::data_local_dir()
            .ok_or_else(|| missing("data"))?
            .join(VENDOR)
            .join(program_component(program)?))
    }

    pub fn cache_dir(program: &str) -> io::Result<PathBuf> {
        let base = dirs::cache_dir()
            .ok_or_else(|| missing("cache"))?
            .join(VENDOR)
            .join(program_component(program)?);
        // `dirs::cache_dir()` is LocalAppData on Windows, the config and data
        // root itself: without this component, clearing the cache would
        // delete the preferences beside it.
        Ok(if cfg!(windows) { base.join(CACHE) } else { base })
    }
}

/// The three roots of one installation.
#[derive(Clone, Debug, PartialEq)]
pub struct Roots {
    pub config: PathBuf,
    pub data: PathBuf,
    /// Nothing is cached yet. The root is computed all the same, so the table
    /// stays whole and its tests cover the Windows `cache` component.
    #[allow(dead_code)]
    pub cache: PathBuf,
}

impl Roots {
    fn platform() -> io::Result<Self> {
        Ok(Self {
            config: locate::config_dir(PROGRAM)?,
            data: locate::data_dir(PROGRAM)?,
            cache: locate::cache_dir(PROGRAM)?,
        })
    }

    /// The roots of portable mode, under `base` (`data/` beside the
    /// executable).
    fn portable(base: &Path) -> Self {
        Self {
            config: base.join("config"),
            data: base.join("data"),
            cache: base.join("cache"),
        }
    }
}

/// Where each file is read and written this run.
#[derive(Clone, Debug, PartialEq)]
pub struct Layout {
    /// Holds `config.toml` and `accounts.json`.
    pub preferences: PathBuf,
    /// `rules.json`.
    pub rules: PathBuf,
    /// Holds `mail.db` and `window_state.json`.
    pub data: PathBuf,
    /// `security.log`.
    pub security_log: PathBuf,
    /// The webview profile (localStorage). `None` where the webview keeps its
    /// own: WKWebView on macOS has no data directory setting.
    pub webview: Option<PathBuf>,
}

impl Layout {
    /// The Colony layout over `roots`.
    fn colony(roots: &Roots) -> Self {
        let preferences = roots.config.join(PREFERENCES);
        Self {
            rules: preferences.join("rules.json"),
            preferences,
            data: roots.data.clone(),
            security_log: roots.data.join("security.log"),
            webview: (!cfg!(target_os = "macos")).then(|| roots.data.join("webview")),
        }
    }

    /// Where versions up to 0.2 kept the same files: everything in
    /// `exospine/` under the roaming configuration directory, except that
    /// portable mode moved config.toml, accounts.json, mail.db and
    /// window_state.json to `portable` (`data/` beside the executable). The
    /// webview profile was Tauri's default, named after the old app
    /// identifier.
    fn legacy(portable: Option<&Path>) -> Option<Self> {
        // The old location is the roaming one on Windows. Reading it is the
        // point here; nothing new is written there.
        let config = dirs::config_dir()?.join("exospine");
        let data = portable.map_or_else(|| config.clone(), Path::to_path_buf);
        Some(Self {
            preferences: data.clone(),
            rules: config.join("rules.json"),
            data,
            security_log: config.join("security.log"),
            webview: if cfg!(target_os = "macos") {
                None
            } else {
                dirs::data_local_dir().map(|dir| dir.join("com.mothersphere.exospine"))
            },
        })
    }
}

/// `data/` beside the executable when `portable.txt` is there too.
fn portable_base() -> Option<PathBuf> {
    let exe_dir = std::env::current_exe().ok()?.parent()?.to_path_buf();
    exe_dir
        .join(PORTABLE_MARKER)
        .exists()
        .then(|| exe_dir.join("data"))
}

struct Resolved {
    layout: Layout,
    /// The older layout, while a copy of it still sits apart from `layout`.
    legacy: Option<Layout>,
}

static RESOLVED: OnceLock<Resolved> = OnceLock::new();

fn resolved() -> &'static Resolved {
    RESOLVED.get_or_init(|| {
        let portable = portable_base();
        let roots = match &portable {
            Some(base) => Roots::portable(base),
            None => Roots::platform().unwrap_or_else(|e| {
                // No home directory: keep the files beside the executable
                // rather than in the working directory.
                tracing::error!("{e}; keeping files beside the executable");
                let exe = std::env::current_exe().unwrap_or_default();
                Roots::portable(&exe.parent().unwrap_or(Path::new(".")).join("data"))
            }),
        };
        let new = Layout::colony(&roots);
        let Some(old) = Layout::legacy(portable.as_deref()) else {
            return Resolved { layout: new, legacy: None };
        };
        let marker = roots.data.join(migrate::MARKER);
        let layout = migrate::run(&new, &old, &marker);
        let legacy = (layout != old).then_some(old);
        Resolved { layout, legacy }
    })
}

/// The file locations of this run. The first call carries the files of older
/// versions over, so call it at startup before anything reads a file.
pub fn layout() -> &'static Layout {
    &resolved().layout
}

/// The directories that hold an accounts.json this run knows of: the one in
/// use, and the old one while a copy of it remains there.
pub fn account_dirs() -> Vec<PathBuf> {
    let resolved = resolved();
    std::iter::once(&resolved.layout)
        .chain(&resolved.legacy)
        .map(|layout| layout.preferences.clone())
        .collect()
}

/// Give a window the webview profile of [`Layout::webview`], so that every
/// window of the app shares one localStorage.
pub fn with_webview_profile<'a, R: tauri::Runtime, M: tauri::Manager<R>>(
    builder: tauri::WebviewWindowBuilder<'a, R, M>,
) -> tauri::WebviewWindowBuilder<'a, R, M> {
    match &layout().webview {
        Some(dir) => builder.data_directory(dir.clone()),
        None => builder,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsafe_program_names_are_rejected() {
        for bad in [
            "", ".", "..", "../etc", "a/b", "a\\b", "nul\0", "Exospine.", "Exospine ", "apps",
            "Apps", "APPS", "app\u{17F}",
        ] {
            for root in [locate::config_dir, locate::data_dir, locate::cache_dir] {
                assert!(root(bad).is_err(), "{bad:?}");
            }
        }
    }

    #[test]
    fn roots_nest_under_colony_then_the_program() {
        let roots = Roots::platform().unwrap();
        let mut cache = roots.cache.clone();
        if cfg!(windows) {
            assert!(cache.ends_with(CACHE), "{cache:?}");
            cache.pop();
        }
        for path in [&roots.config, &roots.data, &cache] {
            assert!(path.ends_with(Path::new(VENDOR).join(PROGRAM)), "{path:?}");
        }
    }

    /// Clearing the cache must never delete preferences or mail.
    fn assert_cache_apart(roots: &Roots) {
        for kept in [&roots.config, &roots.data] {
            assert_ne!(&roots.cache, kept);
            assert!(!kept.starts_with(&roots.cache), "{kept:?} is inside the cache");
        }
    }

    #[test]
    fn the_cache_never_holds_config_or_data() {
        assert_cache_apart(&Roots::platform().unwrap());
        assert_cache_apart(&Roots::portable(Path::new("portable/data")));
    }

    #[test]
    fn portable_roots_sit_beside_the_executable() {
        let base = Path::new("exe-dir").join("data");
        let roots = Roots::portable(&base);
        assert_eq!(roots.config, base.join("config"));
        assert_eq!(roots.data, base.join("data"));
        assert_eq!(roots.cache, base.join("cache"));
    }

    #[test]
    fn files_stay_apart_when_config_and_data_share_a_root() {
        // Windows and macOS: one directory for both roots.
        let shared = PathBuf::from("LocalAppData").join(VENDOR).join(PROGRAM);
        let layout = Layout::colony(&Roots {
            config: shared.clone(),
            data: shared.clone(),
            cache: shared.join(CACHE),
        });
        let mut files = vec![
            layout.preferences.join("config.toml"),
            layout.preferences.join("accounts.json"),
            layout.rules.clone(),
            layout.data.join("mail.db"),
            layout.data.join("window_state.json"),
            layout.security_log.clone(),
        ];
        files.extend(layout.webview.clone());
        let count = files.len();
        files.sort();
        files.dedup();
        assert_eq!(files.len(), count, "two files share a path");
        let preferences = shared.join(PREFERENCES);
        assert!(layout.rules.starts_with(&preferences));
        for data in [&layout.data.join("mail.db"), &layout.security_log] {
            assert!(!data.starts_with(&preferences), "{data:?}");
        }
    }
}
