//! Read-only "FX Graph" overlay: an `FxGraph` drawn as a node graph, with a
//! "view as script" pane showing the DSL the graph compiles through.
//!
//! The graph canvas is drawn with `rataflow` when the `fx-graph` feature is
//! on (the default); without it the canvas falls back to an indented
//! outline. Either way the IR stays Figby's own: the rataflow `Flow` is
//! rebuilt from the `FxGraph` and never written back. Editing nodes is a
//! later phase (docs/fx-scripting-plan.md, phase 5).

use ratatui::crossterm::event::KeyCode;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Padding, Paragraph};
use ratatui::Frame;

use super::compile::{compile, FxCompileError, FxTargets};
use super::graph::{FxGraph, FxGraphError, FxNodeId};
use super::presets::{self, FxPreset};
use crate::tui::theme::Theme;

/// Horizontal gap between graph columns, in cells.
const GAP_X: f64 = 4.0;
/// Vertical gap between stacked leaves, in cells.
const GAP_Y: f64 = 1.0;

/// Where one node sits in graph space, plus what it shows.
#[derive(Debug, Clone, PartialEq)]
pub struct NodePlacement {
    pub id: FxNodeId,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub title: String,
    pub body: String,
}

/// Tree layout: data flows left to right, leaves in the leftmost column and
/// the root on the right. Leaves stack top to bottom in emission order and
/// each parent is centred on its children. Only nodes reachable from the
/// root are placed; an invalid graph places nothing.
pub fn layout(graph: &FxGraph) -> Vec<NodePlacement> {
    if graph.validate().is_err() {
        return Vec::new();
    }
    // Depth of every reachable node (pre-order, so parents come first).
    let order = graph.reachable();
    let mut depth = std::collections::BTreeMap::new();
    depth.insert(graph.root, 0usize);
    for &id in &order {
        let d = depth.get(&id).copied().unwrap_or(0);
        if let Some(n) = graph.node(id) {
            for &c in &n.children {
                depth.insert(c, d + 1);
            }
        }
    }
    let max_depth = depth.values().copied().max().unwrap_or(0);

    let mut placed: Vec<NodePlacement> = order
        .iter()
        .filter_map(|&id| graph.node(id))
        .map(|n| {
            let title = n.kind.title().to_string();
            let body = n.kind.summary();
            // Border (2) + one cell of padding each side (2); the title
            // sits on the top border with a cell of room either side.
            let inner = body.chars().count().max(title.chars().count() + 2);
            NodePlacement {
                id: n.id,
                x: 0.0,
                y: 0.0,
                width: (inner + 4) as f64,
                height: 3.0,
                title,
                body,
            }
        })
        .collect();

    // Column widths, then x per column (column 0 = deepest = leftmost).
    let col_of = |id: FxNodeId| max_depth - depth.get(&id).copied().unwrap_or(0);
    let mut col_w = vec![0.0f64; max_depth + 1];
    for p in &placed {
        let c = col_of(p.id);
        col_w[c] = col_w[c].max(p.width);
    }
    let mut col_x = vec![0.0f64; max_depth + 1];
    for c in 1..=max_depth {
        col_x[c] = col_x[c - 1] + col_w[c - 1] + GAP_X;
    }

    // y: post-order walk, leaves take the next free row.
    let mut cursor = 0.0f64;
    let mut centre = std::collections::BTreeMap::new();
    place_y(graph, graph.root, &mut cursor, &mut centre);

    for p in &mut placed {
        let c = col_of(p.id);
        // Right-align narrower nodes in their column so edges stay short.
        p.x = col_x[c] + (col_w[c] - p.width);
        let mid = centre.get(&p.id).copied().unwrap_or(0.0);
        p.y = (mid - p.height / 2.0).max(0.0);
    }
    placed
}

