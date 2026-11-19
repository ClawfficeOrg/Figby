//! The "FX Graph" overlay: an `FxGraph` drawn as a node graph, with a
//! "view as script" pane showing the DSL the graph compiles through, and an
//! edit mode (docs/fx-scripting-plan.md, phase 5): structural edits, a Props
//! panel for the selected node's parameters, Value nodes, and save/load.
//!
//! The graph canvas is drawn with `rataflow` when the `fx-graph` feature is
//! on (the default); without it the canvas falls back to an indented
//! outline. Either way the IR stays Figby's own: the rataflow `Flow` is
//! rebuilt from the `FxGraph` and never written back. The selected node is
//! highlighted in both (rataflow's selection style, or a marked outline row).
//! The mouse edits too (see [`FxGraphView::handle_mouse`]): click selects,
//! drag onto a parent connects, drag off or right-click detaches.

use std::path::PathBuf;

use ratatui::crossterm::event::{KeyCode, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Padding, Paragraph};
use ratatui::Frame;

use super::compile::{compile, FxCompileError, FxTargets};
use super::edit::{FxEditError, GraphEditor};
use super::graph::{
    value_var, FxColor, FxEffect, FxGraph, FxGraphError, FxNode, FxNodeId, FxNodeKind, FxValue,
};
use super::presets::{self, FxPreset};
use super::{graph_file, props};
use crate::tui::theme::Theme;

/// Horizontal gap between graph columns, in cells.
const GAP_X: f64 = 4.0;
/// Vertical gap between stacked leaves, in cells.
const GAP_Y: f64 = 1.0;
/// Prefix that marks the node chosen with `m` (the child for `c`, the value
/// for `b`), on the canvas title and in the outline.
const MARK: &str = "◆ ";

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

/// Size and text of one node; position is filled in by [`layout`].
fn placement(graph: &FxGraph, n: &FxNode) -> NodePlacement {
    let title = match n.kind {
        FxNodeKind::Value { .. } => format!("{} {}", n.kind.title(), value_var(n.id)),
        _ => n.kind.title().to_string(),
    };
    // A bound parameter shows the value it takes.
    let body = graph
        .resolved_kind(n.id)
        .unwrap_or_else(|| n.kind.clone())
        .summary();
    // Border (2) + one cell of padding each side (2); the title sits on the
    // top border with a cell of room either side, plus room for the mark
    // prefix so marking a node never truncates its title.
    let inner = body
        .chars()
        .count()
        .max(title.chars().count() + 2 + MARK.chars().count());
    NodePlacement {
        id: n.id,
        x: 0.0,
        y: 0.0,
        width: (inner + 4) as f64,
        height: 3.0,
        title,
        body,
    }
}

/// Tree layout: data flows left to right, leaves in the leftmost column and
/// the root on the right. Leaves stack top to bottom in emission order and
/// each parent is centred on its children. Value nodes the tree uses get a
/// column of their own on the far left. Scratch trees (nodes not attached
/// under the root, such as a leaf just added) are laid out the same way and
/// stacked below the main tree, so they can be clicked and dragged onto a
/// parent. An invalid graph places nothing.
pub fn layout(graph: &FxGraph) -> Vec<NodePlacement> {
    if graph.validate().is_err() {
        return Vec::new();
    }
    let mut placed = tree_layout(graph, graph.root);
    let bottom =
        |placed: &[NodePlacement]| placed.iter().map(|p| p.y + p.height).fold(0.0f64, f64::max);
    for root in scratch_roots(graph) {
        // One blank row more than between leaves sets scratch apart.
        let top = bottom(&placed) + GAP_Y * 2.0;
        let mut tree = tree_layout(graph, root);
        for p in &mut tree {
            p.y += top;
        }
        placed.extend(tree);
    }

    let values: Vec<NodePlacement> = graph
        .used_values()
        .into_iter()
        .filter_map(|id| graph.node(id))
        .map(|n| placement(graph, n))
        .collect();
    if !values.is_empty() {
        let w = values.iter().map(|p| p.width).fold(0.0, f64::max);
        for p in &mut placed {
            p.x += w + GAP_X;
        }
        let mut y = 0.0;
        for mut p in values {
            p.x = w - p.width;
            p.y = y;
            y += p.height + GAP_Y;
            placed.push(p);
        }
    }
    placed
}

/// Nodes under `root`, parents before children (pre-order).
fn preorder(graph: &FxGraph, root: FxNodeId) -> Vec<FxNodeId> {
    let mut out = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    let mut stack = vec![root];
    while let Some(id) = stack.pop() {
        if !seen.insert(id) {
            continue;
        }
        out.push(id);
        if let Some(n) = graph.node(id) {
            stack.extend(n.children.iter().rev().copied());
        }
    }
    out
}

/// Tops of the scratch trees, by id: nodes that are neither reachable from
/// the root, nor a Value node the tree uses, nor anyone's child.
pub fn scratch_roots(graph: &FxGraph) -> Vec<FxNodeId> {
    let reachable = graph.reachable();
    let used = graph.used_values();
    let mut ids: Vec<FxNodeId> = graph
        .nodes
        .iter()
        .map(|n| n.id)
        .filter(|id| !reachable.contains(id) && !used.contains(id))
        .filter(|&id| !graph.nodes.iter().any(|n| n.children.contains(&id)))
        .collect();
    ids.sort_unstable();
    ids
}

/// Whether `id` sits under some parent (what `x` / disconnect needs).
fn is_attached(graph: &FxGraph, id: FxNodeId) -> bool {
    graph.nodes.iter().any(|n| n.children.contains(&id))
}

/// [`layout`] of the one tree under `root`, from the origin.
fn tree_layout(graph: &FxGraph, root: FxNodeId) -> Vec<NodePlacement> {
    // Depth of every node (pre-order, so parents come first).
    let order = preorder(graph, root);
    let mut depth = std::collections::BTreeMap::new();
    depth.insert(root, 0usize);
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
        .map(|n| placement(graph, n))
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
    place_y(graph, root, &mut cursor, &mut centre);

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
    /// Edited copy of the selected preset, made on first edit. It lives until
    /// another preset is selected; `S` saves it to disk.
    editor: Option<GraphEditor>,
    /// Edit mode: keys act on nodes instead of presets.
    editing: bool,
    /// Index into the edited graph's node ids (sorted), the selected node.
    node_sel: usize,
    /// A node marked as the child for the next `c` (connect).
    marked: Option<FxNodeId>,
    /// Last edit result or hint, shown in the graph pane's title.
    edit_msg: String,
    /// The Props panel, open over the preset list while editing.
    props: Option<PropsState>,
    /// Where graphs are saved and loaded (`<config dir>/fx-graphs`).
    pub graph_dir: Option<PathBuf>,
    /// The load picker (`L`), open over the preset list.
    picker: Option<PickerState>,
    /// Name of the graph file last loaded; `S` saves back under it. `None`
    /// saves under the selected preset's name.
    graph_name: Option<String>,
    /// Mouse hit targets, recorded by the last render (see [`NodeHit`]).
    hits: Vec<NodeHit>,
    /// Inner rect of the graph pane at the last render.
    graph_pane: Rect,
    /// A left-button drag that started on a node, while it lasts.
    drag: Option<MouseDrag>,
    /// First outline row shown (the outline scrolls with the wheel).
    outline_scroll: usize,
}

/// One node's mouse target on screen, as last drawn.
///
/// On the rataflow canvas the rect is the node's own terminal rect, read
/// back with `Flow::node_terminal_rect` right after the flow is rendered,
/// so it carries rataflow's fit-to-view transform exactly (no second copy
/// of the transform here). When rataflow zooms out far enough that a node
/// is drawn shorter than its box (the bottom border dropped, the handle row
/// on the row below the rect), `slop_row` is that extra row; it only counts
/// when no node's own rect is hit. In the outline the rect is the node's row.
#[derive(Debug, Clone, Copy, PartialEq)]
struct NodeHit {
    id: FxNodeId,
    rect: Rect,
    slop_row: Option<u16>,
}

/// Press on a node, then drag: onto a parent to connect, onto empty graph
/// space to detach.
#[derive(Debug, Clone, Copy, PartialEq)]
struct MouseDrag {
    from: FxNodeId,
    start: (u16, u16),
    moved: bool,
    /// The node under the pointer now (never `from`), and whether
    /// connecting `from` under it would be accepted.
    target: Option<(FxNodeId, bool)>,
}

/// Load picker: the saved graphs (newest first) and the highlighted one.
#[derive(Debug, Clone)]
struct PickerState {
    entries: Vec<graph_file::GraphEntry>,
    sel: usize,
}

/// Props panel cursor: the selected field and a number being typed.
#[derive(Debug, Clone, Default, PartialEq)]
struct PropsState {
    field: usize,
    typing: Option<String>,
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
            editor: None,
            editing: false,
            node_sel: 0,
            marked: None,
            edit_msg: String::new(),
            props: None,
            graph_dir: graph_file::default_dir(),
            picker: None,
            graph_name: None,
            hits: Vec::new(),
            graph_pane: Rect::default(),
            drag: None,
            outline_scroll: 0,
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

    /// The graph on screen: the edited copy if there is one, else the
    /// selected preset's graph.
    pub fn graph(&self) -> Option<&FxGraph> {
        match &self.editor {
            Some(ed) => Some(ed.graph()),
            None => self.presets.get(self.selected).map(|p| &p.graph),
        }
    }

    pub fn is_editing(&self) -> bool {
        self.editing
    }

    pub fn select(&mut self, index: usize) {
        if !self.presets.is_empty() {
            self.selected = index % self.presets.len();
            self.editor = None;
            self.editing = false;
            self.marked = None;
            self.props = None;
            self.picker = None;
            self.graph_name = None;
            self.edit_msg.clear();
            self.state = None;
            self.drag = None;
            self.outline_scroll = 0;
        }
    }

    /// Node ids of the graph on screen, sorted.
    fn node_ids(&self) -> Vec<FxNodeId> {
        let mut ids: Vec<FxNodeId> = self
            .graph()
            .map(|g| g.nodes.iter().map(|n| n.id).collect())
            .unwrap_or_default();
        ids.sort_unstable();
        ids
    }

    fn selected_node(&self) -> Option<FxNodeId> {
        self.node_ids().get(self.node_sel).copied()
    }

    fn start_editing(&mut self) {
        if self.editor.is_none() {
            let Some(p) = self.presets.get(self.selected) else {
                return;
            };
            match GraphEditor::new(p.graph.clone()) {
                Ok(ed) => self.editor = Some(ed),
                Err(e) => {
                    self.edit_msg = e.to_string();
                    return;
                }
            }
        }
        self.editing = true;
        self.node_sel = 0;
        self.edit_msg =
            "edit mode: a add, v value, m mark, c connect, p props, S save, L load".into();
    }

    /// Name the graph on screen is saved under: the file it was loaded from,
    /// else the selected preset's name.
    fn save_name(&self) -> String {
        if let Some(name) = &self.graph_name {
            return name.clone();
        }
        self.presets
            .get(self.selected)
            .map_or_else(|| "graph".into(), |p| p.name.to_string())
    }

    /// Write the graph on screen as IR JSON plus emitted DSL.
    fn save(&mut self) {
        let Some(dir) = self.graph_dir.clone() else {
            self.edit_msg = "save failed: no config dir (set HOME or XDG_CONFIG_HOME)".into();
            return;
        };
        let name = self.save_name();
        let Some(graph) = self.graph().cloned() else {
            return;
        };
        self.edit_msg = match graph_file::save(&dir, &name, &graph) {
            Ok(saved) => format!(
                "saved {} (+ .figfx.toml) in {}",
                saved
                    .graph
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                dir.display()
            ),
            Err(e) => format!("save failed: {e}"),
        };
    }

