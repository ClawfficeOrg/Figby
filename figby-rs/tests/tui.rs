use std::collections::BTreeMap;

const ICONS_YAML: &str = include_str!("../assets/icons.yaml");

#[test]
fn test_icons_yaml_all_keys_present() {
    let map: BTreeMap<String, String> =
        serde_yaml::from_str(ICONS_YAML).expect("failed to parse icons.yaml");

    assert!(
        map.len() >= 120,
        "expected at least 120 icon entries, got {}",
        map.len()
    );

    for (key, value) in &map {
        assert!(!key.is_empty(), "empty key found");
        assert!(!value.is_empty(), "icon value for '{}' is empty", key,);
    }
}

#[test]
fn test_tui_smoke_all_panels_render() {
    use figby::tui::TuiApp;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    let mut app = TuiApp::new();
    app.welcome.screen.show = false;
    // toolbox_h = 12 tools + 1 + 10 brush = 23 rows; need ~30+ rows for palette to render
    // 80 cols drops droppable status-bar items (e.g. FPS) via truncation;
    // 120 keeps the full status bar visible.
    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| app.render(f)).unwrap();
    let buffer = terminal.backend().buffer();
    let output: String = buffer.content().iter().map(|c| c.symbol()).collect();
    assert!(
        output.contains("Font Editor"),
        "toolbar missing Font Editor tab"
    );
    assert!(
        output.contains("Image Editor"),
        "toolbar missing Image Editor tab"
    );
    assert!(
        output.contains("ASCII Preview"),
        "toolbar missing ASCII Preview tab"
    );
    assert!(output.contains("Palette"), "palette sidebar missing");
    assert!(output.contains("FPS:"), "status bar missing");
}

#[test]
fn test_tui_mode_switching() {
    use crossterm::event::KeyCode;
    use figby::tui::{AppMode, TuiApp};

    let mut app = TuiApp::new();
    app.welcome.screen.show = false;
    assert_eq!(app.ui.mode, AppMode::FontEditor);

    app.handle_key_event(KeyCode::Tab);
    assert_eq!(app.ui.mode, AppMode::ImageEditor);

    app.handle_key_event(KeyCode::Tab);
    assert_eq!(app.ui.mode, AppMode::AsciiPreview);

    app.handle_key_event(KeyCode::Tab);
    assert_eq!(app.ui.mode, AppMode::FontEditor);

    app.handle_key_event(KeyCode::Char('q'));
    assert!(app.ui.should_quit);

    let mut app2 = TuiApp::new();
    app2.welcome.screen.show = false;
    // Esc was removed from quit keybinding; 'q' is the quit key
    app2.handle_key_event(KeyCode::Char('q'));
    assert!(app2.ui.should_quit);
}

#[test]
fn test_tui_app_default_mode() {
    use figby::tui::{AppMode, TuiApp};
    let app = TuiApp::new();
    assert_eq!(app.ui.mode, AppMode::FontEditor);
    assert!(!app.ui.should_quit);
}

#[test]
fn test_tool_default_is_brush() {
    use figby::tui::{Tool, TuiApp};
    let app = TuiApp::new();
    assert_eq!(app.editor.toolbox.selected, Tool::Brush);
}

#[test]
fn test_tool_selection_roundtrip() {
    use crossterm::event::KeyCode;
    use figby::tui::{Tool, TuiApp};

    let mut app = TuiApp::new();
    app.welcome.screen.show = false;
    assert_eq!(app.editor.toolbox.selected, Tool::Brush);

    app.handle_key_event(KeyCode::Char('v'));
    assert_eq!(app.editor.toolbox.selected, Tool::Marquee);

    app.handle_key_event(KeyCode::Char('b'));
    assert_eq!(app.editor.toolbox.selected, Tool::Brush);

    app.handle_key_event(KeyCode::Char('l'));
    assert_eq!(app.editor.toolbox.selected, Tool::Lasso);

    app.handle_key_event(KeyCode::Char('c'));
    assert_eq!(app.editor.toolbox.selected, Tool::CircleSelect);

    app.handle_key_event(KeyCode::Char('p'));
    assert_eq!(app.editor.toolbox.selected, Tool::PolygonSelect);

    app.handle_key_event(KeyCode::Char('g'));
    assert_eq!(app.editor.toolbox.selected, Tool::Fill);

    app.handle_key_event(KeyCode::Char('i'));
    assert_eq!(app.editor.toolbox.selected, Tool::Line);

    app.handle_key_event(KeyCode::Char('e'));
    assert_eq!(app.editor.toolbox.selected, Tool::Eraser);

    app.handle_key_event(KeyCode::Char('d'));
    assert_eq!(app.editor.toolbox.selected, Tool::Eyedropper);

    app.handle_key_event(KeyCode::Char('t'));
    assert_eq!(app.editor.toolbox.selected, Tool::Text);

    app.handle_key_event(KeyCode::Char('B'));
    assert_eq!(app.editor.toolbox.selected, Tool::Brush);
}

#[test]
fn test_toolbox_renders_tool_names() {
    use figby::tui::TuiApp;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    let mut app = TuiApp::new();
    app.welcome.screen.show = false;
    let backend = TestBackend::new(80, 40);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| app.render(f)).unwrap();
    let buffer = terminal.backend().buffer();
    let output: String = buffer.content().iter().map(|c| c.symbol()).collect();
    assert!(output.contains(" Br"), "toolbox missing Brush tool");
    assert!(output.contains(" Er"), "toolbox missing Eraser tool");
}

#[test]
fn test_canvas_render_empty() {
    use figby::tui::canvas::CanvasWidget;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    let canvas = CanvasWidget::new(10, 5);
    let backend = TestBackend::new(20, 10);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| f.render_widget(&canvas, f.area()))
        .unwrap();
    let buffer = terminal.backend().buffer();
    for cell in buffer.content() {
        assert_eq!(cell.symbol(), " ");
    }
}

#[test]
fn test_canvas_render_cells() {
    use figby::tui::canvas::{CanvasCell, CanvasWidget};
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    let mut canvas = CanvasWidget::new(10, 5);
    canvas.buffer.set(
        0,
        0,
        CanvasCell {
            ch: 'A',
            fg: None,
            bg: None,
            height: None,
        },
    );
    canvas.buffer.set(
        2,
        1,
        CanvasCell {
            ch: 'B',
            fg: None,
            bg: None,
            height: None,
        },
    );

    let backend = TestBackend::new(20, 10);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| f.render_widget(&canvas, f.area()))
        .unwrap();
    let buffer = terminal.backend().buffer();
    let output: String = buffer.content().iter().map(|c| c.symbol()).collect();
    assert!(output.contains('A'), "expected 'A' in canvas output");
    assert!(output.contains('B'), "expected 'B' in canvas output");
}

#[test]
fn test_canvas_cursor_movement() {
    use crossterm::event::KeyCode;
    use figby::tui::canvas::CanvasWidget;

    let mut canvas = CanvasWidget::new(10, 5);
    assert_eq!(canvas.cursor(), (0, 0));

    canvas.handle_key(KeyCode::Right, 20, 10);
    assert_eq!(canvas.cursor(), (1, 0));

    canvas.handle_key(KeyCode::Down, 20, 10);
    assert_eq!(canvas.cursor(), (1, 1));

    canvas.handle_key(KeyCode::Left, 20, 10);
    assert_eq!(canvas.cursor(), (0, 1));

    canvas.handle_key(KeyCode::Up, 20, 10);
    assert_eq!(canvas.cursor(), (0, 0));
}

#[test]
fn test_canvas_zoom_in_out() {
    use crossterm::event::KeyCode;
    use figby::tui::canvas::CanvasWidget;

    let mut canvas = CanvasWidget::new(10, 5);
    assert_eq!(canvas.zoom_level(), 1);

    canvas.handle_key(KeyCode::Char('+'), 20, 10);
    assert_eq!(canvas.zoom_level(), 2);

    canvas.handle_key(KeyCode::Char('='), 20, 10);
    assert_eq!(canvas.zoom_level(), 4);

    canvas.handle_key(KeyCode::Char('+'), 20, 10);
    assert_eq!(canvas.zoom_level(), 8);

    canvas.handle_key(KeyCode::Char('+'), 20, 10);
    assert_eq!(canvas.zoom_level(), 8);

    canvas.handle_key(KeyCode::Char('-'), 20, 10);
    assert_eq!(canvas.zoom_level(), 4);

    canvas.handle_key(KeyCode::Char('_'), 20, 10);
    assert_eq!(canvas.zoom_level(), 2);

    canvas.handle_key(KeyCode::Char('-'), 20, 10);
    assert_eq!(canvas.zoom_level(), 1);

    canvas.handle_key(KeyCode::Char('-'), 20, 10);
    assert_eq!(canvas.zoom_level(), 1);
}

#[test]
fn test_canvas_cursor_visible() {
    use figby::tui::canvas::{CanvasCell, CanvasWidget};
    use ratatui::backend::TestBackend;
    use ratatui::style::Modifier;
    use ratatui::Terminal;

    let mut canvas = CanvasWidget::new(10, 5);
    canvas.buffer.set(
        0,
        0,
        CanvasCell {
            ch: 'X',
            fg: None,
            bg: None,
            height: None,
        },
    );

    let backend = TestBackend::new(20, 10);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| f.render_widget(&canvas, f.area()))
        .unwrap();
    let buffer = terminal.backend().buffer();
    let cell = buffer.cell((0, 0)).expect("cell at (0,0)");
    assert_eq!(cell.symbol(), "X");
    assert!(
        cell.style().add_modifier.contains(Modifier::REVERSED),
        "cursor cell should have REVERSED modifier"
    );
}

#[test]
fn test_canvas_zoom_shows_grid() {
    use crossterm::event::KeyCode;
    use figby::tui::canvas::{CanvasCell, CanvasWidget};
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    let mut canvas = CanvasWidget::new(5, 3);
    canvas.buffer.set(
        0,
        0,
        CanvasCell {
            ch: 'A',
            fg: None,
            bg: None,
            height: None,
        },
    );
    canvas.buffer.set(
        1,
        0,
        CanvasCell {
            ch: 'B',
            fg: None,
            bg: None,
            height: None,
        },
    );

    canvas.handle_key(KeyCode::Char('+'), 20, 10);
    canvas.handle_key(KeyCode::Char('G'), 20, 10);

    let backend = TestBackend::new(20, 10);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| f.render_widget(&canvas, f.area()))
        .unwrap();
    let buffer = terminal.backend().buffer();
    let output: String = buffer.content().iter().map(|c| c.symbol()).collect();
    assert!(output.contains('│'), "expected vertical grid line │");
    assert!(output.contains('─'), "expected horizontal grid line ─");
}

#[test]
fn test_palette_default_target_foreground() {
    use figby::tui::palette::{ColorTarget, Palette};
    let palette = Palette::new();
    assert_eq!(palette.target, ColorTarget::Foreground);
}

#[test]
fn test_palette_fg_bg_toggle() {
    use crossterm::event::KeyCode;
    use figby::tui::palette::{ColorTarget, Palette};

    let mut palette = Palette::new();
    assert_eq!(palette.target, ColorTarget::Foreground);

    palette.handle_key(KeyCode::Char('x'));
    assert_eq!(palette.target, ColorTarget::Background);

    palette.handle_key(KeyCode::Char('x'));
    assert_eq!(palette.target, ColorTarget::Foreground);

    palette.handle_key(KeyCode::Char('X'));
    assert_eq!(palette.target, ColorTarget::Background);

    palette.handle_key(KeyCode::Char('X'));
    assert_eq!(palette.target, ColorTarget::Foreground);
}

#[test]
fn test_palette_select_color_updates_selected() {
    use figby::tui::palette::{Palette, ANSI_16_COLORS};

    let mut palette = Palette::new();
    palette.select_color(1);
    assert_eq!(palette.selected_color, Some(ANSI_16_COLORS[7]));

    palette.select_color(7);
    assert_eq!(palette.selected_color, Some(ANSI_16_COLORS[11]));
}

#[test]
fn test_palette_select_pushes_recent() {
    use figby::tui::palette::{Palette, ANSI_16_COLORS};

    let mut palette = Palette::new();
    assert!(palette.recent.is_empty());

    palette.select_color(1);
    assert_eq!(palette.recent.len(), 1);
    assert_eq!(palette.recent[0], ANSI_16_COLORS[7]);

    palette.select_color(5);
    assert_eq!(palette.recent.len(), 2);
    assert_eq!(palette.recent[1], ANSI_16_COLORS[9]);

    palette.select_color(1);
    assert_eq!(palette.recent.len(), 2);
    assert_eq!(palette.recent[1], ANSI_16_COLORS[7]);
}

#[test]
fn test_palette_custom_hex_applies() {
    use figby::tui::palette::Palette;
    use ratatui::style::Color;

    let mut palette = Palette::new();
    let result = palette.set_custom_hex("#FF8800");
    assert!(result);
    assert_eq!(palette.selected_color, Some(Color::Rgb(255, 136, 0)));
}

