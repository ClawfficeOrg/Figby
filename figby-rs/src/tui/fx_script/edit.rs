//! Structural editing of an `FxGraph` (docs/fx-scripting-plan.md, phase 5).
//!
//! `GraphEditor` owns one graph and applies edits to it. Every edit is
//! checked before it lands, so the graph stays a valid tree or keeps its old
//! shape on error. Each applied edit is recorded for undo; redo replays the
//! edits that undo took back.
//!
//! Wiring is free: a node is attached to any parent by [`GraphEditor::connect`]
//! and detached with [`GraphEditor::disconnect`]. Checks run on connect:
//! the child must be unattached and not the root, the parent must be a
//! container with room, and the link must not close a cycle.
//!
//! Value nodes are never children. [`GraphEditor::bind`] wires one to a
//! parameter slot of another node, and only when the types match (a colour
//! value to a colour slot, and so on).
//!
//! The core is pure (no I/O, no ratatui), so it can be tested headlessly. Save
//! and load work on strings: [`graph_to_json`] and [`graph_from_json`].

use std::collections::BTreeSet;
use std::fmt;

use super::graph::{Arity, FxGraph, FxGraphError, FxNode, FxNodeId, FxNodeKind, FxValueType};

/// Undo depth. Older edits fall off the bottom of the history.
pub const HISTORY_LIMIT: usize = 200;

/// Why an edit was refused. The graph is unchanged when one is returned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FxEditError {
    NoSuchNode(FxNodeId),
    /// The root cannot be deleted or detached.
    IsRoot(FxNodeId),
    /// The node already has a parent; disconnect it first.
    AlreadyAttached(FxNodeId),
    /// The parent is a leaf effect and takes no children.
    NotAContainer(FxNodeId),
    /// The parent takes exactly one child and already has it.
    ParentFull {
        parent: FxNodeId,
    },
    /// Linking would make a node its own ancestor.
    WouldCycle {
        child: FxNodeId,
        parent: FxNodeId,
    },
    /// The node has no parent to detach from.
    NotAttached(FxNodeId),
    /// A kind change would leave the node with the wrong number of children.
    KindArity {
        node: FxNodeId,
    },
    /// A Value node was offered as a child; it can only fill a parameter.
    ValueNotAChild(FxNodeId),
    /// The node has no parameter slot of that name.
    NoSuchSlot {
        node: FxNodeId,
        slot: String,
    },
    /// The bound node is not a Value of the slot's type (`found` is `None`
    /// when it is not a Value node).
    SlotType {
        slot: String,
        expected: FxValueType,
        found: Option<FxValueType>,
    },
    /// The slot is not bound to a Value node.
    NotBound {
        node: FxNodeId,
        slot: String,
    },
    NothingToUndo,
    NothingToRedo,
    /// The graph failed validation after the edit (should not happen; kept
    /// as a backstop so an edit can never leave a broken graph behind).
    Invalid(FxGraphError),
}

impl fmt::Display for FxEditError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoSuchNode(id) => write!(f, "no node {id}"),
            Self::IsRoot(id) => write!(f, "node {id} is the root"),
            Self::AlreadyAttached(id) => write!(f, "node {id} already has a parent"),
            Self::NotAContainer(id) => write!(f, "node {id} is an effect and takes no children"),
            Self::ParentFull { parent } => write!(f, "node {parent} already has its child"),
            Self::WouldCycle { child, parent } => {
                write!(f, "linking {child} under {parent} would make a cycle")
            }
            Self::NotAttached(id) => write!(f, "node {id} has no parent"),
            Self::KindArity { node } => {
                write!(
                    f,
                    "node {node} has the wrong number of children for that kind"
                )
            }
            Self::ValueNotAChild(id) => write!(
                f,
                "node {id} is a value; bind it to a parameter (p, then b) instead"
            ),
            Self::NoSuchSlot { node, slot } => write!(f, "node {node} has no parameter '{slot}'"),
            Self::SlotType {
                slot,
                expected,
                found,
            } => match found {
                Some(found) => write!(f, "'{slot}' takes a {expected} value, not a {found}"),
                None => write!(f, "'{slot}' takes a {expected} value node"),
            },
            Self::NotBound { node, slot } => {
                write!(f, "node {node} '{slot}' is not bound to a value")
            }
            Self::NothingToUndo => write!(f, "nothing to undo"),
            Self::NothingToRedo => write!(f, "nothing to redo"),
            Self::Invalid(e) => write!(f, "graph invalid: {e}"),
        }
    }
}