    /// Open the load picker over the saved graphs in the graph dir, newest
    /// first. Errors, and an empty dir, go to the message line instead.
    fn open_picker(&mut self) {
        let Some(dir) = self.graph_dir.clone() else {
            self.edit_msg = "load failed: no config dir (set HOME or XDG_CONFIG_HOME)".into();
            return;
        };
        match graph_file::list(&dir) {
            Ok(entries) if entries.is_empty() => {
                self.edit_msg = format!("no saved graphs in {} (S saves one)", dir.display());
            }
            Ok(entries) => {
                self.edit_msg = format!(
                    "load: {} saved graph(s); ↑↓ pick, Enter load, Esc cancel",
                    entries.len()
                );
                self.picker = Some(PickerState { entries, sel: 0 });
            }
            Err(e) => self.edit_msg = format!("load failed: {}: {e}", dir.display()),
        }
    }

    fn handle_picker_key(&mut self, code: KeyCode) -> bool {
        let Some(picker) = self.picker.as_mut() else {
            return false;
        };
        let n = picker.entries.len().max(1);
        match code {
            KeyCode::Up | KeyCode::Char('k') | KeyCode::BackTab => {
                picker.sel = (picker.sel + n - 1) % n;
            }
            KeyCode::Down | KeyCode::Char('j') | KeyCode::Tab => {
                picker.sel = (picker.sel + 1) % n;
            }
            KeyCode::Enter => {
                let entry = picker.entries.get(picker.sel).cloned();
                self.picker = None;
                if let Some(entry) = entry {
                    self.load_entry(&entry);
                }
            }
            KeyCode::Esc | KeyCode::Char('q') => {
                self.picker = None;
                self.edit_msg = "load cancelled".into();
            }
            _ => return false,
        }
        true
    }

    /// Load one saved graph into the editor. With an editor open the load is
    /// one undoable edit. On error nothing changes.
    fn load_entry(&mut self, entry: &graph_file::GraphEntry) {
        let name = entry.name.clone();
        let graph = match graph_file::load_path(&entry.path) {
            Ok(g) => g,
            Err(e) => {
                self.edit_msg = format!("load failed: {}: {e}", entry.path.display());
                return;
            }
        };
        if self.editor.is_none() {
            match GraphEditor::new(graph) {
                Ok(ed) => self.editor = Some(ed),
                Err(e) => {
                    self.edit_msg = format!("load failed: {e}");
                    return;
                }
            }
            self.state = None;
        } else {
            self.edit(|ed| ed.replace(graph).map(|()| String::new()));
            if !self.edit_msg.is_empty() {
                self.edit_msg = format!("load failed: {}", self.edit_msg);
                return;
            }
        }
        self.editing = true;
        self.props = None;
        self.marked = None;
        self.node_sel = 0;
        self.edit_msg = format!("loaded {name}");
        self.graph_name = Some(name);
    }

    /// Run one edit on the editor, then refresh the derived views and clamp
    /// the selection. Errors go to the message line.
    fn edit(&mut self, op: impl FnOnce(&mut GraphEditor) -> Result<String, FxEditError>) {
        let Some(ed) = self.editor.as_mut() else {
            return;
        };
        self.edit_msg = match op(ed) {
            Ok(msg) => msg,
            Err(e) => e.to_string(),
        };
        self.state = None;
        let ids = self.node_ids();
        self.node_sel = self.node_sel.min(ids.len().saturating_sub(1));
        // An undo can take the marked node away; drop a mark that points
        // at nothing so the canvas never claims a missing node is marked.
        if self.marked.is_some_and(|m| !ids.contains(&m)) {
            self.marked = None;
        }
    }

    /// Select the node with the highest id, which is the one just added.
    fn select_newest(&mut self) {
        self.node_sel = self.node_ids().len().saturating_sub(1);
    }

    fn handle_edit_key(&mut self, code: KeyCode) -> bool {
        if self.picker.is_some() {
            return self.handle_picker_key(code);
        }
        if self.props.is_some() {
            return self.handle_props_key(code);
        }
        let n = self.node_ids().len().max(1);
        let sel = self.selected_node();
        match code {
            KeyCode::Char('p') | KeyCode::Enter => {
                if sel.is_some() {
                    self.props = Some(PropsState::default());
                    self.edit_msg =
                        "props: ←→ change, digits type, v make value, b bind marked, n unbind"
                            .into();
                }
            }
            KeyCode::Char('v') => {
                self.edit(|ed| {
                    let id = ed.add_node(FxNodeKind::Value {
                        value: FxValue::Color(FxColor::White),
                    });
                    Ok(format!(
                        "added value {id}; p to set it, m to mark it for binding"
                    ))
                });
                self.select_newest();
            }
            KeyCode::Char('S') => self.save(),
            KeyCode::Char('L') => self.open_picker(),
            KeyCode::Esc => {
                self.editing = false;
                self.marked = None;
                self.edit_msg = "edits kept; press e to edit again".into();
            }
            KeyCode::Tab | KeyCode::Down => self.node_sel = (self.node_sel + 1) % n,
            KeyCode::BackTab | KeyCode::Up => self.node_sel = (self.node_sel + n - 1) % n,
            KeyCode::Char('a') => {
                self.edit(|ed| {
                    let id = ed.add_node(FxNodeKind::Effect {
                        effect: FxEffect::Sleep { ms: 100 },
                    });
                    Ok(format!(
                        "added leaf {id}; mark it (m) and connect (c) under a parent"
                    ))
                });
                self.select_newest();
            }
            KeyCode::Char('A') => {
                self.edit(|ed| {
                    let id = ed.add_node(FxNodeKind::Sequence);
                    Ok(format!(
                        "added sequence {id}; mark a child and connect it here"
                    ))
                });
                self.select_newest();
            }
            KeyCode::Char('m') => {
                self.marked = sel;
                self.edit_msg = match sel {
                    Some(id) => format!("marked {id} as child; select a parent and press c"),
                    None => String::new(),
                };
            }
            KeyCode::Char('c') => match (self.marked, sel) {
                (Some(child), Some(parent)) => {
                    let slot = self
                        .graph()
                        .and_then(|g| g.node(parent))
                        .map_or(0, |p| p.children.len());
                    self.edit(|ed| {
                        ed.connect(child, parent, slot)
                            .map(|()| format!("connected {child} under {parent}"))
                    });
                    if self.edit_msg.starts_with("connected") {
                        self.marked = None;
                    }
                }
                _ => self.edit_msg = "mark a child with m, select its parent, then c".into(),
            },
            KeyCode::Char('x') => {
                if let Some(id) = sel {
                    self.edit(|ed| ed.disconnect(id).map(|()| format!("detached {id}")));
                }
            }
            KeyCode::Char('d') | KeyCode::Delete => {
                if let Some(id) = sel {
                    self.edit(|ed| {
                        ed.delete(id)
                            .map(|()| format!("deleted {id} and its children"))
                    });
                    self.marked = None;
                }
            }
            KeyCode::Char('r') => {
                if let Some(id) = sel {
                    self.edit(|ed| ed.set_root(id).map(|()| format!("{id} is now the root")));
                }
            }
            KeyCode::Char('u') => self.edit(|ed| ed.undo().map(|()| "undone".into())),
            KeyCode::Char('y') => self.edit(|ed| ed.redo().map(|()| "redone".into())),
            _ => return false,
        }
        true
    }

    /// Fields of the selected node: the node's own kind for the layout, with
    /// values shown as resolved (a bound slot shows its Value node's value).
    /// Also returns the bound Value node of each field, if any.
    fn prop_rows(&self) -> Vec<(props::PropField, Option<FxNodeId>)> {
        let Some(id) = self.selected_node() else {
            return Vec::new();
        };
        let Some(g) = self.graph() else {
            return Vec::new();
        };
        let Some(node) = g.node(id) else {
            return Vec::new();
        };
        let resolved = g.resolved_kind(id).unwrap_or_else(|| node.kind.clone());
        props::fields(&resolved)
            .into_iter()
            .map(|f| {
                let bound = f.slot.and_then(|s| node.params.get(s).copied());
                (f, bound)
            })
            .collect()
    }

    /// Change field `index` of the selected node with `change`. A field
    /// whose slot is bound changes the Value node instead, so every user of
    /// that value follows. Goes through `GraphEditor::set_kind`, so it is
    /// validated and undoable.
    fn change_prop(
        &mut self,
        index: usize,
        change: impl FnOnce(&FxNodeKind) -> Option<FxNodeKind>,
    ) {
        let Some(id) = self.selected_node() else {
            return;
        };
        let Some(g) = self.graph() else {
            return;
        };
        let Some(node) = g.node(id) else {
            return;
        };
        let rows = self.prop_rows();
        let Some((field, bound)) = rows.get(index).cloned() else {
            return;
        };
        let target = match (field.slot, bound) {
            (Some(slot), Some(vid)) => {
                let resolved = g.resolved_kind(id).unwrap_or_else(|| node.kind.clone());
                change(&resolved)
                    .and_then(|k| k.slot_value(slot))
                    .map(|value| (vid, FxNodeKind::Value { value }))
            }
            _ => change(&node.kind).map(|k| (id, k)),
        };
        let Some((target, kind)) = target else {
            self.edit_msg = format!("{} does not take a number", field.label);
            return;
        };
        let label = field.label;
        self.edit(|ed| {
            ed.set_kind(target, kind)
                .map(|()| format!("node {target}: {label} changed"))
        });
        let n = self.prop_rows().len().max(1);
        if let Some(p) = self.props.as_mut() {
            p.field = p.field.min(n - 1);
        }
    }

