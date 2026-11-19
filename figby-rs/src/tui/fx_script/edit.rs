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
//! The core is pure (no I/O, no ratatui), so it can be tested headlessly. Save
//! and load work on strings: [`graph_to_json`] and [`graph_from_json`].

use std::collections::BTreeSet;
use std::fmt;

use super::graph::{Arity, FxGraph, FxGraphError, FxNode, FxNodeId, FxNodeKind};

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
            g.nodes.push(FxNode {
                id,
                kind,
                children: Vec::new(),
            });
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
            }
            Ok(())
        })
    }

    /// Replace a node's kind (its parameters or its type). The node keeps its
    /// children, so the new kind must accept that many.
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
                n.kind = kind;
            }
            Ok(())
        })
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
    use crate::tui::fx_script::graph::{FxEffect, FxGraphBuilder, FxInterp, FxTimer};
    use crate::tui::fx_script::presets;

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
}