impl std::error::Error for FxEditError {}

/// An `FxGraph` plus undo and redo history.
#[derive(Debug, Clone)]
pub struct GraphEditor {
    graph: FxGraph,
    past: Vec<FxGraph>,
    future: Vec<FxGraph>,
}

impl GraphEditor {
    /// Start editing `graph`. Fails if it is not a valid tree to begin with.
    pub fn new(graph: FxGraph) -> Result<Self, FxEditError> {
        graph.validate().map_err(FxEditError::Invalid)?;
        Ok(Self {
            graph,
            past: Vec::new(),
            future: Vec::new(),
        })
    }

    pub fn graph(&self) -> &FxGraph {
        &self.graph
    }

    pub fn into_graph(self) -> FxGraph {
        self.graph
    }

    pub fn can_undo(&self) -> bool {
        !self.past.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.future.is_empty()
    }

    /// Add an unattached node of `kind` and return its id. The node is a
    /// scratch node until it is connected; the emitter ignores it meanwhile.
    pub fn add_node(&mut self, kind: FxNodeKind) -> FxNodeId {
        let id = self.graph.nodes.iter().map(|n| n.id).max().unwrap_or(0) + 1;
        self.apply(|g| {
            g.nodes.push(FxNode::new(id, kind));
            Ok(())
        })
        .expect("adding a scratch node cannot fail");
        id
    }

    /// Attach `child` under `parent` at `slot` (clamped to the end of the
    /// parent's children). Checks every rule before changing anything.
    pub fn connect(
        &mut self,
        child: FxNodeId,
        parent: FxNodeId,
        slot: usize,
    ) -> Result<(), FxEditError> {
        let g = &self.graph;
        g.node(child).ok_or(FxEditError::NoSuchNode(child))?;
        let parent_node = g.node(parent).ok_or(FxEditError::NoSuchNode(parent))?;
        if child == g.root {
            return Err(FxEditError::IsRoot(child));
        }
        if matches!(
            g.node(child).map(|n| &n.kind),
            Some(FxNodeKind::Value { .. })
        ) {
            return Err(FxEditError::ValueNotAChild(child));
        }
        if is_attached(g, child) {
            return Err(FxEditError::AlreadyAttached(child));
        }
        match parent_node.kind.arity() {
            Arity::None => return Err(FxEditError::NotAContainer(parent)),
            Arity::Exactly1 if !parent_node.children.is_empty() => {
                return Err(FxEditError::ParentFull { parent });
            }
            _ => {}
        }
        if subtree(g, child).contains(&parent) {
            return Err(FxEditError::WouldCycle { child, parent });
        }
        self.apply(|g| {
            let p = g
                .nodes
                .iter_mut()
                .find(|n| n.id == parent)
                .expect("checked above");
            let at = slot.min(p.children.len());
            p.children.insert(at, child);
            Ok(())
        })
    }

    /// Detach `child` from its parent. The child keeps its own subtree and
    /// becomes a scratch tree.
    pub fn disconnect(&mut self, child: FxNodeId) -> Result<(), FxEditError> {
        let g = &self.graph;
        if g.node(child).is_none() {
            return Err(FxEditError::NoSuchNode(child));
        }
        if child == g.root {
            return Err(FxEditError::IsRoot(child));
        }
        if !is_attached(g, child) {
            return Err(FxEditError::NotAttached(child));
        }
        self.apply(|g| {
            for n in &mut g.nodes {
                n.children.retain(|&c| c != child);
            }
            Ok(())
        })
    }

