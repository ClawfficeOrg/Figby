//! `FxGraph`: Figby's own intermediate representation of an effect graph.
//!
//! The node-graph view (rataflow) is only a view of this IR, so a change in
//! that crate costs the view, not the data (docs/fx-scripting-plan.md, 4).
//! A graph is a tree: every edge runs from a child to its parent, and the
//! root is the effect that runs. The emitter turns the tree into tachyonfx
//! DSL text, which is the single path into the effect runtime.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::{Deserialize, Serialize};

/// Stable id of a node inside one graph.
pub type FxNodeId = u32;

macro_rules! interpolations {
    ($($name:ident),+ $(,)?) => {
        /// Easing curve; one variant per `tachyonfx::Interpolation`.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
        pub enum FxInterp {
            $($name),+
        }

        impl FxInterp {
            /// Every easing, in declaration order.
            pub const ALL: &'static [FxInterp] = &[$(FxInterp::$name),+];

            /// The DSL spelling (also the tachyonfx variant name).
            pub fn dsl_name(self) -> &'static str {
                match self {
                    $(FxInterp::$name => stringify!($name)),+
                }
            }
        }
    };
}

interpolations!(
    Linear,
    QuadIn,
    QuadOut,
    QuadInOut,
    CubicIn,
    CubicOut,
    CubicInOut,
    QuartIn,
    QuartOut,
    QuartInOut,
    QuintIn,
    QuintOut,
    QuintInOut,
    SineIn,
    SineOut,
    SineInOut,
    ExpoIn,
    ExpoOut,
    ExpoInOut,
    CircIn,
    CircOut,
    CircInOut,
    BackIn,
    BackOut,
    BackInOut,
    ElasticIn,
    ElasticOut,
    ElasticInOut,
    BounceIn,
    BounceOut,
    BounceInOut,
    SmoothStep,
    Spring,
    Reverse,
);

/// Duration plus easing; emitted as the DSL tuple `(ms, Interp)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FxTimer {
    pub ms: u32,
    pub interp: FxInterp,
}

impl FxTimer {
    pub const fn new(ms: u32, interp: FxInterp) -> Self {
        Self { ms, interp }
    }

    fn dsl(self) -> String {
        format!("({}, {})", self.ms, self.interp.dsl_name())
    }

    fn summary(self) -> String {
        format!("{}ms {}", self.ms, self.interp.dsl_name())
    }
}

/// A colour argument. Named colours follow the terminal palette.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FxColor {
    Black,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    Gray,
    DarkGray,
    White,
    Rgb(u8, u8, u8),
}

impl FxColor {
    fn dsl(self) -> String {
        match self {
            FxColor::Rgb(r, g, b) => format!("Color::Rgb({r}, {g}, {b})"),
            named => format!("Color::{}", named.name()),
        }
    }

    fn name(self) -> String {
        match self {
            FxColor::Black => "Black".into(),
            FxColor::Red => "Red".into(),
            FxColor::Green => "Green".into(),
            FxColor::Yellow => "Yellow".into(),
            FxColor::Blue => "Blue".into(),
            FxColor::Magenta => "Magenta".into(),
            FxColor::Cyan => "Cyan".into(),
            FxColor::Gray => "Gray".into(),
            FxColor::DarkGray => "DarkGray".into(),
            FxColor::White => "White".into(),
            FxColor::Rgb(r, g, b) => format!("#{r:02x}{g:02x}{b:02x}"),
        }
    }
}

/// Direction of a sweep.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FxMotion {
    LeftToRight,
    RightToLeft,
    UpToDown,
    DownToUp,
}

impl FxMotion {
    fn dsl_name(self) -> &'static str {
        match self {
            FxMotion::LeftToRight => "LeftToRight",
            FxMotion::RightToLeft => "RightToLeft",
            FxMotion::UpToDown => "UpToDown",
            FxMotion::DownToUp => "DownToUp",
        }
    }
}

/// A named region the effect is confined to. Figby binds each name to a
/// `Rect` before compiling (see `compile::FxTargets`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FxTarget {
    Screen,
    Canvas,
    Selection,
    Toolbox,
    SidePanel,
    Status,
    /// 1-based layer index, bound as `layer_<n>`.
    Layer(u16),
}