fn place_y(
    graph: &FxGraph,
    id: FxNodeId,
    cursor: &mut f64,
    centre: &mut std::collections::BTreeMap<FxNodeId, f64>,
) -> f64 {
    let height = 3.0;
    let Some(node) = graph.node(id) else {
        return 0.0;
    };
    let mid = if node.children.is_empty() {
        let mid = *cursor + height / 2.0;
        *cursor += height + GAP_Y;
        mid
    } else {
        let mids: Vec<f64> = node
            .children
            .iter()
            .map(|&c| place_y(graph, c, cursor, centre))
            .collect();
        let first = mids.first().copied().unwrap_or(0.0);
        let last = mids.last().copied().unwrap_or(first);
        (first + last) / 2.0
    };
    centre.insert(id, mid);
    mid
}

/// Outcome of compiling the emitted script against placeholder regions.
#[derive(Debug, Clone, PartialEq)]
pub enum CompileStatus {
    /// Compiled; total run time in ms.
    Ok(u64),
    Graph(FxGraphError),
    Dsl(FxCompileError),
}

/// Everything derived from the selected graph, rebuilt on selection change.
struct ViewState {
    script: String,
    status: CompileStatus,
    #[cfg(feature = "fx-graph")]
    flow: Option<rataflow::Flow>,
}

impl ViewState {
    fn new(graph: &FxGraph) -> Self {
        let (script, status) = match graph.to_dsl() {
            Ok(dsl) => {
                let targets = FxTargets::uniform(Rect::new(0, 0, 80, 24), 9);
                let status = match compile(&dsl, &targets) {
                    Ok(_) => CompileStatus::Ok(graph.duration_ms().unwrap_or(0)),
                    Err(e) => CompileStatus::Dsl(e),
                };
                (dsl, status)
            }
            Err(e) => (String::new(), CompileStatus::Graph(e)),
        };
        Self {
            script,
            status,
            #[cfg(feature = "fx-graph")]
            flow: rataflow_view::build_flow(graph).ok(),
        }
    }
}

/// The overlay's state. Closed by default; F9 toggles it.
pub struct FxGraphView {
    pub open: bool,
    pub show_script: bool,
    presets: Vec<FxPreset>,
    selected: usize,
    state: Option<ViewState>,
}

impl Default for FxGraphView {
    fn default() -> Self {
        Self::new()
    }
}

impl FxGraphView {
    pub fn new() -> Self {
        Self {
            open: false,
            show_script: true,
            presets: presets::all(),
            selected: 0,
            state: None,
        }
    }

    pub fn toggle(&mut self) {
        self.open = !self.open;
    }

    pub fn presets(&self) -> &[FxPreset] {
        &self.presets
    }

    pub fn selected(&self) -> usize {
        self.selected
    }

    /// The selected preset's graph.
    pub fn graph(&self) -> Option<&FxGraph> {
        self.presets.get(self.selected).map(|p| &p.graph)
    }

    pub fn select(&mut self, index: usize) {
        if !self.presets.is_empty() {
            self.selected = index % self.presets.len();
            self.state = None;
        }
    }

    /// Emitted DSL for the selected graph (empty if it does not validate).
    pub fn script(&mut self) -> &str {
        &self.ensure_state().script
    }

    pub fn status(&mut self) -> CompileStatus {
        self.ensure_state().status.clone()
    }

    fn ensure_state(&mut self) -> &mut ViewState {
        let graph = self.presets.get(self.selected).map(|p| &p.graph);
        self.state.get_or_insert_with(|| match graph {
            Some(g) => ViewState::new(g),
            None => ViewState {
                script: String::new(),
                status: CompileStatus::Graph(FxGraphError::Empty),
                #[cfg(feature = "fx-graph")]
                flow: None,
            },
        })
    }

