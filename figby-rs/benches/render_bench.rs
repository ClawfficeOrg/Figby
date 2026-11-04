use std::collections::HashMap;
use std::sync::OnceLock;

use criterion::{black_box, criterion_group, criterion_main, Criterion};

use figby::font::{parse_tlf_font, FIGcharacter, FIGfont};
use figby::render::{
    add_char, calc_smush_amount, lookup_char, render_line, split_line, Justification, OutputLine,
};
use figby::smush::{smush_horizontal, SmushMode};

const STANDARD_FLF: &[u8] = include_bytes!("../../fonts/standard.flf");

fn standard_font() -> &'static FIGfont {
    static FONT: OnceLock<FIGfont> = OnceLock::new();
    FONT.get_or_init(|| {
        let content = std::str::from_utf8(STANDARD_FLF).expect("valid UTF-8");
        parse_tlf_font(content).expect("standard font parses")
    })
}

fn fixture_font() -> FIGfont {
    let height = 6usize;
    let mut chars = HashMap::new();
    chars.insert(0, FIGcharacter::from(vec!["#".to_string(); height]));

    for code in 32..127u32 {
        let ch = char::from_u32(code).unwrap_or('?');
        let width: usize = match ch {
            'W' | 'w' | 'M' | 'm' => 5,
            'l' | 'i' | 'I' | 'j' | 'J' | 't' | '1' => 2,
            _ => 3,
        };
        let pad = width.saturating_sub(1);
        let left = pad / 2;
        let right = pad - left;
        let row = format!("{}{}{}", " ".repeat(left), ch, " ".repeat(right));
        let rows = vec![row; height];
        chars.insert(code, FIGcharacter::from(rows));
    }

    FIGfont {
        charheight: height as u32,
        hardblank: '$',
        ..FIGfont::default()
    }
}

fn codes(s: &str) -> Vec<u32> {
    s.chars().map(|c| c as u32).collect()
}

fn smush_pairs() -> Vec<(char, char, SmushMode)> {
    let hb = '$';
    vec![
        (' ', 'A', SmushMode::new(SmushMode::SMUSH)),
        ('A', ' ', SmushMode::new(SmushMode::SMUSH)),
        (
            'A',
            'A',
            SmushMode::new(SmushMode::SMUSH | SmushMode::EQUAL_CHARS),
        ),
        (
            '_',
            '/',
            SmushMode::new(SmushMode::SMUSH | SmushMode::UNDERSCORE),
        ),
        (
            '|',
            '/',
            SmushMode::new(SmushMode::SMUSH | SmushMode::HIERARCHY),
        ),
        ('[', ']', SmushMode::new(SmushMode::SMUSH | SmushMode::PAIR)),
        (
            '/',
            '\\',
            SmushMode::new(SmushMode::SMUSH | SmushMode::BIGX),
        ),
        (
            hb,
            hb,
            SmushMode::new(SmushMode::SMUSH | SmushMode::HARDBLANK),
        ),
        ('A', 'B', SmushMode::new(SmushMode::KERN)),
        ('A', 'B', SmushMode::new(SmushMode::SMUSH)),
        ('>', '<', SmushMode::new(SmushMode::SMUSH | SmushMode::BIGX)),
        (
            '\\',
            '/',
            SmushMode::new(SmushMode::SMUSH | SmushMode::BIGX),
        ),
    ]
}

fn bench_font_load(c: &mut Criterion) {
    c.bench_function("font_load", |b| {
        b.iter(|| {
            let content = black_box(std::str::from_utf8(STANDARD_FLF).expect("valid UTF-8"));
            parse_tlf_font(content)
        });
    });
}

fn bench_lookup_char(c: &mut Criterion) {
    let font = standard_font();
    let codes: Vec<u32> = (32..126).cycle().take(1000).collect();
    c.bench_function("lookup_char", |b| {
        b.iter(|| {
            let mut current_width = 0usize;
            for &code in &codes {
                black_box(lookup_char(font, black_box(code), &mut current_width));
            }
        });
    });
}

fn bench_smush_horizontal(c: &mut Criterion) {
    let pairs = smush_pairs();
    c.bench_function("smush_horizontal", |b| {
        b.iter(|| {
            for &(lch, rch, mode) in &pairs {
                for _ in 0..(10000 / pairs.len()) {
                    black_box(smush_horizontal(
                        black_box(lch),
                        black_box(rch),
                        black_box(mode),
                        '$',
                        false,
                    ));
                }
            }
        });
    });
}

fn bench_calc_smush_amount(c: &mut Criterion) {
    let mode = SmushMode::new(SmushMode::SMUSH | SmushMode::EQUAL_CHARS);
    let output = vec![" A ".to_string(); 6];
    let curr = vec![" A ".to_string(); 6];

    c.bench_function("calc_smush_amount", |b| {
        b.iter(|| {
            for _ in 0..1000 {
                black_box(calc_smush_amount(
                    &output,
                    &curr,
                    3,
                    3,
                    3,
                    black_box(mode),
                    '$',
                    false,
                ));
            }
        });
    });
}

fn bench_add_char_kerning(c: &mut Criterion) {
    let font = fixture_font();
    let mode = SmushMode::new(SmushMode::KERN);
    let word = codes("Hi World");

    c.bench_function("add_char_kerning", |b| {
        b.iter(|| {
            for _ in 0..100 {
                let mut rows = vec![String::new(); 6];
                let mut len = 0usize;
                let mut prev = 0usize;
                for &code in &word {
                    black_box(add_char(
                        &font,
                        black_box(code),
                        &mut rows,
                        &mut len,
                        &mut prev,
                        mode,
                        false,
                        200,
                    ));
                }
            }
        });
    });
}

