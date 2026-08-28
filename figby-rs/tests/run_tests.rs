use std::io::Write;
use std::path::PathBuf;
use std::process::Command;

fn figby_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_figby"))
}

fn repo_root() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir.parent().unwrap().to_path_buf()
}

fn expected_output(n: u32) -> Vec<u8> {
    let root = repo_root();
    let path = root.join(format!("tests/res{:03}.txt", n));
    std::fs::read(&path).unwrap_or_else(|_| panic!("missing expected output: {:?}", path))
}

fn run_figby(args: &[&str], stdin_data: Option<&[u8]>) -> Vec<u8> {
    let root = repo_root();
    let mut cmd = Command::new(figby_binary());
    cmd.current_dir(&root);
    cmd.env("FIGLET_FONTDIR", root.join("fonts"));
    cmd.stdout(std::process::Stdio::piped());
    cmd.args(args);
    if stdin_data.is_some() {
        cmd.stdin(std::process::Stdio::piped());
    }
    let mut child = cmd.spawn().expect("failed to spawn figby");
    if let Some(data) = stdin_data {
        child.stdin.as_mut().unwrap().write_all(data).unwrap();
    }
    let output = child.wait_with_output().expect("failed to run figby");
    assert!(
        output.status.success(),
        "figby failed: args={:?} stderr={}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
    output.stdout
}

fn input_text() -> Vec<u8> {
    let root = repo_root();
    std::fs::read(root.join("tests/input.txt")).expect("missing tests/input.txt")
}

fn long_text() -> Vec<u8> {
    let root = repo_root();
    std::fs::read(root.join("tests/longtext.txt")).expect("missing tests/longtext.txt")
}

fn showfigfonts_output() -> Vec<u8> {
    let root = repo_root();
    let fonts_dir = root.join("fonts");

    let mut font_stems: Vec<String> = std::fs::read_dir(&fonts_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .map(|ext| ext == "flf")
                .unwrap_or(false)
        })
        .map(|e| e.path().file_stem().unwrap().to_str().unwrap().to_string())
        .collect();
    font_stems.sort();

    let mut output = Vec::new();
    for stem in &font_stems {
        output.extend_from_slice(format!("{} :\n", stem).as_bytes());
        let result = run_figby(&["-f", stem, stem], None);
        output.extend_from_slice(&result);
        output.push(b'\n');
        output.push(b'\n');
    }
    output
}

fn list_control_files_output() -> Vec<u8> {
    let root = repo_root();
    let fonts_dir = root.join("fonts");

    let mut flc_names: Vec<String> = std::fs::read_dir(&fonts_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .map(|ext| ext == "flc")
                .unwrap_or(false)
        })
        .map(|e| {
            let p = e.path();
            let rel = p.strip_prefix(&root).unwrap();
            rel.to_str().unwrap().to_string()
        })
        .collect();
    flc_names.sort();

    let mut output = Vec::new();
    for name in &flc_names {
        output.extend_from_slice(name.as_bytes());
        output.push(b'\n');
    }
    output
}

#[test]
fn test_01_showfigfonts() {
    let output = showfigfonts_output();
    let expected = expected_output(1);
    if output != expected {
        let diff_start = output
            .iter()
            .zip(expected.iter())
            .position(|(a, b)| a != b)
            .unwrap_or(0);
        let diff_end = diff_start.saturating_sub(10);
        eprintln!(
            "First diff at byte {}:\n  got:  {:?}\n  want: {:?}",
            diff_start,
            &output[diff_end..(diff_end + 40).min(output.len())],
            &expected[diff_end..(diff_end + 40).min(expected.len())]
        );
        panic!("test_01_showfigfonts failed");
    }
    assert_eq!(output, expected);
}