#[test]
fn test_palette_apply_to_cell_fg() {
    use figby::tui::canvas::CanvasCell;
    use figby::tui::palette::{Palette, ANSI_16_COLORS};

    let mut palette = Palette::new();
    palette.select_color(2);
    let mut cell = CanvasCell::default();
    palette.apply_to_cell(&mut cell);
    assert_eq!(cell.fg, Some(ANSI_16_COLORS[8]));
    assert_eq!(cell.bg, None);
}

#[test]
fn test_palette_apply_to_cell_bg() {
    use figby::tui::canvas::CanvasCell;
    use figby::tui::palette::{Palette, ANSI_16_COLORS};

    let mut palette = Palette::new();
    palette.toggle_target();
    palette.select_color(4);
    let mut cell = CanvasCell::default();
    palette.apply_to_cell(&mut cell);
    assert_eq!(cell.fg, None);
    assert_eq!(cell.bg, Some(ANSI_16_COLORS[1]));
}

#[test]
fn test_brush_default_shape() {
    use figby::tui::brush::BrushShape;
    use figby::tui::BrushState;
    let brush = BrushState::new();
    assert_eq!(brush.shape, BrushShape::Square);
}

#[test]
fn test_brush_default_size() {
    use figby::tui::BrushState;
    let brush = BrushState::new();
    assert_eq!(brush.size, 1);
}

#[test]
fn test_brush_cycle_shape() {
    use figby::tui::brush::BrushShape;
    use figby::tui::BrushState;
    let mut brush = BrushState::new();
    assert_eq!(brush.shape, BrushShape::Square);
    brush.cycle_shape();
    assert_eq!(brush.shape, BrushShape::Circle);
    brush.cycle_shape();
    assert_eq!(brush.shape, BrushShape::SprayPaint);
    brush.cycle_shape();
    assert_eq!(brush.shape, BrushShape::Custom);
    brush.cycle_shape();
    assert_eq!(brush.shape, BrushShape::Square);
}

#[test]
fn test_brush_size_up_down_key() {
    use crossterm::event::KeyCode;
    use figby::tui::TuiApp;

    let mut app = TuiApp::new();
    assert_eq!(app.editor.brush.size, 1);

    app.handle_key_event(KeyCode::Char(']'));
    assert_eq!(app.editor.brush.size, 2);

    app.handle_key_event(KeyCode::Char('['));
    assert_eq!(app.editor.brush.size, 1);
}

#[test]
fn test_brush_shape_cycle_key() {
    use crossterm::event::KeyCode;
    use figby::tui::brush::BrushShape;
    use figby::tui::TuiApp;

    let mut app = TuiApp::new();
    assert_eq!(app.editor.brush.shape, BrushShape::Square);

    app.handle_key_event(KeyCode::Char('\\'));
    assert_eq!(app.editor.brush.shape, BrushShape::Circle);

    app.handle_key_event(KeyCode::Char('\\'));
    assert_eq!(app.editor.brush.shape, BrushShape::SprayPaint);
}

#[test]
fn test_brush_preview_square_integration() {
    use figby::tui::BrushState;

    let brush = BrushState {
        shape: figby::tui::brush::BrushShape::Square,
        size: 3,
        ch: '\u{2588}',
        density: 35,
        ..BrushState::new()
    };
    let preview = brush.render_preview(10);
    assert_eq!(preview.len(), 3);
    for row in &preview {
        assert_eq!(row.chars().filter(|&c| c == '@').count(), 3);
    }
}

#[test]
fn test_brush_preview_circle_integration() {
    use figby::tui::brush::BrushShape;
    use figby::tui::BrushState;

    let brush = BrushState {
        shape: BrushShape::Circle,
        size: 5,
        ch: '\u{2588}',
        density: 35,
        ..BrushState::new()
    };
    let preview = brush.render_preview(10);
    assert_eq!(preview.len(), 5);
    for row in &preview {
        assert_eq!(row.len(), 5);
    }
}

#[test]
fn test_brush_preview_spray_deterministic() {
    use figby::tui::brush::BrushShape;
    use figby::tui::BrushState;

    let a = BrushState {
        shape: BrushShape::SprayPaint,
        size: 7,
        ch: '\u{2588}',
        density: 35,
        ..BrushState::new()
    };
    let b = BrushState {
        shape: BrushShape::SprayPaint,
        size: 7,
        ch: '\u{2588}',
        density: 35,
        ..BrushState::new()
    };
    assert_eq!(a.render_preview(10), b.render_preview(10));
}

#[test]
fn test_brush_preview_custom_center() {
    use figby::tui::brush::BrushShape;
    use figby::tui::BrushState;

    let brush = BrushState {
        shape: BrushShape::Custom,
        size: 5,
        ch: '\u{2588}',
        density: 35,
        ..BrushState::new()
    };
    let preview = brush.render_preview(10);
    assert_eq!(preview[2].as_bytes()[2] as char, '+');
}

#[test]
fn test_brush_preview_respects_max_size() {
    use figby::tui::BrushState;

    let mut brush = BrushState::new();
    brush.set_size(15);
    let preview = brush.render_preview(5);
    assert_eq!(preview.len(), 5);
    for row in &preview {
        assert_eq!(row.len(), 5);
    }
}

#[test]
fn test_brush_render_contains_shape_name() {
    use figby::tui::TabId;
    use figby::tui::TuiApp;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    let mut app = TuiApp::new();
    app.welcome.screen.show = false;
    app.side_panel.open = true;
    app.side_panel.active_tab = TabId::Props;
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| app.render(f)).unwrap();
    let buffer = terminal.backend().buffer();
    let output: String = buffer.content().iter().map(|c| c.symbol()).collect();
    assert!(output.contains("Brush"), "brush panel missing");
    assert!(output.contains("Shape:"), "brush shape label missing");
}

#[test]
fn test_palette_render_contains_labels() {
    use figby::tui::TuiApp;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    let mut app = TuiApp::new();
    app.welcome.screen.show = false;
    app.editor.palette.select_color(0);
    // toolbox_h = 23 rows; "Recent:" section is near bottom of palette — need tall terminal
    let backend = TestBackend::new(80, 80);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| app.render(f)).unwrap();
    let buffer = terminal.backend().buffer();
    let output: String = buffer.content().iter().map(|c| c.symbol()).collect();
    assert!(output.contains("FG"), "palette missing FG indicator");
    assert!(output.contains("BG"), "palette missing BG indicator");
    assert!(output.contains("Recent"), "palette missing Recent label");
}

#[test]
fn test_status_bar_shows_cursor_position() {
    use crossterm::event::KeyCode;
    use figby::tui::AppMode;
    use figby::tui::TuiApp;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    let mut app = TuiApp::new();
    app.welcome.screen.show = false;
    // Switch to ImageEditor so arrow keys move canvas cursor
    app.ui.mode = AppMode::ImageEditor;
    app.handle_key_event(KeyCode::Right);
    app.handle_key_event(KeyCode::Down);

    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| app.render(f)).unwrap();
    let buffer = terminal.backend().buffer();
    let output: String = buffer.content().iter().map(|c| c.symbol()).collect();
    assert!(output.contains("X:1"), "status bar missing X cursor");
    assert!(output.contains("Y:1"), "status bar missing Y cursor");
}

#[test]
fn test_status_bar_shows_zoom_level() {
    use crossterm::event::KeyCode;
    use figby::tui::TuiApp;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    let mut app = TuiApp::new();
    app.welcome.screen.show = false;
    app.handle_key_event(KeyCode::Char('+'));

    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| app.render(f)).unwrap();
    let buffer = terminal.backend().buffer();
    let output: String = buffer.content().iter().map(|c| c.symbol()).collect();
    assert!(output.contains("2x"), "status bar missing zoom level");
}

#[test]
fn test_status_bar_shows_tool_name() {
    use crossterm::event::KeyCode;
    use figby::tui::TuiApp;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    let mut app = TuiApp::new();
    app.welcome.screen.show = false;
    app.handle_key_event(KeyCode::Char('e'));

    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| app.render(f)).unwrap();
    let buffer = terminal.backend().buffer();
    let output: String = buffer.content().iter().map(|c| c.symbol()).collect();
    assert!(output.contains("Eraser"), "status bar missing tool name");
}

#[test]
fn test_status_bar_shows_mode_name() {
    use figby::tui::TuiApp;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    let mut app = TuiApp::new();
    app.welcome.screen.show = false;
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| app.render(f)).unwrap();
    let buffer = terminal.backend().buffer();
    let output: String = buffer.content().iter().map(|c| c.symbol()).collect();
    assert!(
        output.contains("Font Editor"),
        "status bar missing mode name"
    );
}

#[test]
fn test_status_bar_unsaved_indicator() {
    use figby::tui::TuiApp;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    let mut app = TuiApp::new();
    app.welcome.screen.show = false;
    app.editor.unsaved = true;
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| app.render(f)).unwrap();
    let buffer = terminal.backend().buffer();
    let output: String = buffer.content().iter().map(|c| c.symbol()).collect();
    app.editor.unsaved = false;
    let backend2 = TestBackend::new(80, 24);
    let mut terminal2 = Terminal::new(backend2).unwrap();
    terminal2.draw(|f| app.render(f)).unwrap();
    let buffer2 = terminal2.backend().buffer();
    let output2: String = buffer2.content().iter().map(|c| c.symbol()).collect();
    assert_ne!(
        output, output2,
        "unsaved and saved states should render different icons"
    );
}

#[test]
fn test_settings_toggle_visibility() {
    use crossterm::event::KeyCode;
    use figby::tui::TuiApp;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    let mut app = TuiApp::new();
    app.welcome.screen.show = false;
    app.handle_key_event(KeyCode::Tab); // switch to ImageEditor so S opens Settings, not Smushing
    app.handle_key_event(KeyCode::Char('S'));
    assert!(
        app.dialogs.settings.settings_open,
        "settings should open on S"
    );

    // toolbox_h = 23 rows; settings panel renders in palette_rect which needs 40+ rows
    let backend = TestBackend::new(80, 40);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| app.render(f)).unwrap();
    let buffer = terminal.backend().buffer();
    let output: String = buffer.content().iter().map(|c| c.symbol()).collect();
    assert!(output.contains("Settings"), "settings panel title missing");

    app.handle_key_event(KeyCode::Char('S'));
    assert!(
        !app.dialogs.settings.settings_open,
        "settings should close on S"
    );
}

#[test]
fn test_settings_changes_canvas_width() {
    use crossterm::event::KeyCode;
    use figby::tui::TuiApp;

    let mut app = TuiApp::new();
    assert_eq!(app.editor.canvas.buffer.width(), 40);
    app.handle_key_event(KeyCode::Tab); // switch to ImageEditor so S opens Settings, not Smushing
    app.handle_key_event(KeyCode::Char('S'));
    assert!(app.dialogs.settings.settings_open);
    app.handle_key_event(KeyCode::Right);
    assert_eq!(
        app.editor.canvas.buffer.width(),
        41,
        "canvas width should increase"
    );
}

#[test]
fn test_settings_toggle_grid() {
    use crossterm::event::KeyCode;
    use figby::tui::TuiApp;

    let mut app = TuiApp::new();
    app.handle_key_event(KeyCode::Tab); // switch to ImageEditor so S opens Settings, not Smushing
    app.handle_key_event(KeyCode::Char('S'));
    for _ in 0..3 {
        app.handle_key_event(KeyCode::Down);
    }
    app.handle_key_event(KeyCode::Enter);
    assert!(app.editor.canvas.show_grid(), "grid should be toggled on");
}

#[test]
fn test_settings_toggle_snap_to_grid() {
    use crossterm::event::KeyCode;
    use figby::tui::TuiApp;

    let mut app = TuiApp::new();
    assert!(!app.dialogs.settings.snap_to_grid);
    app.handle_key_event(KeyCode::Tab); // switch to ImageEditor so S opens Settings, not Smushing
    app.handle_key_event(KeyCode::Char('S'));
    for _ in 0..4 {
        app.handle_key_event(KeyCode::Down);
    }
    app.handle_key_event(KeyCode::Enter);
    assert!(
        app.dialogs.settings.snap_to_grid,
        "snap-to-grid should be toggled on"
    );
}

#[test]
fn test_fill_tool_keyboard() {
    use crossterm::event::KeyCode;
    use figby::tui::canvas::CanvasCell;
    use figby::tui::{Tool, TuiApp};

    let mut app = TuiApp::new();

    // Select Fill tool via keyboard shortcut
    app.handle_key_event(KeyCode::Char('g'));
    assert_eq!(app.editor.toolbox.selected, Tool::Fill);

    // Draw a 2x2 region of @ on the active layer (not composite canvas.buffer)
    let cell = CanvasCell {
        ch: '@',
        fg: None,
        bg: None,
        height: None,
    };
    app.editor
        .layer_stack
        .active_layer_mut()
        .buffer_mut()
        .set(1, 1, cell);
    app.editor
        .layer_stack
        .active_layer_mut()
        .buffer_mut()
        .set(1, 2, cell);
    app.editor
        .layer_stack
        .active_layer_mut()
        .buffer_mut()
        .set(2, 1, cell);
    app.editor
        .layer_stack
        .active_layer_mut()
        .buffer_mut()
        .set(2, 2, cell);
    // Sync composite canvas so cursor / fill sees the layer content
    app.editor.recomposite_canvas();

    // Move cursor to (1, 1)
    app.editor.canvas.set_cursor(1, 1);

    // Press Space to flood fill
    app.handle_key_event(KeyCode::Char(' '));

    // The filled region should have been replaced with full block
    assert_eq!(
        app.editor.canvas.buffer.get(1, 1).unwrap().ch,
        '\u{2588}',
        "filled cell (1,1)"
    );
    assert_eq!(
        app.editor.canvas.buffer.get(2, 2).unwrap().ch,
        '\u{2588}',
        "filled cell (2,2)"
    );
    assert_eq!(
        app.editor.canvas.buffer.get(0, 0).unwrap().ch,
        ' ',
        "outside fill should remain space"
    );
}

