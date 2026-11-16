//! The Effects side-panel tab: the loaded script's effects with play and
//! stop buttons, then the FX Graph pointer and preset list. Layout and
//! hit-testing share [`effects_layout`], so a click always lands on the
//! button that was drawn.

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use super::presets;
use super::runtime::FxRuntime;
use crate::tui::theme::Theme;

/// Rows above the first effect row: script name, hint, blank.
const HEADER_ROWS: u16 = 3;
/// `[▶]` at column 1, `[■]` at column 5, name from column 9.
const PLAY_X: u16 = 1;
const STOP_X: u16 = 5;
const BUTTON_W: u16 = 3;

/// A click on the Effects tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FxPanelHit {
    Play(usize),
    Stop(usize),
}

/// Screen rows of the effect list: `(effect index, row y)` for each row
/// that fits in `area`.
fn effects_layout(area: Rect, count: usize) -> Vec<(usize, u16)> {
    let first = area.y + HEADER_ROWS;
    (0..count)
        .map(|i| (i, first + i as u16))
        .take_while(|&(_, y)| y < area.y + area.height)
        .collect()
}

/// What a left click at `(x, y)` in the tab's content `area` hits.
pub fn hit(rt: &FxRuntime, area: Rect, x: u16, y: u16) -> Option<FxPanelHit> {
    let host = rt.script.as_ref()?;
    let (idx, _) = effects_layout(area, host.script.effects.len())
        .into_iter()
        .find(|&(_, row)| row == y)?;
    let in_button = |bx: u16| x >= area.x + bx && x < area.x + bx + BUTTON_W;
    if in_button(PLAY_X) {
        Some(FxPanelHit::Play(idx))
    } else if in_button(STOP_X) {
        Some(FxPanelHit::Stop(idx))
    } else {
        None
    }
}

/// Draw the tab into its content `area`.
pub fn render(frame: &mut Frame<'_>, area: Rect, theme: &Theme, rt: Option<&FxRuntime>) {
    let dim = Style::default().fg(theme.general.secondary);
    let strong = Style::default()
        .fg(theme.general.primary)
        .add_modifier(Modifier::BOLD);
    let mut lines = Vec::new();
    let host = rt.and_then(|r| r.script.as_ref().map(|h| (r, h)));
    match host {
        None => {
            lines.push(Line::from(Span::styled(" No effect script", strong)));
            lines.push(Line::styled(" Open a .figfx.toml (Ctrl+O)", dim));
            lines.push(Line::from(""));
        }
        Some((rt, host)) => {
            let name = host
                .file
                .as_ref()
                .map_or_else(|| "script".to_string(), |f| f.display_name());
            lines.push(Line::from(Span::styled(format!(" {name}"), strong)));
            let hint = if host.script.effects.is_empty() {
                " no effects; edits reload"
            } else {
                " edits reload live"
            };
            lines.push(Line::styled(hint, dim));
            lines.push(Line::from(""));
            let rows = effects_layout(area, host.script.effects.len());
            for (i, _) in rows {
                let e = &host.script.effects[i];
                let running = rt.slot_running(host.script.slot_of(i)) || host.is_looping(i);
                let name_style = if running {
                    strong
                } else {
                    Style::default().fg(theme.menu.fg)
                };
                lines.push(Line::from(vec![
                    Span::raw(" "),
                    Span::styled("[▶]", Style::default().fg(theme.general.primary)),
                    Span::raw(" "),
                    Span::styled("[■]", dim),
                    Span::raw(" "),
                    Span::styled(e.name.clone(), name_style),
                    Span::styled(format!(" {}", e.trigger.label()), dim),
                ]));
            }
        }
    }
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled(" FX Graph ", strong),
        Span::styled("F9", Style::default().fg(theme.general.primary)),
    ]));
    lines.push(Line::styled(" Read-only node graphs", dim));
    lines.push(Line::styled(" of the built-in effects:", dim));
    for preset in presets::all() {
        lines.push(Line::from(format!("  {}", preset.name)));
    }
    frame.render_widget(Paragraph::new(lines), area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::fx_script::script::parse_script;
    use crate::tui::fx_script::FxScriptHost;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn runtime() -> FxRuntime {
        let src =
            "[[effect]]\nname = \"intro\"\ntrigger = \"on_open\"\nsource = 'fx::sleep(100)'\n\
                   [[effect]]\nname = \"spin\"\ntrigger = \"loop\"\nsource = 'fx::sleep(100)'\n";
        let mut rt = FxRuntime::new();
        rt.script = Some(FxScriptHost::new(parse_script(src).unwrap(), None));
        rt
    }

    fn text(rt: Option<&FxRuntime>, area: Rect) -> Vec<String> {
        let mut terminal = Terminal::new(TestBackend::new(area.width, area.height)).unwrap();
        terminal
            .draw(|f| render(f, area, &Theme::default(), rt))
            .unwrap();
        let buf = terminal.backend().buffer();
        (0..area.height)
            .map(|y| (0..area.width).map(|x| buf[(x, y)].symbol()).collect())
            .collect()
    }

    #[test]
    fn lists_effects_with_buttons_where_hit_finds_them() {
        let rt = runtime();
        let area = Rect::new(0, 0, 30, 16);
        let rows = text(Some(&rt), area);
        assert!(rows[0].contains("script"), "{rows:?}");
        let intro_y = rows.iter().position(|r| r.contains("intro")).unwrap() as u16;
        let spin_y = rows.iter().position(|r| r.contains("spin")).unwrap() as u16;
        assert!(
            rows[intro_y as usize].contains("[▶] [■] intro on_open"),
            "{rows:?}"
        );
        assert!(rows[spin_y as usize].contains("loop"));
        assert_eq!(hit(&rt, area, 2, intro_y), Some(FxPanelHit::Play(0)));
        assert_eq!(hit(&rt, area, 6, spin_y), Some(FxPanelHit::Stop(1)));
        assert_eq!(
            hit(&rt, area, 12, intro_y),
            None,
            "the name is not a button"
        );
        assert_eq!(hit(&rt, area, 2, 0), None);
        assert!(rows.iter().any(|r| r.contains("FX Graph")));
    }

    #[test]
    fn rows_past_the_area_are_not_drawn_or_hit() {
        let rt = runtime();
        let area = Rect::new(0, 0, 30, 4);
        assert_eq!(effects_layout(area, 2), [(0, 3)]);
        assert_eq!(hit(&rt, area, 2, 4), None);
    }

    #[test]
    fn without_a_script_it_says_how_to_load_one() {
        let rows = text(None, Rect::new(0, 0, 32, 10));
        assert!(rows[0].contains("No effect script"));
        assert!(rows[1].contains(".figfx.toml"));
        assert_eq!(hit(&FxRuntime::new(), Rect::new(0, 0, 32, 10), 2, 3), None);
    }
}