    /// Handle a key while open. The overlay is modal, so every key is
    /// consumed; returns whether anything changed (needs a redraw).
    pub fn handle_key(&mut self, code: KeyCode) -> bool {
        let n = self.presets.len().max(1);
        match code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::F(9) => self.open = false,
            KeyCode::Up | KeyCode::Char('k') => self.select(self.selected + n - 1),
            KeyCode::Down | KeyCode::Char('j') => self.select(self.selected + 1),
            KeyCode::Char('s') => self.show_script = !self.show_script,
            _ => return false,
        }
        true
    }

    /// Draw the overlay over `area` (normally the whole frame).
    pub fn render(&mut self, frame: &mut Frame<'_>, area: Rect, theme: &Theme) {
        if area.width < 4 || area.height < 4 {
            return;
        }
        frame.render_widget(Clear, area);
        let base = Style::default()
            .bg(theme.menu.dropdown_bg)
            .fg(theme.menu.fg);
        let dim = Style::default().fg(theme.general.secondary);
        let outer = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(theme.general.primary))
            .title(Line::from(vec![
                Span::styled(
                    " FX Graph ",
                    Style::default()
                        .fg(theme.general.primary)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled("read-only ", dim),
            ]))
            .title_bottom(Line::styled(" ↑↓ preset  s script  Esc close ", dim))
            .style(base);
        let inner = outer.inner(area);
        frame.render_widget(outer, area);

        let list_w = if inner.width >= 60 { 22 } else { 14 };
        let [list_area, body] =
            Layout::horizontal([Constraint::Length(list_w), Constraint::Fill(1)])
                .spacing(1)
                .areas(inner);
        self.render_list(frame, list_area, theme);

        let (graph_area, script_area) = if !self.show_script {
            (body, None)
        } else if body.width >= 100 {
            let [g, s] = Layout::horizontal([Constraint::Fill(3), Constraint::Fill(2)])
                .spacing(1)
                .areas(body);
            (g, Some(s))
        } else {
            let [g, s] = Layout::vertical([Constraint::Fill(3), Constraint::Fill(2)]).areas(body);
            (g, Some(s))
        };
        self.render_graph(frame, graph_area, theme);
        if let Some(s) = script_area {
            self.render_script(frame, s, theme);
        }
    }

    fn render_list(&self, frame: &mut Frame<'_>, area: Rect, theme: &Theme) {
        let mut lines = vec![Line::styled(
            "Presets",
            Style::default().add_modifier(Modifier::BOLD),
        )];
        for (i, p) in self.presets.iter().enumerate() {
            let selected = i == self.selected;
            let style = if selected {
                Style::default()
                    .fg(theme.general.primary)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.menu.fg)
            };
            let marker = if selected { "▸ " } else { "  " };
            lines.push(Line::styled(format!("{marker}{}", p.name), style));
        }
        if let Some(p) = self.presets.get(self.selected) {
            lines.push(Line::from(""));
            lines.push(Line::styled(
                p.description,
                Style::default().fg(theme.general.secondary),
            ));
        }
        frame.render_widget(
            Paragraph::new(lines)
                .wrap(ratatui::widgets::Wrap { trim: true })
                .block(Block::default().padding(Padding::horizontal(1))),
            area,
        );
    }

    fn render_graph(&mut self, frame: &mut Frame<'_>, area: Rect, theme: &Theme) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(theme.general.secondary))
            .title(" graph ");
        let inner = block.inner(area);
        frame.render_widget(block, area);
        if inner.width == 0 || inner.height == 0 {
            return;
        }
        #[cfg(feature = "fx-graph")]
        {
            let palette = rataflow_view::palette(theme);
            if let Some(flow) = self.ensure_state().flow.as_mut() {
                flow.theme = rataflow::Theme::Custom(palette);
                // Read-only view: refit every frame so a resize keeps the
                // whole graph in view.
                flow.request_fit_view();
                frame.render_widget(flow, inner);
                return;
            }
        }
        let outline = self.graph().map(FxGraph::outline).unwrap_or_default();
        let lines: Vec<Line> = outline.into_iter().map(Line::from).collect();
        frame.render_widget(Paragraph::new(lines), inner);
    }

    fn render_script(&mut self, frame: &mut Frame<'_>, area: Rect, theme: &Theme) {
        let status = self.status();
        let (label, colour) = match &status {
            CompileStatus::Ok(ms) => (format!(" ✓ compiles · {ms}ms "), theme.general.success),
            CompileStatus::Graph(e) => (format!(" ✗ graph: {e} "), theme.general.error),
            CompileStatus::Dsl(e) => (format!(" ✗ {e} "), theme.general.error),
        };
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(theme.general.secondary))
            .title(" script ")
            .title_bottom(Line::styled(label, Style::default().fg(colour)))
            .padding(Padding::horizontal(1));
        let lines: Vec<Line> = self
            .script()
            .lines()
            .map(|l| Line::from(l.to_string()))
            .collect();
        frame.render_widget(Paragraph::new(lines).block(block), area);
    }
}