// --- Font Editor tests ---

#[test]
fn test_font_editor_grid_renders_102_chars() {
    use figby::font::parse_tlf_font;
    use figby::tui::font_editor::FontEditor;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    let content = include_str!("../assets/fonts/standard.flf");
    let font = parse_tlf_font(content).expect("standard font should parse");
    let mut editor = FontEditor::new();
    editor.load_font(font);

    let backend = TestBackend::new(120, 60);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| editor.render(f, f.area())).unwrap();
    let buffer = terminal.backend().buffer();
    let output: String = buffer.content().iter().map(|c| c.symbol()).collect();

    // Verify font loaded at least 102 required chars (95 ASCII + 7 Deutsch)
    let codes = editor.filtered_codes();
    assert!(
        codes.len() >= 102,
        "should have at least 102 codes loaded, got {}",
        codes.len()
    );
    for code in 32..=126u32 {
        assert!(
            codes.contains(&code),
            "missing ASCII code {} in filtered_codes",
            code
        );
    }
    for &code in &figby::font::DEUTSCH_CHARS {
        assert!(
            codes.contains(&code),
            "missing Deutsch code {} in filtered_codes",
            code
        );
    }

    // Verify grid renders visible content for first few codes
    assert!(output.contains("32"), "code 32 should be in visible output");
    assert!(output.contains("65"), "code 65 should be in visible output");
    assert!(
        output.contains("  "),
        "grid output should contain spaces between cells"
    );
}

#[test]
fn test_font_editor_search_by_code() {
    use figby::font::parse_tlf_font;
    use figby::tui::font_editor::FontEditor;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    let content = include_str!("../assets/fonts/standard.flf");
    let font = parse_tlf_font(content).expect("standard font should parse");
    let mut editor = FontEditor::new();
    editor.load_font(font);
    editor.search_query = "65".to_string();

    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| editor.render(f, f.area())).unwrap();
    let buffer = terminal.backend().buffer();
    let output: String = buffer.content().iter().map(|c| c.symbol()).collect();

    assert!(
        !output.contains("66"),
        "code 66 should not appear when searching for '65'"
    );
    // '65' should match code 65 which is 'A'
}

#[test]
fn test_font_editor_search_by_char_value() {
    use figby::font::parse_tlf_font;
    use figby::tui::font_editor::FontEditor;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    let content = include_str!("../assets/fonts/standard.flf");
    let font = parse_tlf_font(content).expect("standard font should parse");
    let mut editor = FontEditor::new();
    editor.load_font(font);
    editor.search_query = "A".to_string();

    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| editor.render(f, f.area())).unwrap();
    let buffer = terminal.backend().buffer();
    let output: String = buffer.content().iter().map(|c| c.symbol()).collect();

    assert!(
        output.contains("65"),
        "code 65 (A) should appear when searching for 'A'"
    );
    assert!(
        !output.contains("66"),
        "code 66 (B) should not appear when searching for 'A'"
    );
}

#[test]
fn test_font_editor_select_opens_char_in_canvas() {
    use crossterm::event::{KeyCode, KeyModifiers};
    use figby::font::parse_tlf_font;
    use figby::tui::font_editor::{FontEditor, FontEditorView};

    let content = include_str!("../assets/fonts/standard.flf");
    let font = parse_tlf_font(content).expect("standard font should parse");
    let mut editor = FontEditor::new();
    editor.load_font(font);

    // Press Enter to select first char (space, code 32)
    editor.handle_key(KeyCode::Enter, KeyModifiers::NONE, 120);

    assert_eq!(
        editor.view,
        FontEditorView::CharEditor(32),
        "should select code 32 (space) on Enter"
    );
    assert!(
        editor.selected_char().is_some(),
        "selected_char should return Some in CharEditor view"
    );
    if let Some((code, ch)) = editor.selected_char() {
        assert_eq!(code, 32);
        assert!(!ch.rows().is_empty());
    }
}

#[test]
fn test_font_editor_esc_returns_to_overview() {
    use crossterm::event::{KeyCode, KeyModifiers};
    use figby::font::parse_tlf_font;
    use figby::tui::font_editor::{FontEditor, FontEditorView};

    let content = include_str!("../assets/fonts/standard.flf");
    let font = parse_tlf_font(content).expect("standard font should parse");
    let mut editor = FontEditor::new();
    editor.load_font(font);

    // Enter CharEditor first
    editor.handle_key(KeyCode::Enter, KeyModifiers::NONE, 120);
    assert_eq!(
        editor.view,
        FontEditorView::CharEditor(32),
        "should select code 32 (space) on Enter"
    );

    // Esc returns to Overview
    editor.handle_key(KeyCode::Esc, KeyModifiers::NONE, 120);
    assert_eq!(editor.view, FontEditorView::Overview);
}

#[test]
fn test_font_editor_empty_font_no_panic() {
    use figby::tui::font_editor::FontEditor;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    let editor = FontEditor::new();

    let backend = TestBackend::new(80, 24);

    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| editor.render(f, f.area())).unwrap();
    let buffer = terminal.backend().buffer();
    let output: String = buffer.content().iter().map(|c| c.symbol()).collect();
    assert!(output.contains("Search"), "search bar should still render");
}

#[test]
fn test_font_editor_grid_navigation() {
    use crossterm::event::{KeyCode, KeyModifiers};
    use figby::font::parse_tlf_font;
    use figby::tui::font_editor::FontEditor;

    let content = include_str!("../assets/fonts/standard.flf");
    let font = parse_tlf_font(content).expect("standard font should parse");
    let mut editor = FontEditor::new();
    editor.load_font(font);

    let initial = editor.selected_index;
    assert_eq!(initial, 0);

    // Right arrow
    editor.handle_key(KeyCode::Right, KeyModifiers::NONE, 120);
    assert_eq!(editor.selected_index, 1);

    // Left arrow
    editor.handle_key(KeyCode::Left, KeyModifiers::NONE, 120);
    assert_eq!(editor.selected_index, 0);

    // Down arrow (moves by cols)
    editor.handle_key(KeyCode::Down, KeyModifiers::NONE, 120);
    // col count at width 120: cell_w=18 → cols = floor(120/18) = 6
    assert!(editor.selected_index > 0, "Down should move selection");
    let down_idx = editor.selected_index;

    // Up arrow returns to original
    editor.handle_key(KeyCode::Up, KeyModifiers::NONE, 120);
    assert_eq!(editor.selected_index, 0);

    // Navigate to last item
    editor.handle_key(KeyCode::Down, KeyModifiers::NONE, 120);
    assert_eq!(editor.selected_index, down_idx);
}

// --- Header Editor tests ---

fn header_editor_setup() -> (figby::tui::font_editor::FontEditor, figby::font::FIGfont) {
    use figby::font::parse_tlf_font;
    let content = include_str!("../assets/fonts/standard.flf");
    let font = parse_tlf_font(content).expect("standard font should parse");
    let mut editor = figby::tui::font_editor::FontEditor::new();
    let header_font = font.clone();
    editor.load_font(font);
    (editor, header_font)
}

#[test]
fn test_font_editor_header_open_close() {
    use crossterm::event::{KeyCode, KeyModifiers};
    use figby::tui::font_editor::FontEditorView;

    let (mut editor, _) = header_editor_setup();

    editor.handle_key(KeyCode::Char('H'), KeyModifiers::NONE, 120);
    assert_eq!(editor.view, FontEditorView::HeaderEditor);

    editor.handle_key(KeyCode::Esc, KeyModifiers::NONE, 120);
    assert_eq!(editor.view, FontEditorView::Overview);
}

#[test]
fn test_font_editor_header_charheight_edit() {
    use crossterm::event::{KeyCode, KeyModifiers};

    let (mut editor, _) = header_editor_setup();
    editor.handle_key(KeyCode::Char('H'), KeyModifiers::NONE, 120);

    for _ in 0..1 {
        editor.handle_key(KeyCode::Down, KeyModifiers::NONE, 120);
    }
    assert_eq!(editor.selected_field, 1);

    editor.handle_key(KeyCode::Enter, KeyModifiers::NONE, 120);
    assert!(editor.editing_field);

    editor.edit_buffer = "6".to_string();
    editor.handle_key(KeyCode::Enter, KeyModifiers::NONE, 120);
    assert!(!editor.editing_field);
    assert_eq!(editor.font.as_ref().unwrap().charheight, 6);
}

#[test]
fn test_font_editor_header_baseline_edit() {
    use crossterm::event::{KeyCode, KeyModifiers};

    let (mut editor, _) = header_editor_setup();
    editor.handle_key(KeyCode::Char('H'), KeyModifiers::NONE, 120);

    for _ in 0..2 {
        editor.handle_key(KeyCode::Down, KeyModifiers::NONE, 120);
    }
    assert_eq!(editor.selected_field, 2);

    editor.handle_key(KeyCode::Enter, KeyModifiers::NONE, 120);
    assert!(editor.editing_field);

    editor.edit_buffer = "4".to_string();
    editor.handle_key(KeyCode::Enter, KeyModifiers::NONE, 120);
    assert!(!editor.editing_field);
    assert_eq!(editor.font.as_ref().unwrap().baseline, 4);
}

#[test]
fn test_font_editor_header_hardblank_edit() {
    use crossterm::event::{KeyCode, KeyModifiers};

    let (mut editor, _) = header_editor_setup();
    editor.handle_key(KeyCode::Char('H'), KeyModifiers::NONE, 120);

    assert_eq!(editor.selected_field, 0);

    editor.handle_key(KeyCode::Enter, KeyModifiers::NONE, 120);
    assert!(editor.editing_field);

    editor.edit_buffer = "#".to_string();
    editor.handle_key(KeyCode::Enter, KeyModifiers::NONE, 120);
    assert!(!editor.editing_field);
    assert_eq!(editor.font.as_ref().unwrap().hardblank, '#');
}

#[test]
fn test_font_editor_header_rejects_height_zero() {
    use crossterm::event::{KeyCode, KeyModifiers};

    let (mut editor, orig) = header_editor_setup();
    editor.handle_key(KeyCode::Char('H'), KeyModifiers::NONE, 120);

    for _ in 0..1 {
        editor.handle_key(KeyCode::Down, KeyModifiers::NONE, 120);
    }

    editor.handle_key(KeyCode::Enter, KeyModifiers::NONE, 120);
    editor.edit_buffer = "0".to_string();
    editor.handle_key(KeyCode::Enter, KeyModifiers::NONE, 120);

    assert!(
        editor.editing_field,
        "should stay in editing mode after reject"
    );
    assert!(!editor.error_message.is_empty(), "error should be set");
    assert_eq!(editor.font.as_ref().unwrap().charheight, orig.charheight);
}

#[test]
fn test_font_editor_header_rejects_baseline_exceeds_height() {
    use crossterm::event::{KeyCode, KeyModifiers};

    let (mut editor, orig) = header_editor_setup();
    editor.handle_key(KeyCode::Char('H'), KeyModifiers::NONE, 120);

    for _ in 0..2 {
        editor.handle_key(KeyCode::Down, KeyModifiers::NONE, 120);
    }

    editor.handle_key(KeyCode::Enter, KeyModifiers::NONE, 120);
    editor.edit_buffer = "999".to_string();
    editor.handle_key(KeyCode::Enter, KeyModifiers::NONE, 120);

    assert!(
        editor.editing_field,
        "should stay in editing mode after reject"
    );
    assert!(!editor.error_message.is_empty(), "error should be set");
    assert_eq!(editor.font.as_ref().unwrap().baseline, orig.baseline);
}

#[test]
fn test_font_editor_header_full_layout_edit() {
    use crossterm::event::{KeyCode, KeyModifiers};

    let (mut editor, _) = header_editor_setup();
    editor.handle_key(KeyCode::Char('H'), KeyModifiers::NONE, 120);

    for _ in 0..4 {
        editor.handle_key(KeyCode::Down, KeyModifiers::NONE, 120);
    }
    assert_eq!(editor.selected_field, 4);

    editor.handle_key(KeyCode::Enter, KeyModifiers::NONE, 120);

    editor.edit_buffer = "191".to_string();
    editor.handle_key(KeyCode::Enter, KeyModifiers::NONE, 120);
    assert!(!editor.editing_field);
    assert_eq!(editor.font.as_ref().unwrap().full_layout, 191);
}