fn bench_add_char_smushing(c: &mut Criterion) {
    let font = fixture_font();
    let mode = SmushMode::new(SmushMode::SMUSH | SmushMode::EQUAL_CHARS | SmushMode::HARDBLANK);
    let word = codes("Hi World");

    c.bench_function("add_char_smushing", |b| {
        b.iter(|| {
            for _ in 0..100 {
                let mut rows = vec![String::new(); 6];
                let mut len = 0usize;
                let mut prev = 0usize;
                for &code in &word {
                    black_box(add_char(
                        &font,
                        black_box(code),
                        &mut rows,
                        &mut len,
                        &mut prev,
                        mode,
                        false,
                        200,
                    ));
                }
            }
        });
    });
}

fn bench_render_line(c: &mut Criterion) {
    let font = standard_font();
    let mode = SmushMode::new(SmushMode::KERN);
    let mut rows = vec![String::new(); font.charheight as usize];
    let mut len = 0;
    let mut prev = 0;
    for &code in &[72, 101, 108, 108, 111] {
        add_char(font, code, &mut rows, &mut len, &mut prev, mode, false, 200);
    }

    let mut group = c.benchmark_group("render_line");
    group.bench_function("left/80", |b| {
        b.iter(|| black_box(render_line(black_box(&rows), '$', Justification::Left, 80)));
    });
    group.bench_function("center/80", |b| {
        b.iter(|| {
            black_box(render_line(
                black_box(&rows),
                '$',
                Justification::Center,
                80,
            ))
        });
    });
    group.bench_function("right/80", |b| {
        b.iter(|| black_box(render_line(black_box(&rows), '$', Justification::Right, 80)));
    });
    group.bench_function("left/40", |b| {
        b.iter(|| black_box(render_line(black_box(&rows), '$', Justification::Left, 40)));
    });
    group.finish();
}

fn bench_split_line(c: &mut Criterion) {
    let font = fixture_font();
    let mode = SmushMode::new(SmushMode::KERN);
    let buffer = codes("A B Hi World More");

    c.bench_function("split_line", |b| {
        b.iter(|| {
            let mut output_rows = vec![String::new(); 6];
            let mut outlinelen = 0usize;
            let mut prev_width = 0usize;
            black_box(split_line(
                &font,
                black_box(&buffer),
                &mut output_rows,
                &mut outlinelen,
                &mut prev_width,
                mode,
                false,
                200,
            ));
        });
    });
}

fn bench_full_pipeline(c: &mut Criterion) {
    let font = standard_font();
    let hb = font.hardblank;
    let height = font.charheight as usize;
    let mode = SmushMode::new(SmushMode::KERN);
    let text = "Lorem ipsum dolor sit amet consectetur adipiscing elit sed do eiusmod tempor incididunt ut labore et dolore magna aliqua Ut enim ad minim veniam quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur Excepteur sint occaecat cupidatat non proident sunt in culpa qui officia deserunt mollit anim id est laborum";
    let big_text: String = std::iter::repeat_n(text, 10).collect::<Vec<_>>().join(" ");

    c.bench_function("full_pipeline", |b| {
        b.iter(|| {
            let mut rows = vec![String::new(); height];
            let mut len = 0usize;
            let mut prev = 0usize;
            let mut char_buffer: Vec<u32> = Vec::new();

            for ch in big_text.chars() {
                let code = ch as u32;

                if !add_char(font, code, &mut rows, &mut len, &mut prev, mode, false, 80) {
                    if let Some((part1, part2_start)) = split_line(
                        font,
                        &char_buffer,
                        &mut rows,
                        &mut len,
                        &mut prev,
                        mode,
                        false,
                        80,
                    ) {
                        black_box(render_line(&part1, hb, Justification::Left, 80));
                        char_buffer.drain(..part2_start);
                    } else {
                        black_box(render_line(&rows, hb, Justification::Left, 80));
                        rows = vec![String::new(); height];
                        len = 0;
                        prev = 0;
                        char_buffer.clear();
                    }

                    add_char(font, code, &mut rows, &mut len, &mut prev, mode, false, 80);
                }

                char_buffer.push(code);
            }

            if len > 0 {
                black_box(render_line(&rows, hb, Justification::Left, 80));
            }
        });
    });
}

/// One very long line (`-w` huge): the case that used to be quadratic.
fn bench_long_line(c: &mut Criterion) {
    let font = standard_font();
    let mode = SmushMode::new(font.full_layout as u32);
    let text: Vec<u32> = "abcdefghij "
        .repeat(1000)
        .chars()
        .map(|c| c as u32)
        .collect();
    for (name, rtl) in [("long_line_ltr_11k", false), ("long_line_rtl_11k", true)] {
        c.bench_function(name, |b| {
            b.iter(|| {
                let mut line = OutputLine::new(font.charheight as usize);
                for &code in &text {
                    line.add_char(font, code, mode, rtl, usize::MAX);
                }
                black_box(line.to_rows())
            });
        });
    }
}

criterion_group!(
    benches,
    bench_font_load,
    bench_lookup_char,
    bench_smush_horizontal,
    bench_calc_smush_amount,
    bench_add_char_kerning,
    bench_add_char_smushing,
    bench_render_line,
    bench_split_line,
    bench_full_pipeline,
    bench_long_line,
);
criterion_main!(benches);