#[cfg(feature = "fx-graph")]
mod rataflow_view {
    //! The only code that touches rataflow (pinned, behind `fx-graph`).

    use rataflow::{Edge, Flow, Node, Palette, StepEdge, TextContent};

    use super::{layout, FxGraph};
    use crate::tui::theme::Theme;

    pub(super) fn build_flow(graph: &FxGraph) -> Result<Flow, rataflow::Error> {
        let nodes: Vec<Node<TextContent>> = layout(graph)
            .into_iter()
            .map(|p| {
                let content = TextContent::new(p.body).with_title(p.title);
                Node::new(p.id.to_string(), (p.x, p.y), (p.width, p.height), content)
                    .with_draggable(false)
                    .with_connectable(false)
                    .with_deletable(false)
            })
            .collect();
        let edges: Vec<Edge<StepEdge>> = graph
            .edges()
            .into_iter()
            .filter(|e| nodes.iter().any(|n| n.id == e.child.to_string()))
            .map(|e| {
                Edge::new(
                    format!("e{}-{}", e.child, e.parent),
                    e.child.to_string(),
                    e.parent.to_string(),
                )
                .with_deletable(false)
            })
            .collect();
        let mut flow = Flow::with_graph(nodes, edges)?.with_locked(true);
        flow.request_fit_view();
        Ok(flow)
    }