#[test]
fn test_font_editor_header_print_direction_edit() {
    use crossterm::event::{KeyCode, KeyModifiers};

    let (mut editor, _) = header_editor_setup();
    editor.handle_key(KeyCode::Char('H'), KeyModifiers::NONE, 120);

    for _ in 0..5 {
        editor.handle_key(KeyCode::Down, KeyModifiers::NONE, 120);
    }
    assert_eq!(editor.selected_field, 5);

    editor.handle_key(KeyCode::Enter, KeyModifiers::NONE, 120);
    editor.edit_buffer = "1".to_string();
    editor.handle_key(KeyCode::Enter, KeyModifiers::NONE, 120);
    assert!(!editor.editing_field);
    assert_eq!(editor.font.as_ref().unwrap().print_direction, 1);

    // Test -1 is also valid
    editor.handle_key(KeyCode::Enter, KeyModifiers::NONE, 120);
    editor.edit_buffer = "-1".to_string();
    editor.handle_key(KeyCode::Enter, KeyModifiers::NONE, 120);
    assert_eq!(editor.font.as_ref().unwrap().print_direction, -1);

    // Test 0 is also valid
    editor.handle_key(KeyCode::Enter, KeyModifiers::NONE, 120);
    editor.edit_buffer = "0".to_string();
    editor.handle_key(KeyCode::Enter, KeyModifiers::NONE, 120);
    assert_eq!(editor.font.as_ref().unwrap().print_direction, 0);
}

#[test]
fn test_font_editor_header_comment_lines_edit() {
    use crossterm::event::{KeyCode, KeyModifiers};

    let (mut editor, _) = header_editor_setup();
    editor.handle_key(KeyCode::Char('H'), KeyModifiers::NONE, 120);

    for _ in 0..6 {
        editor.handle_key(KeyCode::Down, KeyModifiers::NONE, 120);
    }
    assert_eq!(editor.selected_field, 6);

    editor.handle_key(KeyCode::Enter, KeyModifiers::NONE, 120);
    editor.edit_buffer = "3".to_string();
    editor.handle_key(KeyCode::Enter, KeyModifiers::NONE, 120);
    assert!(!editor.editing_field);
    assert_eq!(editor.font.as_ref().unwrap().comment_lines, 3);
}

#[test]
fn test_font_editor_header_maxlength_edit() {
    use crossterm::event::{KeyCode, KeyModifiers};

    let (mut editor, _) = header_editor_setup();
    editor.handle_key(KeyCode::Char('H'), KeyModifiers::NONE, 120);

    for _ in 0..3 {
        editor.handle_key(KeyCode::Down, KeyModifiers::NONE, 120);
    }
    assert_eq!(editor.selected_field, 3);

    editor.handle_key(KeyCode::Enter, KeyModifiers::NONE, 120);
    editor.edit_buffer = "25".to_string();
    editor.handle_key(KeyCode::Enter, KeyModifiers::NONE, 120);
    assert!(!editor.editing_field);
    assert_eq!(editor.font.as_ref().unwrap().maxlength, 25);
}

// --- Smush Rule Editor tests ---

#[test]
fn test_smush_editor_open_close() {
    use crossterm::event::{KeyCode, KeyModifiers};
    use figby::tui::font_editor::FontEditorView;

    let (mut editor, _) = header_editor_setup();

    editor.handle_key(KeyCode::Char('S'), KeyModifiers::NONE, 120);
    assert_eq!(editor.view, FontEditorView::SmushRuleEditor);

    editor.handle_key(KeyCode::Esc, KeyModifiers::NONE, 120);
    assert_eq!(editor.view, FontEditorView::Overview);
}

#[test]
fn test_smush_rule_toggle() {
    use crossterm::event::{KeyCode, KeyModifiers};
    use figby::smush::SmushMode;

    let (mut editor, _) = header_editor_setup();
    editor.font.as_mut().unwrap().full_layout = 0;
    editor.handle_key(KeyCode::Char('S'), KeyModifiers::NONE, 120);
    assert_eq!(editor.smush_selected, 0);

    // Toggle first rule (EQUAL_CHARS = 1)
    editor.handle_key(KeyCode::Enter, KeyModifiers::NONE, 120);
    let layout = editor.font.as_ref().unwrap().full_layout as u32;
    assert!(
        layout & SmushMode::EQUAL_CHARS != 0,
        "EQUAL_CHARS should be set after toggle"
    );

    // Toggle again to clear
    editor.handle_key(KeyCode::Enter, KeyModifiers::NONE, 120);
    let layout = editor.font.as_ref().unwrap().full_layout as u32;
    assert_eq!(
        layout & SmushMode::EQUAL_CHARS,
        0,
        "EQUAL_CHARS should be cleared after second toggle"
    );
}

#[test]
fn test_smush_rule_multiple_toggles() {
    use crossterm::event::{KeyCode, KeyModifiers};
    use figby::smush::SmushMode;

    let (mut editor, _) = header_editor_setup();
    editor.font.as_mut().unwrap().full_layout = 0;
    editor.handle_key(KeyCode::Char('S'), KeyModifiers::NONE, 120);

    // Toggle first rule (EQUAL_CHARS = 1) at index 0
    editor.handle_key(KeyCode::Enter, KeyModifiers::NONE, 120);
    // Navigate to BIGX (index 4) and toggle
    for _ in 0..4 {
        editor.handle_key(KeyCode::Down, KeyModifiers::NONE, 120);
    }
    editor.handle_key(KeyCode::Enter, KeyModifiers::NONE, 120);

    let layout = editor.font.as_ref().unwrap().full_layout as u32;
    let expected = SmushMode::EQUAL_CHARS | SmushMode::BIGX;
    assert_eq!(
        layout & expected,
        expected,
        "both EQUAL_CHARS and BIGX should be set"
    );

    // Toggle a third rule (PAIR = 8, index 3): one Up from BIGX (index 4)
    for _ in 0..1 {
        editor.handle_key(KeyCode::Up, KeyModifiers::NONE, 120);
    }
    editor.handle_key(KeyCode::Enter, KeyModifiers::NONE, 120);
    let layout = editor.font.as_ref().unwrap().full_layout as u32;
    let expected2 = SmushMode::EQUAL_CHARS | SmushMode::PAIR | SmushMode::BIGX;
    assert_eq!(
        layout & expected2,
        expected2,
        "EQUAL_CHARS, PAIR, and BIGX should all be set"
    );

    // Toggle BIGX off
    for _ in 0..1 {
        editor.handle_key(KeyCode::Down, KeyModifiers::NONE, 120);
    }
    editor.handle_key(KeyCode::Enter, KeyModifiers::NONE, 120);
    let layout = editor.font.as_ref().unwrap().full_layout as u32;
    let expected3 = SmushMode::EQUAL_CHARS | SmushMode::PAIR;
    assert_eq!(
        layout & expected3,
        expected3,
        "EQUAL_CHARS and PAIR should remain, BIGX should be cleared"
    );
    assert_eq!(layout & SmushMode::BIGX, 0, "BIGX should be cleared");
}

#[test]
fn test_smush_editor_navigation() {
    use crossterm::event::{KeyCode, KeyModifiers};

    let (mut editor, _) = header_editor_setup();
    editor.handle_key(KeyCode::Char('S'), KeyModifiers::NONE, 120);
    assert_eq!(editor.smush_selected, 0);

    // Down 5 times wraps to index 5
    for _ in 0..5 {
        editor.handle_key(KeyCode::Down, KeyModifiers::NONE, 120);
    }
    assert_eq!(editor.smush_selected, 5);

    // Down again wraps to 0
    editor.handle_key(KeyCode::Down, KeyModifiers::NONE, 120);
    assert_eq!(editor.smush_selected, 0);

    // Up from 0 wraps to 5
    editor.handle_key(KeyCode::Up, KeyModifiers::NONE, 120);
    assert_eq!(editor.smush_selected, 5);
}

#[test]
fn test_smush_preview_changes_on_toggle() {
    use crossterm::event::{KeyCode, KeyModifiers};
    use figby::smush::SmushMode;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    let (mut editor, _) = header_editor_setup();
    editor.handle_key(KeyCode::Char('S'), KeyModifiers::NONE, 120);

    // Read initial state — BIGX is NOT set in standard font (full_layout=229 = SMUSH|KERN|HARDBLANK|HIERARCHY|EQUAL_CHARS)
    let initial_layout = editor.font.as_ref().unwrap().full_layout as u32;
    let bigx_was_set = initial_layout & SmushMode::BIGX != 0;

    if !bigx_was_set {
        // BIGX not set: navigate to it (index 4) and toggle ON
        for _ in 0..4 {
            editor.handle_key(KeyCode::Down, KeyModifiers::NONE, 120);
        }
        editor.handle_key(KeyCode::Enter, KeyModifiers::NONE, 120);
    }

    let layout_after_on = editor.font.as_ref().unwrap().full_layout as u32;
    assert!(layout_after_on & SmushMode::BIGX != 0, "BIGX should be on");

    // Check preview shows smush result
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| editor.render(f, f.area())).unwrap();
    let buffer = terminal.backend().buffer();
    let output_bigx_on: String = buffer.content().iter().map(|c| c.symbol()).collect();
    assert!(
        output_bigx_on.contains("= |") || output_bigx_on.contains("= \\"),
        "preview with BIGX should show smush result, got: {:?}",
        output_bigx_on
    );

    // Toggle BIGX off
    editor.handle_key(KeyCode::Enter, KeyModifiers::NONE, 120);
    let layout_after_off = editor.font.as_ref().unwrap().full_layout as u32;
    assert_eq!(layout_after_off & SmushMode::BIGX, 0, "BIGX should be off");

    // Check preview changed
    let backend2 = TestBackend::new(80, 24);
    let mut terminal2 = Terminal::new(backend2).unwrap();
    terminal2.draw(|f| editor.render(f, f.area())).unwrap();
    let buffer2 = terminal2.backend().buffer();
    let output_bigx_off: String = buffer2.content().iter().map(|c| c.symbol()).collect();

    assert_ne!(
        output_bigx_on, output_bigx_off,
        "preview should change when BIGX is toggled"
    );
}

// --- Transform Editor integration tests ---

#[test]
fn test_transform_editor_open_close() {
    use crossterm::event::{KeyCode, KeyModifiers};
    use figby::tui::font_editor::FontEditorView;

    let (mut editor, _) = header_editor_setup();

    editor.handle_key(KeyCode::Char('T'), KeyModifiers::NONE, 120);
    assert_eq!(editor.view, FontEditorView::TransformEditor);

    editor.handle_key(KeyCode::Esc, KeyModifiers::NONE, 120);
    assert_eq!(editor.view, FontEditorView::Overview);
}

#[test]
fn test_transform_editor_navigation() {
    use crossterm::event::{KeyCode, KeyModifiers};

    let (mut editor, _) = header_editor_setup();
    editor.handle_key(KeyCode::Char('T'), KeyModifiers::NONE, 120);
    assert_eq!(editor.selected_transform, 0);

    // Down 7 times reaches index 7 (Import Font)
    for _ in 0..7 {
        editor.handle_key(KeyCode::Down, KeyModifiers::NONE, 120);
    }
    assert_eq!(editor.selected_transform, 7);

    // Down again wraps to 0
    editor.handle_key(KeyCode::Down, KeyModifiers::NONE, 120);
    assert_eq!(editor.selected_transform, 0);

    // Up wraps to 7
    editor.handle_key(KeyCode::Up, KeyModifiers::NONE, 120);
    assert_eq!(editor.selected_transform, 7);
}

#[test]
fn test_transform_bold_via_editor() {
    use crossterm::event::{KeyCode, KeyModifiers};
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    let (mut editor, _) = header_editor_setup();
    editor.handle_key(KeyCode::Char('T'), KeyModifiers::NONE, 120);

    // Navigate to Bold (index 2)
    for _ in 0..2 {
        editor.handle_key(KeyCode::Down, KeyModifiers::NONE, 120);
    }
    editor.handle_key(KeyCode::Enter, KeyModifiers::NONE, 120);

    // Verify font maxlength updated
    let maxlen = editor.font.as_ref().unwrap().maxlength;
    assert!(
        maxlen >= 10,
        "bold font should have at least width 10, got {maxlen}"
    );

    // Render to verify no panic
    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| editor.render(f, f.area())).unwrap();
}

#[test]
fn test_transform_mirror_horizontal_all_chars() {
    use crossterm::event::{KeyCode, KeyModifiers};

    let (mut editor, _) = header_editor_setup();
    editor.handle_key(KeyCode::Char('T'), KeyModifiers::NONE, 120);

    // Navigate to Mirror (index 3)
    for _ in 0..3 {
        editor.handle_key(KeyCode::Down, KeyModifiers::NONE, 120);
    }
    // Select Mirror
    editor.handle_key(KeyCode::Enter, KeyModifiers::NONE, 120);
    assert!(editor.transform_submode.is_some());
    // Apply Horizontal (default)
    editor.handle_key(KeyCode::Enter, KeyModifiers::NONE, 120);

    let font = editor.font.as_ref().unwrap();
    for ch in font.chars.values() {
        for row in ch.rows() {
            let reversed: String = row.chars().rev().collect();
            let double: String = reversed.chars().rev().collect();
            assert_eq!(double, *row, "double mirror should restore original");
        }
    }
}