impl FxTarget {
    /// The bound variable name used in DSL text.
    pub fn ident(self) -> String {
        match self {
            FxTarget::Screen => "screen".into(),
            FxTarget::Canvas => "canvas".into(),
            FxTarget::Selection => "selection".into(),
            FxTarget::Toolbox => "toolbox".into(),
            FxTarget::SidePanel => "side_panel".into(),
            FxTarget::Status => "status".into(),
            FxTarget::Layer(n) => format!("layer_{n}"),
        }
    }
}

/// Which cells an effect touches (`CellFilter` subset).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FxFilter {
    All,
    Text,
    NonEmpty,
    FgColor(FxColor),
    BgColor(FxColor),
    Inner { horizontal: u16, vertical: u16 },
    Outer { horizontal: u16, vertical: u16 },
}

impl FxFilter {
    fn dsl(self) -> String {
        match self {
            FxFilter::All => "CellFilter::All".into(),
            FxFilter::Text => "CellFilter::Text".into(),
            FxFilter::NonEmpty => "CellFilter::NonEmpty".into(),
            FxFilter::FgColor(c) => format!("CellFilter::FgColor({})", c.dsl()),
            FxFilter::BgColor(c) => format!("CellFilter::BgColor({})", c.dsl()),
            FxFilter::Inner {
                horizontal,
                vertical,
            } => format!("CellFilter::Inner(Margin::new({horizontal}, {vertical}))"),
            FxFilter::Outer {
                horizontal,
                vertical,
            } => format!("CellFilter::Outer(Margin::new({horizontal}, {vertical}))"),
        }
    }

    fn summary(self) -> String {
        match self {
            FxFilter::All => "all cells".into(),
            FxFilter::Text => "text".into(),
            FxFilter::NonEmpty => "non-empty".into(),
            FxFilter::FgColor(c) => format!("fg {}", c.name()),
            FxFilter::BgColor(c) => format!("bg {}", c.name()),
            FxFilter::Inner {
                horizontal,
                vertical,
            } => format!("inner {horizontal}x{vertical}"),
            FxFilter::Outer {
                horizontal,
                vertical,
            } => format!("outer {horizontal}x{vertical}"),
        }
    }
}

/// A leaf effect: one `fx::` constructor with its parameters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "fx", rename_all = "snake_case")]
pub enum FxEffect {
    FadeFrom {
        bg: FxColor,
        fg: FxColor,
        timer: FxTimer,
    },
    FadeTo {
        bg: FxColor,
        fg: FxColor,
        timer: FxTimer,
    },
    FadeFromFg {
        color: FxColor,
        timer: FxTimer,
    },
    FadeToFg {
        color: FxColor,
        timer: FxTimer,
    },
    PaintFg {
        color: FxColor,
        timer: FxTimer,
    },
    HslShiftFg {
        hsl: [f32; 3],
        timer: FxTimer,
    },
    Dissolve {
        timer: FxTimer,
    },
    Coalesce {
        timer: FxTimer,
    },
    SweepIn {
        motion: FxMotion,
        gradient_len: u16,
        randomness: u16,
        color: FxColor,
        timer: FxTimer,
    },
    SweepOut {
        motion: FxMotion,
        gradient_len: u16,
        randomness: u16,
        color: FxColor,
        timer: FxTimer,
    },
    Sleep {
        ms: u32,
    },
}

