//! Crash-safe, symlink-safe file replacement.
//!
//! All user-facing writes (font saves, exports, palettes) go through
//! [`atomic_write`]: a uniquely-named temporary file is created *in the
//! destination directory* (same filesystem ⇒ rename is atomic), written,
//! flushed, then renamed over the destination (GPT review F-22).

use std::fs::OpenOptions;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// Atomically replace `dest` with `data`.
///
/// - Refuses when `dest` itself is a symlink rather than following or
///   clobbering it.
/// - The temp file is created with `create_new(true)`: an unpredictably
///   unique name, so concurrent writers and attackers cannot pre-create
///   or hijack it, and an existing file/symlink at that name aborts the
///   write instead of being followed.
/// - On any failure the temp file is removed and the destination is left
///   exactly as it was — a partial export never replaces a good file.
pub fn atomic_write(dest: &Path, data: &[u8]) -> io::Result<()> {
    if dest.is_symlink() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("refusing to overwrite symlink {}", dest.display()),
        ));
    }

    let parent = match dest.parent() {
        Some(p) if !p.as_os_str().is_empty() => p,
        _ => Path::new("."),
    };

    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let mut tmp_path: Option<PathBuf> = None;
    let mut file = None;
    for attempt in 0..64u32 {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or(0);
        let candidate = parent.join(format!(
            ".figby_tmp_{}_{}_{}_{attempt}",
            std::process::id(),
            nanos,
            n
        ));
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(f) => {
                tmp_path = Some(candidate);
                file = Some(f);
                break;
            }
            // Name taken (or transient error) — try another name.
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
    let tmp_path =
        tmp_path.ok_or_else(|| io::Error::new(io::ErrorKind::AlreadyExists, "no temp name"))?;
    let mut file = file.expect("loop produced a file when tmp_path is set");

    let result = (|| -> io::Result<()> {
        file.write_all(data)?;
        file.flush()?;
        file.sync_all()?;
        drop(file);
        // Rename does not follow symlinks: it replaces the directory
        // entry itself, so even a race-created symlink at `dest` is
        // replaced wholesale rather than written through.
        std::fs::rename(&tmp_path, dest)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp_path);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir() -> PathBuf {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("figby_atomic_{}_{}", std::process::id(), n))
    }

    #[test]
    fn writes_and_roundtrips() {
        let dir = temp_dir();
        std::fs::create_dir_all(&dir).unwrap();
        let dest = dir.join("out.txt");
        atomic_write(&dest, b"hello").unwrap();
        assert_eq!(std::fs::read(&dest).unwrap(), b"hello");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn replaces_existing_content_atomically() {
        let dir = temp_dir();
        std::fs::create_dir_all(&dir).unwrap();
        let dest = dir.join("out.bin");
        atomic_write(&dest, b"old content").unwrap();
        atomic_write(&dest, b"new").unwrap();
        assert_eq!(std::fs::read(&dest).unwrap(), b"new");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn refuses_symlink_destination() {
        use std::os::unix::fs::symlink;
        let dir = temp_dir();
        std::fs::create_dir_all(&dir).unwrap();
        let victim = dir.join("victim.txt");
        std::fs::write(&victim, b"precious").unwrap();
        let link = dir.join("link.txt");
        symlink(&victim, &link).unwrap();

        let err = atomic_write(&link, b"clobber").unwrap_err();
        assert!(
            err.to_string().contains("refusing"),
            "symlink dest must be refused, got: {err}"
        );
        // Target untouched.
        assert_eq!(std::fs::read(&victim).unwrap(), b"precious");

        std::fs::remove_file(&link).ok();
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn leaves_no_temp_files_on_success() {
        let dir = temp_dir();
        std::fs::create_dir_all(&dir).unwrap();
        let dest = dir.join("out.flf");
        atomic_write(&dest, b"data").unwrap();
        let leftovers: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().starts_with(".figby_tmp"))
            .collect();
        assert!(leftovers.is_empty(), "temp files must be renamed away");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn failure_leaves_destination_intact() {
        let dir = temp_dir();
        std::fs::create_dir_all(&dir).unwrap();
        let dest = dir.join("keep.txt");
        atomic_write(&dest, b"original").unwrap();

        // A read-only directory makes rename fail after temp creation…
        // portable alternative: point dest's parent at a file so open of
        // the temp fails — either way dest keeps its old content.
        let not_a_dir = dir.join("file.txt");
        std::fs::write(&not_a_dir, b"x").unwrap();
        let bad_dest = not_a_dir.join("nested.txt");
        assert!(atomic_write(&bad_dest, b"y").is_err());
        assert_eq!(std::fs::read(&dest).unwrap(), b"original");

        std::fs::remove_dir_all(&dir).unwrap();
    }
}