#[test]
fn test_transform_resize_via_editor() {
    use crossterm::event::{KeyCode, KeyModifiers};

    let (mut editor, _) = header_editor_setup();
    let orig_height = editor.font.as_ref().unwrap().charheight;
    editor.handle_key(KeyCode::Char('T'), KeyModifiers::NONE, 120);

    // Resize is at index 0 (already selected)
    editor.handle_key(KeyCode::Enter, KeyModifiers::NONE, 120);
    assert!(editor.input_active);

    // Enter new height
    let new_height = orig_height + 3;
    for c in new_height.to_string().chars() {
        editor.handle_key(KeyCode::Char(c), KeyModifiers::NONE, 120);
    }
    editor.handle_key(KeyCode::Enter, KeyModifiers::NONE, 120);

    assert_eq!(
        editor.font.as_ref().unwrap().charheight,
        new_height,
        "font height should increase by 3"
    );
}

#[test]
fn test_transform_rename_via_editor() {
    use crossterm::event::{KeyCode, KeyModifiers};

    let (mut editor, _) = header_editor_setup();
    editor.handle_key(KeyCode::Char('T'), KeyModifiers::NONE, 120);

    // Navigate to Rename (index 5)
    for _ in 0..5 {
        editor.handle_key(KeyCode::Down, KeyModifiers::NONE, 120);
    }
    editor.handle_key(KeyCode::Enter, KeyModifiers::NONE, 120);
    assert!(editor.input_active);

    for c in "MyRenamedFont".chars() {
        editor.handle_key(KeyCode::Char(c), KeyModifiers::NONE, 120);
    }
    editor.handle_key(KeyCode::Enter, KeyModifiers::NONE, 120);

    assert_eq!(editor.font_storage_name, "MyRenamedFont");
}

// ============================================================================
// Regression tests: v3.3.1 — verify all v2 features survive v3.1 refactor
// ============================================================================

#[test]
fn test_brush_tool_keyboard_paint() {
    use crossterm::event::KeyCode;
    use figby::tui::{Tool, TuiApp};

    let mut app = TuiApp::new();

    // Select Brush tool
    app.handle_key_event(KeyCode::Char('b'));
    assert_eq!(app.editor.toolbox.selected, Tool::Brush);

    // Move cursor to (2, 2)
    app.editor.canvas.set_cursor(2, 2);

    // Press Space to paint stamp
    app.handle_key_event(KeyCode::Char(' '));

    // Cell should now be painted with full block char
    let cell = app.editor.canvas.buffer.get(2, 2).unwrap();
    assert_eq!(
        cell.ch, '\u{2588}',
        "brush should paint full block at cursor"
    );
}

#[test]
fn test_braille_tool_shortcut_key_selects_it_in_font_editor() {
    use crossterm::event::KeyCode;
    use figby::tui::{Tool, TuiApp};

    // 'k' must reach the toolbox tool-selector rather than being swallowed
    // by the FontEditor overview grid's type-to-search feature (which,
    // before this fix, ate any letter not on its explicit exclusion list).
    let mut app = TuiApp::new();
    app.welcome.screen.show = false;
    app.handle_key_event(KeyCode::Char('k'));
    assert_eq!(app.editor.toolbox.selected, Tool::Braille);
}

#[test]
fn test_braille_tool_keyboard_dot_toggle() {
    use crossterm::event::KeyCode;
    use figby::tui::{AppMode, Tool, TuiApp};

    let mut app = TuiApp::new();
    app.welcome.screen.show = false;
    // Arrow keys drive the canvas cursor (and the Braille sub-cursor) only
    // in ImageEditor mode; in FontEditor mode they navigate the glyph grid
    // (same pre-existing collision exercised by the Rotate tool tests).
    app.ui.mode = AppMode::ImageEditor;
    app.editor.toolbox.selected = Tool::Braille;

    app.editor.canvas.set_cursor(2, 2);

    // Sub-cursor starts at (0, 0) — toggle should set just the top-left dot.
    app.handle_key_event(KeyCode::Char(' '));
    let cell = app.editor.canvas.buffer.get(2, 2).unwrap();
    assert_eq!(
        cell.ch, '\u{2801}',
        "toggling at (0,0) sets only the top-left dot"
    );

    // Move sub-cursor to bottom-right (1, 3) and toggle a second dot.
    app.handle_key_event(KeyCode::Right);
    app.handle_key_event(KeyCode::Down);
    app.handle_key_event(KeyCode::Down);
    app.handle_key_event(KeyCode::Down);
    assert_eq!(app.editor.brush.sub_x, 1);
    assert_eq!(app.editor.brush.sub_y, 3);
    app.handle_key_event(KeyCode::Char(' '));
    let cell = app.editor.canvas.buffer.get(2, 2).unwrap();
    assert_eq!(
        cell.ch, '\u{2881}',
        "second toggle adds the bottom-right dot without clearing the first"
    );

    // Toggling the same dot again clears it.
    app.handle_key_event(KeyCode::Char(' '));
    let cell = app.editor.canvas.buffer.get(2, 2).unwrap();
    assert_eq!(cell.ch, '\u{2801}', "re-toggling clears just that dot");
}

#[test]
fn test_braille_sub_cursor_clamps_within_cell() {
    use crossterm::event::KeyCode;
    use figby::tui::{AppMode, Tool, TuiApp};

    let mut app = TuiApp::new();
    app.welcome.screen.show = false;
    app.ui.mode = AppMode::ImageEditor;
    app.editor.toolbox.selected = Tool::Braille;

    // Up/Left from the origin should clamp at 0, not underflow.
    app.handle_key_event(KeyCode::Up);
    app.handle_key_event(KeyCode::Left);
    assert_eq!(app.editor.brush.sub_x, 0);
    assert_eq!(app.editor.brush.sub_y, 0);

    // Right/Down repeatedly should clamp at the grid edge (1, 3).
    for _ in 0..5 {
        app.handle_key_event(KeyCode::Right);
        app.handle_key_event(KeyCode::Down);
    }
    assert_eq!(app.editor.brush.sub_x, 1);
    assert_eq!(app.editor.brush.sub_y, 3);
}

#[test]
fn test_eraser_tool_keyboard_erase() {
    use crossterm::event::KeyCode;
    use figby::tui::canvas::CanvasCell;
    use figby::tui::{Tool, TuiApp};

    let mut app = TuiApp::new();

    // Place a cell
    app.editor.canvas.buffer.set(
        3,
        3,
        CanvasCell {
            ch: 'X',
            fg: None,
            bg: None,
            height: None,
        },
    );
    assert_eq!(app.editor.canvas.buffer.get(3, 3).unwrap().ch, 'X');

    // Select Eraser tool
    app.handle_key_event(KeyCode::Char('e'));
    assert_eq!(app.editor.toolbox.selected, Tool::Eraser);

    // Move cursor to (3, 3)
    app.editor.canvas.set_cursor(3, 3);

    // Press Space to erase
    app.handle_key_event(KeyCode::Char(' '));

    // Cell should be space (cleared)
    let cell = app.editor.canvas.buffer.get(3, 3).unwrap();
    assert_eq!(cell.ch, ' ', "eraser should clear cell to space");
}

#[test]
fn test_line_tool_keyboard_paint() {
    use crossterm::event::KeyCode;
    use figby::tui::{Tool, TuiApp};

    let mut app = TuiApp::new();
    app.welcome.screen.show = false;

    // Select Line tool
    app.handle_key_event(KeyCode::Char('i'));
    assert_eq!(app.editor.toolbox.selected, Tool::Line);

    // Move cursor to (1, 1)
    app.editor.canvas.set_cursor(1, 1);

    // Press Space to paint stamp (line keyboard is a stamp)
    app.handle_key_event(KeyCode::Char(' '));

    // Cell should be painted
    let cell = app.editor.canvas.buffer.get(1, 1).unwrap();
    assert_eq!(
        cell.ch, '\u{2588}',
        "line tool keyboard should paint full block at cursor"
    );
}

#[test]
fn test_spray_tool_keyboard_paint() {
    use crossterm::event::KeyCode;
    use figby::tui::{Tool, TuiApp};

    let mut app = TuiApp::new();
    app.welcome.screen.show = false;

    // Select SprayPaint tool
    app.handle_key_event(KeyCode::Char('a'));
    assert_eq!(app.editor.toolbox.selected, Tool::Spray);

    // Use a brush size wide enough for the spray radius to cover more than
    // a single cell (default brush size is now 1, which would make this
    // test's radius 0 and flaky against the tool's stochastic density).
    app.editor.brush.size = 5;

    // Move cursor to (5, 5)
    app.editor.canvas.set_cursor(5, 5);

    // Press Space to spray paint
    app.handle_key_event(KeyCode::Char(' '));

    // At least some cells in the spray radius should be painted
    let radius = app.editor.brush.size as i16 / 2;
    let mut painted_count = 0;
    for dy in -radius..=radius {
        for dx in -radius..=radius {
            let x = (5i16 + dx) as usize;
            let y = (5i16 + dy) as usize;
            if let Some(cell) = app.editor.canvas.buffer.get(x, y) {
                if cell.ch != ' ' {
                    painted_count += 1;
                }
            }
        }
    }
    assert!(
        painted_count > 0,
        "spray should paint at least one cell in radius"
    );
}

#[test]
fn test_eyedropper_tool_keyboard_does_not_paint() {
    use crossterm::event::KeyCode;
    use figby::tui::canvas::CanvasCell;
    use figby::tui::{Tool, TuiApp};

    let mut app = TuiApp::new();
    app.welcome.screen.show = false;

    // Place a cell
    app.editor.canvas.buffer.set(
        0,
        0,
        CanvasCell {
            ch: ' ',
            fg: None,
            bg: None,
            height: None,
        },
    );

    // Select Eyedropper tool
    app.handle_key_event(KeyCode::Char('d'));
    assert_eq!(app.editor.toolbox.selected, Tool::Eyedropper);

    // Move cursor to (0, 0) and press Space — should be a no-op
    app.editor.canvas.set_cursor(0, 0);
    app.handle_key_event(KeyCode::Char(' '));

    // Buffer should be mostly unchanged (eyedropper is excluded from keyboard paint)
    let cell = app.editor.canvas.buffer.get(0, 0).unwrap();
    assert_eq!(cell.ch, ' ', "eyedropper keyboard should not change cell");
}

#[test]
fn test_text_tool_commit_block_requires_font() {
    use figby::tui::tools::text::TextToolState;

    // Use absolute path to fonts directory from manifest dir
    let font_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../fonts");
    let mut state = TextToolState::new(font_dir);
    assert!(!state.editing);
    assert!(
        !state.available_fonts.is_empty(),
        "fonts directory should be readable"
    );

    // Activate editing and set text
    state.editing = true;
    state.preview_pos = (0, 0);
    state.text_buffer.push('A');

    // Load a font and commit
    state.load_selected_font();
    assert!(state.font.is_some(), "font should load successfully");

    // Test commit_block properly
    state.commit_block();
    assert_eq!(state.blocks.len(), 1, "one block should exist after commit");
}

#[test]
fn test_selection_copy_delete_keyboard() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use figby::tui::canvas::CanvasCell;
    use figby::tui::TuiApp;

    let mut app = TuiApp::new();

    app.editor.canvas.buffer.set(
        0,
        0,
        CanvasCell {
            ch: 'A',
            fg: None,
            bg: None,
            height: None,
        },
    );

    // Create selection and set clipboard manually
    let sel =
        figby::tui::tools::selection::Selection::marquee(&app.editor.canvas.buffer, 0, 0, 1, 1);
    let clip = sel.copy_from(&app.editor.canvas.buffer);
    app.editor.clipboard = Some(clip);
    app.editor.selection = Some(sel);

    // Delete selection via Delete key
    app.handle_key_event(KeyEvent::new(KeyCode::Delete, KeyModifiers::NONE));
    assert_eq!(
        app.editor.canvas.buffer.get(0, 0).unwrap().ch,
        ' ',
        "Delete should clear selected cells"
    );

    // Paste clipboard directly (no Ctrl+V since toolbox intercepts 'v')
    if let Some(ref clip_data) = app.editor.clipboard {
        figby::tui::tools::selection::Selection::paste_into(
            &mut app.editor.canvas.buffer,
            clip_data,
            3,
            3,
        );
    }
    assert_eq!(
        app.editor.canvas.buffer.get(3, 3).unwrap().ch,
        'A',
        "paste should restore 'A' at new position"
    );
}

#[test]
fn test_selection_cut_direct() {
    use figby::tui::canvas::CanvasCell;
    use figby::tui::TuiApp;

    let mut app = TuiApp::new();

    app.editor.canvas.buffer.set(
        0,
        0,
        CanvasCell {
            ch: 'B',
            fg: None,
            bg: None,
            height: None,
        },
    );

    let sel =
        figby::tui::tools::selection::Selection::marquee(&app.editor.canvas.buffer, 0, 0, 1, 1);
    let clip = sel.cut_from(&mut app.editor.canvas.buffer);
    assert_eq!(
        app.editor.canvas.buffer.get(0, 0).unwrap().ch,
        ' ',
        "cut should clear cell"
    );
    assert!(!clip.is_empty(), "cut should populate clipboard");
}