impl FxEffect {
    /// The `fx::` constructor name.
    pub fn fn_name(&self) -> &'static str {
        match self {
            FxEffect::FadeFrom { .. } => "fade_from",
            FxEffect::FadeTo { .. } => "fade_to",
            FxEffect::FadeFromFg { .. } => "fade_from_fg",
            FxEffect::FadeToFg { .. } => "fade_to_fg",
            FxEffect::PaintFg { .. } => "paint_fg",
            FxEffect::HslShiftFg { .. } => "hsl_shift_fg",
            FxEffect::Dissolve { .. } => "dissolve",
            FxEffect::Coalesce { .. } => "coalesce",
            FxEffect::SweepIn { .. } => "sweep_in",
            FxEffect::SweepOut { .. } => "sweep_out",
            FxEffect::Sleep { .. } => "sleep",
        }
    }

    fn args(&self) -> Vec<String> {
        match *self {
            FxEffect::FadeFrom { bg, fg, timer } | FxEffect::FadeTo { bg, fg, timer } => {
                vec![bg.dsl(), fg.dsl(), timer.dsl()]
            }
            FxEffect::FadeFromFg { color, timer }
            | FxEffect::FadeToFg { color, timer }
            | FxEffect::PaintFg { color, timer } => vec![color.dsl(), timer.dsl()],
            FxEffect::HslShiftFg { hsl, timer } => vec![
                format!("[{:?}, {:?}, {:?}]", hsl[0], hsl[1], hsl[2]),
                timer.dsl(),
            ],
            FxEffect::Dissolve { timer } | FxEffect::Coalesce { timer } => vec![timer.dsl()],
            FxEffect::SweepIn {
                motion,
                gradient_len,
                randomness,
                color,
                timer,
            }
            | FxEffect::SweepOut {
                motion,
                gradient_len,
                randomness,
                color,
                timer,
            } => vec![
                motion.dsl_name().to_string(),
                gradient_len.to_string(),
                randomness.to_string(),
                color.dsl(),
                timer.dsl(),
            ],
            FxEffect::Sleep { ms } => vec![ms.to_string()],
        }
    }

    fn summary(&self) -> String {
        match *self {
            FxEffect::FadeFrom { bg, fg, timer } | FxEffect::FadeTo { bg, fg, timer } => {
                format!("{}/{} {}", bg.name(), fg.name(), timer.summary())
            }
            FxEffect::FadeFromFg { color, timer }
            | FxEffect::FadeToFg { color, timer }
            | FxEffect::PaintFg { color, timer } => {
                format!("{} {}", color.name(), timer.summary())
            }
            FxEffect::HslShiftFg { hsl, timer } => format!(
                "h{:+} s{:+} l{:+} {}",
                hsl[0],
                hsl[1],
                hsl[2],
                timer.summary()
            ),
            FxEffect::Dissolve { timer } | FxEffect::Coalesce { timer } => timer.summary(),
            FxEffect::SweepIn {
                motion,
                color,
                timer,
                ..
            }
            | FxEffect::SweepOut {
                motion,
                color,
                timer,
                ..
            } => format!("{} {} {}", motion.dsl_name(), color.name(), timer.summary()),
            FxEffect::Sleep { ms } => format!("{ms}ms"),
        }
    }

    /// Total run time of this leaf in milliseconds.
    pub fn duration_ms(&self) -> u64 {
        match *self {
            FxEffect::FadeFrom { timer, .. }
            | FxEffect::FadeTo { timer, .. }
            | FxEffect::FadeFromFg { timer, .. }
            | FxEffect::FadeToFg { timer, .. }
            | FxEffect::PaintFg { timer, .. }
            | FxEffect::HslShiftFg { timer, .. }
            | FxEffect::Dissolve { timer }
            | FxEffect::Coalesce { timer }
            | FxEffect::SweepIn { timer, .. }
            | FxEffect::SweepOut { timer, .. } => u64::from(timer.ms),
            FxEffect::Sleep { ms } => u64::from(ms),
        }
    }
}

/// What a node is. Effect nodes are leaves; the rest wrap their children.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FxNodeKind {
    /// A leaf `fx::` constructor.
    Effect { effect: FxEffect },
    /// Children run one after another.
    Sequence,
    /// Children run together.
    Parallel,
    /// Repeat the child a fixed number of times.
    Repeat { times: u32 },
    /// Run the child forward, then backward.
    PingPong,
    /// Wait, then run the child.
    Delay { ms: u32 },
    /// Hold the child's first frame for `timer` before it runs.
    ProlongStart { timer: FxTimer },
    /// Hold the child's last frame for `timer` after it runs.
    ProlongEnd { timer: FxTimer },
    /// Hold the child at one progress value.
    FreezeAt { alpha: f32 },
    /// Confine the child to a named region.
    Area { target: FxTarget },
    /// Confine the child to matching cells.
    Filter { filter: FxFilter },
}