    fn handle_props_key(&mut self, code: KeyCode) -> bool {
        let rows = self.prop_rows();
        let n = rows.len().max(1);
        let Some(state) = self.props.clone() else {
            return false;
        };
        let field = state.field.min(n - 1);
        let sel = self.selected_node();
        let slot = rows.get(field).and_then(|(f, _)| f.slot);
        let numeric = rows.get(field).is_some_and(|(f, _)| f.numeric);
        let set = |v: &mut Self, s: PropsState| v.props = Some(s);
        match code {
            KeyCode::Esc if state.typing.is_some() => set(
                self,
                PropsState {
                    field,
                    typing: None,
                },
            ),
            KeyCode::Esc | KeyCode::Char('p') => {
                self.props = None;
                self.edit_msg = "props closed".into();
            }
            KeyCode::Down | KeyCode::Tab => set(
                self,
                PropsState {
                    field: (field + 1) % n,
                    typing: None,
                },
            ),
            KeyCode::Up | KeyCode::BackTab => set(
                self,
                PropsState {
                    field: (field + n - 1) % n,
                    typing: None,
                },
            ),
            KeyCode::Left | KeyCode::Char('-') | KeyCode::Right | KeyCode::Char('+' | '=') => {
                let delta = if matches!(code, KeyCode::Left | KeyCode::Char('-')) {
                    -1
                } else {
                    1
                };
                set(
                    self,
                    PropsState {
                        field,
                        typing: None,
                    },
                );
                self.change_prop(field, |k| props::step(k, field, delta));
            }
            KeyCode::Char(c) if numeric && (c.is_ascii_digit() || c == '.') => {
                let mut typed = state.typing.unwrap_or_default();
                typed.push(c);
                set(
                    self,
                    PropsState {
                        field,
                        typing: Some(typed),
                    },
                );
            }
            KeyCode::Backspace => {
                let mut typed = state.typing.unwrap_or_default();
                typed.pop();
                set(
                    self,
                    PropsState {
                        field,
                        typing: (!typed.is_empty()).then_some(typed),
                    },
                );
            }
            KeyCode::Enter => {
                let Some(typed) = state.typing else {
                    return false;
                };
                set(
                    self,
                    PropsState {
                        field,
                        typing: None,
                    },
                );
                match typed.parse::<f64>() {
                    Ok(v) => self.change_prop(field, |k| props::set_number(k, field, v)),
                    Err(_) => self.edit_msg = format!("'{typed}' is not a number"),
                }
            }
            KeyCode::Char('v') => match (sel, slot) {
                (Some(id), Some(slot)) => self.edit(|ed| {
                    ed.extract_value(id, slot)
                        .map(|v| format!("'{slot}' now reads value {v}"))
                }),
                _ => self.edit_msg = "this field cannot be a value node".into(),
            },
            KeyCode::Char('b') => match (sel, slot, self.marked) {
                (Some(id), Some(slot), Some(v)) => self.edit(|ed| {
                    ed.bind(id, slot, v)
                        .map(|()| format!("'{slot}' bound to {v}"))
                }),
                (_, None, _) => self.edit_msg = "this field cannot be bound".into(),
                _ => self.edit_msg = "mark a value node with m first (Esc, select, m)".into(),
            },
            KeyCode::Char('n') => match (sel, slot) {
                (Some(id), Some(slot)) => {
                    self.edit(|ed| ed.unbind(id, slot).map(|()| format!("'{slot}' unbound")))
                }
                _ => self.edit_msg = "this field cannot be bound".into(),
            },
            KeyCode::Char('u') => self.edit(|ed| ed.undo().map(|()| "undone".into())),
            KeyCode::Char('y') => self.edit(|ed| ed.redo().map(|()| "redone".into())),
            _ => return false,
        }
        true
    }

    /// Emitted DSL for the selected graph (empty if it does not validate).
    pub fn script(&mut self) -> &str {
        &self.ensure_state().script
    }

    pub fn status(&mut self) -> CompileStatus {
        self.ensure_state().status.clone()
    }

    fn ensure_state(&mut self) -> &mut ViewState {
        let graph = self.graph().cloned();
        self.state.get_or_insert_with(|| match graph {
            Some(g) => ViewState::new(&g),
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
        if self.picker.is_some() {
            return self.handle_picker_key(code);
        }
        if self.editing {
            return self.handle_edit_key(code);
        }
        let n = self.presets.len().max(1);
        match code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::F(9) => self.open = false,
            KeyCode::Up | KeyCode::Char('k') => self.select(self.selected + n - 1),
            KeyCode::Down | KeyCode::Char('j') => self.select(self.selected + 1),
            KeyCode::Char('s') => self.show_script = !self.show_script,
            KeyCode::Char('e') => self.start_editing(),
            KeyCode::Char('L') => self.open_picker(),
            _ => return false,
        }
        true
    }

    /// The node drawn at screen cell (`col`, `row`) by the last render, if
    /// any. A node's own rect wins over another node's slop row.
    fn node_at(&self, col: u16, row: u16) -> Option<FxNodeId> {
        let pos = (col, row).into();
        if !self.graph_pane.contains(pos) {
            return None;
        }
        self.hits
            .iter()
            .find(|h| h.rect.contains(pos))
            .or_else(|| {
                self.hits
                    .iter()
                    .find(|h| h.slop_row == Some(row) && col >= h.rect.x && col < h.rect.right())
            })
            .map(|h| h.id)
    }

    /// Select `id` as if Tab had walked to it, entering edit mode first
    /// (as `e` does) when the overlay is only viewing.
    fn select_node_id(&mut self, id: FxNodeId) -> bool {
        if !self.editing {
            self.start_editing();
            if !self.editing {
                return false;
            }
        }
        let Some(i) = self.node_ids().iter().position(|&n| n == id) else {
            return false;
        };
        if i != self.node_sel {
            if let Some(p) = self.props.as_mut() {
                // The panel shows the new node's fields from the top.
                *p = PropsState::default();
            }
        }
        self.node_sel = i;
        true
    }

    /// Whether connecting `child` under `parent` would be accepted, tried on
    /// a throwaway copy of the editor so nothing changes.
    fn connect_check(&self, child: FxNodeId, parent: FxNodeId) -> Result<(), FxEditError> {
        let Some(ed) = self.editor.as_ref() else {
            return Ok(());
        };
        let slot = ed.graph().node(parent).map_or(0, |p| p.children.len());
        ed.clone().connect(child, parent, slot)
    }

    fn attached(&self, id: FxNodeId) -> bool {
        self.graph().is_some_and(|g| is_attached(g, id))
    }

    /// Handle a mouse event while open. The overlay is modal, so every event
    /// is consumed by the caller whatever this returns; the return value is
    /// whether anything changed (needs a redraw).
    ///
    /// - Left click on a node: select it (entering edit mode from view mode).
    /// - Left press on a node, drag, release on another node: connect the
    ///   pressed node under it, at the end of its children. Refusals go to
    ///   the message line and change nothing.
    /// - Release on empty graph space: detach the pressed node (`x`).
    ///   Release outside the graph pane cancels.
    /// - Right click on a node: select it and detach it (`x`).
    /// - Wheel over the graph pane: scrolls the outline (no-op on the
    ///   rataflow canvas, which always fits the graph); never changes presets.
    pub fn handle_mouse(&mut self, mouse: MouseEvent) -> bool {
        let (col, row) = (mouse.column, mouse.row);
        // The load picker owns the overlay while it is open.
        if self.picker.is_some() {
            return false;
        }
        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                self.drag = None;
                let Some(id) = self.node_at(col, row) else {
                    return false;
                };
                if !self.select_node_id(id) {
                    return true;
                }
                self.drag = Some(MouseDrag {
                    from: id,
                    start: (col, row),
                    moved: false,
                    target: None,
                });
                let title = self
                    .graph()
                    .and_then(|g| g.node(id))
                    .map(|n| n.kind.title())
                    .unwrap_or_default();
                self.edit_msg = format!(
                    "selected {id} {title}; drag onto a parent to connect, right-click to detach"
                );
                true
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                let Some(mut drag) = self.drag else {
                    return false;
                };
                drag.moved |= (col, row) != drag.start;
                let in_pane = self.graph_pane.contains((col, row).into());
                let child = drag.from;
                let over = self.node_at(col, row).filter(|&n| n != child);
                let check = over.map(|p| (p, self.connect_check(child, p)));
                drag.target = check.as_ref().map(|(p, r)| (*p, r.is_ok()));
                self.edit_msg = match check {
                    Some((p, Ok(()))) => format!("release to connect {child} under {p}"),
                    Some((p, Err(e))) => format!("cannot connect {child} under {p}: {e}"),
                    None if !in_pane => "release outside the graph to cancel".into(),
                    None if self.node_at(col, row) == Some(child) => {
                        format!("drag {child} onto a parent to connect it")
                    }
                    None if self.attached(child) => format!("release to detach {child}"),
                    None => format!("drag {child} onto a parent to connect it"),
                };
                self.drag = Some(drag);
                true
            }
            MouseEventKind::Up(MouseButton::Left) => {
                let Some(drag) = self.drag.take() else {
                    return false;
                };
                if !drag.moved {
                    // A plain click: the press already selected the node.
                    return true;
                }
                let child = drag.from;
                let in_pane = self.graph_pane.contains((col, row).into());
                match self.node_at(col, row) {
                    Some(p) if p == child => {
                        self.edit_msg = format!("dropped {child} on itself; nothing changed");
                    }
                    Some(parent) => {
                        let slot = self
                            .graph()
                            .and_then(|g| g.node(parent))
                            .map_or(0, |p| p.children.len());
                        self.edit(|ed| {
                            ed.connect(child, parent, slot)
                                .map(|()| format!("connected {child} under {parent}"))
                        });
                        if self.edit_msg.starts_with("connected") && self.marked == Some(child) {
                            self.marked = None;
                        }
                    }
                    None if !in_pane => self.edit_msg = "drag cancelled".into(),
                    None if self.attached(child) => {
                        self.edit(|ed| ed.disconnect(child).map(|()| format!("detached {child}")));
                    }
                    None => {
                        self.edit_msg =
                            format!("{child} is not attached; drop it on a parent to connect it");
                    }
                }
                true
            }
            MouseEventKind::Down(MouseButton::Right) => {
                self.drag = None;
                let Some(id) = self.node_at(col, row) else {
                    return false;
                };
                if !self.select_node_id(id) {
                    return true;
                }
                self.edit(|ed| ed.disconnect(id).map(|()| format!("detached {id}")));
                true
            }
            MouseEventKind::ScrollDown | MouseEventKind::ScrollUp
                if self.graph_pane.contains((col, row).into()) && self.scrolls_outline() =>
            {
                let before = self.outline_scroll;
                self.outline_scroll = if mouse.kind == MouseEventKind::ScrollDown {
                    before + 1
                } else {
                    before.saturating_sub(1)
                };
                // Clamped against the row count when drawn.
                self.outline_scroll != before
            }
            _ => false,
        }
    }

    /// Whether the graph pane shows the outline (which the wheel scrolls)
    /// rather than the rataflow canvas.
    fn scrolls_outline(&self) -> bool {
        #[cfg(feature = "fx-graph")]
        {
            if self.state.as_ref().is_some_and(|s| s.flow.is_some()) {
                return false;
            }
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
                Span::styled(
                    if self.editing {
                        "editing "
                    } else if self.editor.is_some() {
                        "edited (unsaved) "
                    } else {
                        "read-only "
                    },
                    dim,
                ),
            ]))
            .title_bottom(Line::styled(
                if self.picker.is_some() {
                    " ↑↓ graph  Enter load  Esc cancel "
                } else if self.props.is_some() {
                    " ↑↓ field  ←→ change  0-9 Enter type  v value  b bind  n unbind  u/y undo/redo  Esc back "
                } else if self.editing {
                    " Tab node  a/A add  v value  m mark  c connect  x detach  d del  r root  p props  u/y undo  S save  L load  Esc done  · mouse: click select, drag onto parent connect, drag off / right-click detach "
                } else {
                    " ↑↓ preset  s script  e edit  L load  Esc close  · click a node to edit "
                },
                dim,
            ))
            .style(base);
        let inner = outer.inner(area);
        frame.render_widget(outer, area);

        let wide_list = self.props.is_some() || self.picker.is_some();
        let list_w = match (wide_list, inner.width) {
            (true, w) if w >= 90 => 34,
            (true, w) if w >= 60 => 26,
            (_, w) if w >= 60 => 22,
            _ => 14,
        };
        let [list_area, body] =
            Layout::horizontal([Constraint::Length(list_w), Constraint::Fill(1)])
                .spacing(1)
                .areas(inner);
        if self.picker.is_some() {
            self.render_picker(frame, list_area, theme);
        } else if self.props.is_some() {
            self.render_props(frame, list_area, theme);
        } else {
            self.render_list(frame, list_area, theme);
        }

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

    fn render_picker(&self, frame: &mut Frame<'_>, area: Rect, theme: &Theme) {
        let accent = Style::default()
            .fg(theme.general.primary)
            .add_modifier(Modifier::BOLD);
        let dim = Style::default().fg(theme.general.secondary);
        let mut lines = vec![Line::styled(
            "Load graph",
            Style::default().add_modifier(Modifier::BOLD),
        )];
        lines.push(Line::styled("newest first", dim));
        if let Some(p) = &self.picker {
            for (i, e) in p.entries.iter().enumerate() {
                let on = i == p.sel;
                lines.push(Line::styled(
                    format!("{}{}", if on { "▸ " } else { "  " }, e.name),
                    if on {
                        accent
                    } else {
                        Style::default().fg(theme.menu.fg)
                    },
                ));
            }
        }
        if let Some(dir) = &self.graph_dir {
            lines.push(Line::from(""));
            lines.push(Line::styled(dir.display().to_string(), dim));
        }
        frame.render_widget(
            Paragraph::new(lines)
                .wrap(ratatui::widgets::Wrap { trim: false })
                .block(Block::default().padding(Padding::horizontal(1))),
            area,
        );
    }

    fn render_props(&self, frame: &mut Frame<'_>, area: Rect, theme: &Theme) {
        let bold = Style::default().add_modifier(Modifier::BOLD);
        let accent = Style::default()
            .fg(theme.general.primary)
            .add_modifier(Modifier::BOLD);
        let dim = Style::default().fg(theme.general.secondary);
        let head = self
            .selected_node()
            .and_then(|id| {
                self.graph()
                    .and_then(|g| g.node(id))
                    .map(|n| format!("Props · {id} {}", n.kind.title()))
            })
            .unwrap_or_else(|| "Props".into());
        let mut lines = vec![Line::styled(head, bold)];
        let rows = self.prop_rows();
        let state = self.props.clone().unwrap_or_default();
        if rows.is_empty() {
            lines.push(Line::styled("no parameters", dim));
        }
        for (i, (f, bound)) in rows.iter().enumerate() {
            let on = i == state.field;
            let value = match (&state.typing, on) {
                (Some(t), true) => format!("{t}▏"),
                _ => f.value.clone(),
            };
            let mut spans = vec![
                Span::styled(if on { "▸ " } else { "  " }, accent),
                Span::styled(
                    format!("{:<9}", f.label),
                    if on { accent } else { Style::default() },
                ),
                Span::raw(value),
            ];
            if let Some(v) = bound {
                spans.push(Span::styled(format!(" ← {}", value_var(*v)), dim));
            } else if f.slot.is_some() {
                spans.push(Span::styled(" ◦", dim));
            }
            lines.push(Line::from(spans));
        }
        lines.push(Line::from(""));
        lines.push(Line::styled("◦ can take a value node", dim));
        if let Some(m) = self.marked {
            lines.push(Line::styled(format!("marked: {m}"), dim));
        }
        frame.render_widget(
            Paragraph::new(lines)
                .wrap(ratatui::widgets::Wrap { trim: false })
                .block(Block::default().padding(Padding::horizontal(1))),
            area,
        );
    }

    fn render_graph(&mut self, frame: &mut Frame<'_>, area: Rect, theme: &Theme) {
        let mut title = String::from(" graph ");
        if self.editing {
            let node = self.selected_node().and_then(|id| {
                self.graph()
                    .and_then(|g| g.node(id))
                    .map(|n| format!("{id} {}", n.kind.title()))
            });
            title = format!(" node {} ", node.unwrap_or_else(|| "none".into()));
            if let Some(m) = self.marked {
                // Names the mark even when the pane clips the marked node.
                title.push_str(&format!("· {MARK}marked {m} "));
            }
        }
        let mut block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(theme.general.secondary))
            .title(title);
        if !self.edit_msg.is_empty() {
            block = block.title_bottom(Line::styled(
                format!(" {} ", self.edit_msg),
                Style::default().fg(theme.general.secondary),
            ));
        }
        let inner = block.inner(area);
        frame.render_widget(block, area);
        self.graph_pane = inner;
        self.hits.clear();
        if inner.width == 0 || inner.height == 0 {
            return;
        }
        // Only edit mode has a node cursor to show.
        let highlight = self.selected_node().filter(|_| self.editing);
        let marked = self.marked.filter(|_| self.editing);
        // While dragging onto a node: green if it would take the child, red
        // if the connect would be refused.
        let target = self.drag.and_then(|d| d.target).map(|(id, ok)| {
            let colour = if ok {
                theme.general.success
            } else {
                theme.general.error
            };
            (id, Style::default().fg(colour).add_modifier(Modifier::BOLD))
        });
        #[cfg(feature = "fx-graph")]
        {
            let palette = rataflow_view::palette(theme);
            let mark_style = mark_style(theme);
            if let Some(flow) = self.ensure_state().flow.as_mut() {
                flow.theme = rataflow::Theme::Custom(palette);
                rataflow_view::select(flow, highlight);
                rataflow_view::mark(flow, marked, mark_style, target);
                // Read-only view: refit every frame so a resize keeps the
                // whole graph in view.
                flow.request_fit_view();
                frame.render_widget(&mut *flow, inner);
                let hits = rataflow_view::hits(flow, inner);
                self.hits = hits;
                return;
            }
        }
        let rows = self
            .graph()
            .map(|g| outline_lines_with(g, highlight, marked, target, theme))
            .unwrap_or_default();
        let max_scroll = rows.len().saturating_sub(inner.height as usize);
        self.outline_scroll = self.outline_scroll.min(max_scroll);
        let scroll = self.outline_scroll;
        self.hits = rows
            .iter()
            .enumerate()
            .skip(scroll)
            .take(inner.height as usize)
            .filter_map(|(i, (id, _))| {
                id.map(|id| NodeHit {
                    id,
                    rect: Rect::new(inner.x, inner.y + (i - scroll) as u16, inner.width, 1),
                    slop_row: None,
                })
            })
            .collect();
        let lines: Vec<Line> = rows.into_iter().map(|(_, l)| l).collect();
        frame.render_widget(Paragraph::new(lines).scroll((scroll as u16, 0)), inner);
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

