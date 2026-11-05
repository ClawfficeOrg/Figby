//! `FIGBY_ICONS=ascii` (review U3): without a Nerd Font the default icon
//! set renders as tofu, so the ASCII set must leave no private-use or emoji
//! glyphs on screen. A separate test binary because it sets a
//! process-wide environment variable that the other TUI tests must not see.

use figby::tui::{IconSet, TuiApp};
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::KeyCode;
use ratatui::Terminal;

/// Nerd Font glyphs live in the Private Use Areas; emoji from U+1F000 up.
fn needs_special_font(c: char) -> bool {
    let c = c as u32;
    (0xE000..=0xF8FF).contains(&c) || c >= 0xF0000 || (0x1F000..0xF0000).contains(&c)
}

/// Screens worth checking: welcome, then each editor mode.
fn rendered_screens() -> Vec<String> {
    let mut app = TuiApp::new();
    let mut terminal = Terminal::new(TestBackend::new(160, 48)).unwrap();
    let mut screens = Vec::new();
    let mut grab = |app: &mut TuiApp| {
        terminal.draw(|f| app.render(f)).unwrap();
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect::<String>()
    };
    screens.push(grab(&mut app));
    app.welcome.screen.show = false;
    for _ in 0..3 {
        screens.push(grab(&mut app));
        app.handle_key_event(KeyCode::Tab);
    }
    screens
}

#[test]
fn test_ascii_icon_set_needs_no_special_font() {
    // Control: the default set really does draw Nerd Font glyphs, so the
    // check below can't pass by rendering nothing.
    std::env::remove_var("FIGBY_ICONS");
    let nerd: String = rendered_screens().concat();
    assert!(
        nerd.chars().any(needs_special_font),
        "default icon set should use Nerd Font glyphs"
    );

    std::env::set_var("FIGBY_ICONS", "ascii");
    for (i, screen) in rendered_screens().iter().enumerate() {
        let bad: String = screen.chars().filter(|&c| needs_special_font(c)).collect();
        assert!(
            bad.is_empty(),
            "screen {i} still draws font-dependent glyphs with FIGBY_ICONS=ascii: {bad:?}"
        );
    }
    std::env::remove_var("FIGBY_ICONS");
}

#[test]
fn test_ascii_icon_values_are_ascii() {
    for (key, value) in IconSet::Ascii.icons() {
        assert!(
            !value.is_empty() && value.is_ascii(),
            "icons-ascii.yaml {key} = {value:?} must be non-empty ASCII"
        );
    }
}

#[test]
fn test_nerd_icon_values_are_glyphs_not_names() {
    // `tool_flood_fill: nf-fa-paint_bucket` was a literal icon name.
    for (key, value) in IconSet::Nerd.icons() {
        assert!(
            !value.is_ascii(),
            "icons.yaml {key} = {value:?} looks like an icon name, not a glyph"
        );
    }
}