    /// Delete `id` and everything under it. The root cannot be deleted.
    /// Slots bound to a deleted Value node fall back to their inline value.
    pub fn delete(&mut self, id: FxNodeId) -> Result<(), FxEditError> {
        if self.graph.node(id).is_none() {
            return Err(FxEditError::NoSuchNode(id));
        }
        if id == self.graph.root {
            return Err(FxEditError::IsRoot(id));
        }
        let doomed = subtree(&self.graph, id);
        self.apply(|g| {
            g.nodes.retain(|n| !doomed.contains(&n.id));
            for n in &mut g.nodes {
                n.children.retain(|c| !doomed.contains(c));
                n.params.retain(|_, v| !doomed.contains(v));
            }
            Ok(())
        })
    }

    /// Replace a node's kind (its parameters or its type). The node keeps its
    /// children, so the new kind must accept that many. Bindings whose slot
    /// the new kind lacks (or types differently) are dropped. A Value node
    /// cannot change type while a slot of the old type is bound to it.
    pub fn set_kind(&mut self, id: FxNodeId, kind: FxNodeKind) -> Result<(), FxEditError> {
        let node = self.graph.node(id).ok_or(FxEditError::NoSuchNode(id))?;
        let found = node.children.len();
        let fits = match kind.arity() {
            Arity::None => found == 0,
            Arity::Exactly1 => found == 1,
            Arity::AtLeast1 => found >= 1,
        };
        if !fits {
            return Err(FxEditError::KindArity { node: id });
        }
        self.apply(|g| {
            if let Some(n) = g.nodes.iter_mut().find(|n| n.id == id) {
                let old = std::mem::replace(&mut n.kind, kind);
                let new = &n.kind;
                n.params.retain(|slot, _| {
                    new.slot_type(slot).is_some() && new.slot_type(slot) == old.slot_type(slot)
                });
            }
            Ok(())
        })
    }

    /// Bind parameter `slot` of `node` to the Value node `value`. The value
    /// must have the slot's type. Rebinding a bound slot replaces it.
    pub fn bind(&mut self, node: FxNodeId, slot: &str, value: FxNodeId) -> Result<(), FxEditError> {
        let g = &self.graph;
        let n = g.node(node).ok_or(FxEditError::NoSuchNode(node))?;
        let v = g.node(value).ok_or(FxEditError::NoSuchNode(value))?;
        let expected = n
            .kind
            .slot_type(slot)
            .ok_or_else(|| FxEditError::NoSuchSlot {
                node,
                slot: slot.to_string(),
            })?;
        let found = match v.kind {
            FxNodeKind::Value { value } => Some(value.ty()),
            _ => None,
        };
        if found != Some(expected) {
            return Err(FxEditError::SlotType {
                slot: slot.to_string(),
                expected,
                found,
            });
        }
        self.apply(|g| {
            if let Some(n) = g.nodes.iter_mut().find(|n| n.id == node) {
                n.params.insert(slot.to_string(), value);
            }
            Ok(())
        })
    }

    /// Remove the binding of `slot`; the node goes back to its inline value.
    pub fn unbind(&mut self, node: FxNodeId, slot: &str) -> Result<(), FxEditError> {
        let n = self.graph.node(node).ok_or(FxEditError::NoSuchNode(node))?;
        if !n.params.contains_key(slot) {
            return Err(FxEditError::NotBound {
                node,
                slot: slot.to_string(),
            });
        }
        self.apply(|g| {
            if let Some(n) = g.nodes.iter_mut().find(|n| n.id == node) {
                n.params.remove(slot);
            }
            Ok(())
        })
    }

    /// Move the inline value of `slot` into a new Value node and bind the
    /// slot to it, as one undoable edit. Returns the Value node's id.
    pub fn extract_value(&mut self, node: FxNodeId, slot: &str) -> Result<FxNodeId, FxEditError> {
        self.graph.node(node).ok_or(FxEditError::NoSuchNode(node))?;
        let value = self
            .graph
            .resolved_kind(node)
            .and_then(|k| k.slot_value(slot))
            .ok_or_else(|| FxEditError::NoSuchSlot {
                node,
                slot: slot.to_string(),
            })?;
        let id = self.graph.nodes.iter().map(|n| n.id).max().unwrap_or(0) + 1;
        self.apply(|g| {
            g.nodes.push(FxNode::new(id, FxNodeKind::Value { value }));
            if let Some(n) = g.nodes.iter_mut().find(|n| n.id == node) {
                n.params.insert(slot.to_string(), id);
            }
            Ok(())
        })?;
        Ok(id)
    }