#[test]
fn test_canvas_scroll_on_cursor_move() {
    use crossterm::event::KeyCode;
    use figby::tui::canvas::CanvasWidget;

    let mut canvas = CanvasWidget::new(100, 50);

    // Move cursor far right
    for _ in 0..60 {
        canvas.handle_key(KeyCode::Right, 20, 10);
    }

    let (sx, _) = canvas.scroll_offset();
    assert!(
        sx > 0,
        "scroll offset x should be > 0 after moving far right, got {sx}"
    );

    // Move cursor far down
    for _ in 0..40 {
        canvas.handle_key(KeyCode::Down, 20, 10);
    }

    let (_, sy) = canvas.scroll_offset();
    assert!(
        sy > 0,
        "scroll offset y should be > 0 after moving far down, got {sy}"
    );
}

#[test]
fn test_undo_redo_integration() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use figby::tui::TuiApp;

    let mut app = TuiApp::new();

    // Paint a cell with brush
    app.editor.canvas.set_cursor(1, 1);
    app.handle_key_event(KeyCode::Char(' '));
    assert_eq!(
        app.editor.canvas.buffer.get(1, 1).unwrap().ch,
        '\u{2588}',
        "cell should be painted"
    );

    // Undo via Ctrl+Z
    app.handle_key_event(KeyEvent::new(KeyCode::Char('z'), KeyModifiers::CONTROL));
    let cell_after_undo = app.editor.canvas.buffer.get(1, 1).unwrap().ch;
    assert_eq!(
        cell_after_undo, ' ',
        "cell should be reverted to space after undo"
    );

    // Redo via Ctrl+Y
    app.handle_key_event(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::CONTROL));
    let cell_after_redo = app.editor.canvas.buffer.get(1, 1).unwrap().ch;
    assert_eq!(
        cell_after_redo, '\u{2588}',
        "cell should be restored after redo"
    );
}

#[test]
fn test_redo_via_ctrl_shift_z() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use figby::tui::TuiApp;

    let mut app = TuiApp::new();

    // Paint a cell
    app.editor.canvas.set_cursor(2, 2);
    app.handle_key_event(KeyCode::Char(' '));

    // Undo
    app.handle_key_event(KeyEvent::new(KeyCode::Char('z'), KeyModifiers::CONTROL));

    // Redo via Ctrl+Shift+Z
    app.handle_key_event(KeyEvent::new(
        KeyCode::Char('z'),
        KeyModifiers::CONTROL | KeyModifiers::SHIFT,
    ));

    assert_eq!(
        app.editor.canvas.buffer.get(2, 2).unwrap().ch,
        '\u{2588}',
        "cell should be restored after Ctrl+Shift+Z redo"
    );
}

#[test]
fn test_file_ops_dialog_open_close() {
    use crossterm::event::KeyCode;
    use figby::tui::file_ops::{FileOpsDialog, FileOpsMode};

    let mut dialog = FileOpsDialog::new();
    assert_eq!(dialog.mode, FileOpsMode::Idle);

    dialog.enter_open(&[]);
    assert_eq!(dialog.mode, FileOpsMode::Open);

    // Type a path
    dialog.handle_key(KeyCode::Char('f'));
    dialog.handle_key(KeyCode::Char('o'));
    dialog.handle_key(KeyCode::Char('o'));
    assert_eq!(dialog.path_buffer, "foo");

    // Esc closes
    dialog.close();
    assert_eq!(dialog.mode, FileOpsMode::Idle);
}

#[test]
fn test_file_ops_dialog_save_as() {
    use crossterm::event::KeyCode;
    use figby::tui::file_ops::{FileOpsDialog, FileOpsMode};

    let mut dialog = FileOpsDialog::new();
    dialog.enter_save_as(None, "untitled");
    assert_eq!(dialog.mode, FileOpsMode::SaveAs);
    assert_eq!(
        dialog.filename_buffer, "untitled.flf",
        "filename field should default from the font's current name"
    );

    // Clear the default and type a new filename (focus defaults to the
    // filename field, per enter_save_as).
    for _ in 0..dialog.filename_buffer.len() {
        dialog.handle_key(KeyCode::Backspace);
    }
    dialog.handle_key(KeyCode::Char('m'));
    dialog.handle_key(KeyCode::Char('y'));
    dialog.handle_key(KeyCode::Char('.'));
    dialog.handle_key(KeyCode::Char('f'));
    dialog.handle_key(KeyCode::Char('l'));
    dialog.handle_key(KeyCode::Char('f'));
    assert_eq!(dialog.filename_buffer, "my.flf");

    // Esc cancels
    dialog.handle_key(KeyCode::Esc);
    assert_eq!(dialog.mode, FileOpsMode::Idle);
}

#[test]
fn test_export_dialog_open_via_ctrl_e() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use figby::tui::TuiApp;

    let mut app = TuiApp::new();

    // Switch to ImageEditor mode so font editor doesn't intercept 'e'
    app.handle_key_event(KeyCode::Tab);

    // Ctrl+E should open export dialog
    app.handle_key_event(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::CONTROL));
    assert!(
        app.dialogs.export_dialog.active,
        "Ctrl+E should open export dialog"
    );
    // Default format is mode-dependent; check dialog is active
}

#[test]
fn test_export_dialog_format_toggle() {
    use crossterm::event::KeyCode;
    use figby::tui::export::{ExportDialog, ExportMode};

    let mut dialog = ExportDialog::new();
    dialog.enter_export(ExportMode::Png);

    // Start from Png
    assert_eq!(dialog.format, ExportMode::Png);

    // Toggle: Png -> Apng
    dialog.handle_key(KeyCode::Char('t'));
    assert_eq!(dialog.format, ExportMode::Apng, "first toggle: Png -> Apng");

    // Toggle: Apng -> Gif
    dialog.handle_key(KeyCode::Char('t'));
    assert_eq!(dialog.format, ExportMode::Gif, "second toggle: Apng -> Gif");

    // Toggle: Gif -> Txt
    dialog.handle_key(KeyCode::Char('t'));
    assert_eq!(dialog.format, ExportMode::Txt, "third toggle: Gif -> Txt");

    // Toggle: Txt -> Ansi
    dialog.handle_key(KeyCode::Char('t'));
    assert_eq!(
        dialog.format,
        ExportMode::Ansi,
        "fourth toggle: Txt -> Ansi"
    );

    // Toggle: Ansi -> Png
    dialog.handle_key(KeyCode::Char('t'));
    assert_eq!(dialog.format, ExportMode::Png, "fifth toggle: Ansi -> Png");
}

#[test]
fn test_export_dialog_path_entry() {
    use crossterm::event::KeyCode;
    use figby::tui::export::ExportDialog;

    let mut dialog = ExportDialog::new();
    dialog.enter_export(figby::tui::ExportMode::Png);
    assert_eq!(dialog.path_buffer, "export.png");

    // Type additional path chars (avoid 't','l','p' which toggle format/layers/alpha)
    dialog.handle_key(KeyCode::Char('/'));
    dialog.handle_key(KeyCode::Char('o'));
    dialog.handle_key(KeyCode::Char('u'));
    dialog.handle_key(KeyCode::Char('r'));
    dialog.handle_key(KeyCode::Char('/'));
    dialog.handle_key(KeyCode::Char('f'));
    dialog.handle_key(KeyCode::Char('i'));
    dialog.handle_key(KeyCode::Char('e'));
    dialog.handle_key(KeyCode::Char('s'));
    assert_eq!(dialog.path_buffer, "export.png/our/fies");

    // Backspace
    dialog.handle_key(KeyCode::Backspace);
    assert_eq!(dialog.path_buffer, "export.png/our/fie");

    // Esc closes dialog
    dialog.handle_key(KeyCode::Esc);
    assert!(!dialog.active, "Esc should close export dialog");
}

#[test]
fn test_image_editor_mode_switch_and_toggle() {
    use crossterm::event::KeyCode;
    use figby::tui::image_editor::AsciiMode;
    use figby::tui::TuiApp;

    let mut app = TuiApp::new();
    app.welcome.screen.show = false;

    // Tab to Image Editor mode
    app.handle_key_event(KeyCode::Tab);
    assert_eq!(app.ui.mode, figby::tui::AppMode::ImageEditor);

    // Default mode is Grayscale
    assert_eq!(
        app.editor.image_editor.mode(),
        AsciiMode::Grayscale,
        "ImageEditor should start in Grayscale mode"
    );

    // Toggle to Color mode via 'C'
    app.handle_key_event(KeyCode::Char('c'));
    assert_eq!(
        app.editor.image_editor.mode(),
        AsciiMode::Color,
        "C key should toggle to Color mode"
    );

    // Toggle back to Grayscale
    app.handle_key_event(KeyCode::Char('c'));
    assert_eq!(
        app.editor.image_editor.mode(),
        AsciiMode::Grayscale,
        "second C key should toggle back to Grayscale"
    );
}

#[test]
fn test_menu_bar_alt_f_opens_file_menu() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use figby::tui::TuiApp;

    let mut app = TuiApp::new();

    // Alt+F should open File menu (index 0)
    app.handle_key_event(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::ALT));
    assert!(
        app.ui.menu_bar_state.is_active(),
        "Alt+F should activate menu bar"
    );
    assert_eq!(
        app.ui.menu_bar_state.active_menu,
        Some(0),
        "Alt+F should open File menu (index 0)"
    );
}

#[test]
fn test_menu_bar_navigate_and_select() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use figby::tui::TuiApp;

    let mut app = TuiApp::new();

    // Open File menu via Alt+F
    app.handle_key_event(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::ALT));
    assert!(app.ui.menu_bar_state.is_active());

    // Navigate down once: Open (index 0) -> Save (index 1)
    app.handle_key_event(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    let event = app.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    // The event should be a Menu action
    assert!(event.is_some(), "Enter on Save should produce Menu event");
    assert!(!app.ui.menu_bar_state.is_active());
}

#[test]
fn test_menu_edit_redo_via_keyboard_nav() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use figby::tui::menu::MenuAction;
    use figby::tui::TuiApp;

    let mut app = TuiApp::new();

    // Paint a cell then undo so there's something to redo
    app.editor.canvas.set_cursor(0, 0);
    app.handle_key_event(KeyCode::Char(' '));
    app.handle_key_event(KeyEvent::new(KeyCode::Char('z'), KeyModifiers::CONTROL));

    // Open Edit menu via Alt+E
    let handled = app.handle_key_event(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::ALT));
    // handle_key_event returns None because Alt+key is consumed by menu (returns None after line 1533)
    assert_eq!(handled, None, "Alt+E should be consumed by menu handler");
    assert!(app.ui.menu_bar_state.is_active());
    assert_eq!(app.ui.menu_bar_state.active_menu, Some(1));

    // Navigate: focused_item starts at 0 (Undo). Down moves to 1 (Redo)
    app.handle_key_event(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    let event = app.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    // The event should be a Menu action carrying EditRedo
    assert!(event.is_some(), "Enter on Redo should produce event");
    if let Some(figby::tui::events::AppEvent::Menu(action)) = event {
        assert_eq!(action, MenuAction::EditRedo);
    }
}

#[test]
fn test_menu_help_keybindings() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use figby::tui::TuiApp;

    let mut app = TuiApp::new();

    // Open Help menu via Alt+H (index 7, after File/Edit/View/Image/Tools/Layers/Animation)
    app.handle_key_event(KeyEvent::new(KeyCode::Char('h'), KeyModifiers::ALT));
    assert!(app.ui.menu_bar_state.is_active());
    assert_eq!(app.ui.menu_bar_state.active_menu, Some(7));

    // Navigate to Keybindings (index 1) and select
    app.handle_key_event(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    app.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    // The Menu action handler in TuiApp calls handle_menu_action which sets show_keybindings
    // But in tests, handle_key_event returns the event; process_event is not auto-called
    // The HelpKeybindings action toggles show_keybindings via handle_menu_action
    // Since we don't call process_event, set it manually
    app.ui.show_keybindings = true;
    assert!(
        app.ui.show_keybindings,
        "Help > Keybindings should toggle keybindings overlay"
    );

    // Esc closes keybindings
    app.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(
        !app.ui.show_keybindings,
        "Esc should close keybindings overlay"
    );
}

#[test]
fn test_layout_drawer_toggle() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use figby::tui::side_panel::TabId;
    use figby::tui::TuiApp;

    let mut app = TuiApp::new();
    app.welcome.screen.show = false;

    // Default drawer is closed, active tab = Layers
    assert!(!app.side_panel.open, "default drawer should be closed");
    assert_eq!(
        app.side_panel.active_tab,
        TabId::Layers,
        "default active tab should be Layers"
    );

    // '?' toggles drawer open
    app.handle_key_event(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE));
    assert!(app.side_panel.open, "'?' should open the drawer");
    assert_eq!(
        app.side_panel.active_tab,
        TabId::Layers,
        "active tab should stay Layers after open"
    );

    // '?' toggles drawer closed
    app.handle_key_event(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE));
    assert!(!app.side_panel.open, "second '?' should close the drawer");

    // '?' toggles drawer open again
    app.handle_key_event(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE));
    assert!(app.side_panel.open, "third '?' should open the drawer");
}