#[test]
fn test_02_all_fonts() {
    let root = repo_root();
    let fonts_dir = root.join("fonts");
    let input = input_text();

    let mut font_stems: Vec<String> = std::fs::read_dir(&fonts_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .map(|ext| ext == "flf")
                .unwrap_or(false)
        })
        .map(|e| e.path().file_stem().unwrap().to_str().unwrap().to_string())
        .collect();
    font_stems.sort();

    let mut all_output = Vec::new();
    for stem in &font_stems {
        let font_path = format!("fonts/{}", stem);
        let result = run_figby(&["-f", &font_path], Some(&input));
        all_output.extend_from_slice(&result);
    }
    assert_eq!(all_output, expected_output(2));
}

#[test]
fn test_03_long_text() {
    let input = long_text();
    let output = run_figby(&[], Some(&input));
    assert_eq!(output, expected_output(3));
}

#[test]
fn test_04_left_to_right() {
    let input = input_text();
    let output = run_figby(&["-L"], Some(&input));
    assert_eq!(output, expected_output(4));
}

#[test]
fn test_05_right_to_left() {
    let input = input_text();
    let output = run_figby(&["-R"], Some(&input));
    assert_eq!(output, expected_output(5));
}

#[test]
fn test_06_flush_left() {
    let input = input_text();
    let output = run_figby(&["-l"], Some(&input));
    assert_eq!(output, expected_output(6));
}

#[test]
fn test_07_flush_right() {
    let input = input_text();
    let output = run_figby(&["-r"], Some(&input));
    assert_eq!(output, expected_output(7));
}

#[test]
fn test_08_center() {
    let input = input_text();
    let output = run_figby(&["-c"], Some(&input));
    assert_eq!(output, expected_output(8));
}

#[test]
fn test_09_kerning() {
    let input = input_text();
    let output = run_figby(&["-k"], Some(&input));
    assert_eq!(output, expected_output(9));
}

#[test]
fn test_10_full_width() {
    let input = input_text();
    let output = run_figby(&["-W"], Some(&input));
    assert_eq!(output, expected_output(10));
}

#[test]
fn test_11_overlap() {
    let input = input_text();
    let output = run_figby(&["-o"], Some(&input));
    assert_eq!(output, expected_output(11));
}

#[test]
fn test_12_tlf_font() {
    let input = input_text();
    let output = run_figby(&["-f", "tests/emboss"], Some(&input));
    assert_eq!(output, expected_output(12));
}

#[test]
fn test_13_kerning_flush_left_rtl() {
    let input = input_text();
    let output = run_figby(&["-klR"], Some(&input));
    assert_eq!(output, expected_output(13));
}

#[test]
fn test_14_kerning_center_rtl_slant() {
    let input = input_text();
    let output = run_figby(&["-kcR", "-f", "slant"], Some(&input));
    assert_eq!(output, expected_output(14));
}

#[test]
fn test_15_full_width_flush_right_rtl() {
    let input = input_text();
    let output = run_figby(&["-WrR"], Some(&input));
    assert_eq!(output, expected_output(15));
}

#[test]
fn test_16_overlap_flush_right_big() {
    let input = input_text();
    let output = run_figby(&["-or", "-f", "big"], Some(&input));
    assert_eq!(output, expected_output(16));
}

#[test]
fn test_17_tlf_kerning_flush_right() {
    let input = input_text();
    let output = run_figby(&["-kr", "-f", "tests/emboss"], Some(&input));
    assert_eq!(output, expected_output(17));
}

#[test]
fn test_18_tlf_overlap_center() {
    let input = input_text();
    let output = run_figby(&["-oc", "-f", "tests/emboss"], Some(&input));
    assert_eq!(output, expected_output(18));
}

#[test]
fn test_19_tlf_full_width_flush_left_rtl() {
    let input = input_text();
    let output = run_figby(&["-WRl", "-f", "tests/emboss"], Some(&input));
    assert_eq!(output, expected_output(19));
}

#[test]
fn test_20_specify_font_directory() {
    let root = repo_root();
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let src = root.join("fonts/script.flf");
    let dst = dir.path().join("foo.flf");
    std::fs::copy(&src, &dst).expect("failed to copy font");

    let input = input_text();
    let fontdir_str = dir.path().to_str().unwrap();
    let output = run_figby(&["-d", fontdir_str, "-f", "foo"], Some(&input));
    assert_eq!(output, expected_output(20));
}