/// Style of the `m`-marked node: the theme's warning colour, so it reads as
/// distinct from the selection (primary) on the canvas and in the outline.
fn mark_style(theme: &Theme) -> Style {
    Style::default()
        .fg(theme.general.warning)
        .add_modifier(Modifier::BOLD)
}

/// The text fallback for the canvas: the tree outline, then the Value nodes
/// it uses. The selected node gets the `▸` cursor and the primary style;
/// the marked node gets the [`MARK`] prefix and the warning style (the
/// selection style wins when one node is both, but keeps the prefix).
#[cfg(test)]
fn outline_lines(
    graph: &FxGraph,
    selected: Option<FxNodeId>,
    marked: Option<FxNodeId>,
    theme: &Theme,
) -> Vec<Line<'static>> {
    outline_lines_with(graph, selected, marked, None, theme)
        .into_iter()
        .map(|(_, l)| l)
        .collect()
}

/// One outline line of a Value node.
fn value_row(graph: &FxGraph, id: FxNodeId, depth: usize) -> Option<String> {
    let n = graph.node(id)?;
    Some(format!(
        "{}{} {} {}",
        "  ".repeat(depth),
        n.kind.title(),
        value_var(id),
        n.kind.summary()
    ))
}

/// Outline rows of the scratch tree under `id`, indented by depth.
fn scratch_rows(
    graph: &FxGraph,
    id: FxNodeId,
    depth: usize,
    out: &mut Vec<(Option<FxNodeId>, String)>,
) {
    let Some(n) = graph.node(id) else {
        return;
    };
    let text = if matches!(n.kind, FxNodeKind::Value { .. }) {
        value_row(graph, id, depth).unwrap_or_default()
    } else {
        let kind = graph.resolved_kind(id).unwrap_or_else(|| n.kind.clone());
        format!(
            "{}{} {}",
            "  ".repeat(depth),
            n.kind.title(),
            kind.summary()
        )
    };
    out.push((Some(id), text));
    for &c in &n.children {
        scratch_rows(graph, c, depth + 1, out);
    }
}