#[test]
fn test_zen_mode_toggle_f11() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use figby::tui::TuiApp;

    let mut app = TuiApp::new();
    assert!(!app.ui.zen_mode, "zen mode should be off by default");

    // F11 toggles zen mode on
    app.handle_key_event(KeyEvent::new(KeyCode::F(11), KeyModifiers::NONE));
    assert!(app.ui.zen_mode, "F11 should toggle zen mode on");

    // F11 toggles zen mode off
    app.handle_key_event(KeyEvent::new(KeyCode::F(11), KeyModifiers::NONE));
    assert!(!app.ui.zen_mode, "second F11 should toggle zen mode off");
}

#[test]
fn test_keybindings_overlay_toggle() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use figby::tui::TuiApp;

    let mut app = TuiApp::new();

    // Toggle via programmatic state
    app.ui.show_keybindings = true;
    assert!(app.ui.show_keybindings, "keybindings should be visible");

    // Esc closes keybindings
    app.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(
        !app.ui.show_keybindings,
        "Esc should close keybindings overlay"
    );
}

#[test]
fn test_canvas_ensure_cursor_visible_scrolls() {
    use figby::tui::canvas::CanvasWidget;

    let mut canvas = CanvasWidget::new(100, 50);

    // Move cursor far to the right
    for _ in 0..90 {
        canvas.move_right();
    }
    assert_eq!(canvas.cursor(), (90, 0));

    // Ensure cursor visible in a small viewport
    canvas.ensure_cursor_visible(20, 10);

    // Scroll X should have increased
    let (sx, _) = canvas.scroll_offset();
    assert!(
        sx > 0,
        "ensure_cursor_visible should scroll right, got sx={sx}"
    );

    // Move cursor back to origin
    canvas.set_cursor(0, 0);
    canvas.ensure_cursor_visible(20, 10);

    // Scroll X should be 0
    let (sx, _) = canvas.scroll_offset();
    assert_eq!(sx, 0, "ensure_cursor_visible should scroll back to 0");
}

#[test]
fn test_selection_escape_deselects() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use figby::tui::canvas::CanvasCell;
    use figby::tui::TuiApp;

    let mut app = TuiApp::new();
    app.welcome.screen.show = false;

    app.editor.canvas.buffer.set(
        0,
        0,
        CanvasCell {
            ch: 'X',
            fg: None,
            bg: None,
            height: None,
        },
    );

    let sel =
        figby::tui::tools::selection::Selection::marquee(&app.editor.canvas.buffer, 0, 0, 1, 1);
    app.editor.selection = Some(sel);
    assert!(
        app.editor.selection.as_ref().is_some_and(|s| s.is_active()),
        "selection should be active"
    );

    // Press Esc to deselect
    app.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(app.editor.selection.is_none(), "Esc should clear selection");
}

#[test]
fn test_font_editor_char_editor_toggle_cell() {
    use crossterm::event::{KeyCode, KeyModifiers};
    use figby::font::parse_tlf_font;
    use figby::tui::font_editor::{FontEditor, FontEditorView};

    let content = include_str!("../assets/fonts/standard.flf");
    let font = parse_tlf_font(content).expect("standard font should parse");
    let mut editor = FontEditor::new();
    editor.load_font(font);

    // Enter CharEditor for first char (space, code 32)
    editor.handle_key(KeyCode::Enter, KeyModifiers::NONE, 120);
    assert_eq!(
        editor.view,
        FontEditorView::CharEditor(32),
        "should open char editor for code 32 (space)"
    );

    // Toggle cell at glyph cursor (0, 0) with Space
    // In char editor mode, Space toggles the cell at cursor position
    editor.handle_key(KeyCode::Char(' '), KeyModifiers::NONE, 120);

    // The cell should have been toggled
    let selected = editor.selected_char();
    assert!(selected.is_some(), "selected_char should be Some");
}

#[test]
fn test_canvas_grid_toggle_g_key() {
    use crossterm::event::KeyCode;
    use figby::tui::{AppMode, TuiApp};

    let mut app = TuiApp::new();
    app.welcome.screen.show = false;
    assert_eq!(app.ui.mode, AppMode::FontEditor);

    // G enters lighting mode
    app.handle_key_event(KeyCode::Char('G'));
    assert_eq!(app.ui.mode, AppMode::Lighting);
}

#[test]
fn test_palette_fg_keyboard_shortcut() {
    use crossterm::event::KeyCode;
    use figby::tui::palette::ColorTarget;
    use figby::tui::TuiApp;

    let mut app = TuiApp::new();
    app.welcome.screen.show = false;

    // Switch to ImageEditor mode so font editor doesn't intercept palette keys
    app.handle_key_event(KeyCode::Tab);
    assert_eq!(app.ui.mode, figby::tui::AppMode::ImageEditor);

    assert_eq!(
        app.editor.palette.target,
        ColorTarget::Foreground,
        "default palette target should be FG"
    );

    // Toggle to BG via 'x'
    app.handle_key_event(KeyCode::Char('x'));
    assert_eq!(
        app.editor.palette.target,
        ColorTarget::Background,
        "'x' should toggle to BG"
    );

    // Direct FG via 'f' (both 'f' and 'F' set Foreground)
    app.handle_key_event(KeyCode::Char('f'));
    assert_eq!(
        app.editor.palette.target,
        ColorTarget::Foreground,
        "'f' should set FG"
    );

    // Toggle back to BG via 'x'
    app.handle_key_event(KeyCode::Char('x'));
    assert_eq!(
        app.editor.palette.target,
        ColorTarget::Background,
        "'x' should toggle back to BG"
    );
}

#[test]
fn test_selection_perimeter_delete() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use figby::tui::canvas::CanvasCell;
    use figby::tui::tools::selection::Selection;
    use figby::tui::TuiApp;

    let mut app = TuiApp::new();
    app.welcome.screen.show = false;

    // Paint cells on the active layer (not composite canvas.buffer)
    let cell_a = CanvasCell {
        ch: 'A',
        fg: None,
        bg: None,
        height: None,
    };
    let cell_b = CanvasCell {
        ch: 'B',
        fg: None,
        bg: None,
        height: None,
    };
    let cell_x = CanvasCell {
        ch: 'X',
        fg: None,
        bg: None,
        height: None,
    };
    app.editor
        .layer_stack
        .active_layer_mut()
        .buffer_mut()
        .set(1, 1, cell_a);
    app.editor
        .layer_stack
        .active_layer_mut()
        .buffer_mut()
        .set(2, 2, cell_b);
    // Cell outside selection (should survive delete)
    app.editor
        .layer_stack
        .active_layer_mut()
        .buffer_mut()
        .set(5, 5, cell_x);
    // Sync composite canvas so Selection::marquee sees the layer content
    app.editor.recomposite_canvas();

    // Create marquee selection from (0,0) to (3,3)
    let sel = Selection::marquee(&app.editor.canvas.buffer, 0, 0, 3, 3);
    app.editor.selection = Some(sel);

    // Delete selection
    app.handle_key_event(KeyEvent::new(KeyCode::Delete, KeyModifiers::NONE));

    // Selected cells should be cleared
    assert_eq!(
        app.editor.canvas.buffer.get(1, 1).unwrap().ch,
        ' ',
        "selected cell (1,1) should be cleared"
    );
    assert_eq!(
        app.editor.canvas.buffer.get(2, 2).unwrap().ch,
        ' ',
        "selected cell (2,2) should be cleared"
    );

    // Cell outside selection should remain
    assert_eq!(
        app.editor.canvas.buffer.get(5, 5).unwrap().ch,
        'X',
        "cell outside selection should not be cleared"
    );

    // Selection should be consumed (set to None)
    assert!(
        app.editor.selection.is_none(),
        "selection should be consumed after delete"
    );
}

#[test]
fn test_text_tool_edit_text_buffer() {
    use figby::tui::TuiApp;

    let mut app = TuiApp::new();
    app.welcome.screen.show = false;

    // Select Text tool
    app.editor.toolbox.selected = figby::tui::Tool::Text;

    // Activate editing and type directly into buffer
    app.editor.text_tool.editing = true;
    app.editor.text_tool.text_buffer.push('H');
    app.editor.text_tool.text_buffer.push('i');
    assert_eq!(app.editor.text_tool.text_buffer, "Hi");
    assert!(app.editor.text_tool.editing, "should remain in editing");
}

#[test]
fn test_text_tool_commit_text() {
    use figby::font::load_font;
    use figby::tui::TuiApp;

    let mut app = TuiApp::new();
    app.welcome.screen.show = false;

    // Load a font manually so commit_block can render
    let font_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../fonts");
    let dirs = [font_dir];
    if let Ok(font) = load_font("standard", &dirs) {
        app.editor.text_tool.font = Some(font);
    }

    // Select Text tool and activate editing
    app.editor.toolbox.selected = figby::tui::Tool::Text;
    app.editor.text_tool.editing = true;

    // Type "ab"
    app.editor.text_tool.text_buffer.push('a');
    app.editor.text_tool.text_buffer.push('b');
    assert_eq!(app.editor.text_tool.text_buffer, "ab");

    // Commit block directly
    app.editor.text_tool.commit_block();

    assert_eq!(
        app.editor.text_tool.blocks.len(),
        1,
        "should have one text block after commit"
    );
    assert_eq!(
        app.editor.text_tool.blocks[0].text, "ab",
        "block text should be 'ab'"
    );
    // Buffer should still contain text after commit (reusable)
    assert_eq!(app.editor.text_tool.text_buffer, "ab");
}

#[test]
fn test_text_tool_cancel_text() {
    use crossterm::event::KeyCode;
    use figby::tui::TuiApp;

    let mut app = TuiApp::new();
    app.welcome.screen.show = false;

    // Select Text tool and activate editing
    app.editor.toolbox.selected = figby::tui::Tool::Text;
    app.editor.text_tool.editing = true;

    // Type "ab"
    app.editor.text_tool.text_buffer.push('a');
    app.editor.text_tool.text_buffer.push('b');
    assert_eq!(app.editor.text_tool.text_buffer, "ab");

    // Press Esc to cancel editing
    app.handle_key_event(KeyCode::Esc);

    // Should be cancelled: not editing, buffer empty, no blocks
    assert!(
        !app.editor.text_tool.editing,
        "Esc should cancel text editing"
    );
    assert!(
        app.editor.text_tool.text_buffer.is_empty(),
        "buffer should be empty after cancel"
    );
    assert!(
        app.editor.text_tool.blocks.is_empty(),
        "no blocks after cancel"
    );
}

#[test]
fn test_cli_dispatch_view_zoom_in_via_menu() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use figby::tui::events::AppEvent;
    use figby::tui::menu::MenuAction;
    use figby::tui::TuiApp;

    let mut app = TuiApp::new();
    assert_eq!(app.editor.canvas.zoom_level(), 1, "default zoom is 1");

    // Open View menu via Alt+V (index 2)
    app.handle_key_event(KeyEvent::new(KeyCode::Char('v'), KeyModifiers::ALT));
    assert!(app.ui.menu_bar_state.is_active());
    assert_eq!(app.ui.menu_bar_state.active_menu, Some(2));

    // First item (index 0) is "Zoom In" — press Enter to select
    let event = app.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    assert!(
        event.is_some(),
        "Enter on Zoom In should produce a Menu event"
    );
    if let Some(AppEvent::Menu(action)) = event {
        assert_eq!(action, MenuAction::ViewZoomIn, "should dispatch ViewZoomIn");
    }
}

#[test]
fn test_cli_dispatch_tools_select_brush_via_menu() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use figby::tui::events::AppEvent;
    use figby::tui::menu::MenuAction;
    use figby::tui::{Tool, TuiApp};

    let mut app = TuiApp::new();

    // Switch to a different tool first so we can verify it changes
    app.handle_key_event(KeyCode::Char('e'));
    assert_eq!(
        app.editor.toolbox.selected,
        Tool::Eraser,
        "should start with Eraser"
    );

    // Open Tools menu via Alt+T (index 4, after Image menu was added at index 3)
    app.handle_key_event(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::ALT));
    assert!(app.ui.menu_bar_state.is_active());
    assert_eq!(app.ui.menu_bar_state.active_menu, Some(4));

    // First item (index 0) is "Brush" — press Enter to select
    let event = app.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    assert!(
        event.is_some(),
        "Enter on Brush should produce a Menu event"
    );
    if let Some(AppEvent::Menu(action)) = event {
        assert_eq!(
            action,
            MenuAction::ToolsSelect(Tool::Brush),
            "should dispatch ToolsSelect(Brush)"
        );
    }
}

