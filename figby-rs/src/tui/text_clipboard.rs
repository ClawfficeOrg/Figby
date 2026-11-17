//! The system text clipboard, for TUI text editors (the FX script editor).
//!
//! [`SystemClipboard`] wraps `arboard` and connects on first use. Every call
//! can fail (no display server, headless CI, a locked clipboard); callers
//! then fall back to Figby's in-app clipboard. Unit tests get a
//! [`MemoryClipboard`] from [`default_clipboard`], so `cargo test` never
//! touches the real clipboard.
//!
//! Native-only, like the rest of `tui`.

/// A text clipboard. `None`/`false` means "unavailable, use a fallback".
pub trait TextClipboard {
    fn get_text(&mut self) -> Option<String>;
    fn set_text(&mut self, text: &str) -> bool;
}

/// The OS clipboard via `arboard`, opened lazily and kept open (on X11 the
/// owner must stay alive for pastes to work).
#[derive(Default)]
pub struct SystemClipboard {
    inner: Option<arboard::Clipboard>,
    /// Opening failed once; do not retry every keystroke.
    unavailable: bool,
}

impl SystemClipboard {
    fn handle(&mut self) -> Option<&mut arboard::Clipboard> {
        if self.inner.is_none() && !self.unavailable {
            match arboard::Clipboard::new() {
                Ok(c) => self.inner = Some(c),
                Err(_) => self.unavailable = true,
            }
        }
        self.inner.as_mut()
    }
}

impl TextClipboard for SystemClipboard {
    fn get_text(&mut self) -> Option<String> {
        self.handle()?.get_text().ok()
    }

    fn set_text(&mut self, text: &str) -> bool {
        self.handle()
            .is_some_and(|c| c.set_text(text.to_string()).is_ok())
    }
}

/// An in-memory clipboard: the test stand-in. With `fail` set it behaves
/// like an unavailable system clipboard.
#[derive(Debug, Default, Clone)]
pub struct MemoryClipboard {
    pub text: Option<String>,
    pub fail: bool,
}

impl TextClipboard for MemoryClipboard {
    fn get_text(&mut self) -> Option<String> {
        if self.fail {
            return None;
        }
        self.text.clone()
    }

    fn set_text(&mut self, text: &str) -> bool {
        if self.fail {
            return false;
        }
        self.text = Some(text.to_string());
        true
    }
}

/// The clipboard a new app uses: the system one, or memory under unit
/// tests.
pub fn default_clipboard() -> Box<dyn TextClipboard> {
    #[cfg(test)]
    {
        Box::new(MemoryClipboard::default())
    }
    #[cfg(not(test))]
    {
        Box::new(SystemClipboard::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_clipboard_round_trips_and_can_fail() {
        let mut c = MemoryClipboard::default();
        assert_eq!(c.get_text(), None);
        assert!(c.set_text("中 x"));
        assert_eq!(c.get_text().as_deref(), Some("中 x"));
        c.fail = true;
        assert!(!c.set_text("y"));
        assert_eq!(c.get_text(), None);
    }
}