/// How many children a kind takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arity {
    None,
    Exactly1,
    AtLeast1,
}

impl fmt::Display for Arity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Arity::None => "no children",
            Arity::Exactly1 => "exactly 1 child",
            Arity::AtLeast1 => "at least 1 child",
        })
    }
}

impl FxNodeKind {
    pub fn arity(&self) -> Arity {
        match self {
            FxNodeKind::Effect { .. } => Arity::None,
            FxNodeKind::Sequence | FxNodeKind::Parallel => Arity::AtLeast1,
            _ => Arity::Exactly1,
        }
    }

    /// Short node title, as shown on the graph node's border.
    pub fn title(&self) -> &'static str {
        match self {
            FxNodeKind::Effect { effect } => effect.fn_name(),
            FxNodeKind::Sequence => "sequence",
            FxNodeKind::Parallel => "parallel",
            FxNodeKind::Repeat { .. } => "repeat",
            FxNodeKind::PingPong => "ping_pong",
            FxNodeKind::Delay { .. } => "delay",
            FxNodeKind::ProlongStart { .. } => "prolong_start",
            FxNodeKind::ProlongEnd { .. } => "prolong_end",
            FxNodeKind::FreezeAt { .. } => "freeze_at",
            FxNodeKind::Area { .. } => "area",
            FxNodeKind::Filter { .. } => "filter",
        }
    }

    /// One-line parameter summary shown inside the node.
    pub fn summary(&self) -> String {
        match self {
            FxNodeKind::Effect { effect } => effect.summary(),
            FxNodeKind::Sequence => "in order".into(),
            FxNodeKind::Parallel => "together".into(),
            FxNodeKind::Repeat { times } => format!("x{times}"),
            FxNodeKind::PingPong => "there and back".into(),
            FxNodeKind::Delay { ms } => format!("{ms}ms"),
            FxNodeKind::ProlongStart { timer } | FxNodeKind::ProlongEnd { timer } => {
                timer.summary()
            }
            FxNodeKind::FreezeAt { alpha } => format!("at {alpha:?}"),
            FxNodeKind::Area { target } => target.ident(),
            FxNodeKind::Filter { filter } => filter.summary(),
        }
    }
}

/// One node of the graph. `children` is ordered: it is the order of a
/// sequence and of the emitted argument list.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FxNode {
    pub id: FxNodeId,
    #[serde(flatten)]
    pub kind: FxNodeKind,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<FxNodeId>,
}

/// A child-to-parent edge, derived from `FxNode::children`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FxEdge {
    pub child: FxNodeId,
    pub parent: FxNodeId,
    /// Position of the child in the parent's ordered children.
    pub slot: usize,
}

/// Why a graph cannot be emitted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FxGraphError {
    Empty,
    UnknownRoot(FxNodeId),
    DuplicateId(FxNodeId),
    UnknownChild {
        parent: FxNodeId,
        child: FxNodeId,
    },
    /// A node is the child of more than one parent (or twice of one).
    SharedChild(FxNodeId),
    /// The root is some node's child.
    RootHasParent(FxNodeId),
    Cycle(FxNodeId),
    Arity {
        node: FxNodeId,
        expected: Arity,
        found: usize,
    },
}

impl fmt::Display for FxGraphError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FxGraphError::Empty => write!(f, "graph has no nodes"),
            FxGraphError::UnknownRoot(id) => write!(f, "root node {id} does not exist"),
            FxGraphError::DuplicateId(id) => write!(f, "node id {id} is used twice"),
            FxGraphError::UnknownChild { parent, child } => {
                write!(f, "node {parent} links to missing node {child}")
            }
            FxGraphError::SharedChild(id) => write!(f, "node {id} has more than one parent"),
            FxGraphError::RootHasParent(id) => write!(f, "root node {id} has a parent"),
            FxGraphError::Cycle(id) => write!(f, "cycle through node {id}"),
            FxGraphError::Arity {
                node,
                expected,
                found,
            } => write!(f, "node {node} takes {expected}, has {found}"),
        }
    }
}