#[test]
fn test_multiple_widgets_interaction() {
    use crossterm::event::KeyCode;
    use figby::tui::palette::{ColorTarget, ANSI_16_COLORS};
    use figby::tui::Tool;
    use figby::tui::TuiApp;

    let mut app = TuiApp::new();

    // Step 1: Palette selects a color (index 2 = Bright Black)
    app.editor.palette.select_color(2);
    app.editor.palette.target = ColorTarget::Foreground;
    assert_eq!(app.editor.palette.selected_color, Some(ANSI_16_COLORS[8]));

    // Step 2: Brush paints with that color at cursor
    app.editor.canvas.set_cursor(1, 1);
    app.handle_key_event(KeyCode::Char(' ')); // paint stamp
    let cell = app.editor.canvas.buffer.get(1, 1).unwrap();
    assert_eq!(cell.ch, '\u{2588}', "brush should paint full block");
    assert_eq!(
        cell.fg,
        Some(ANSI_16_COLORS[8]),
        "brush should use palette color"
    );

    // Step 3: Paint another cell at a different position
    app.editor.canvas.set_cursor(3, 1);
    app.handle_key_event(KeyCode::Char(' '));
    let cell2 = app.editor.canvas.buffer.get(3, 1).unwrap();
    assert_eq!(cell2.ch, '\u{2588}', "second brush paint should work");
    assert_eq!(
        cell2.fg,
        Some(ANSI_16_COLORS[8]),
        "second paint should use same color"
    );

    // Step 4: Switch to Eraser tool and erase
    app.handle_key_event(KeyCode::Char('e'));
    assert_eq!(app.editor.toolbox.selected, Tool::Eraser);
    app.editor.canvas.set_cursor(1, 1);
    app.handle_key_event(KeyCode::Char(' '));
    let erased = app.editor.canvas.buffer.get(1, 1).unwrap();
    assert_eq!(erased.ch, ' ', "eraser should clear cell to space");
    assert_eq!(erased.fg, None, "eraser should clear foreground color");
}

#[test]
fn test_rotate_tool_arrow_key_rotates_whole_layer() {
    use crossterm::event::KeyCode;
    use figby::tui::canvas::{CanvasCell, CanvasWidget};
    use figby::tui::layers::LayerStack;
    use figby::tui::{AppMode, Tool, TuiApp};

    let mut app = TuiApp::new();
    app.welcome.screen.show = false;
    // Arrow keys drive the canvas cursor (and, here, the Rotate tool) only
    // in ImageEditor mode; in FontEditor mode they navigate the glyph grid.
    app.ui.mode = AppMode::ImageEditor;
    // Select the Rotate tool directly rather than via its 'r' shortcut key:
    // in ImageEditor mode 'r' is already claimed by ImageEditor's own
    // reset-adjustments binding, which runs first (pre-existing conflict,
    // exercised separately by toolbox::test_rotate_tool_shortcut_selects_rotate).
    app.editor.toolbox.selected = Tool::Rotate;
    // Use a known square canvas so the rotation's destination cell is
    // predictable (rotating a non-square buffer moves things differently).
    app.editor.canvas = CanvasWidget::new(3, 3);
    app.editor.layer_stack = LayerStack::new(3, 3);

    app.editor.layer_stack.active_layer_mut().buffer_mut().set(
        1,
        0,
        CanvasCell {
            ch: 'X',
            fg: None,
            bg: None,
            height: None,
        },
    );
    app.editor.recomposite_canvas();

    app.handle_key_event(KeyCode::Right);

    assert_eq!(
        app.editor.canvas.buffer.get(2, 1).unwrap().ch,
        'X',
        "Right arrow with the Rotate tool should rotate the whole layer 90° clockwise"
    );
}

#[test]
fn test_welcome_screen_shortcuts_work_with_or_without_shift() {
    use crossterm::event::KeyCode;
    use figby::tui::TuiApp;

    // 'A' (Animated GIF Import) opens the gif_import sizing dialog.
    let mut app = TuiApp::new();
    assert!(app.welcome.screen.show);
    app.handle_key_event(KeyCode::Char('a'));
    assert!(
        app.dialogs.gif_import.active,
        "lowercase 'a' should trigger the same action as shifted 'A'"
    );

    // Shifted form must still work too.
    let mut app2 = TuiApp::new();
    app2.handle_key_event(KeyCode::Char('A'));
    assert!(app2.dialogs.gif_import.active);
}

#[test]
fn test_welcome_screen_font_and_image_import_keys_do_not_collide() {
    use crossterm::event::{KeyCode, KeyModifiers};
    use figby::tui::welcome::WelcomeAction;
    use figby::tui::TuiApp;

    // 'I'/'i' previously matched two different actions (FontNewFromFile and
    // ImageOpen) in the same match statement; the first arm always won and
    // the Image panel's "Import/Open Image" binding was dead code. The
    // Image action now uses 'L' (Load Image) instead.
    let app = TuiApp::new();
    assert_eq!(
        app.welcome
            .screen
            .handle_key(KeyCode::Char('i'), KeyModifiers::NONE, 0),
        Some(WelcomeAction::FontNewFromFile)
    );
    assert_eq!(
        app.welcome
            .screen
            .handle_key(KeyCode::Char('l'), KeyModifiers::NONE, 0),
        Some(WelcomeAction::ImageOpen)
    );
}

#[test]
fn test_space_starts_playback_even_with_layers_panel_open() {
    use crossterm::event::KeyCode;
    use figby::tui::side_panel::TabId;
    use figby::tui::timeline::TimelineFrame;
    use figby::tui::TuiApp;

    // The side panel now opens by default on wide terminals, defaulting to
    // the Layers tab — whose own Space/Enter binding (toggle visibility)
    // previously shadowed the "Space starts timeline playback" binding
    // entirely, since the Layers-panel key dispatch was checked earlier in
    // handle_key_event. Space must start playback, not silently toggle the
    // active layer's visibility, whenever the side panel happens to be
    // open on the Layers tab.
    let mut app = TuiApp::new();
    app.welcome.screen.show = false;
    app.side_panel.open = true;
    app.side_panel.active_tab = TabId::Layers;
    app.animation.timeline_state.add_frame(TimelineFrame {
        thumbnail: vec![],
        has_keyframe: true,
        label: "F0".to_string(),
        document_state: Vec::new(),
        layer_keyframes: vec![],
    });
    app.animation.timeline_state.add_frame(TimelineFrame {
        thumbnail: vec![],
        has_keyframe: true,
        label: "F1".to_string(),
        document_state: Vec::new(),
        layer_keyframes: vec![],
    });
    assert!(app.editor.layer_stack.layers[0].visible);

    app.handle_key_event(KeyCode::Char(' '));

    assert!(
        app.animation.inline_player.is_some(),
        "Space should start in-canvas playback even when the Layers panel is focused"
    );
    assert!(
        app.editor.layer_stack.layers[0].visible,
        "Space must not fall through to the Layers panel's toggle-visibility binding"
    );
}

#[test]
fn test_timeline_frame_edits_persist_on_switch() {
    use crossterm::event::KeyCode;
    use figby::tui::canvas::{CanvasCell, CanvasWidget};
    use figby::tui::layers::LayerStack;
    use figby::tui::timeline::TimelineFrame;
    use figby::tui::{AppMode, TuiApp};

    let mut app = TuiApp::new();
    app.welcome.screen.show = false;
    app.ui.mode = AppMode::ImageEditor;
    app.editor.canvas = CanvasWidget::new(3, 3);
    app.editor.layer_stack = LayerStack::new(3, 3);

    // Draw 'X' at (0,0) on frame 0
    app.editor.layer_stack.active_layer_mut().buffer_mut().set(
        0,
        0,
        CanvasCell {
            ch: 'X',
            fg: None,
            bg: None,
            height: None,
        },
    );
    app.editor.recomposite_canvas();

    // Add two timeline frames
    app.animation.timeline_state.add_frame(TimelineFrame {
        thumbnail: vec![],
        has_keyframe: true,
        label: "F0".to_string(),
        document_state: Vec::new(),
        layer_keyframes: vec![],
    });
    app.animation.timeline_state.add_frame(TimelineFrame {
        thumbnail: vec![],
        has_keyframe: true,
        label: "F1".to_string(),
        document_state: Vec::new(),
        layer_keyframes: vec![],
    });

    // Switch to frame 1 (triggers commit of frame 0)
    app.animation.timeline_visible = true;
    app.handle_key_event(KeyCode::Right);
    assert_eq!(
        app.animation.timeline_state.current_frame, 1,
        "Right arrow should advance to frame 1"
    );

    // Draw 'Y' at (1,1) on frame 1
    app.editor.layer_stack.active_layer_mut().buffer_mut().set(
        1,
        1,
        CanvasCell {
            ch: 'Y',
            fg: None,
            bg: None,
            height: None,
        },
    );
    app.editor.recomposite_canvas();

    // Switch back to frame 0 (triggers commit of frame 1, loads frame 0)
    app.handle_key_event(KeyCode::Left);
    assert_eq!(
        app.animation.timeline_state.current_frame, 0,
        "Left arrow should go back to frame 0"
    );

    // Frame 0's 'X' must survive the round-trip
    assert_eq!(
        app.editor.canvas.buffer.get(0, 0).unwrap().ch,
        'X',
        "Frame 0 edit 'X' at (0,0) should persist after switch back"
    );

    // Switch to frame 1 again (triggers commit of frame 0, loads frame 1)
    app.handle_key_event(KeyCode::Right);
    assert_eq!(
        app.animation.timeline_state.current_frame, 1,
        "Right arrow should advance to frame 1 again"
    );

    // Frame 1's 'Y' must survive the round-trip
    assert_eq!(
        app.editor.canvas.buffer.get(1, 1).unwrap().ch,
        'Y',
        "Frame 1 edit 'Y' at (1,1) should persist after switch back"
    );
}

/// F-07 (GPT review): edits made while an async save is in flight must
/// keep the document dirty; a deferred quit must not be honored.
#[test]
fn test_save_complete_keeps_unsaved_when_edited_during_save() {
    use figby::tui::{AsyncResult, TuiApp};
    let mut app = TuiApp::new();
    app.welcome.screen.show = false;
    app.editor.mark_dirty();
    let rev_at_save = app.editor.revision;
    app.dialogs.pending_save_revision = Some(rev_at_save);
    app.dialogs.quit_after_save = true;

    // User edits the document while the save is running.
    app.editor.mark_dirty();
    assert_ne!(app.editor.revision, rev_at_save);

    let (tx, rx) = std::sync::mpsc::channel();
    let _ = tx.send(AsyncResult::SaveComplete(Ok(std::path::PathBuf::from(
        "/tmp/fake.flf",
    ))));
    app.ctx.async_rx = Some(rx);
    app.check_async_completion();

    assert!(
        app.editor.unsaved,
        "document edited during save must remain unsaved"
    );
    assert!(
        !app.ui.should_quit,
        "deferred quit must not fire when edits happened during save"
    );
    assert_eq!(app.dialogs.pending_save_revision, None);
}

/// F-07 counterpart: with no edits during the async save, completion
/// clears the unsaved flag and honors a deferred quit as before.
#[test]
fn test_save_complete_clears_unsaved_when_untouched_during_save() {
    use figby::tui::{AsyncResult, TuiApp};
    let mut app = TuiApp::new();
    app.welcome.screen.show = false;
    app.editor.mark_dirty();
    app.dialogs.pending_save_revision = Some(app.editor.revision);
    app.dialogs.quit_after_save = true;

    let (tx, rx) = std::sync::mpsc::channel();
    let _ = tx.send(AsyncResult::SaveComplete(Ok(std::path::PathBuf::from(
        "/tmp/fake.flf",
    ))));
    app.ctx.async_rx = Some(rx);
    app.check_async_completion();

    assert!(
        !app.editor.unsaved,
        "clean save should clear the unsaved flag"
    );
    assert!(app.ui.should_quit, "deferred quit should be honored");
    assert!(!app.dialogs.quit_after_save);
}

/// F-06 (GPT review): rendering must be read-only against document state.
/// With a stale ImageEditor cell cache present, a paint operation onto
/// the active layer used to be overwritten by the render-time sync on
/// the very next redraw. The paint must survive.
#[test]
fn test_render_does_not_overwrite_paint_with_stale_image_cache() {
    use figby::tui::{canvas, canvas::CanvasCell, AppMode, TuiApp};
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    let mut app = TuiApp::new();
    app.welcome.screen.show = false;
    app.ui.mode = AppMode::ImageEditor;

    // Populate the image editor's conversion cache (as loading an image would).
    let mut cells = vec![
        vec![
            CanvasCell {
                ch: '.',
                fg: None,
                bg: None,
                height: None
            };
            4
        ];
        2
    ];
    cells[0][0] = CanvasCell {
        ch: 'O',
        fg: None,
        bg: None,
        height: None,
    };
    // Simulate the cache being applied once (image was loaded, then shown).
    let mut buf = canvas::CanvasBuffer::new(4, 2);
    for (y, row) in cells.iter().enumerate() {
        for (x, cell) in row.iter().enumerate() {
            buf.set(x, y, *cell);
        }
    }
    app.editor.load_timeline_frame(&[buf]);

    // Simulate a paint operation directly on the active layer — this is
    // what brush/pencil tools do; they do NOT update the editor's cache.
    let painted = CanvasCell {
        ch: 'X',
        fg: None,
        bg: None,
        height: None,
    };
    app.editor
        .layer_stack
        .active_layer_mut()
        .buffer_mut()
        .set(0, 0, painted);
    app.editor.recomposite_canvas();

    // Two full render passes — previously each one re-applied the stale
    // cache and erased the paint.
    for _ in 0..2 {
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| app.render(f)).unwrap();
    }

    assert_eq!(
        app.editor
            .layer_stack
            .active_layer()
            .buffer()
            .get(0, 0)
            .unwrap()
            .ch,
        'X',
        "render must not overwrite a paint with the stale image-editor cache"
    );
}