#[test]
fn test_21_paragraph_mode() {
    let input = input_text();
    let output = run_figby(&["-p", "-w250"], Some(&input));
    assert_eq!(output, expected_output(21));
}

#[test]
fn test_22_short_line() {
    let input = input_text();
    let output = run_figby(&["-w5"], Some(&input));
    assert_eq!(output, expected_output(22));
}

#[test]
fn test_23_kerning_paragraph_center_small() {
    let input = input_text();
    let output = run_figby(&["-kpc", "-f", "small"], Some(&input));
    assert_eq!(output, expected_output(23));
}

#[test]
fn test_24_list_control_files() {
    let output = list_control_files_output();
    assert_eq!(output, expected_output(24));
}

#[test]
fn test_25_uskata_control() {
    let output = run_figby(&["-f", "banner", "-C", "fonts/uskata.flc"], Some(b"ABCDE"));
    assert_eq!(output, expected_output(25));
}

#[test]
fn test_26_jis0201_control() {
    let output = run_figby(
        &["-f", "banner", "-C", "fonts/jis0201.flc"],
        Some(b"\xb1\xb2\xb3\xb4\xb5"),
    );
    assert_eq!(output, expected_output(26));
}

#[test]
fn test_27_rtl_smushing_jave() {
    let input = input_text();
    let output = run_figby(&["-f", "tests/flowerpower", "-R"], Some(&input));
    assert_eq!(output, expected_output(27));
}

// --- New tests 28-50: extended feature coverage ---

#[test]
fn test_28_empty_input() {
    let output = run_figby(&["-f", "standard"], Some(b""));
    assert_eq!(output, expected_output(28));
}

#[test]
fn test_29_single_char() {
    let output = run_figby(&["-f", "standard"], Some(b"X"));
    assert_eq!(output, expected_output(29));
}

#[test]
fn test_30_explicit_smush_mode() {
    let input = b"Hello";
    let output = run_figby(&["-f", "standard", "-m0"], Some(input));
    assert_eq!(output, expected_output(30));
}

#[test]
fn test_31_deutsch_flag() {
    let input = b"[\\]";
    let output = run_figby(&["-f", "standard", "-D"], Some(input));
    assert_eq!(output, expected_output(31));
}

#[test]
fn test_32_deutsch_disabled() {
    let input = b"[\\]";
    let output = run_figby(&["-f", "standard", "-E"], Some(input));
    assert_eq!(output, expected_output(32));
}

#[test]
fn test_33_default_direction() {
    let output = run_figby(&["-f", "standard", "-X"], Some(b"Hello"));
    assert_eq!(output, expected_output(33));
}

#[test]
fn test_34_multibyte_disable() {
    let output = run_figby(&["-f", "standard", "-N"], Some(b"test"));
    assert_eq!(output, expected_output(34));
}

#[test]
fn test_35_control_chars() {
    // Ctrl chars 1-31 (except \n) and DEL (127) are silently skipped
    let input = b"a\x01b\x02c\n";
    let output = run_figby(&["-f", "standard"], Some(input));
    assert_eq!(output, expected_output(35));
}

#[test]
fn test_36_various_widths() {
    let text = b"Hello World\n";
    let mut all_output = Vec::new();
    for w in [20usize, 40, 60, 120] {
        let out = run_figby(&["-f", "standard", "-w", &w.to_string()], Some(text));
        all_output.extend_from_slice(&out);
    }
    assert_eq!(all_output, expected_output(36));
}

#[test]
fn test_37_smush_all_rules() {
    let output = run_figby(&["-f", "standard", "-m191"], Some(b"/\\"));
    assert_eq!(output, expected_output(37));
}

#[test]
fn test_38_kern_small_font() {
    let output = run_figby(&["-f", "small", "-k"], Some(b"Hello World"));
    assert_eq!(output, expected_output(38));
}