    /// Make an unattached node the root (the node that runs).
    pub fn set_root(&mut self, id: FxNodeId) -> Result<(), FxEditError> {
        if self.graph.node(id).is_none() {
            return Err(FxEditError::NoSuchNode(id));
        }
        if is_attached(&self.graph, id) {
            return Err(FxEditError::AlreadyAttached(id));
        }
        self.apply(|g| {
            g.root = id;
            Ok(())
        })
    }

    /// Replace the whole graph (a load), as one undoable edit.
    pub fn replace(&mut self, graph: FxGraph) -> Result<(), FxEditError> {
        self.apply(|g| {
            *g = graph;
            Ok(())
        })
    }

    /// Undo the last applied edit.
    pub fn undo(&mut self) -> Result<(), FxEditError> {
        let prev = self.past.pop().ok_or(FxEditError::NothingToUndo)?;
        let current = std::mem::replace(&mut self.graph, prev);
        self.future.push(current);
        Ok(())
    }

    /// Redo the last undone edit.
    pub fn redo(&mut self) -> Result<(), FxEditError> {
        let next = self.future.pop().ok_or(FxEditError::NothingToRedo)?;
        let current = std::mem::replace(&mut self.graph, next);
        self.push_past(current);
        Ok(())
    }

    /// Run `change` on a copy. On success the copy must still validate,
    /// and it replaces the graph and goes on the undo stack. A new edit
    /// clears redo.
    fn apply(
        &mut self,
        change: impl FnOnce(&mut FxGraph) -> Result<(), FxEditError>,
    ) -> Result<(), FxEditError> {
        let mut next = self.graph.clone();
        change(&mut next)?;
        next.validate().map_err(FxEditError::Invalid)?;
        let before = std::mem::replace(&mut self.graph, next);
        self.push_past(before);
        self.future.clear();
        Ok(())
    }

    fn push_past(&mut self, g: FxGraph) {
        self.past.push(g);
        if self.past.len() > HISTORY_LIMIT {
            self.past.remove(0);
        }
    }
}

/// Whether `id` has a parent in `g`.
fn is_attached(g: &FxGraph, id: FxNodeId) -> bool {
    g.nodes.iter().any(|n| n.children.contains(&id))
}

/// `id` and every node under it.
fn subtree(g: &FxGraph, id: FxNodeId) -> BTreeSet<FxNodeId> {
    let mut out = BTreeSet::new();
    let mut stack = vec![id];
    while let Some(x) = stack.pop() {
        if !out.insert(x) {
            continue;
        }
        if let Some(n) = g.node(x) {
            stack.extend(n.children.iter().copied());
        }
    }
    out
}

/// Serialise a graph as pretty JSON (the IR half of a saved graph; the DSL
/// is emitted from it by [`FxGraph::to_dsl`]).
pub fn graph_to_json(g: &FxGraph) -> String {
    serde_json::to_string_pretty(g).expect("FxGraph serialises to JSON")
}

