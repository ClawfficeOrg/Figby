//! Bounded file reads.
//!
//! Every untrusted-input read goes through these helpers so a huge file
//! (or sparse/zip-bomb-style amplification) is rejected before memory is
//! committed, rather than being read fully and amplified later (GPT
//! review F-21).

use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

/// Cap for font files (plain .flf/.tlf or ZIP archives).
pub const MAX_FONT_BYTES: u64 = 32 * 1024 * 1024;
/// Cap for text inputs: templates, themes, configs, palettes, recent files.
pub const MAX_TEXT_BYTES: u64 = 8 * 1024 * 1024;

/// Read at most `max` bytes from `path`.
///
/// Errors with `InvalidData` when the file is larger than the cap — the
/// size check uses metadata first (cheap rejection) and then enforces
/// again on the actual byte count via `take(max + 1)`, so a lying/sparse
/// file cannot slip through.
pub fn read_bounded(path: &Path, max: u64) -> io::Result<Vec<u8>> {
    let mut file = File::open(path)?;
    if let Ok(meta) = file.metadata() {
        if meta.len() > max {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "file {} is {} bytes, exceeding the {}-byte input limit",
                    path.display(),
                    meta.len(),
                    max
                ),
            ));
        }
    }
    let mut buf = Vec::new();
    (&mut file).take(max + 1).read_to_end(&mut buf)?;
    if buf.len() as u64 > max {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "file {} exceeds the {}-byte input limit",
                path.display(),
                max
            ),
        ));
    }
    Ok(buf)
}

/// [`read_bounded`] plus UTF-8 validation for text formats.
pub fn read_bounded_string(path: &Path, max: u64) -> io::Result<String> {
    let bytes = read_bounded(path, max)?;
    String::from_utf8(bytes).map_err(|e| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("file {} is not valid UTF-8: {}", path.display(), e),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    fn temp_path(label: &str) -> PathBuf {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "figby_iolim_{}_{}_{}",
            std::process::id(),
            n,
            label
        ))
    }

    #[test]
    fn reads_within_limit() {
        let p = temp_path("ok");
        std::fs::write(&p, vec![b'a'; 1024]).unwrap();
        assert_eq!(read_bounded(&p, 2048).unwrap().len(), 1024);
        std::fs::remove_file(&p).ok();
    }

    #[test]
    fn rejects_oversized_by_metadata() {
        let p = temp_path("big");
        std::fs::write(&p, vec![b'a'; 4096]).unwrap();
        let err = read_bounded(&p, 1024).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
        std::fs::remove_file(&p).ok();
    }

    #[test]
    fn rejects_string_over_limit_and_invalid_utf8() {
        let p = temp_path("txt");
        std::fs::write(&p, "hello".repeat(100)).unwrap();
        assert!(read_bounded_string(&p, 10).is_err());

        let p2 = temp_path("bin");
        std::fs::write(&p2, [0xFF, 0xFE]).unwrap();
        let err = read_bounded_string(&p2, 1024).unwrap_err();
        assert!(err.to_string().contains("UTF-8"));
        std::fs::remove_file(&p).ok();
        std::fs::remove_file(&p2).ok();
    }
}