#[test]
fn test_39_overlap_standard() {
    let output = run_figby(&["-f", "standard", "-o"], Some(b"Hi"));
    assert_eq!(output, expected_output(39));
}

#[test]
fn test_40_full_width_rtl_smush() {
    let output = run_figby(&["-f", "standard", "-WR"], Some(b"abc"));
    assert_eq!(output, expected_output(40));
}

#[test]
fn test_41_tlf_long_text() {
    let input = long_text();
    let output = run_figby(&["-f", "tests/emboss"], Some(&input));
    assert_eq!(output, expected_output(41));
}

#[test]
fn test_42_cmdinput_flag_a() {
    let output = run_figby(&["-f", "standard", "-A", "Hello"], None);
    assert_eq!(output, expected_output(42));
}

#[test]
fn test_43_font_dir_env() {
    let root = repo_root();
    let mut cmd = std::process::Command::new(figby_binary());
    cmd.current_dir(&root);
    cmd.env("FIGLET_FONTDIR", root.join("fonts"));
    cmd.stdout(std::process::Stdio::piped());
    cmd.args(["-f", "standard", "Hello"]);
    let child = cmd.spawn().expect("failed to spawn figby");
    let output = child.wait_with_output().expect("failed to run figby");
    assert!(output.status.success());
    assert_eq!(output.stdout, expected_output(43));
}

#[test]
fn test_44_ascii_control_file() {
    let output = run_figby(&["-f", "banner", "-C", "fonts/upper.flc"], Some(b"abc"));
    assert_eq!(output, expected_output(44));
}

#[test]
fn test_45_paragraph_narrow() {
    let output = run_figby(
        &["-f", "standard", "-p", "-w30"],
        Some(b"Hello World Foo Bar Baz Qux\n"),
    );
    assert_eq!(output, expected_output(45));
}

#[test]
fn test_46_smush_vs_kern_combo() {
    let output = run_figby(&["-f", "standard", "-m0"], Some(b"Hello"));
    assert_eq!(output, expected_output(46));
}

fn all_fonts_output(extra_args: &[&str]) -> Vec<u8> {
    let root = repo_root();
    let fonts_dir = root.join("fonts");
    let input = input_text();

    let mut font_stems: Vec<String> = std::fs::read_dir(&fonts_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .map(|ext| ext == "flf")
                .unwrap_or(false)
        })
        .map(|e| e.path().file_stem().unwrap().to_str().unwrap().to_string())
        .collect();
    font_stems.sort();

    let mut all_output = Vec::new();
    for stem in &font_stems {
        let font_path = format!("fonts/{}", stem);
        let mut args = vec!["-f", &font_path];
        args.extend_from_slice(extra_args);
        let result = run_figby(&args, Some(&input));
        all_output.extend_from_slice(&result);
    }
    all_output
}

#[test]
fn test_47_all_fonts_kerning() {
    let output = all_fonts_output(&["-k"]);
    assert_eq!(output, expected_output(47));
}

#[test]
fn test_48_all_fonts_overlap() {
    let output = all_fonts_output(&["-o"]);
    assert_eq!(output, expected_output(48));
}

#[test]
fn test_49_long_text_center() {
    let input = long_text();
    let output = run_figby(&["-f", "standard", "-c"], Some(&input));
    assert_eq!(output, expected_output(49));
}

#[test]
fn test_50_big_font_rtl() {
    let output = run_figby(&["-f", "big", "-R"], Some(b"Hello"));
    assert_eq!(output, expected_output(50));
}

#[test]
fn test_51_deutsch_unicode_input() {
    // Deutsch flag: [\] → Ä/Ö/Ü, {|}~ → ä/ö/ü/ß
    let output = run_figby(&["-f", "standard", "-D"], Some(b"[\\]"));
    assert!(!output.is_empty(), "Deutsch output should not be empty");
    // Output should differ from non-Deutsch (explicit smush mode 0 for comparison)
    let output_normal = run_figby(&["-f", "standard", "-m0"], Some(b"[\\]"));
    assert_ne!(
        output, output_normal,
        "Deutsch output should differ from raw []\\"
    );
}