impl std::error::Error for FxGraphError {}

/// The canonical effect graph (serde). Nodes not reachable from `root` are
/// kept (an editor needs scratch nodes) but ignored by the emitter.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FxGraph {
    pub root: FxNodeId,
    pub nodes: Vec<FxNode>,
}

impl FxGraph {
    pub fn node(&self, id: FxNodeId) -> Option<&FxNode> {
        self.nodes.iter().find(|n| n.id == id)
    }

    /// All child-to-parent edges, in node then slot order.
    pub fn edges(&self) -> Vec<FxEdge> {
        self.nodes
            .iter()
            .flat_map(|n| {
                n.children.iter().enumerate().map(|(slot, &child)| FxEdge {
                    child,
                    parent: n.id,
                    slot,
                })
            })
            .collect()
    }

    /// Check the graph is a well-formed tree under `root`.
    pub fn validate(&self) -> Result<(), FxGraphError> {
        if self.nodes.is_empty() {
            return Err(FxGraphError::Empty);
        }
        let mut by_id: BTreeMap<FxNodeId, &FxNode> = BTreeMap::new();
        for n in &self.nodes {
            if by_id.insert(n.id, n).is_some() {
                return Err(FxGraphError::DuplicateId(n.id));
            }
        }
        if !by_id.contains_key(&self.root) {
            return Err(FxGraphError::UnknownRoot(self.root));
        }
        let mut parented: BTreeSet<FxNodeId> = BTreeSet::new();
        for n in &self.nodes {
            for &child in &n.children {
                if !by_id.contains_key(&child) {
                    return Err(FxGraphError::UnknownChild {
                        parent: n.id,
                        child,
                    });
                }
                if !parented.insert(child) {
                    return Err(FxGraphError::SharedChild(child));
                }
            }
            let found = n.children.len();
            let ok = match n.kind.arity() {
                Arity::None => found == 0,
                Arity::Exactly1 => found == 1,
                Arity::AtLeast1 => found >= 1,
            };
            if !ok {
                return Err(FxGraphError::Arity {
                    node: n.id,
                    expected: n.kind.arity(),
                    found,
                });
            }
        }
        if parented.contains(&self.root) {
            return Err(FxGraphError::RootHasParent(self.root));
        }
        // With one parent per node and an unparented root, a walk from the
        // root cannot revisit a node; orphan cycles are never walked. Keep
        // an explicit check anyway so the emitter can never loop.
        let mut seen = BTreeSet::new();
        let mut stack = vec![self.root];
        while let Some(id) = stack.pop() {
            if !seen.insert(id) {
                return Err(FxGraphError::Cycle(id));
            }
            if let Some(n) = by_id.get(&id) {
                stack.extend(n.children.iter().copied());
            }
        }
        Ok(())
    }

    /// Ids reachable from the root, parents before children (pre-order).
    pub fn reachable(&self) -> Vec<FxNodeId> {
        let mut out = Vec::new();
        let mut seen = BTreeSet::new();
        let mut stack = vec![self.root];
        while let Some(id) = stack.pop() {
            if !seen.insert(id) {
                continue;
            }
            out.push(id);
            if let Some(n) = self.node(id) {
                stack.extend(n.children.iter().rev().copied());
            }
        }
        out
    }

    /// Emit tachyonfx DSL text for the tree under `root`.
    pub fn to_dsl(&self) -> Result<String, FxGraphError> {
        self.validate()?;
        Ok(self.emit(self.root))
    }

