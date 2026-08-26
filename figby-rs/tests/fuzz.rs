// Property-based fuzz testing for FIGfont parser.
//
// Each test generates random (often malformed) inputs and asserts
// the parser never panics — only returns Ok or Err.

use figby::font::{parse_char_data, parse_codetagged, parse_header, parse_tlf_font, FIGfont};
use proptest::prelude::*;

proptest! {
    #[test]
    fn fuzz_parse_header(s in any::<String>()) {
        let _ = parse_header(&s);
    }

    #[test]
    fn fuzz_parse_tlf_font(s in any::<String>()) {
        let _ = parse_tlf_font(&s);
    }

    #[test]
    fn fuzz_parse_char_data(
        lines in prop::collection::vec(any::<String>(), 0..200),
        height in 1..20u32,
    ) {
        let mut font = FIGfont {
            charheight: height,
            ..FIGfont::default()
        };
        let _ = parse_char_data(&mut font, &lines);
    }

    #[test]
    fn fuzz_parse_codetagged(
        lines in prop::collection::vec(any::<String>(), 0..100),
        height in 1..10u32,
    ) {
        let mut font = FIGfont {
            charheight: height,
            ..FIGfont::default()
        };
        let _ = parse_codetagged(&mut font, &lines);
    }
}

// F-17 (GPT review): palette parsing must never panic on arbitrary input —
// multibyte/odd-shaped hex strings previously panicked on byte-offset
// slicing. Every format is fuzzed for no-panic below.
proptest! {
    #[test]
    fn fuzz_parse_hex_rgb_arbitrary_string(s in any::<String>()) {
        let _ = figby::tui::theme::parse_hex_rgb(&s);
    }

    #[test]
    fn fuzz_paletty_json_arbitrary_bytes(s in any::<Vec<u8>>()) {
        let _ = figby::palette_import::import_swatches(
            &s,
            figby::palette_import::ImportFormat::PalettyJson,
        );
    }

    #[test]
    fn fuzz_wezterm_json_arbitrary_bytes(s in any::<Vec<u8>>()) {
        let _ = figby::palette_import::import_swatches(
            &s,
            figby::palette_import::ImportFormat::WezTermJson,
        );
    }

    #[test]
    fn fuzz_windows_terminal_json_arbitrary_bytes(s in any::<Vec<u8>>()) {
        let _ = figby::palette_import::import_swatches(
            &s,
            figby::palette_import::ImportFormat::WindowsTerminalJson,
        );
    }

    #[test]
    fn fuzz_ase_arbitrary_bytes(buf in prop::collection::vec(any::<u8>(), 0..1024)) {
        let _ = figby::palette_import::import_swatches(
            &buf,
            figby::palette_import::ImportFormat::AdobeAse,
        );
    }
}
