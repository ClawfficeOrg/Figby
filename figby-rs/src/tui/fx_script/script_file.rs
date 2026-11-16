//! The filesystem side of effect scripts: read a `.figfx.toml` and notice
//! when it changes, for hot reload. Polls the file's modification time and
//! length (no watcher dependency); the event loop calls [`FxScriptFile::poll`]
//! a few times a second.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

/// How often the event loop checks the file.
pub const POLL_INTERVAL: Duration = Duration::from_millis(500);

/// What identifies one version of the file on disk. `None` while the file
/// is missing.
pub type FileStamp = Option<(SystemTime, u64)>;

fn stamp_of(path: &Path) -> FileStamp {
    let meta = std::fs::metadata(path).ok()?;
    Some((meta.modified().ok()?, meta.len()))
}

/// A script file being watched.
#[derive(Debug, Clone)]
pub struct FxScriptFile {
    pub path: PathBuf,
    stamp: FileStamp,
    last_poll: Option<Instant>,
}

impl FxScriptFile {
    /// Read `path` and start watching it from this version.
    pub fn open(path: &Path) -> std::io::Result<(Self, String)> {
        let stamp = stamp_of(path);
        let text = std::fs::read_to_string(path)?;
        Ok((
            Self {
                path: path.to_path_buf(),
                stamp,
                last_poll: None,
            },
            text,
        ))
    }

    /// Whether the file changed since it was read or last reported. A
    /// change is reported once; a file that disappears is not a change
    /// (editors often delete and rewrite), its reappearance is.
    pub fn changed(&mut self) -> bool {
        let now = stamp_of(&self.path);
        if now.is_none() || now == self.stamp {
            return false;
        }
        self.stamp = now;
        true
    }

    /// Like [`Self::changed`], but at most once per [`POLL_INTERVAL`]; on a
    /// change, returns the new text. A read error counts as no change and
    /// is retried at the next poll.
    pub fn poll(&mut self, now: Instant) -> Option<String> {
        if self
            .last_poll
            .is_some_and(|t| now.saturating_duration_since(t) < POLL_INTERVAL)
        {
            return None;
        }
        self.last_poll = Some(now);
        let before = self.stamp;
        if !self.changed() {
            return None;
        }
        match std::fs::read_to_string(&self.path) {
            Ok(text) => Some(text),
            Err(_) => {
                self.stamp = before;
                None
            }
        }
    }

    /// File name for the status bar and the Effects tab.
    pub fn display_name(&self) -> String {
        self.path.file_name().map_or_else(
            || self.path.display().to_string(),
            |n| n.to_string_lossy().into_owned(),
        )
    }
}

/// The per-user default script, loaded at startup when it exists:
/// `<config dir>/effects.figfx.toml`.
pub fn user_script_path() -> Option<PathBuf> {
    crate::config::config_dir().map(|d| d.join("effects.figfx.toml"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_file(name: &str, text: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("figby-fx-watch-{}-{name}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("watch.figfx.toml");
        std::fs::write(&path, text).unwrap();
        path
    }

    /// Give the file a distinct mtime: some filesystems have coarse
    /// timestamps, and tests write faster than they tick.
    fn bump_mtime(path: &Path, secs: u64) {
        let f = std::fs::OpenOptions::new().write(true).open(path).unwrap();
        f.set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000 + secs))
            .unwrap();
    }

    #[test]
    fn unchanged_file_is_not_reported() {
        let path = temp_file("same", "a");
        let (mut f, text) = FxScriptFile::open(&path).unwrap();
        assert_eq!(text, "a");
        assert!(!f.changed());
        assert!(!f.changed());
    }

    #[test]
    fn edit_is_reported_once() {
        let path = temp_file("edit", "a");
        let (mut f, _) = FxScriptFile::open(&path).unwrap();
        std::fs::write(&path, "bb").unwrap();
        bump_mtime(&path, 1);
        assert!(f.changed());
        assert!(!f.changed(), "a change is reported once");
        // Same length, new mtime: still a change.
        std::fs::write(&path, "cc").unwrap();
        bump_mtime(&path, 2);
        assert!(f.changed());
    }

    #[test]
    fn missing_file_is_not_a_change_but_its_return_is() {
        let path = temp_file("gone", "a");
        let (mut f, _) = FxScriptFile::open(&path).unwrap();
        std::fs::remove_file(&path).unwrap();
        assert!(!f.changed());
        std::fs::write(&path, "back").unwrap();
        bump_mtime(&path, 3);
        assert!(f.changed());
    }

    #[test]
    fn poll_is_throttled_and_returns_the_new_text() {
        let path = temp_file("poll", "a");
        let (mut f, _) = FxScriptFile::open(&path).unwrap();
        let t0 = Instant::now();
        assert_eq!(f.poll(t0), None);
        std::fs::write(&path, "new text").unwrap();
        bump_mtime(&path, 4);
        assert_eq!(f.poll(t0 + Duration::from_millis(10)), None, "throttled");
        assert_eq!(f.poll(t0 + POLL_INTERVAL).as_deref(), Some("new text"));
        assert_eq!(f.poll(t0 + POLL_INTERVAL * 2), None);
        assert_eq!(f.display_name(), "watch.figfx.toml");
    }
}