    fn emit(&self, id: FxNodeId) -> String {
        // validate() has run, so every id resolves.
        let Some(node) = self.node(id) else {
            return String::new();
        };
        let child = |i: usize| self.emit(node.children[i]);
        match &node.kind {
            FxNodeKind::Effect { effect } => {
                format!("fx::{}({})", effect.fn_name(), effect.args().join(", "))
            }
            FxNodeKind::Sequence | FxNodeKind::Parallel => {
                let items: Vec<String> = (0..node.children.len())
                    .map(|i| indent(&child(i)))
                    .collect();
                format!("fx::{}(&[\n{}\n])", node.kind.title(), items.join(",\n"))
            }
            FxNodeKind::Repeat { times } => wrap_call(
                "repeat",
                &[],
                child(0),
                &[format!("RepeatMode::Times({times})")],
            ),
            FxNodeKind::PingPong => wrap_call("ping_pong", &[], child(0), &[]),
            FxNodeKind::Delay { ms } => wrap_call("delay", &[ms.to_string()], child(0), &[]),
            FxNodeKind::ProlongStart { timer } => {
                wrap_call("prolong_start", &[timer.dsl()], child(0), &[])
            }
            FxNodeKind::ProlongEnd { timer } => {
                wrap_call("prolong_end", &[timer.dsl()], child(0), &[])
            }
            FxNodeKind::FreezeAt { alpha } => wrap_call(
                "freeze_at",
                &[format!("{alpha:?}"), "false".into()],
                child(0),
                &[],
            ),
            FxNodeKind::Area { target } => {
                format!("{}\n.with_area({})", child(0), target.ident())
            }
            FxNodeKind::Filter { filter } => {
                format!("{}\n.with_filter({})", child(0), filter.dsl())
            }
        }
    }

    /// Total run time in milliseconds of the tree under `root`, or `None`
    /// if the graph is invalid.
    pub fn duration_ms(&self) -> Option<u64> {
        self.validate().ok()?;
        Some(self.duration_of(self.root))
    }

    fn duration_of(&self, id: FxNodeId) -> u64 {
        let Some(node) = self.node(id) else {
            return 0;
        };
        let kids = node.children.iter().map(|&c| self.duration_of(c));
        match &node.kind {
            FxNodeKind::Effect { effect } => effect.duration_ms(),
            FxNodeKind::Sequence => kids.sum(),
            FxNodeKind::Parallel => kids.max().unwrap_or(0),
            FxNodeKind::Repeat { times } => kids.sum::<u64>().saturating_mul(u64::from(*times)),
            FxNodeKind::PingPong => kids.sum::<u64>().saturating_mul(2),
            FxNodeKind::Delay { ms } => kids.sum::<u64>() + u64::from(*ms),
            FxNodeKind::ProlongStart { timer } | FxNodeKind::ProlongEnd { timer } => {
                kids.sum::<u64>() + u64::from(timer.ms)
            }
            FxNodeKind::FreezeAt { .. } | FxNodeKind::Area { .. } | FxNodeKind::Filter { .. } => {
                kids.sum()
            }
        }
    }

    /// Indented outline of the tree, one node per line (text fallback for
    /// the graph view and a cheap thing to assert on in tests).
    pub fn outline(&self) -> Vec<String> {
        let mut out = Vec::new();
        if self.validate().is_ok() {
            self.outline_into(self.root, 0, &mut out);
        }
        out
    }

    fn outline_into(&self, id: FxNodeId, depth: usize, out: &mut Vec<String>) {
        if let Some(node) = self.node(id) {
            out.push(format!(
                "{}{} {}",
                "  ".repeat(depth),
                node.kind.title(),
                node.kind.summary()
            ));
            for &c in &node.children {
                self.outline_into(c, depth + 1, out);
            }
        }
    }
}