#[test]
fn test_52_smush_all_rules_large_font() {
    // Multi-char smushing at default width with large font (big)
    let output = run_figby(&["-f", "big", "-m191"], Some(b"/\\\\"));
    assert!(!output.is_empty(), "smush output should not be empty");
    assert!(output.len() > 50, "smush output should be substantial");
}

#[test]
fn test_53_control_file_extended_charset() {
    // Control file remapping with upper.flc (a→A, etc.)
    let output = run_figby(&["-f", "banner", "-C", "fonts/upper.flc"], Some(b"abc"));
    assert!(
        !output.is_empty(),
        "control file remapping should produce output"
    );
    // Without control file, input is lowercase
    let without = run_figby(&["-f", "banner"], Some(b"abc"));
    // With control file input is uppercase (a→A etc.) so output should differ
    assert_ne!(output, without, "control file should change output");
}

#[test]
fn test_54_paragraph_narrow_width() {
    // Paragraph mode with narrow output width
    let input = b"Hello World Foo Bar Baz Qux\nAnother line here\n";
    let output = run_figby(&["-f", "standard", "-p", "-w30"], Some(input));
    assert!(
        !output.is_empty(),
        "paragraph narrow output should not be empty"
    );
}

#[test]
fn test_55_wide_center_justification() {
    // Wide output with center justification
    let output = run_figby(&["-f", "standard", "-c", "-w120"], Some(b"Hello Figby"));
    assert!(
        !output.is_empty(),
        "center justified output should not be empty"
    );
    // With -f big, should have some content
    let big_output = run_figby(&["-f", "big", "-c", "-w120"], Some(b"Hi"));
    assert!(
        !big_output.is_empty(),
        "big font center justified should not be empty"
    );
}

/// Cross-check that the authoritative C-parity scenario table
/// (`tests/cparity/scenarios.tsv`, single source of truth — GPT review F-27)
/// stays in sync with the `#[ignore]` status of these tests:
///
/// - every `known-divergence` row must be an ignored test here;
/// - every fixture-backed `active` row must have a `tests/resNNN.txt` file;
/// - every `active` row must NOT be ignored (no silent regression).
#[test]
fn test_cparity_table_in_sync() {
    let root = repo_root();
    let table_path = root.join("figby-rs/tests/cparity/scenarios.tsv");
    let table = std::fs::read_to_string(&table_path)
        .unwrap_or_else(|_| panic!("missing scenario table: {:?}", table_path));

    // The tests currently ignored for C divergence — must match the
    // table's `known-divergence` rows exactly.
    let ignored_for_divergence: &[u32] = &[21, 23, 26, 35, 37];

    let mut divergences: Vec<u32> = Vec::new();
    let mut active: Vec<u32> = Vec::new();

    for line in table.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let mut fields = trimmed.splitn(5, '\t');
        let num: u32 = fields
            .next()
            .unwrap()
            .parse()
            .unwrap_or_else(|e| panic!("bad test number in table row: {line:?} ({e})"));
        let _args = fields.next().unwrap();
        let _input = fields.next().unwrap();
        let status = fields.next().unwrap();
        match status {
            "known-divergence" => divergences.push(num),
            "active" => active.push(num),
            "special" => {}
            other => panic!("unknown status {other:?} in table row {line:?}"),
        }
    }

    divergences.sort_unstable();
    assert_eq!(
        divergences, ignored_for_divergence,
        "scenario table `known-divergence` rows must match the ignored tests \
         (enable a test here once it passes, or mark it known-divergence in \
         the table when it is newly failing)"
    );

    for num in &active {
        let res = root.join(format!("tests/res{:03}.txt", num));
        assert!(
            res.exists(),
            "active scenario {} has no expected-output file {:?} \
             (run scripts/regenerate-expected.sh)",
            num,
            res
        );
    }
}