/// Parse and validate a graph saved by [`graph_to_json`].
pub fn graph_from_json(text: &str) -> Result<FxGraph, String> {
    let g: FxGraph = serde_json::from_str(text).map_err(|e| e.to_string())?;
    g.validate().map_err(|e| e.to_string())?;
    Ok(g)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::fx_script::compile::{compile, FxTargets};
    use crate::tui::fx_script::graph::{
        FxColor, FxEffect, FxGraphBuilder, FxInterp, FxTimer, FxValue,
    };
    use crate::tui::fx_script::presets;
    use ratatui::layout::Rect;

    fn leaf(b: &mut FxGraphBuilder, ms: u32) -> FxNodeId {
        b.effect(FxEffect::Sleep { ms })
    }

    /// A sequence root over two sleeps: ids 1, 2 (sleeps) and 3 (sequence).
    fn seq_graph() -> FxGraph {
        let mut b = FxGraphBuilder::new();
        let a = leaf(&mut b, 1);
        let c = leaf(&mut b, 2);
        let root = b.add(FxNodeKind::Sequence, &[a, c]);
        b.build(root)
    }

    #[test]
    fn every_preset_loads_into_an_editor() {
        for p in presets::all() {
            GraphEditor::new(p.graph).unwrap_or_else(|e| panic!("{}: {e}", p.name));
        }
    }

    #[test]
    fn add_then_connect_grows_the_tree() {
        let mut ed = GraphEditor::new(seq_graph()).unwrap();
        let s = ed.add_node(FxNodeKind::Effect {
            effect: FxEffect::Sleep { ms: 9 },
        });
        assert!(!ed.graph().reachable().contains(&s));
        ed.connect(s, 3, 1).unwrap();
        assert_eq!(ed.graph().node(3).unwrap().children, vec![1, s, 2]);
        assert!(ed.graph().to_dsl().unwrap().contains("sleep"));
    }

    #[test]
    fn connect_refuses_bad_links() {
        let mut ed = GraphEditor::new(seq_graph()).unwrap();
        assert_eq!(ed.connect(3, 1, 0), Err(FxEditError::IsRoot(3)));
        assert_eq!(ed.connect(1, 3, 0), Err(FxEditError::AlreadyAttached(1)));
        let scratch = ed.add_node(FxNodeKind::Sequence);
        assert_eq!(
            ed.connect(scratch, 1, 0),
            Err(FxEditError::NotAContainer(1))
        );
        assert_eq!(ed.connect(9, 3, 0), Err(FxEditError::NoSuchNode(9)));
        let t = ed.add_node(FxNodeKind::Repeat { times: 2 });
        let e1 = ed.add_node(FxNodeKind::Effect {
            effect: FxEffect::Sleep { ms: 3 },
        });
        ed.connect(e1, t, 0).unwrap();
        // A repeat takes exactly one child, so a second one is refused.
        let e2 = ed.add_node(FxNodeKind::Effect {
            effect: FxEffect::Sleep { ms: 4 },
        });
        assert_eq!(
            ed.connect(e2, t, 0),
            Err(FxEditError::ParentFull { parent: t })
        );
    }

    #[test]
    fn connect_refuses_a_cycle() {
        // Scratch tree: c (PingPong) under s (Sequence). Linking s under c
        // would make s its own ancestor.
        let mut ed = GraphEditor::new(seq_graph()).unwrap();
        let s = ed.add_node(FxNodeKind::Sequence);
        let c = ed.add_node(FxNodeKind::PingPong);
        ed.connect(c, s, 0).unwrap();
        assert_eq!(
            ed.connect(s, c, 0),
            Err(FxEditError::WouldCycle {
                child: s,
                parent: c
            })
        );
    }

    #[test]
    fn disconnect_detaches_subtree_and_keeps_it() {
        let mut ed = GraphEditor::new(seq_graph()).unwrap();
        ed.disconnect(1).unwrap();
        assert_eq!(ed.graph().node(3).unwrap().children, vec![2]);
        assert!(ed.graph().node(1).is_some());
        assert_eq!(ed.disconnect(1), Err(FxEditError::NotAttached(1)));
        assert_eq!(ed.disconnect(3), Err(FxEditError::IsRoot(3)));
    }

    #[test]
    fn delete_removes_subtree_but_not_root() {
        let mut ed = GraphEditor::new(seq_graph()).unwrap();
        ed.delete(1).unwrap();
        assert!(ed.graph().node(1).is_none());
        assert_eq!(ed.graph().node(3).unwrap().children, vec![2]);
        assert_eq!(ed.delete(3), Err(FxEditError::IsRoot(3)));
        assert_eq!(ed.delete(42), Err(FxEditError::NoSuchNode(42)));
    }

    #[test]
    fn set_kind_respects_child_count() {
        let mut ed = GraphEditor::new(seq_graph()).unwrap();
        // Sequence (two children) cannot become a single-child delay.
        let delay = FxNodeKind::Delay { ms: 5 };
        assert_eq!(
            ed.set_kind(3, delay),
            Err(FxEditError::KindArity { node: 3 })
        );
        ed.set_kind(3, FxNodeKind::Parallel).unwrap();
        assert_eq!(ed.graph().node(3).unwrap().kind, FxNodeKind::Parallel);
        let ms = FxNodeKind::Effect {
            effect: FxEffect::Sleep { ms: 7 },
        };
        assert_eq!(ed.set_kind(1, ms.clone()), Ok(()));
        assert_eq!(ed.graph().node(1).unwrap().kind, ms);
    }

    #[test]
    fn undo_and_redo_walk_history() {
        let mut ed = GraphEditor::new(seq_graph()).unwrap();
        let original = ed.graph().clone();
        ed.delete(1).unwrap();
        let after = ed.graph().clone();
        assert!(ed.can_undo());
        ed.undo().unwrap();
        assert_eq!(ed.graph(), &original);
        assert!(ed.can_redo());
        ed.redo().unwrap();
        assert_eq!(ed.graph(), &after);
        ed.undo().unwrap();
        // A new edit drops the redo branch.
        ed.set_kind(3, FxNodeKind::Parallel).unwrap();
        assert!(!ed.can_redo());
        assert_eq!(ed.redo(), Err(FxEditError::NothingToRedo));
    }

    #[test]
    fn undo_on_empty_history_is_an_error() {
        let mut ed = GraphEditor::new(seq_graph()).unwrap();
        assert_eq!(ed.undo(), Err(FxEditError::NothingToUndo));
    }

    #[test]
    fn refused_edit_leaves_graph_and_history_alone() {
        let mut ed = GraphEditor::new(seq_graph()).unwrap();
        let before = ed.graph().clone();
        let _ = ed.delete(3);
        assert_eq!(ed.graph(), &before);
        assert!(!ed.can_undo());
    }

    #[test]
    fn history_is_bounded() {
        let mut ed = GraphEditor::new(seq_graph()).unwrap();
        for i in 0..(HISTORY_LIMIT + 10) {
            let kind = FxNodeKind::Delay { ms: i as u32 };
            ed.add_node(kind);
        }
        let mut steps = 0;
        while ed.undo().is_ok() {
            steps += 1;
        }
        assert_eq!(steps, HISTORY_LIMIT);
    }

    #[test]
    fn json_round_trip_keeps_graph_and_dsl() {
        for p in presets::all() {
            let text = graph_to_json(&p.graph);
            let back = graph_from_json(&text).unwrap_or_else(|e| panic!("{}: {e}", p.name));
            assert_eq!(back, p.graph, "{}", p.name);
            assert_eq!(back.to_dsl().unwrap(), p.graph.to_dsl().unwrap());
        }
    }

    #[test]
    fn json_load_rejects_an_invalid_tree() {
        let mut g = seq_graph();
        // Point the sequence at a node that does not exist.
        g.nodes[2].children.push(99);
        let text = graph_to_json(&g);
        assert!(graph_from_json(&text).is_err());
        assert!(graph_from_json("{ not json").is_err());
    }

    #[test]
    fn set_root_promotes_a_scratch_tree() {
        let mut ed = GraphEditor::new(seq_graph()).unwrap();
        let s = ed.add_node(FxNodeKind::Effect {
            effect: FxEffect::Dissolve {
                timer: FxTimer::new(100, FxInterp::Linear),
            },
        });
        ed.set_root(s).unwrap();
        assert_eq!(ed.graph().root, s);
        assert_eq!(ed.set_root(1), Err(FxEditError::AlreadyAttached(1)));
    }

    /// A fade leaf under a ping-pong root: ids 1 (fade) and 2 (root).
    fn fade_graph() -> FxGraph {
        let mut b = FxGraphBuilder::new();
        let f = b.effect(FxEffect::FadeToFg {
            color: FxColor::Cyan,
            timer: FxTimer::new(200, FxInterp::Linear),
        });
        let root = b.add(FxNodeKind::PingPong, &[f]);
        b.build(root)
    }

    fn colour_value(c: FxColor) -> FxNodeKind {
        FxNodeKind::Value {
            value: FxValue::Color(c),
        }
    }

    fn assert_compiles(g: &FxGraph) {
        let dsl = g.to_dsl().unwrap();
        let targets = FxTargets::uniform(Rect::new(0, 0, 40, 10), 2);
        compile(&dsl, &targets).unwrap_or_else(|e| panic!("{e:?}\n{dsl}"));
    }

    #[test]
    fn bind_is_type_checked() {
        let mut ed = GraphEditor::new(fade_graph()).unwrap();
        let red = ed.add_node(colour_value(FxColor::Red));
        let slow = ed.add_node(FxNodeKind::Value {
            value: FxValue::Timer(FxTimer::new(900, FxInterp::QuadIn)),
        });
        assert_eq!(
            ed.bind(1, "timer", red),
            Err(FxEditError::SlotType {
                slot: "timer".into(),
                expected: FxValueType::Timer,
                found: Some(FxValueType::Color),
            })
        );
        assert_eq!(
            ed.bind(1, "color", 2),
            Err(FxEditError::SlotType {
                slot: "color".into(),
                expected: FxValueType::Color,
                found: None,
            })
        );
        assert_eq!(
            ed.bind(1, "motion", red),
            Err(FxEditError::NoSuchSlot {
                node: 1,
                slot: "motion".into()
            })
        );
        ed.bind(1, "color", red).unwrap();
        ed.bind(1, "timer", slow).unwrap();
        let dsl = ed.graph().to_dsl().unwrap();
        assert!(
            dsl.contains(&format!("fx::fade_to_fg(v{red}, v{slow})")),
            "{dsl}"
        );
        assert_eq!(ed.graph().duration_ms(), Some(1800));
        assert_compiles(ed.graph());
        ed.unbind(1, "timer").unwrap();
        assert_eq!(ed.graph().duration_ms(), Some(400));
        assert_eq!(
            ed.unbind(1, "timer"),
            Err(FxEditError::NotBound {
                node: 1,
                slot: "timer".into()
            })
        );
    }

    #[test]
    fn value_nodes_cannot_be_children() {
        let mut ed = GraphEditor::new(seq_graph()).unwrap();
        let v = ed.add_node(colour_value(FxColor::Red));
        assert_eq!(ed.connect(v, 3, 0), Err(FxEditError::ValueNotAChild(v)));
        // Nor can a value become the root.
        assert!(matches!(ed.set_root(v), Err(FxEditError::Invalid(_))));
    }

    #[test]
    fn extract_value_is_one_undoable_edit() {
        let mut ed = GraphEditor::new(fade_graph()).unwrap();
        let before = ed.graph().to_dsl().unwrap();
        let v = ed.extract_value(1, "color").unwrap();
        assert_eq!(ed.graph().node(1).unwrap().params.get("color"), Some(&v));
        assert_eq!(
            ed.graph().node(v).unwrap().kind,
            colour_value(FxColor::Cyan)
        );
        let dsl = ed.graph().to_dsl().unwrap();
        assert!(
            dsl.starts_with(&format!("let v{v} = Color::Cyan;\n")),
            "{dsl}"
        );
        assert_compiles(ed.graph());
        ed.undo().unwrap();
        assert!(ed.graph().node(v).is_none());
        assert_eq!(ed.graph().to_dsl().unwrap(), before);
        assert!(matches!(
            ed.extract_value(1, "target"),
            Err(FxEditError::NoSuchSlot { .. })
        ));
    }

    #[test]
    fn deleting_a_value_unbinds_its_slots() {
        let mut ed = GraphEditor::new(fade_graph()).unwrap();
        let v = ed.extract_value(1, "timer").unwrap();
        ed.delete(v).unwrap();
        assert!(ed.graph().node(1).unwrap().params.is_empty());
        assert_compiles(ed.graph());
    }

    #[test]
    fn set_kind_drops_bindings_the_new_kind_lacks() {
        let mut ed = GraphEditor::new(fade_graph()).unwrap();
        ed.extract_value(1, "color").unwrap();
        let t = ed.extract_value(1, "timer").unwrap();
        ed.set_kind(
            1,
            FxNodeKind::Effect {
                effect: FxEffect::Dissolve {
                    timer: FxTimer::new(10, FxInterp::Linear),
                },
            },
        )
        .unwrap();
        let params = &ed.graph().node(1).unwrap().params;
        assert_eq!(params.len(), 1);
        assert_eq!(params.get("timer"), Some(&t));
        // A bound value cannot change to another type.
        assert!(matches!(
            ed.set_kind(t, colour_value(FxColor::Red)),
            Err(FxEditError::Invalid(FxGraphError::SlotType { .. }))
        ));
    }

    #[test]
    fn numeric_hsl_and_filter_slots_extract_and_bind_with_type_checks() {
        use crate::tui::fx_script::graph::FxFilter;
        // repeat(4) > delay(10) > filter(fg White) > hsl_shift_fg; plus a
        // spare sleep(20) under a sequence for a second ms user.
        let mut b = FxGraphBuilder::new();
        let shift = b.effect(FxEffect::HslShiftFg {
            hsl: [30.0, 0.0, 0.0],
            timer: FxTimer::new(100, FxInterp::Linear),
        });
        let sleep = leaf(&mut b, 20);
        let filter = b.add(
            FxNodeKind::Filter {
                filter: FxFilter::FgColor(FxColor::White),
            },
            &[shift],
        );
        let seq = b.add(FxNodeKind::Sequence, &[filter, sleep]);
        let delay = b.add(FxNodeKind::Delay { ms: 10 }, &[seq]);
        let root = b.add(FxNodeKind::Repeat { times: 4 }, &[delay]);
        let mut ed = GraphEditor::new(b.build(root)).unwrap();

        let ms = ed.extract_value(delay, "ms").unwrap();
        assert_eq!(
            ed.graph().node(ms).unwrap().kind,
            FxNodeKind::Value {
                value: FxValue::Ms(10)
            }
        );
        ed.bind(sleep, "ms", ms).unwrap();
        let times = ed.extract_value(root, "times").unwrap();
        let hsl = ed.extract_value(shift, "hsl").unwrap();
        let colour = ed.extract_value(filter, "color").unwrap();
        // Wrong types are refused and change nothing.
        let before = ed.graph().clone();
        assert!(matches!(
            ed.bind(root, "times", ms),
            Err(FxEditError::SlotType {
                expected: FxValueType::Times,
                found: Some(FxValueType::Ms),
                ..
            })
        ));
        assert!(matches!(
            ed.bind(filter, "color", hsl),
            Err(FxEditError::SlotType { .. })
        ));
        assert!(matches!(
            ed.bind(delay, "ms", times),
            Err(FxEditError::SlotType { .. })
        ));
        assert_eq!(ed.graph(), &before);
        let dsl = ed.graph().to_dsl().unwrap();
        for needle in [
            format!("fx::sleep(v{ms})"),
            format!("RepeatMode::Times(v{times})"),
            format!("fx::hsl_shift_fg(v{hsl}, "),
            format!("CellFilter::FgColor(v{colour})"),
        ] {
            assert!(dsl.contains(&needle), "{needle}\n{dsl}");
        }
        assert_compiles(ed.graph());
        // (100 + 10 sleep) + 10 delay, x4.
        assert_eq!(ed.graph().duration_ms(), Some(480));
        // Switching the filter to one without a colour drops the binding.
        ed.set_kind(
            filter,
            FxNodeKind::Filter {
                filter: FxFilter::Text,
            },
        )
        .unwrap();
        assert!(ed.graph().node(filter).unwrap().params.is_empty());
        assert_compiles(ed.graph());
    }

    #[test]
    fn json_round_trip_keeps_bindings() {
        let mut ed = GraphEditor::new(fade_graph()).unwrap();
        ed.extract_value(1, "color").unwrap();
        let text = graph_to_json(ed.graph());
        let back = graph_from_json(&text).unwrap();
        assert_eq!(&back, ed.graph());
        assert_compiles(&back);
    }
}