    pub(super) fn palette(theme: &Theme) -> Palette {
        Palette {
            canvas_bg: theme.menu.dropdown_bg,
            surface: theme.toolbox.bg,
            muted: theme.general.secondary,
            subtle: theme.canvas.grid,
            accent: theme.general.primary,
            text: theme.menu.fg,
            success: theme.general.success,
            error: theme.general.error,
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::tui::fx_script::presets;

        #[test]
        fn flow_mirrors_reachable_nodes_and_edges() {
            for p in presets::all() {
                let flow = build_flow(&p.graph).unwrap_or_else(|e| panic!("{}: {e}", p.name));
                let reachable = p.graph.reachable().len();
                assert_eq!(flow.nodes().count(), reachable, "{}", p.name);
                assert_eq!(flow.edges().len(), reachable - 1, "{}", p.name);
                assert!(flow.locked, "{}", p.name);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::fx_script::graph::{FxEffect, FxGraphBuilder, FxInterp, FxNodeKind, FxTimer};
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn overlaps(a: &NodePlacement, b: &NodePlacement) -> bool {
        a.x < b.x + b.width && b.x < a.x + a.width && a.y < b.y + b.height && b.y < a.y + a.height
    }

    fn screen_text(term: &Terminal<TestBackend>) -> String {
        let buf = term.backend().buffer();
        let mut s = String::new();
        for y in 0..buf.area.height {
            for x in 0..buf.area.width {
                s.push_str(buf[(x, y)].symbol());
            }
            s.push('\n');
        }
        s
    }

    #[test]
    fn layout_puts_children_left_of_parents_without_overlap() {
        for p in presets::all() {
            let placed = layout(&p.graph);
            assert_eq!(placed.len(), p.graph.reachable().len(), "{}", p.name);
            let at = |id| placed.iter().find(|n| n.id == id).unwrap();
            for e in p.graph.edges() {
                let (c, par) = (at(e.child), at(e.parent));
                assert!(c.x + c.width <= par.x, "{}: {e:?}", p.name);
            }
            for (i, a) in placed.iter().enumerate() {
                for b in &placed[i + 1..] {
                    assert!(!overlaps(a, b), "{}: {} overlaps {}", p.name, a.id, b.id);
                }
            }
        }
    }

    #[test]
    fn layout_centres_parent_between_children() {
        let mut b = FxGraphBuilder::new();
        let t = FxTimer::new(100, FxInterp::Linear);
        let a = b.effect(FxEffect::Dissolve { timer: t });
        let c = b.effect(FxEffect::Coalesce { timer: t });
        let root = b.add(FxNodeKind::Parallel, &[a, c]);
        let placed = layout(&b.build(root));
        let y = |id| placed.iter().find(|n| n.id == id).unwrap().y;
        assert!(y(a) < y(root) && y(root) < y(c));
    }

    #[test]
    fn layout_of_invalid_graph_is_empty() {
        let mut b = FxGraphBuilder::new();
        let s = b.add(FxNodeKind::Sequence, &[]);
        assert!(layout(&b.build(s)).is_empty());
    }

    #[test]
    fn keys_close_cycle_and_toggle_script() {
        let mut v = FxGraphView::new();
        v.toggle();
        assert!(v.open);
        let n = v.presets().len();
        assert!(v.handle_key(KeyCode::Up));
        assert_eq!(v.selected(), n - 1);
        assert!(v.handle_key(KeyCode::Down));
        assert_eq!(v.selected(), 0);
        assert!(v.show_script);
        assert!(v.handle_key(KeyCode::Char('s')));
        assert!(!v.show_script);
        assert!(!v.handle_key(KeyCode::Char('x')));
        assert!(v.open);
        assert!(v.handle_key(KeyCode::Esc));
        assert!(!v.open);
    }

    #[test]
    fn selection_change_rebuilds_script() {
        let mut v = FxGraphView::new();
        let first = v.script().to_string();
        v.handle_key(KeyCode::Down);
        assert_ne!(v.script(), first);
        let expected = v.graph().unwrap().to_dsl().unwrap();
        assert_eq!(v.script(), expected);
    }

    #[test]
    fn every_preset_reports_compiles_ok() {
        let mut v = FxGraphView::new();
        for i in 0..v.presets().len() {
            v.select(i);
            assert!(
                matches!(v.status(), CompileStatus::Ok(ms) if ms > 0),
                "{:?}",
                v.status()
            );
        }
    }

    #[test]
    fn renders_at_several_sizes_without_panicking() {
        for (w, h) in [(30, 10), (60, 20), (80, 24), (140, 40), (4, 4), (3, 3)] {
            let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
            let mut v = FxGraphView::new();
            v.open = true;
            term.draw(|f| v.render(f, f.area(), &Theme::default()))
                .unwrap();
        }
    }

    #[test]
    fn render_shows_presets_script_and_status() {
        let mut term = Terminal::new(TestBackend::new(140, 40)).unwrap();
        let mut v = FxGraphView::new();
        v.open = true;
        term.draw(|f| v.render(f, f.area(), &Theme::default()))
            .unwrap();
        let text = screen_text(&term);
        assert!(text.contains("FX Graph"));
        assert!(text.contains("intro"));
        assert!(text.contains("pulse"));
        assert!(text.contains("fx::sequence(&["));
        assert!(text.contains("compiles"));
        // Node titles reach the buffer through rataflow (or the outline).
        assert!(text.contains("sweep_in"), "{text}");
    }

    #[test]
    fn hiding_script_removes_script_pane() {
        let mut term = Terminal::new(TestBackend::new(140, 40)).unwrap();
        let mut v = FxGraphView::new();
        v.open = true;
        v.show_script = false;
        term.draw(|f| v.render(f, f.area(), &Theme::default()))
            .unwrap();
        assert!(!screen_text(&term).contains("fx::sequence"));
    }
}