fn indent(text: &str) -> String {
    text.lines()
        .map(|l| format!("    {l}"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// `fx::name(pre.., child, post..)`, on one line when the child fits on
/// one, otherwise with the arguments on their own indented lines.
fn wrap_call(name: &str, pre: &[String], child: String, post: &[String]) -> String {
    let mut args: Vec<String> = pre.to_vec();
    args.push(child);
    args.extend(post.iter().cloned());
    if args.iter().all(|a| !a.contains('\n')) {
        format!("fx::{name}({})", args.join(", "))
    } else {
        let body: Vec<String> = args.iter().map(|a| indent(a)).collect();
        format!("fx::{name}(\n{}\n)", body.join(",\n"))
    }
}

/// Small builder that hands out ids, for presets and tests.
#[derive(Debug, Default)]
pub struct FxGraphBuilder {
    nodes: Vec<FxNode>,
}

impl FxGraphBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a node over already-added `children`; returns its id.
    pub fn add(&mut self, kind: FxNodeKind, children: &[FxNodeId]) -> FxNodeId {
        let id = self.nodes.len() as FxNodeId + 1;
        self.nodes.push(FxNode {
            id,
            kind,
            children: children.to_vec(),
        });
        id
    }

    pub fn effect(&mut self, effect: FxEffect) -> FxNodeId {
        self.add(FxNodeKind::Effect { effect }, &[])
    }

    pub fn build(self, root: FxNodeId) -> FxGraph {
        FxGraph {
            root,
            nodes: self.nodes,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fade() -> FxEffect {
        FxEffect::FadeToFg {
            color: FxColor::Cyan,
            timer: FxTimer::new(500, FxInterp::QuadOut),
        }
    }

    #[test]
    fn leaf_emits_single_call() {
        let mut b = FxGraphBuilder::new();
        let root = b.effect(fade());
        let dsl = b.build(root).to_dsl().unwrap();
        assert_eq!(dsl, "fx::fade_to_fg(Color::Cyan, (500, QuadOut))");
    }

    #[test]
    fn sequence_emits_indented_children_without_trailing_comma() {
        let mut b = FxGraphBuilder::new();
        let a = b.effect(fade());
        let s = b.effect(FxEffect::Sleep { ms: 300 });
        let root = b.add(FxNodeKind::Sequence, &[a, s]);
        let dsl = b.build(root).to_dsl().unwrap();
        assert_eq!(
            dsl,
            "fx::sequence(&[\n    fx::fade_to_fg(Color::Cyan, (500, QuadOut)),\n    fx::sleep(300)\n])"
        );
    }

    #[test]
    fn wrappers_inline_short_children_and_chain_modifiers() {
        let mut b = FxGraphBuilder::new();
        let a = b.effect(fade());
        let p = b.add(FxNodeKind::PingPong, &[a]);
        let r = b.add(FxNodeKind::Repeat { times: 3 }, &[p]);
        let f = b.add(
            FxNodeKind::Filter {
                filter: FxFilter::Text,
            },
            &[r],
        );
        let root = b.add(
            FxNodeKind::Area {
                target: FxTarget::Layer(2),
            },
            &[f],
        );
        let dsl = b.build(root).to_dsl().unwrap();
        assert_eq!(
            dsl,
            "fx::repeat(fx::ping_pong(fx::fade_to_fg(Color::Cyan, (500, QuadOut))), RepeatMode::Times(3))\n.with_filter(CellFilter::Text)\n.with_area(layer_2)"
        );
    }

    #[test]
    fn edges_run_child_to_parent_in_slot_order() {
        let mut b = FxGraphBuilder::new();
        let a = b.effect(fade());
        let c = b.effect(FxEffect::Sleep { ms: 1 });
        let root = b.add(FxNodeKind::Parallel, &[a, c]);
        let g = b.build(root);
        assert_eq!(
            g.edges(),
            vec![
                FxEdge {
                    child: a,
                    parent: root,
                    slot: 0
                },
                FxEdge {
                    child: c,
                    parent: root,
                    slot: 1
                },
            ]
        );
    }

    #[test]
    fn validate_rejects_bad_shapes() {
        let empty = FxGraph {
            root: 1,
            nodes: vec![],
        };
        assert_eq!(empty.validate(), Err(FxGraphError::Empty));

        let mut b = FxGraphBuilder::new();
        let a = b.effect(fade());
        let mut g = b.build(a);
        g.root = 9;
        assert_eq!(g.validate(), Err(FxGraphError::UnknownRoot(9)));

        // Sequence with no children.
        let mut b = FxGraphBuilder::new();
        let s = b.add(FxNodeKind::Sequence, &[]);
        assert_eq!(
            b.build(s).validate(),
            Err(FxGraphError::Arity {
                node: s,
                expected: Arity::AtLeast1,
                found: 0
            })
        );

        // Effect leaf given a child.
        let mut b = FxGraphBuilder::new();
        let a = b.effect(fade());
        let leaf = b.add(FxNodeKind::Effect { effect: fade() }, &[a]);
        assert!(matches!(
            b.build(leaf).validate(),
            Err(FxGraphError::Arity { .. })
        ));

        // Shared child.
        let mut b = FxGraphBuilder::new();
        let a = b.effect(fade());
        let s = b.add(FxNodeKind::Sequence, &[a, a]);
        assert_eq!(b.build(s).validate(), Err(FxGraphError::SharedChild(a)));

        // Missing child.
        let mut b = FxGraphBuilder::new();
        let s = b.add(FxNodeKind::PingPong, &[42]);
        assert_eq!(
            b.build(s).validate(),
            Err(FxGraphError::UnknownChild {
                parent: s,
                child: 42
            })
        );

        // Duplicate id.
        let mut b = FxGraphBuilder::new();
        let a = b.effect(fade());
        let mut g = b.build(a);
        g.nodes.push(g.nodes[0].clone());
        assert_eq!(g.validate(), Err(FxGraphError::DuplicateId(a)));
    }

    #[test]
    fn validate_rejects_cycles_through_root() {
        // 1 -> 2 -> 1: the root ends up with a parent.
        let g = FxGraph {
            root: 1,
            nodes: vec![
                FxNode {
                    id: 1,
                    kind: FxNodeKind::PingPong,
                    children: vec![2],
                },
                FxNode {
                    id: 2,
                    kind: FxNodeKind::PingPong,
                    children: vec![1],
                },
            ],
        };
        assert_eq!(g.validate(), Err(FxGraphError::RootHasParent(1)));
        assert!(g.to_dsl().is_err());
    }

    #[test]
    fn orphans_are_kept_but_not_emitted() {
        let mut b = FxGraphBuilder::new();
        let root = b.effect(fade());
        let _orphan = b.effect(FxEffect::Sleep { ms: 5 });
        let g = b.build(root);
        assert_eq!(g.reachable(), vec![root]);
        assert!(!g.to_dsl().unwrap().contains("sleep"));
    }

    #[test]
    fn duration_follows_combinator_semantics() {
        let mut b = FxGraphBuilder::new();
        let a = b.effect(fade()); // 500
        let s = b.effect(FxEffect::Sleep { ms: 300 });
        let par = b.add(FxNodeKind::Parallel, &[a, s]); // 500
        let d = b.add(FxNodeKind::Delay { ms: 100 }, &[par]); // 600
        let pp = b.add(FxNodeKind::PingPong, &[d]); // 1200
        let root = b.add(FxNodeKind::Repeat { times: 2 }, &[pp]); // 2400
        assert_eq!(b.build(root).duration_ms(), Some(2400));
    }

    #[test]
    fn serde_json_round_trip_preserves_graph() {
        let mut b = FxGraphBuilder::new();
        let a = b.effect(FxEffect::SweepIn {
            motion: FxMotion::UpToDown,
            gradient_len: 8,
            randomness: 2,
            color: FxColor::Rgb(1, 2, 3),
            timer: FxTimer::new(250, FxInterp::Spring),
        });
        let h = b.effect(FxEffect::HslShiftFg {
            hsl: [40.0, -0.5, 0.25],
            timer: FxTimer::new(800, FxInterp::Linear),
        });
        let f = b.add(
            FxNodeKind::Filter {
                filter: FxFilter::Outer {
                    horizontal: 1,
                    vertical: 2,
                },
            },
            &[h],
        );
        let root = b.add(FxNodeKind::Parallel, &[a, f]);
        let g = b.build(root);
        let json = serde_json::to_string(&g).unwrap();
        let back: FxGraph = serde_json::from_str(&json).unwrap();
        assert_eq!(back, g);
        assert_eq!(back.to_dsl().unwrap(), g.to_dsl().unwrap());
    }

    #[test]
    fn outline_indents_by_depth() {
        let mut b = FxGraphBuilder::new();
        let a = b.effect(FxEffect::Dissolve {
            timer: FxTimer::new(500, FxInterp::Linear),
        });
        let root = b.add(
            FxNodeKind::Area {
                target: FxTarget::Canvas,
            },
            &[a],
        );
        assert_eq!(
            b.build(root).outline(),
            vec![
                "area canvas".to_string(),
                "  dissolve 500ms Linear".to_string()
            ]
        );
    }
}