/// [`outline_lines`] with the node id of each row (`None` for a heading)
/// and an optional drag target style, which wins over the mark style. After
/// the tree and its values come the scratch trees, under a heading.
fn outline_lines_with(
    graph: &FxGraph,
    selected: Option<FxNodeId>,
    marked: Option<FxNodeId>,
    target: Option<(FxNodeId, Style)>,
    theme: &Theme,
) -> Vec<(Option<FxNodeId>, Line<'static>)> {
    let mut rows: Vec<(Option<FxNodeId>, String)> = graph
        .outline_rows()
        .into_iter()
        .map(|(id, t)| (Some(id), t))
        .collect();
    for id in graph.used_values() {
        if let Some(text) = value_row(graph, id, 0) {
            rows.push((Some(id), text));
        }
    }
    let scratch = if rows.is_empty() {
        Vec::new()
    } else {
        scratch_roots(graph)
    };
    let heading = (!scratch.is_empty()).then_some(rows.len());
    for id in scratch {
        scratch_rows(graph, id, 0, &mut rows);
    }
    let mut out: Vec<(Option<FxNodeId>, Line<'static>)> = rows
        .into_iter()
        .map(|(id, text)| {
            let is_sel = id.is_some() && id == selected;
            let is_mark = id.is_some() && id == marked;
            let cursor = if is_sel { "▸ " } else { "  " };
            let mark = if is_mark { MARK } else { "" };
            let line = format!("{cursor}{mark}{text}");
            let line = match target {
                Some((t, style)) if Some(t) == id => Line::styled(line, style),
                _ if is_sel => Line::styled(
                    line,
                    Style::default()
                        .fg(theme.general.primary)
                        .add_modifier(Modifier::BOLD),
                ),
                _ if is_mark => Line::styled(line, mark_style(theme)),
                _ => Line::from(line),
            };
            (id, line)
        })
        .collect();
    if let Some(at) = heading {
        out.insert(
            at,
            (
                None,
                Line::styled(
                    "scratch (not in the script)",
                    Style::default().fg(theme.general.secondary),
                ),
            ),
        );
    }
    out
}

#[cfg(feature = "fx-graph")]
mod rataflow_view {
    //! The only code that touches rataflow (pinned, behind `fx-graph`).

    use rataflow::{Edge, Flow, Node, Palette, StepEdge, TextContent};
    use ratatui::layout::Rect;
    use ratatui::style::Style;

    use super::{layout, FxGraph, FxNodeId, NodeHit, MARK};
    use crate::tui::theme::Theme;

    /// Show `id` as the marked node: [`MARK`] in front of its title and
    /// `style` on its border (rataflow's selection style still wins on the
    /// border when it is also selected). Every other node is unmarked, so
    /// `None` clears the mark. `target` (a drag's drop target) gets its own
    /// border style, over the mark's. Idempotent; runs every frame.
    pub(super) fn mark(
        flow: &mut Flow,
        id: Option<FxNodeId>,
        style: Style,
        target: Option<(FxNodeId, Style)>,
    ) {
        let want = id.map(|i| i.to_string());
        let target = target.map(|(i, s)| (i.to_string(), s));
        for (nid, content) in flow.nodes_content_mut() {
            let on = want.as_deref() == Some(nid);
            if let Some(title) = content.title.as_mut() {
                let bare = title.strip_prefix(MARK).unwrap_or(title).to_string();
                *title = if on { format!("{MARK}{bare}") } else { bare };
            }
            content.border_style = match &target {
                Some((t, s)) if t == nid => Some(*s),
                _ => on.then_some(style),
            };
        }
    }

    /// Mouse targets of every node as rataflow just drew them in `pane`:
    /// `Flow::node_terminal_rect` (rataflow's own fit-to-view transform),
    /// clipped to the pane. Call right after rendering the flow.
    ///
    /// rataflow reports (left, top, right, bottom) with right and bottom
    /// exclusive. Zoomed out below one row per world row, a node is drawn
    /// one row taller than that rect (its handle row lands on `bottom`), so
    /// that row becomes the slop row.
    pub(super) fn hits(flow: &Flow, pane: Rect) -> Vec<NodeHit> {
        let clip = |lo: i32, hi: i32, min: u16, max: u16| -> (u16, u16) {
            let a = lo.clamp(i32::from(min), i32::from(max)) as u16;
            let b = hi.clamp(i32::from(min), i32::from(max)) as u16;
            (a, b.max(a))
        };
        flow.nodes()
            .filter_map(|n| {
                let id: FxNodeId = n.id.parse().ok()?;
                let (l, t, r, b) = flow.node_terminal_rect(&n.id)?;
                let (x0, x1) = clip(l, r, pane.x, pane.right());
                let (y0, y1) = clip(t, b, pane.y, pane.bottom());
                let slop_row =
                    (b - t < 3 && b >= i32::from(pane.y) && b < i32::from(pane.bottom()))
                        .then_some(b as u16);
                Some(NodeHit {
                    id,
                    rect: Rect::new(x0, y0, x1 - x0, y1 - y0),
                    slop_row,
                })
            })
            .collect()
    }

    /// Show `id` as rataflow's selected node (accent border), or nothing.
    /// An id that is not on the canvas clears it.
    pub(super) fn select(flow: &mut Flow, id: Option<FxNodeId>) {
        match id {
            Some(id) => flow.select_node(&id.to_string()),
            None => flow.clear_selection(),
        }
    }

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
        let on_canvas = |id: u32| nodes.iter().any(|n| n.id == id.to_string());
        let mut edges: Vec<Edge<StepEdge>> = graph
            .edges()
            .into_iter()
            .filter(|e| on_canvas(e.child))
            .map(|e| {
                Edge::new(
                    format!("e{}-{}", e.child, e.parent),
                    e.child.to_string(),
                    e.parent.to_string(),
                )
                .with_deletable(false)
            })
            .collect();
        // One edge from each used Value node to each node that binds it.
        let mut links = std::collections::BTreeSet::new();
        for n in &graph.nodes {
            if on_canvas(n.id) {
                for &v in n.params.values() {
                    links.insert((v, n.id));
                }
            }
        }
        for (v, user) in links {
            edges.push(
                Edge::new(format!("v{v}-{user}"), v.to_string(), user.to_string())
                    .with_deletable(false),
            );
        }
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

        #[test]
        fn flow_draws_value_nodes_and_one_edge_per_binding() {
            let mut ed =
                crate::tui::fx_script::edit::GraphEditor::new(presets::all().remove(0).graph)
                    .unwrap();
            let users: Vec<_> = ed
                .graph()
                .reachable()
                .into_iter()
                .filter(|&id| {
                    ed.graph()
                        .node(id)
                        .unwrap()
                        .kind
                        .slot_type("timer")
                        .is_some()
                })
                .collect();
            assert!(users.len() >= 2);
            let v = ed.extract_value(users[0], "timer").unwrap();
            ed.bind(users[1], "timer", v).unwrap();
            let g = ed.graph();
            let flow = build_flow(g).unwrap();
            assert_eq!(flow.nodes().count(), g.reachable().len() + 1);
            assert_eq!(flow.edges().len(), g.reachable().len() - 1 + 2);
        }

        #[test]
        fn select_highlights_one_node_or_none() {
            let g = presets::all().remove(0).graph;
            let mut flow = build_flow(&g).unwrap();
            select(&mut flow, Some(g.root));
            assert_eq!(flow.first_selected_node_id(), Some(g.root.to_string()));
            assert_eq!(flow.selected_nodes().count(), 1);
            select(&mut flow, None);
            assert!(!flow.has_selected_nodes());
            // An id that is not on the canvas clears it.
            select(&mut flow, Some(9999));
            assert!(!flow.has_selected_nodes());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::fx_script::graph::{
        FxEffect, FxGraphBuilder, FxInterp, FxNodeKind, FxTimer, FxValueType,
    };
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
    fn edit_keys_wire_a_leaf_under_a_parent_and_undo_it() {
        let mut v = FxGraphView::new();
        v.open = true;
        let before = v.graph().unwrap().clone();
        assert!(v.handle_key(KeyCode::Char('e')));
        assert!(v.is_editing());
        // Add a leaf (selected), mark it, add a sequence (selected), connect.
        v.handle_key(KeyCode::Char('a'));
        let leaf = v.selected_node().unwrap();
        v.handle_key(KeyCode::Char('m'));
        v.handle_key(KeyCode::Char('A'));
        let seq = v.selected_node().unwrap();
        assert_ne!(seq, leaf);
        v.handle_key(KeyCode::Char('c'));
        assert!(v.edit_msg.starts_with("connected"), "{}", v.edit_msg);
        assert!(v
            .graph()
            .unwrap()
            .node(seq)
            .unwrap()
            .children
            .contains(&leaf));
        assert_eq!(v.marked, None);
        // Undo removes the attachment.
        v.handle_key(KeyCode::Char('u'));
        assert!(v.graph().unwrap().node(seq).unwrap().children.is_empty());
        assert_ne!(v.graph().unwrap(), &before);
        // Esc keeps the edit but leaves edit mode; the preset list moves again.
        v.handle_key(KeyCode::Esc);
        assert!(!v.is_editing());
        v.handle_key(KeyCode::Down);
        assert!(v.graph().is_some() && v.editor.is_none());
    }

    #[test]
    fn edit_errors_show_in_the_message_and_change_nothing() {
        let mut v = FxGraphView::new();
        v.open = true;
        v.handle_key(KeyCode::Char('e'));
        let before = v.graph().unwrap().clone();
        // Walk the selection onto the root, then try to delete it.
        let root = before.root;
        while v.selected_node() != Some(root) {
            v.handle_key(KeyCode::Tab);
        }
        v.handle_key(KeyCode::Char('d'));
        assert!(v.edit_msg.contains("root"), "{}", v.edit_msg);
        assert_eq!(v.graph().unwrap(), &before);
    }

    #[test]
    fn edited_graph_renders_and_recompiles() {
        let mut term = Terminal::new(TestBackend::new(140, 40)).unwrap();
        let mut v = FxGraphView::new();
        v.open = true;
        v.handle_key(KeyCode::Char('e'));
        v.handle_key(KeyCode::Char('A'));
        term.draw(|f| v.render(f, f.area(), &Theme::default()))
            .unwrap();
        assert!(screen_text(&term).contains("editing"));
        // A scratch sequence is not reachable, so the emitted script is the root's.
        assert!(matches!(v.status(), CompileStatus::Ok(_)));
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

    /// Move the node cursor to the first node `pred` accepts.
    fn select_where(v: &mut FxGraphView, pred: impl Fn(&FxNodeKind) -> bool) -> FxNodeId {
        for _ in 0..v.node_ids().len() {
            let id = v.selected_node().unwrap();
            if pred(&v.graph().unwrap().node(id).unwrap().kind) {
                return id;
            }
            v.handle_key(KeyCode::Tab);
        }
        panic!("no matching node");
    }

    fn field_index(v: &FxGraphView, label: &str) -> usize {
        v.prop_rows()
            .iter()
            .position(|(f, _)| f.label == label)
            .unwrap_or_else(|| panic!("no field {label}"))
    }

    fn goto_field(v: &mut FxGraphView, label: &str) {
        let want = field_index(v, label);
        while v.props.as_ref().unwrap().field != want {
            v.handle_key(KeyCode::Down);
        }
    }

    fn has_timer(k: &FxNodeKind) -> bool {
        matches!(k, FxNodeKind::Effect { .. }) && k.slot_type("timer").is_some()
    }

    fn timer_ms(v: &FxGraphView, id: FxNodeId) -> u32 {
        match v
            .graph()
            .unwrap()
            .resolved_kind(id)
            .unwrap()
            .slot_value("timer")
        {
            Some(FxValue::Timer(t)) => t.ms,
            other => panic!("{other:?}"),
        }
    }

    fn editing_view() -> FxGraphView {
        let mut v = FxGraphView::new();
        v.graph_dir = None;
        v.open = true;
        v.handle_key(KeyCode::Char('e'));
        v
    }

    #[test]
    fn props_step_and_typed_values_are_undoable_edits() {
        let mut v = editing_view();
        let id = select_where(&mut v, has_timer);
        let ms = timer_ms(&v, id);
        v.handle_key(KeyCode::Char('p'));
        assert!(v.props.is_some());
        goto_field(&mut v, "ms");
        v.handle_key(KeyCode::Right);
        assert_eq!(timer_ms(&v, id), ms + 50);
        // Type a number and commit it.
        for c in "1234".chars() {
            v.handle_key(KeyCode::Char(c));
        }
        v.handle_key(KeyCode::Backspace);
        v.handle_key(KeyCode::Enter);
        assert_eq!(timer_ms(&v, id), 123);
        assert!(matches!(v.status(), CompileStatus::Ok(_)));
        v.handle_key(KeyCode::Char('u'));
        assert_eq!(timer_ms(&v, id), ms + 50);
        v.handle_key(KeyCode::Char('u'));
        assert_eq!(timer_ms(&v, id), ms);
        // Esc closes the panel but stays in edit mode.
        v.handle_key(KeyCode::Esc);
        assert!(v.props.is_none() && v.is_editing());
    }

    #[test]
    fn props_make_value_then_steps_change_the_value_node() {
        let mut v = editing_view();
        let id = select_where(&mut v, has_timer);
        v.handle_key(KeyCode::Char('p'));
        goto_field(&mut v, "ms");
        v.handle_key(KeyCode::Char('v'));
        let vid = *v
            .graph()
            .unwrap()
            .node(id)
            .unwrap()
            .params
            .get("timer")
            .unwrap();
        assert!(
            v.script().contains(&format!("let v{vid} = (")),
            "{}",
            v.script()
        );
        let inline = v.graph().unwrap().node(id).unwrap().kind.clone();
        v.handle_key(KeyCode::Right);
        // The Value node changed; the node's own inline value did not.
        assert_eq!(v.graph().unwrap().node(id).unwrap().kind, inline);
        assert!(matches!(
            v.graph().unwrap().node(vid).unwrap().kind,
            FxNodeKind::Value { .. }
        ));
        assert!(
            matches!(v.status(), CompileStatus::Ok(_)),
            "{:?}",
            v.status()
        );
        // Unbind falls back to the inline value.
        v.handle_key(KeyCode::Char('n'));
        assert!(v.graph().unwrap().node(id).unwrap().params.is_empty());
    }

    #[test]
    fn binding_a_marked_value_is_type_checked() {
        let mut v = editing_view();
        // A new colour value, marked as the thing to bind.
        v.handle_key(KeyCode::Char('v'));
        let colour = v.selected_node().unwrap();
        v.handle_key(KeyCode::Char('m'));
        let id = select_where(&mut v, has_timer);
        v.handle_key(KeyCode::Char('p'));
        goto_field(&mut v, "ms");
        let before = v.graph().unwrap().clone();
        v.handle_key(KeyCode::Char('b'));
        assert!(v.edit_msg.contains("takes a timer value"), "{}", v.edit_msg);
        assert_eq!(v.graph().unwrap(), &before);
        // Turn the value into a timer from its own Props, then bind again.
        v.handle_key(KeyCode::Esc);
        select_where(&mut v, |k| matches!(k, FxNodeKind::Value { .. }));
        v.handle_key(KeyCode::Char('p'));
        goto_field(&mut v, "type");
        v.handle_key(KeyCode::Right);
        assert_eq!(
            match v.graph().unwrap().node(colour).unwrap().kind {
                FxNodeKind::Value { value } => value.ty(),
                _ => unreachable!(),
            },
            FxValueType::Timer
        );
        v.handle_key(KeyCode::Esc);
        while v.selected_node() != Some(id) {
            v.handle_key(KeyCode::Tab);
        }
        v.handle_key(KeyCode::Char('p'));
        goto_field(&mut v, "ms");
        v.handle_key(KeyCode::Char('b'));
        assert_eq!(
            v.graph().unwrap().node(id).unwrap().params.get("timer"),
            Some(&colour),
            "{}",
            v.edit_msg
        );
        assert_eq!(timer_ms(&v, id), 500);
        assert!(matches!(v.status(), CompileStatus::Ok(_)));
    }

    #[test]
    fn value_nodes_refuse_to_connect_as_children() {
        let mut v = editing_view();
        v.handle_key(KeyCode::Char('v'));
        v.handle_key(KeyCode::Char('m'));
        v.handle_key(KeyCode::Char('A'));
        v.handle_key(KeyCode::Char('c'));
        assert!(v.edit_msg.contains("is a value"), "{}", v.edit_msg);
    }

    #[test]
    fn save_writes_both_files_and_load_restores_the_graph() {
        let dir = tempfile::tempdir().unwrap();
        let mut v = editing_view();
        v.graph_dir = Some(dir.path().to_path_buf());
        let id = select_where(&mut v, has_timer);
        v.handle_key(KeyCode::Char('p'));
        goto_field(&mut v, "ms");
        v.handle_key(KeyCode::Char('v'));
        v.handle_key(KeyCode::Esc);
        let edited = v.graph().unwrap().clone();
        v.handle_key(KeyCode::Char('S'));
        assert!(v.edit_msg.starts_with("saved"), "{}", v.edit_msg);
        let name = v.save_name();
        assert!(graph_file::graph_path(dir.path(), &name).exists());
        let script = std::fs::read_to_string(graph_file::script_path(dir.path(), &name)).unwrap();
        assert!(script.contains("let v"), "{script}");
        // Leave edit mode and reselect the preset: the edit is gone ...
        v.handle_key(KeyCode::Esc);
        v.select(v.selected());
        assert!(v.graph().unwrap().node(id).unwrap().params.is_empty());
        // ... until it is loaded back through the picker.
        v.handle_key(KeyCode::Char('L'));
        assert!(v.picker.is_some(), "{}", v.edit_msg);
        v.handle_key(KeyCode::Enter);
        assert!(v.picker.is_none());
        assert!(v.is_editing(), "{}", v.edit_msg);
        assert_eq!(v.graph().unwrap(), &edited);
        // A second load is one undoable edit on top.
        v.handle_key(KeyCode::Char('L'));
        v.handle_key(KeyCode::Enter);
        assert!(v.editor.as_ref().unwrap().can_undo());
    }

    #[test]
    fn save_and_load_errors_show_in_the_message_line() {
        let mut v = editing_view();
        v.handle_key(KeyCode::Char('S'));
        assert!(v.edit_msg.contains("no config dir"), "{}", v.edit_msg);
        let dir = tempfile::tempdir().unwrap();
        v.graph_dir = Some(dir.path().to_path_buf());
        let before = v.graph().unwrap().clone();
        // An empty dir opens no picker and says so.
        v.handle_key(KeyCode::Char('L'));
        assert!(v.edit_msg.starts_with("no saved graphs"), "{}", v.edit_msg);
        assert!(v.picker.is_none());
        assert_eq!(v.graph().unwrap(), &before);
        // A file that is not a graph is listed, but picking it is reported,
        // not loaded.
        std::fs::write(graph_file::graph_path(dir.path(), "broken"), "[]").unwrap();
        v.handle_key(KeyCode::Char('L'));
        assert!(v.picker.is_some(), "{}", v.edit_msg);
        v.handle_key(KeyCode::Enter);
        assert!(v.edit_msg.starts_with("load failed"), "{}", v.edit_msg);
        assert_eq!(v.graph().unwrap(), &before);
        assert!(v.graph_name.is_none());
        // A dir that cannot be listed (a file in its place) is reported.
        let not_dir = dir.path().join("plain");
        std::fs::write(&not_dir, "x").unwrap();
        v.graph_dir = Some(not_dir);
        v.handle_key(KeyCode::Char('L'));
        assert!(v.edit_msg.starts_with("load failed"), "{}", v.edit_msg);
        assert!(v.picker.is_none());
        assert_eq!(v.graph().unwrap(), &before);
    }

    /// Save the first preset's graph under each name, oldest first, with
    /// distinct mtimes. Returns the dir.
    fn saved_graphs(names: &[&str]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let all = presets::all();
        for (i, name) in names.iter().enumerate() {
            // A distinct Sleep leaf per file so loads are told apart.
            let mut ed = GraphEditor::new(all[0].graph.clone()).unwrap();
            ed.add_node(FxNodeKind::Effect {
                effect: FxEffect::Sleep { ms: i as u32 + 1 },
            });
            let saved = graph_file::save(dir.path(), name, ed.graph()).unwrap();
            let f = std::fs::OpenOptions::new()
                .write(true)
                .open(&saved.graph)
                .unwrap();
            f.set_modified(
                std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_000 + i as u64),
            )
            .unwrap();
        }
        dir
    }

    fn picker_names(v: &FxGraphView) -> Vec<String> {
        v.picker
            .as_ref()
            .unwrap()
            .entries
            .iter()
            .map(|e| e.name.clone())
            .collect()
    }

    #[test]
    fn load_picker_lists_newest_first_and_loads_the_pick() {
        let dir = saved_graphs(&["alpha", "beta", "gamma"]);
        let mut v = FxGraphView::new();
        v.open = true;
        v.graph_dir = Some(dir.path().to_path_buf());
        // Works from the read-only view too.
        v.handle_key(KeyCode::Char('L'));
        assert_eq!(picker_names(&v), vec!["gamma", "beta", "alpha"]);
        // Up wraps to the oldest; Down comes back round to the newest.
        v.handle_key(KeyCode::Up);
        assert_eq!(v.picker.as_ref().unwrap().sel, 2);
        v.handle_key(KeyCode::Down);
        v.handle_key(KeyCode::Down);
        assert_eq!(v.picker.as_ref().unwrap().sel, 1);
        // Other keys are not taken by the edit mode while picking.
        assert!(!v.handle_key(KeyCode::Char('a')));
        assert!(v.editor.is_none());
        v.handle_key(KeyCode::Enter);
        assert!(v.picker.is_none());
        assert!(v.is_editing());
        assert_eq!(v.edit_msg, "loaded beta");
        let want = graph_file::load(dir.path(), "beta").unwrap();
        assert_eq!(v.graph().unwrap(), &want);
        // S now saves back under the loaded name.
        assert_eq!(v.save_name(), "beta");
        // Selecting a preset forgets the loaded name.
        v.handle_key(KeyCode::Esc);
        v.select(0);
        assert_ne!(v.save_name(), "beta");
    }

    #[test]
    fn load_picker_esc_cancels_and_changes_nothing() {
        let dir = saved_graphs(&["one", "two"]);
        let mut v = editing_view();
        v.graph_dir = Some(dir.path().to_path_buf());
        let before = v.graph().unwrap().clone();
        v.handle_key(KeyCode::Char('L'));
        v.handle_key(KeyCode::Down);
        v.handle_key(KeyCode::Esc);
        assert!(v.picker.is_none());
        assert_eq!(v.edit_msg, "load cancelled");
        assert_eq!(v.graph().unwrap(), &before);
        // Esc only closed the picker; edit mode is still on.
        assert!(v.is_editing());
        assert!(!v.editor.as_ref().unwrap().can_undo());
    }

    #[test]
    fn load_picker_renders_names() {
        let dir = saved_graphs(&["first_graph", "second_graph"]);
        let mut v = FxGraphView::new();
        v.open = true;
        v.graph_dir = Some(dir.path().to_path_buf());
        v.handle_key(KeyCode::Char('L'));
        let mut term = Terminal::new(TestBackend::new(140, 40)).unwrap();
        term.draw(|f| v.render(f, f.area(), &Theme::default()))
            .unwrap();
        let text = screen_text(&term);
        assert!(text.contains("Load graph"), "{text}");
        assert!(text.contains("▸ second_graph"), "{text}");
        assert!(text.contains("  first_graph"), "{text}");
        assert!(text.contains("Enter load"), "{text}");
        for (w, h) in [(30, 10), (60, 20), (4, 4)] {
            let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
            term.draw(|f| v.render(f, f.area(), &Theme::default()))
                .unwrap();
        }
    }

    #[test]
    fn outline_marks_the_m_node_distinctly_from_the_selection() {
        let g = presets::all().remove(0).graph;
        let rows = g.outline_rows();
        let (root, other) = (rows[0].0, rows[1].0);
        let theme = Theme::default();
        let lines = outline_lines(&g, Some(root), Some(other), &theme);
        let text: Vec<String> = lines.iter().map(|l| l.to_string()).collect();
        assert!(text[1].starts_with(&format!("  {MARK}")), "{text:?}");
        assert_eq!(lines[1].style.fg, Some(theme.general.warning));
        assert_eq!(text.iter().filter(|l| l.contains(MARK)).count(), 1);
        // Marked and selected at once: the cursor and the mark both show.
        let lines = outline_lines(&g, Some(root), Some(root), &theme);
        assert!(lines[0].to_string().starts_with(&format!("▸ {MARK}")));
        // No mark, no prefix.
        let lines = outline_lines(&g, Some(root), None, &theme);
        assert!(lines.iter().all(|l| !l.to_string().contains(MARK)));
    }

    fn render_text(v: &mut FxGraphView) -> String {
        let mut term = Terminal::new(TestBackend::new(160, 44)).unwrap();
        term.draw(|f| v.render(f, f.area(), &Theme::default()))
            .unwrap();
        screen_text(&term)
    }

    #[test]
    fn marked_node_shows_on_screen_until_the_mark_is_cleared() {
        let mut v = editing_view();
        v.handle_key(KeyCode::Tab);
        let id = v.selected_node().unwrap();
        assert!(!render_text(&mut v).contains(MARK.trim()));
        v.handle_key(KeyCode::Char('m'));
        let text = render_text(&mut v);
        // In the graph pane title, and on the node itself (canvas title
        // under fx-graph, outline row without it).
        assert!(text.contains(&format!("marked {id}")), "{text}");
        assert!(
            text.matches(MARK.trim()).count() >= 2,
            "mark on the node too:\n{text}"
        );
        // Esc leaves edit mode and clears the mark.
        v.handle_key(KeyCode::Esc);
        assert_eq!(v.marked, None);
        assert!(!render_text(&mut v).contains(MARK.trim()));
    }

    #[test]
    fn mark_is_cleared_by_connect_and_by_undoing_the_marked_node_away() {
        let mut v = editing_view();
        v.handle_key(KeyCode::Char('a'));
        let leaf = v.selected_node().unwrap();
        v.handle_key(KeyCode::Char('m'));
        assert_eq!(v.marked, Some(leaf));
        // Undo the add: the marked node is gone, so is the mark.
        v.handle_key(KeyCode::Char('u'));
        assert_eq!(v.marked, None);
        assert!(!render_text(&mut v).contains(MARK.trim()));
        // Mark, connect: cleared.
        v.handle_key(KeyCode::Char('a'));
        v.handle_key(KeyCode::Char('m'));
        v.handle_key(KeyCode::Char('A'));
        v.handle_key(KeyCode::Char('c'));
        assert!(v.edit_msg.starts_with("connected"), "{}", v.edit_msg);
        assert_eq!(v.marked, None);
        assert!(!render_text(&mut v).contains(MARK.trim()));
    }

    #[cfg(feature = "fx-graph")]
    #[test]
    fn canvas_marks_the_m_node_title_and_border() {
        let mut v = editing_view();
        v.handle_key(KeyCode::Tab);
        let id = v.selected_node().unwrap();
        v.handle_key(KeyCode::Char('m'));
        // Move the cursor off it, so the mark is not hidden by selection.
        v.handle_key(KeyCode::Tab);
        render_text(&mut v);
        let theme = Theme::default();
        let flow = v.state.as_ref().unwrap().flow.as_ref().unwrap();
        for n in flow.nodes() {
            let title = n.content.title.clone().unwrap_or_default();
            if n.id == id.to_string() {
                assert!(title.starts_with(MARK), "{title}");
                assert_eq!(n.content.border_style, Some(mark_style(&theme)));
            } else {
                assert!(!title.starts_with(MARK), "{} {title}", n.id);
                assert_eq!(n.content.border_style, None);
            }
        }
        // Re-marking is idempotent: one prefix, not two.
        render_text(&mut v);
        let flow = v.state.as_ref().unwrap().flow.as_ref().unwrap();
        let title = flow.node(&id.to_string()).unwrap().content.title.clone();
        assert_eq!(title.unwrap().matches(MARK).count(), 1);
        // Clearing the mark restores the plain title.
        v.handle_key(KeyCode::Esc);
        render_text(&mut v);
        let flow = v.state.as_ref().unwrap().flow.as_ref().unwrap();
        assert!(flow.nodes().all(
            |n| !n.content.title.clone().unwrap_or_default().contains(MARK)
                && n.content.border_style.is_none()
        ));
    }

    #[test]
    fn layout_places_used_values_in_a_column_of_their_own() {
        let mut ed = GraphEditor::new(presets::all().remove(0).graph).unwrap();
        let user = ed
            .graph()
            .reachable()
            .into_iter()
            .find(|&id| {
                ed.graph()
                    .node(id)
                    .unwrap()
                    .kind
                    .slot_type("timer")
                    .is_some()
            })
            .unwrap();
        let vid = ed.extract_value(user, "timer").unwrap();
        let g = ed.graph();
        let placed = layout(g);
        assert_eq!(placed.len(), g.reachable().len() + 1);
        let at = |id| placed.iter().find(|n| n.id == id).unwrap();
        assert!(at(vid).x + at(vid).width <= at(user).x);
        assert!(at(vid).title.contains(&format!("v{vid}")));
        for (i, a) in placed.iter().enumerate() {
            for b in &placed[i + 1..] {
                assert!(!overlaps(a, b), "{} overlaps {}", a.id, b.id);
            }
        }
    }

    #[test]
    fn outline_fallback_marks_the_selected_node_and_lists_values() {
        let mut ed = GraphEditor::new(presets::all().remove(0).graph).unwrap();
        let root = ed.graph().root;
        let user = ed
            .graph()
            .reachable()
            .into_iter()
            .find(|&id| {
                ed.graph()
                    .node(id)
                    .unwrap()
                    .kind
                    .slot_type("timer")
                    .is_some()
            })
            .unwrap();
        let vid = ed.extract_value(user, "timer").unwrap();
        let lines = outline_lines(ed.graph(), Some(root), None, &Theme::default());
        let text: Vec<String> = lines.iter().map(|l| l.to_string()).collect();
        assert!(text[0].starts_with("▸ "), "{text:?}");
        assert!(text[1..].iter().all(|l| l.starts_with("  ")), "{text:?}");
        assert!(
            text.last().unwrap().contains(&format!("timer v{vid}")),
            "{text:?}"
        );
        assert!(lines[0].style.add_modifier.contains(Modifier::BOLD));
    }

    #[cfg(feature = "fx-graph")]
    #[test]
    fn edit_mode_highlights_the_selected_node_on_the_canvas() {
        let mut term = Terminal::new(TestBackend::new(140, 40)).unwrap();
        let mut v = editing_view();
        v.handle_key(KeyCode::Tab);
        let sel = v.selected_node().unwrap();
        term.draw(|f| v.render(f, f.area(), &Theme::default()))
            .unwrap();
        let flow = v.state.as_ref().unwrap().flow.as_ref().unwrap();
        assert_eq!(flow.first_selected_node_id(), Some(sel.to_string()));
        // Read-only view: no highlight.
        v.handle_key(KeyCode::Esc);
        term.draw(|f| v.render(f, f.area(), &Theme::default()))
            .unwrap();
        let flow = v.state.as_ref().unwrap().flow.as_ref().unwrap();
        assert!(!flow.has_selected_nodes());
    }

    fn mouse(kind: MouseEventKind, column: u16, row: u16) -> MouseEvent {
        MouseEvent {
            kind,
            column,
            row,
            modifiers: ratatui::crossterm::event::KeyModifiers::NONE,
        }
    }

    fn draw_at(v: &mut FxGraphView, w: u16, h: u16) -> Terminal<TestBackend> {
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
        term.draw(|f| v.render(f, f.area(), &Theme::default()))
            .unwrap();
        term
    }

    /// A cell inside `id`'s mouse target, from the last render.
    fn cell_of(v: &FxGraphView, id: FxNodeId) -> (u16, u16) {
        let h = v
            .hits
            .iter()
            .find(|h| h.id == id && h.rect.width > 0 && h.rect.height > 0)
            .unwrap_or_else(|| panic!("node {id} not on screen: {:?}", v.hits));
        (h.rect.x + h.rect.width / 2, h.rect.y)
    }

    /// A graph-pane cell over no node.
    fn empty_cell(v: &FxGraphView) -> (u16, u16) {
        let p = v.graph_pane;
        for row in (p.y..p.bottom()).rev() {
            for col in (p.x..p.right()).rev() {
                if v.node_at(col, row).is_none() {
                    return (col, row);
                }
            }
        }
        panic!("no empty cell in {p:?}");
    }

    fn drag(v: &mut FxGraphView, from: (u16, u16), to: (u16, u16)) {
        let left = MouseButton::Left;
        v.handle_mouse(mouse(MouseEventKind::Down(left), from.0, from.1));
        v.handle_mouse(mouse(MouseEventKind::Drag(left), to.0, to.1));
        v.handle_mouse(mouse(MouseEventKind::Up(left), to.0, to.1));
    }

    fn children(v: &FxGraphView, id: FxNodeId) -> Vec<FxNodeId> {
        v.graph().unwrap().node(id).unwrap().children.clone()
    }

    #[test]
    fn hit_test_maps_cells_to_the_nodes_drawn_there() {
        // Small and large panes: rataflow zooms out (< 1) and in (> 1).
        for (w, h) in [(140, 40), (300, 60), (80, 24)] {
            let mut v = editing_view();
            v.show_script = false;
            let term = draw_at(&mut v, w, h);
            let buf = term.backend().buffer();
            let g = v.graph().unwrap().clone();
            let placed = layout(&g);
            assert_eq!(v.hits.len(), placed.len(), "{w}x{h}: one target per node");
            for p in &placed {
                let hit = v.hits.iter().find(|h| h.id == p.id).unwrap();
                let r = hit.rect;
                assert!(
                    v.graph_pane.intersection(r) == r,
                    "{w}x{h}: clipped to pane"
                );
                if r.width == 0 || r.height == 0 {
                    continue;
                }
                // Every cell of the target maps back to this node.
                for y in r.y..r.bottom() {
                    for x in r.x..r.right() {
                        assert_eq!(v.node_at(x, y), Some(p.id), "{w}x{h} ({x},{y})");
                    }
                }
                // ...and the node is really drawn there.
                let row: String = (r.x..r.right())
                    .map(|x| buf[(x, r.y)].symbol().to_string())
                    .collect();
                #[cfg(feature = "fx-graph")]
                {
                    // Corners and title, unless the pane clips the node.
                    let pane = v.graph_pane;
                    if r.x > pane.x {
                        assert!(row.starts_with('╭'), "{w}x{h} {}: {row:?}", p.id);
                        let name: String = p.title.chars().take(3).collect();
                        assert!(row.contains(&name), "{w}x{h} {}: {row:?}", p.id);
                    }
                    if r.x > pane.x && r.right() < pane.right() {
                        assert!(row.trim_end().ends_with('╮'), "{w}x{h} {}: {row:?}", p.id);
                        // The cells either side are not this node's.
                        assert_ne!(v.node_at(r.x - 1, r.y), Some(p.id));
                        assert_ne!(v.node_at(r.right(), r.y), Some(p.id));
                    }
                }
                #[cfg(not(feature = "fx-graph"))]
                assert!(row.contains(g.node(p.id).unwrap().kind.title()), "{row:?}");
            }
            // Outside the graph pane nothing is a node.
            assert_eq!(v.node_at(0, 0), None);
        }
    }

    #[test]
    fn click_selects_the_node_and_enters_edit_mode() {
        let mut v = FxGraphView::new();
        v.graph_dir = None;
        v.open = true;
        draw_at(&mut v, 160, 44);
        let g = v.graph().unwrap().clone();
        let target = g.reachable()[2];
        let (c, r) = cell_of(&v, target);
        assert!(!v.is_editing());
        let left = MouseButton::Left;
        assert!(v.handle_mouse(mouse(MouseEventKind::Down(left), c, r)));
        v.handle_mouse(mouse(MouseEventKind::Up(left), c, r));
        assert!(v.is_editing(), "a click enters edit mode, as e does");
        assert_eq!(v.selected_node(), Some(target));
        assert_eq!(v.graph().unwrap(), &g, "a click changes nothing");
        // Same selection Tab would reach.
        let mut k = editing_view();
        while k.selected_node() != Some(target) {
            k.handle_key(KeyCode::Tab);
        }
        assert_eq!(k.node_sel, v.node_sel);
        // A click on empty space keeps the selection.
        draw_at(&mut v, 160, 44);
        let (c, r) = empty_cell(&v);
        v.handle_mouse(mouse(MouseEventKind::Down(left), c, r));
        assert_eq!(v.selected_node(), Some(target));
    }

    #[test]
    fn drag_connects_a_scratch_child_under_a_parent() {
        let mut v = editing_view();
        v.handle_key(KeyCode::Char('a'));
        let leaf = v.selected_node().unwrap();
        v.handle_key(KeyCode::Char('A'));
        let seq = v.selected_node().unwrap();
        // Scratch nodes are on screen, so they can be dragged.
        draw_at(&mut v, 160, 44);
        let (from, to) = (cell_of(&v, leaf), cell_of(&v, seq));
        let left = MouseButton::Left;
        v.handle_mouse(mouse(MouseEventKind::Down(left), from.0, from.1));
        assert_eq!(v.selected_node(), Some(leaf));
        v.handle_mouse(mouse(MouseEventKind::Drag(left), to.0, to.1));
        assert_eq!(v.drag.unwrap().target, Some((seq, true)));
        assert!(
            v.edit_msg.contains(&format!("connect {leaf} under {seq}")),
            "{}",
            v.edit_msg
        );
        // The drop target is highlighted while dragging.
        let theme = Theme::default();
        let ok = Style::default()
            .fg(theme.general.success)
            .add_modifier(Modifier::BOLD);
        draw_at(&mut v, 160, 44);
        #[cfg(feature = "fx-graph")]
        {
            let flow = v.state.as_ref().unwrap().flow.as_ref().unwrap();
            let node = flow.node(&seq.to_string()).unwrap();
            assert_eq!(node.content.border_style, Some(ok));
        }
        #[cfg(not(feature = "fx-graph"))]
        {
            let rows = outline_lines_with(v.graph().unwrap(), None, None, Some((seq, ok)), &theme);
            let (_, line) = rows.iter().find(|(id, _)| *id == Some(seq)).unwrap();
            assert_eq!(line.style, ok);
        }
        v.handle_mouse(mouse(MouseEventKind::Up(left), to.0, to.1));
        assert_eq!(v.edit_msg, format!("connected {leaf} under {seq}"));
        assert_eq!(children(&v, seq), vec![leaf]);
        assert_eq!(v.drag, None);
        // Undoable like the keyboard connect.
        v.handle_key(KeyCode::Char('u'));
        assert!(children(&v, seq).is_empty());
    }

    #[test]
    fn refused_drag_connect_reports_and_changes_nothing() {
        let mut v = editing_view();
        let g = v.graph().unwrap().clone();
        let root = g.root;
        let attached = g.reachable()[1];
        draw_at(&mut v, 160, 44);
        // The root cannot become a child.
        {
            let (a, b) = (cell_of(&v, root), cell_of(&v, attached));
            drag(&mut v, a, b);
        }
        assert!(v.edit_msg.contains("root"), "{}", v.edit_msg);
        assert_eq!(v.graph().unwrap(), &g);
        // An attached node cannot be attached a second time.
        let other = *g
            .reachable()
            .iter()
            .find(|&&id| {
                id != attached && id != root && !g.node(attached).unwrap().children.contains(&id)
            })
            .unwrap();
        draw_at(&mut v, 160, 44);
        let (from, to) = (cell_of(&v, attached), cell_of(&v, other));
        let left = MouseButton::Left;
        v.handle_mouse(mouse(MouseEventKind::Down(left), from.0, from.1));
        v.handle_mouse(mouse(MouseEventKind::Drag(left), to.0, to.1));
        assert_eq!(v.drag.unwrap().target, Some((other, false)));
        assert!(v.edit_msg.starts_with("cannot connect"), "{}", v.edit_msg);
        v.handle_mouse(mouse(MouseEventKind::Up(left), to.0, to.1));
        assert!(!v.edit_msg.starts_with("connected"), "{}", v.edit_msg);
        assert_eq!(v.graph().unwrap(), &g);
        // A leaf (no children allowed) refuses a child: not a container.
        v.handle_key(KeyCode::Char('a'));
        let leaf = v.selected_node().unwrap();
        let effect = *g
            .reachable()
            .iter()
            .find(|&&id| g.node(id).unwrap().children.is_empty())
            .unwrap();
        draw_at(&mut v, 160, 44);
        let before = v.graph().unwrap().clone();
        {
            let (a, b) = (cell_of(&v, leaf), cell_of(&v, effect));
            drag(&mut v, a, b);
        }
        assert_eq!(v.graph().unwrap(), &before, "{}", v.edit_msg);
        assert!(!v.edit_msg.starts_with("connected"));
    }

    #[test]
    fn drag_to_empty_space_and_right_click_detach() {
        let mut v = editing_view();
        let g = v.graph().unwrap().clone();
        // A child whose parent stays valid without it (a fixed-arity
        // parent would be left empty, which the editor refuses).
        let child = *g
            .reachable()
            .iter()
            .find(|&&id| {
                id != g.root && GraphEditor::new(g.clone()).unwrap().disconnect(id).is_ok()
            })
            .unwrap();
        // A refused detach by drag changes nothing.
        let fixed = g.reachable().into_iter().find(|&id| {
            id != g.root && GraphEditor::new(g.clone()).unwrap().disconnect(id).is_err()
        });
        draw_at(&mut v, 160, 44);
        if let Some(fixed) = fixed {
            let (a, b) = (cell_of(&v, fixed), empty_cell(&v));
            drag(&mut v, a, b);
            assert_eq!(v.graph().unwrap(), &g, "{}", v.edit_msg);
            assert!(!v.edit_msg.starts_with("detached"));
            draw_at(&mut v, 160, 44);
        }
        {
            let (a, b) = (cell_of(&v, child), empty_cell(&v));
            drag(&mut v, a, b);
        }
        assert_eq!(v.edit_msg, format!("detached {child}"));
        assert!(!is_attached(v.graph().unwrap(), child));
        // Undo puts it back; a right-click detaches it again.
        v.handle_key(KeyCode::Char('u'));
        assert_eq!(v.graph().unwrap(), &g);
        draw_at(&mut v, 160, 44);
        let (c, r) = cell_of(&v, child);
        assert!(v.handle_mouse(mouse(MouseEventKind::Down(MouseButton::Right), c, r)));
        assert_eq!(v.selected_node(), Some(child));
        assert!(!is_attached(v.graph().unwrap(), child));
        // Releasing outside the graph pane cancels a drag.
        v.handle_key(KeyCode::Char('u'));
        draw_at(&mut v, 160, 44);
        {
            let (a, b) = (cell_of(&v, child), (0, 0));
            drag(&mut v, a, b);
        }
        assert_eq!(v.graph().unwrap(), &g);
        assert_eq!(v.edit_msg, "drag cancelled");
        // Right-click on the root: refused, nothing changes.
        draw_at(&mut v, 160, 44);
        let (c, r) = cell_of(&v, g.root);
        v.handle_mouse(mouse(MouseEventKind::Down(MouseButton::Right), c, r));
        assert!(v.edit_msg.contains("root"), "{}", v.edit_msg);
        assert_eq!(v.graph().unwrap(), &g);
    }

    #[test]
    fn wheel_over_the_graph_never_changes_the_preset() {
        let mut v = FxGraphView::new();
        v.open = true;
        draw_at(&mut v, 160, 44);
        let (c, r) = cell_of(&v, v.graph().unwrap().root);
        let g = v.graph().unwrap().clone();
        for kind in [MouseEventKind::ScrollDown, MouseEventKind::ScrollUp] {
            for _ in 0..3 {
                v.handle_mouse(mouse(kind, c, r));
                v.handle_mouse(mouse(kind, 2, 3));
            }
        }
        assert_eq!(v.selected(), 0);
        assert_eq!(v.graph().unwrap(), &g);
        assert!(!v.is_editing());
    }

    #[test]
    fn wheel_scrolls_a_long_outline() {
        // The outline is the canvas without fx-graph, and the fallback with
        // it when there is no flow; drive it directly.
        let mut v = editing_view();
        for _ in 0..30 {
            v.handle_key(KeyCode::Char('a'));
        }
        v.state = None;
        let rows = outline_lines_with(v.graph().unwrap(), None, None, None, &Theme::default());
        assert!(rows.len() > 30);
        assert!(rows.iter().any(|(id, _)| id.is_none()), "scratch heading");
        // Under fx-graph the canvas is drawn instead, which the wheel leaves
        // alone (rataflow fits the whole graph).
        #[cfg(feature = "fx-graph")]
        {
            draw_at(&mut v, 160, 30);
            assert!(!v.scrolls_outline());
            let p = v.graph_pane;
            assert!(!v.handle_mouse(mouse(MouseEventKind::ScrollDown, p.x + 1, p.y + 1)));
        }
        #[cfg(not(feature = "fx-graph"))]
        {
            draw_at(&mut v, 160, 30);
            let p = v.graph_pane;
            let first = v.hits.first().unwrap().id;
            assert!(v.handle_mouse(mouse(MouseEventKind::ScrollDown, p.x + 1, p.y + 1)));
            draw_at(&mut v, 160, 30);
            assert_eq!(v.outline_scroll, 1);
            assert_ne!(v.hits.first().unwrap().id, first);
            // Rows still map to the node drawn on them after scrolling.
            let hit = v.hits[3];
            assert_eq!(v.node_at(hit.rect.x, hit.rect.y), Some(hit.id));
            for _ in 0..100 {
                v.handle_mouse(mouse(MouseEventKind::ScrollDown, p.x + 1, p.y + 1));
            }
            draw_at(&mut v, 160, 30);
            assert_eq!(v.outline_scroll, rows.len() - p.height as usize);
        }
    }

    #[test]
    fn keyboard_editing_still_works_after_mouse_use() {
        let mut v = editing_view();
        draw_at(&mut v, 160, 44);
        let root = v.graph().unwrap().root;
        let (c, r) = cell_of(&v, root);
        v.handle_mouse(mouse(MouseEventKind::Down(MouseButton::Left), c, r));
        v.handle_mouse(mouse(MouseEventKind::Up(MouseButton::Left), c, r));
        v.handle_key(KeyCode::Char('a'));
        let leaf = v.selected_node().unwrap();
        v.handle_key(KeyCode::Char('m'));
        v.handle_key(KeyCode::Char('A'));
        v.handle_key(KeyCode::Char('c'));
        assert!(v.edit_msg.starts_with("connected"), "{}", v.edit_msg);
        assert!(children(&v, v.selected_node().unwrap()).contains(&leaf));
    }

    #[test]
    fn layout_and_outline_show_scratch_trees_below_the_tree() {
        let mut v = editing_view();
        v.handle_key(KeyCode::Char('A'));
        let seq = v.selected_node().unwrap();
        v.handle_key(KeyCode::Char('a'));
        let leaf = v.selected_node().unwrap();
        v.edit(|ed| ed.connect(leaf, seq, 0).map(|()| String::new()));
        let g = v.graph().unwrap();
        assert_eq!(scratch_roots(g), vec![seq]);
        let placed = layout(g);
        assert_eq!(placed.len(), g.nodes.len());
        let at = |id| placed.iter().find(|n| n.id == id).unwrap();
        let tree_bottom = g
            .reachable()
            .iter()
            .map(|&id| at(id).y + at(id).height)
            .fold(0.0, f64::max);
        assert!(at(seq).y >= tree_bottom && at(leaf).x + at(leaf).width <= at(seq).x);
        for (i, a) in placed.iter().enumerate() {
            for b in &placed[i + 1..] {
                assert!(!overlaps(a, b), "{} overlaps {}", a.id, b.id);
            }
        }
        let rows = outline_lines_with(g, None, None, None, &Theme::default());
        let ids: Vec<_> = rows.iter().map(|(id, _)| *id).collect();
        let head = ids.iter().position(|id| id.is_none()).unwrap();
        assert_eq!(&ids[head + 1..], &[Some(seq), Some(leaf)]);
        assert!(
            rows[head + 2].1.to_string().starts_with("    "),
            "indented child"
        );
    }

    #[test]
    fn props_panel_renders_fields() {
        let mut term = Terminal::new(TestBackend::new(140, 40)).unwrap();
        let mut v = editing_view();
        select_where(&mut v, has_timer);
        v.handle_key(KeyCode::Char('p'));
        term.draw(|f| v.render(f, f.area(), &Theme::default()))
            .unwrap();
        let text = screen_text(&term);
        assert!(text.contains("Props"), "{text}");
        assert!(text.contains("interp"), "{text}");
        for (w, h) in [(30, 10), (60, 20), (4, 4)] {
            let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
            term.draw(|f| v.render(f, f.area(), &Theme::default()))
                .unwrap();
        }
    }
}
