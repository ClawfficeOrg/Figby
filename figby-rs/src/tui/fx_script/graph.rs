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

    pub fn name(self) -> String {
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
    pub fn dsl_name(self) -> &'static str {
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

/// The type of a parameter slot or of a Value node. A Value node may only be
/// bound to a slot of the same type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FxValueType {
    Color,
    Timer,
    Motion,
    Target,
    /// A plain duration in milliseconds (`sleep`, `glitch`, `delay`).
    Ms,
    /// A repeat count (`RepeatMode::Times`).
    Times,
    /// A progress value in 0..=1 (`freeze_at`).
    Alpha,
    /// An HSL shift `[hue, saturation, lightness]` (`hsl_shift_fg`).
    Hsl,
}

impl FxValueType {
    /// Short name, as shown in node titles and error messages.
    pub fn name(self) -> &'static str {
        match self {
            FxValueType::Color => "colour",
            FxValueType::Timer => "timer",
            FxValueType::Motion => "motion",
            FxValueType::Target => "target",
            FxValueType::Ms => "ms",
            FxValueType::Times => "times",
            FxValueType::Alpha => "alpha",
            FxValueType::Hsl => "hsl",
        }
    }
}

impl fmt::Display for FxValueType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// The payload of a Value node: one typed value that effect parameters can
/// reference instead of carrying it inline.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FxValue {
    Color(FxColor),
    Timer(FxTimer),
    Motion(FxMotion),
    Target(FxTarget),
    Ms(u32),
    Times(u32),
    Alpha(f32),
    Hsl([f32; 3]),
}

impl FxValue {
    pub fn ty(self) -> FxValueType {
        match self {
            FxValue::Color(_) => FxValueType::Color,
            FxValue::Timer(_) => FxValueType::Timer,
            FxValue::Motion(_) => FxValueType::Motion,
            FxValue::Target(_) => FxValueType::Target,
            FxValue::Ms(_) => FxValueType::Ms,
            FxValue::Times(_) => FxValueType::Times,
            FxValue::Alpha(_) => FxValueType::Alpha,
            FxValue::Hsl(_) => FxValueType::Hsl,
        }
    }

    /// The DSL expression for the value (the right side of its `let`).
    pub fn dsl(self) -> String {
        match self {
            FxValue::Color(c) => c.dsl(),
            FxValue::Timer(t) => t.dsl(),
            FxValue::Motion(m) => m.dsl_name().to_string(),
            FxValue::Target(t) => t.ident(),
            FxValue::Ms(n) | FxValue::Times(n) => n.to_string(),
            FxValue::Alpha(a) => format!("{a:?}"),
            FxValue::Hsl(hsl) => hsl_dsl(hsl),
        }
    }

    pub fn summary(self) -> String {
        match self {
            FxValue::Color(c) => c.name(),
            FxValue::Timer(t) => t.summary(),
            FxValue::Motion(m) => m.dsl_name().to_string(),
            FxValue::Target(t) => t.ident(),
            FxValue::Ms(n) => format!("{n}ms"),
            FxValue::Times(n) => format!("x{n}"),
            FxValue::Alpha(a) => format!("at {a:?}"),
            FxValue::Hsl(hsl) => hsl_summary(hsl),
        }
    }
}

/// `[h, s, l]` as a DSL array literal (floats keep their decimal point).
fn hsl_dsl(hsl: [f32; 3]) -> String {
    format!("[{:?}, {:?}, {:?}]", hsl[0], hsl[1], hsl[2])
}

fn hsl_summary(hsl: [f32; 3]) -> String {
    format!("h{:+} s{:+} l{:+}", hsl[0], hsl[1], hsl[2])
}

/// The DSL variable a Value node is bound to with `let`.
pub fn value_var(id: FxNodeId) -> String {
    format!("v{id}")
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
    /// Figby's own `glitch(ms)`, registered with the DSL in
    /// `fx_script::runtime` (tachyonfx 0.25 has no glitch constructor).
    Glitch {
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
            FxEffect::Glitch { .. } => "glitch",
        }
    }

    /// Arguments in DSL order, each tagged with the parameter slot it fills
    /// (`None` for arguments no Value node can stand in for).
    fn args(&self) -> Vec<(Option<&'static str>, String)> {
        let timer = |t: FxTimer| (Some("timer"), t.dsl());
        match *self {
            FxEffect::FadeFrom { bg, fg, timer: t } | FxEffect::FadeTo { bg, fg, timer: t } => {
                vec![(Some("bg"), bg.dsl()), (Some("fg"), fg.dsl()), timer(t)]
            }
            FxEffect::FadeFromFg { color, timer: t }
            | FxEffect::FadeToFg { color, timer: t }
            | FxEffect::PaintFg { color, timer: t } => vec![(Some("color"), color.dsl()), timer(t)],
            FxEffect::HslShiftFg { hsl, timer: t } => vec![(Some("hsl"), hsl_dsl(hsl)), timer(t)],
            FxEffect::Dissolve { timer: t } | FxEffect::Coalesce { timer: t } => vec![timer(t)],
            FxEffect::SweepIn {
                motion,
                gradient_len,
                randomness,
                color,
                timer: t,
            }
            | FxEffect::SweepOut {
                motion,
                gradient_len,
                randomness,
                color,
                timer: t,
            } => vec![
                (Some("motion"), motion.dsl_name().to_string()),
                (None, gradient_len.to_string()),
                (None, randomness.to_string()),
                (Some("color"), color.dsl()),
                timer(t),
            ],
            FxEffect::Sleep { ms } | FxEffect::Glitch { ms } => vec![(Some("ms"), ms.to_string())],
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
            FxEffect::HslShiftFg { hsl, timer } => {
                format!("{} {}", hsl_summary(hsl), timer.summary())
            }
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
            FxEffect::Sleep { ms } | FxEffect::Glitch { ms } => format!("{ms}ms"),
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
            FxEffect::Sleep { ms } | FxEffect::Glitch { ms } => u64::from(ms),
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
    /// A typed value. It is never a child: effect parameters reference it
    /// through [`FxNode::params`], and the emitter binds it with `let`.
    Value { value: FxValue },
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
            FxNodeKind::Effect { .. } | FxNodeKind::Value { .. } => Arity::None,
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
            FxNodeKind::Value { value } => value.ty().name(),
        }
    }

    /// The parameter slots a Value node can be bound to, with their types.
    pub fn param_slots(&self) -> &'static [(&'static str, FxValueType)] {
        use FxValueType as T;
        match self {
            FxNodeKind::Effect { effect } => match effect {
                FxEffect::FadeFrom { .. } | FxEffect::FadeTo { .. } => {
                    &[("bg", T::Color), ("fg", T::Color), ("timer", T::Timer)]
                }
                FxEffect::FadeFromFg { .. }
                | FxEffect::FadeToFg { .. }
                | FxEffect::PaintFg { .. } => &[("color", T::Color), ("timer", T::Timer)],
                FxEffect::HslShiftFg { .. } => &[("hsl", T::Hsl), ("timer", T::Timer)],
                FxEffect::Dissolve { .. } | FxEffect::Coalesce { .. } => &[("timer", T::Timer)],
                FxEffect::SweepIn { .. } | FxEffect::SweepOut { .. } => &[
                    ("motion", T::Motion),
                    ("color", T::Color),
                    ("timer", T::Timer),
                ],
                FxEffect::Sleep { .. } | FxEffect::Glitch { .. } => &[("ms", T::Ms)],
            },
            FxNodeKind::Repeat { .. } => &[("times", T::Times)],
            FxNodeKind::Delay { .. } => &[("ms", T::Ms)],
            FxNodeKind::FreezeAt { .. } => &[("alpha", T::Alpha)],
            FxNodeKind::ProlongStart { .. } | FxNodeKind::ProlongEnd { .. } => {
                &[("timer", T::Timer)]
            }
            FxNodeKind::Area { .. } => &[("target", T::Target)],
            // Only the colour filters carry a value; the slot exists while
            // the filter is one of them.
            FxNodeKind::Filter {
                filter: FxFilter::FgColor(_) | FxFilter::BgColor(_),
            } => &[("color", T::Color)],
            FxNodeKind::Sequence
            | FxNodeKind::Parallel
            | FxNodeKind::PingPong
            | FxNodeKind::Filter { .. }
            | FxNodeKind::Value { .. } => &[],
        }
    }

    /// The type of parameter slot `slot`, if this kind has it.
    pub fn slot_type(&self, slot: &str) -> Option<FxValueType> {
        self.param_slots()
            .iter()
            .find(|(name, _)| *name == slot)
            .map(|&(_, ty)| ty)
    }

    /// The inline value of parameter slot `slot`.
    pub fn slot_value(&self, slot: &str) -> Option<FxValue> {
        let mut out = None;
        let mut k = self.clone();
        k.visit_slots(&mut |name, v| {
            if name == slot {
                out = Some(*v);
            }
        });
        out
    }

    /// This kind with slot `slot` set to `value` inline. Unchanged when the
    /// slot does not exist or the value has another type.
    pub fn with_slot_value(&self, slot: &str, value: FxValue) -> FxNodeKind {
        let mut k = self.clone();
        k.visit_slots(&mut |name, v| {
            if name == slot && v.ty() == value.ty() {
                *v = value;
            }
        });
        k
    }

    /// Call `f` with every slot's inline value; writes to the value land in
    /// the kind.
    fn visit_slots(&mut self, f: &mut dyn FnMut(&'static str, &mut FxValue)) {
        fn color(
            name: &'static str,
            c: &mut FxColor,
            f: &mut dyn FnMut(&'static str, &mut FxValue),
        ) {
            let mut v = FxValue::Color(*c);
            f(name, &mut v);
            if let FxValue::Color(n) = v {
                *c = n;
            }
        }
        fn timer(t: &mut FxTimer, f: &mut dyn FnMut(&'static str, &mut FxValue)) {
            let mut v = FxValue::Timer(*t);
            f("timer", &mut v);
            if let FxValue::Timer(n) = v {
                *t = n;
            }
        }
        fn ms(name: &'static str, n: &mut u32, f: &mut dyn FnMut(&'static str, &mut FxValue)) {
            let mut v = FxValue::Ms(*n);
            f(name, &mut v);
            if let FxValue::Ms(x) = v {
                *n = x;
            }
        }
        match self {
            FxNodeKind::Effect { effect } => match effect {
                FxEffect::FadeFrom { bg, fg, timer: t } | FxEffect::FadeTo { bg, fg, timer: t } => {
                    color("bg", bg, f);
                    color("fg", fg, f);
                    timer(t, f);
                }
                FxEffect::FadeFromFg { color: c, timer: t }
                | FxEffect::FadeToFg { color: c, timer: t }
                | FxEffect::PaintFg { color: c, timer: t } => {
                    color("color", c, f);
                    timer(t, f);
                }
                FxEffect::HslShiftFg { hsl, timer: t } => {
                    let mut v = FxValue::Hsl(*hsl);
                    f("hsl", &mut v);
                    if let FxValue::Hsl(n) = v {
                        *hsl = n;
                    }
                    timer(t, f);
                }
                FxEffect::Dissolve { timer: t } | FxEffect::Coalesce { timer: t } => timer(t, f),
                FxEffect::SweepIn {
                    motion,
                    color: c,
                    timer: t,
                    ..
                }
                | FxEffect::SweepOut {
                    motion,
                    color: c,
                    timer: t,
                    ..
                } => {
                    let mut v = FxValue::Motion(*motion);
                    f("motion", &mut v);
                    if let FxValue::Motion(n) = v {
                        *motion = n;
                    }
                    color("color", c, f);
                    timer(t, f);
                }
                FxEffect::Sleep { ms: n } | FxEffect::Glitch { ms: n } => ms("ms", n, f),
            },
            FxNodeKind::Repeat { times } => {
                let mut v = FxValue::Times(*times);
                f("times", &mut v);
                if let FxValue::Times(n) = v {
                    *times = n;
                }
            }
            FxNodeKind::Delay { ms: n } => ms("ms", n, f),
            FxNodeKind::FreezeAt { alpha } => {
                let mut v = FxValue::Alpha(*alpha);
                f("alpha", &mut v);
                if let FxValue::Alpha(n) = v {
                    *alpha = n;
                }
            }
            FxNodeKind::ProlongStart { timer: t } | FxNodeKind::ProlongEnd { timer: t } => {
                timer(t, f)
            }
            FxNodeKind::Area { target } => {
                let mut v = FxValue::Target(*target);
                f("target", &mut v);
                if let FxValue::Target(n) = v {
                    *target = n;
                }
            }
            FxNodeKind::Filter {
                filter: FxFilter::FgColor(c) | FxFilter::BgColor(c),
            } => color("color", c, f),
            FxNodeKind::Sequence
            | FxNodeKind::Parallel
            | FxNodeKind::PingPong
            | FxNodeKind::Filter { .. }
            | FxNodeKind::Value { .. } => {}
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
            FxNodeKind::Value { value } => value.summary(),
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
    /// Parameter slots bound to Value nodes (slot name to Value node id).
    /// A bound slot takes the Value node's value; the inline one is kept as
    /// the fallback for when the binding is removed.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub params: BTreeMap<String, FxNodeId>,
}

impl FxNode {
    /// A node with no children and no bindings.
    pub fn new(id: FxNodeId, kind: FxNodeKind) -> Self {
        Self {
            id,
            kind,
            children: Vec::new(),
            params: BTreeMap::new(),
        }
    }
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
    /// A Value node is linked as a child (it may only fill a parameter).
    ValueAsChild {
        parent: FxNodeId,
        child: FxNodeId,
    },
    /// The root is a Value node, which is not an effect.
    RootIsValue(FxNodeId),
    /// A binding names a slot the node does not have.
    UnknownSlot {
        node: FxNodeId,
        slot: String,
    },
    /// A binding points at a missing node.
    UnknownValue {
        node: FxNodeId,
        value: FxNodeId,
    },
    /// A binding points at a node that is not a Value of the slot's type
    /// (`found` is `None` when it is not a Value node at all).
    SlotType {
        node: FxNodeId,
        slot: String,
        expected: FxValueType,
        found: Option<FxValueType>,
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
            FxGraphError::ValueAsChild { parent, child } => write!(
                f,
                "value node {child} is linked under {parent}; values only fill parameters"
            ),
            FxGraphError::RootIsValue(id) => write!(f, "root node {id} is a value, not an effect"),
            FxGraphError::UnknownSlot { node, slot } => {
                write!(f, "node {node} has no parameter '{slot}'")
            }
            FxGraphError::UnknownValue { node, value } => {
                write!(f, "node {node} binds missing node {value}")
            }
            FxGraphError::SlotType {
                node,
                slot,
                expected,
                found,
            } => match found {
                Some(found) => write!(
                    f,
                    "node {node} '{slot}' takes a {expected} value, not a {found}"
                ),
                None => write!(f, "node {node} '{slot}' takes a {expected} value node"),
            },
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
        }
        if parented.contains(&self.root) {
            return Err(FxGraphError::RootHasParent(self.root));
        }
        if matches!(by_id[&self.root].kind, FxNodeKind::Value { .. }) {
            return Err(FxGraphError::RootIsValue(self.root));
        }
        // Type checks, on every node (scratch ones too, so an editor can
        // never hold a mistyped binding).
        for n in &self.nodes {
            for &child in &n.children {
                if matches!(by_id[&child].kind, FxNodeKind::Value { .. }) {
                    return Err(FxGraphError::ValueAsChild {
                        parent: n.id,
                        child,
                    });
                }
            }
            for (slot, &value) in &n.params {
                let Some(expected) = n.kind.slot_type(slot) else {
                    return Err(FxGraphError::UnknownSlot {
                        node: n.id,
                        slot: slot.clone(),
                    });
                };
                let Some(v) = by_id.get(&value) else {
                    return Err(FxGraphError::UnknownValue { node: n.id, value });
                };
                let found = match v.kind {
                    FxNodeKind::Value { value } => Some(value.ty()),
                    _ => None,
                };
                if found != Some(expected) {
                    return Err(FxGraphError::SlotType {
                        node: n.id,
                        slot: slot.clone(),
                        expected,
                        found,
                    });
                }
            }
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
        // Arity is checked on reachable nodes only, so an unattached scratch
        // node (an editor's fresh Sequence) does not invalidate the tree.
        for id in &seen {
            let Some(n) = by_id.get(id) else { continue };
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

    /// Emit tachyonfx DSL text for the tree under `root`. Value nodes used
    /// by the tree come first, one `let` each, in id order.
    pub fn to_dsl(&self) -> Result<String, FxGraphError> {
        self.validate()?;
        let mut out = String::new();
        for id in self.used_values() {
            if let Some(FxNodeKind::Value { value }) = self.node(id).map(|n| &n.kind) {
                out.push_str(&format!("let {} = {};\n", value_var(id), value.dsl()));
            }
        }
        out.push_str(&self.emit(self.root));
        Ok(out)
    }

    /// Value nodes bound by nodes reachable from the root, sorted by id.
    pub fn used_values(&self) -> Vec<FxNodeId> {
        let set: BTreeSet<FxNodeId> = self
            .reachable()
            .into_iter()
            .filter_map(|id| self.node(id))
            .flat_map(|n| n.params.values().copied())
            .collect();
        set.into_iter().collect()
    }

    /// A node's kind with every bound slot replaced by its Value node's
    /// value: what the node actually does.
    pub fn resolved_kind(&self, id: FxNodeId) -> Option<FxNodeKind> {
        let node = self.node(id)?;
        let mut kind = node.kind.clone();
        for (slot, &vid) in &node.params {
            if let Some(FxNodeKind::Value { value }) = self.node(vid).map(|n| &n.kind) {
                kind = kind.with_slot_value(slot, *value);
            }
        }
        Some(kind)
    }

    fn emit(&self, id: FxNodeId) -> String {
        // validate() has run, so every id resolves.
        let Some(node) = self.node(id) else {
            return String::new();
        };
        let child = |i: usize| self.emit(node.children[i]);
        // A bound slot emits the Value node's variable instead of the inline
        // value.
        let arg = |slot: &str, inline: String| match node.params.get(slot) {
            Some(&vid) => value_var(vid),
            None => inline,
        };
        match &node.kind {
            FxNodeKind::Effect { effect } => {
                let args: Vec<String> = effect
                    .args()
                    .into_iter()
                    .map(|(slot, text)| match slot {
                        Some(slot) => arg(slot, text),
                        None => text,
                    })
                    .collect();
                format!("fx::{}({})", effect.fn_name(), args.join(", "))
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
                &[format!(
                    "RepeatMode::Times({})",
                    arg("times", times.to_string())
                )],
            ),
            FxNodeKind::PingPong => wrap_call("ping_pong", &[], child(0), &[]),
            FxNodeKind::Delay { ms } => {
                wrap_call("delay", &[arg("ms", ms.to_string())], child(0), &[])
            }
            FxNodeKind::ProlongStart { timer } => {
                wrap_call("prolong_start", &[arg("timer", timer.dsl())], child(0), &[])
            }
            FxNodeKind::ProlongEnd { timer } => {
                wrap_call("prolong_end", &[arg("timer", timer.dsl())], child(0), &[])
            }
            FxNodeKind::FreezeAt { alpha } => wrap_call(
                "freeze_at",
                &[arg("alpha", format!("{alpha:?}")), "false".into()],
                child(0),
                &[],
            ),
            FxNodeKind::Area { target } => {
                format!(
                    "{}\n.with_area({})",
                    child(0),
                    arg("target", target.ident())
                )
            }
            FxNodeKind::Filter { filter } => {
                let filter = match *filter {
                    FxFilter::FgColor(c) => {
                        format!("CellFilter::FgColor({})", arg("color", c.dsl()))
                    }
                    FxFilter::BgColor(c) => {
                        format!("CellFilter::BgColor({})", arg("color", c.dsl()))
                    }
                    other => other.dsl(),
                };
                format!("{}\n.with_filter({filter})", child(0))
            }
            // validate() keeps Value nodes out of the tree.
            FxNodeKind::Value { .. } => String::new(),
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
        let kind = self.resolved_kind(id).unwrap_or_else(|| node.kind.clone());
        match &kind {
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
            FxNodeKind::Value { .. } => 0,
        }
    }

    /// Indented outline of the tree, one node per line (text fallback for
    /// the graph view and a cheap thing to assert on in tests).
    pub fn outline(&self) -> Vec<String> {
        self.outline_rows().into_iter().map(|(_, l)| l).collect()
    }

    /// [`Self::outline`] with the node id of each line.
    pub fn outline_rows(&self) -> Vec<(FxNodeId, String)> {
        let mut out = Vec::new();
        if self.validate().is_ok() {
            self.outline_into(self.root, 0, &mut out);
        }
        out
    }

    fn outline_into(&self, id: FxNodeId, depth: usize, out: &mut Vec<(FxNodeId, String)>) {
        if let Some(node) = self.node(id) {
            let kind = self.resolved_kind(id).unwrap_or_else(|| node.kind.clone());
            out.push((
                id,
                format!(
                    "{}{} {}",
                    "  ".repeat(depth),
                    node.kind.title(),
                    kind.summary()
                ),
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
        let mut node = FxNode::new(id, kind);
        node.children = children.to_vec();
        self.nodes.push(node);
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
                    children: vec![2],
                    ..FxNode::new(1, FxNodeKind::PingPong)
                },
                FxNode {
                    children: vec![1],
                    ..FxNode::new(2, FxNodeKind::PingPong)
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

    use crate::tui::fx_script::compile::{compile, FxTargets};
    use ratatui::layout::Rect;

    fn compiles(dsl: &str) {
        let targets = FxTargets::uniform(Rect::new(0, 0, 80, 24), 3);
        if let Err(e) = compile(dsl, &targets) {
            panic!("{e:?}\n{dsl}");
        }
    }

    fn value(b: &mut FxGraphBuilder, value: FxValue) -> FxNodeId {
        b.add(FxNodeKind::Value { value }, &[])
    }

    fn bind(g: &mut FxGraph, node: FxNodeId, slot: &str, value: FxNodeId) {
        let n = g.nodes.iter_mut().find(|n| n.id == node).unwrap();
        n.params.insert(slot.to_string(), value);
    }

    /// One Value node of every type, each bound to a matching slot.
    fn valued_graph() -> FxGraph {
        let mut b = FxGraphBuilder::new();
        let colour = value(&mut b, FxValue::Color(FxColor::Rgb(10, 20, 30)));
        let timer = value(&mut b, FxValue::Timer(FxTimer::new(300, FxInterp::SineOut)));
        let motion = value(&mut b, FxValue::Motion(FxMotion::DownToUp));
        let target = value(&mut b, FxValue::Target(FxTarget::Layer(2)));
        let fade = b.effect(fade());
        let sweep = b.effect(FxEffect::SweepIn {
            motion: FxMotion::LeftToRight,
            gradient_len: 4,
            randomness: 1,
            color: FxColor::Black,
            timer: FxTimer::new(100, FxInterp::Linear),
        });
        let hold = b.add(
            FxNodeKind::ProlongEnd {
                timer: FxTimer::new(50, FxInterp::Linear),
            },
            &[sweep],
        );
        let seq = b.add(FxNodeKind::Sequence, &[fade, hold]);
        let root = b.add(
            FxNodeKind::Area {
                target: FxTarget::Canvas,
            },
            &[seq],
        );
        let mut g = b.build(root);
        bind(&mut g, fade, "color", colour);
        bind(&mut g, fade, "timer", timer);
        bind(&mut g, sweep, "motion", motion);
        bind(&mut g, sweep, "color", colour);
        bind(&mut g, sweep, "timer", timer);
        bind(&mut g, hold, "timer", timer);
        bind(&mut g, root, "target", target);
        g
    }

    #[test]
    fn value_nodes_emit_lets_that_compile() {
        let g = valued_graph();
        let dsl = g.to_dsl().unwrap();
        assert!(dsl.starts_with(
            "let v1 = Color::Rgb(10, 20, 30);\nlet v2 = (300, SineOut);\nlet v3 = DownToUp;\nlet v4 = layer_2;\n"
        ), "{dsl}");
        assert!(dsl.contains("fx::fade_to_fg(v1, v2)"), "{dsl}");
        assert!(dsl.contains("fx::sweep_in(v3, 4, 1, v1, v2)"), "{dsl}");
        assert!(dsl.contains("fx::prolong_end(v2, "), "{dsl}");
        assert!(dsl.ends_with(".with_area(v4)"), "{dsl}");
        compiles(&dsl);
    }

    #[test]
    fn every_value_type_compiles_in_each_slot_it_fits() {
        // Each slot bound alone, so a broken binding is pinned to one slot.
        let g = valued_graph();
        for n in &g.nodes {
            for slot in n.params.keys() {
                let mut one = g.clone();
                for m in &mut one.nodes {
                    m.params.retain(|s, _| m.id == n.id && s == slot);
                }
                compiles(&one.to_dsl().unwrap());
            }
        }
    }

    #[test]
    fn bound_values_drive_duration_and_outline() {
        let g = valued_graph();
        // fade 300 (bound timer) + sweep 300 + hold 300.
        assert_eq!(g.duration_ms(), Some(900));
        let outline = g.outline();
        assert_eq!(outline[0], "area layer_2");
        assert!(outline
            .iter()
            .any(|l| l.contains("fade_to_fg #0a141e 300ms SineOut")));
    }

    #[test]
    fn unused_values_are_not_emitted() {
        let mut b = FxGraphBuilder::new();
        let _spare = value(&mut b, FxValue::Motion(FxMotion::UpToDown));
        let root = b.effect(fade());
        let dsl = b.build(root).to_dsl().unwrap();
        assert!(!dsl.contains("let"), "{dsl}");
    }

    #[test]
    fn validate_type_checks_bindings() {
        let base = valued_graph();
        let (colour, timer, fade, root) = (1, 2, 5, base.root);

        let mut g = base.clone();
        bind(&mut g, fade, "color", timer);
        assert_eq!(
            g.validate(),
            Err(FxGraphError::SlotType {
                node: fade,
                slot: "color".into(),
                expected: FxValueType::Color,
                found: Some(FxValueType::Timer),
            })
        );

        let mut g = base.clone();
        bind(&mut g, fade, "timer", fade);
        assert!(matches!(
            g.validate(),
            Err(FxGraphError::SlotType { found: None, .. })
        ));

        let mut g = base.clone();
        bind(&mut g, fade, "motion", colour);
        assert_eq!(
            g.validate(),
            Err(FxGraphError::UnknownSlot {
                node: fade,
                slot: "motion".into()
            })
        );

        let mut g = base.clone();
        bind(&mut g, root, "target", 77);
        assert_eq!(
            g.validate(),
            Err(FxGraphError::UnknownValue {
                node: root,
                value: 77
            })
        );

        let mut g = base.clone();
        let seq = g.node(root).unwrap().children[0];
        g.nodes
            .iter_mut()
            .find(|n| n.id == seq)
            .unwrap()
            .children
            .push(colour);
        assert_eq!(
            g.validate(),
            Err(FxGraphError::ValueAsChild {
                parent: seq,
                child: colour
            })
        );

        let mut g = base;
        g.root = colour;
        assert_eq!(g.validate(), Err(FxGraphError::RootIsValue(colour)));
    }

    #[test]
    fn serde_round_trip_keeps_values_and_bindings() {
        let g = valued_graph();
        let json = serde_json::to_string(&g).unwrap();
        let back: FxGraph = serde_json::from_str(&json).unwrap();
        assert_eq!(back, g);
        assert_eq!(back.to_dsl().unwrap(), g.to_dsl().unwrap());
    }

    /// One Value node of each numeric type, HSL and a filter colour, each
    /// bound to every slot it fits. Ids: 1 ms, 2 times, 3 alpha, 4 hsl,
    /// 5 colour, 6 sleep, 7 glitch, 8 hsl_shift_fg, 9 fg filter, 10 bg
    /// filter, 11 freeze_at, 12 sequence, 13 delay, 14 repeat (root).
    fn numeric_valued_graph() -> FxGraph {
        let mut b = FxGraphBuilder::new();
        let ms = value(&mut b, FxValue::Ms(300));
        let times = value(&mut b, FxValue::Times(3));
        let alpha = value(&mut b, FxValue::Alpha(0.5));
        let hsl = value(&mut b, FxValue::Hsl([40.0, -0.5, 0.25]));
        let colour = value(&mut b, FxValue::Color(FxColor::Red));
        let sleep = b.effect(FxEffect::Sleep { ms: 1 });
        let glitch = b.effect(FxEffect::Glitch { ms: 1 });
        let shift = b.effect(FxEffect::HslShiftFg {
            hsl: [0.0, 0.0, 0.0],
            timer: FxTimer::new(100, FxInterp::Linear),
        });
        let fg = b.add(
            FxNodeKind::Filter {
                filter: FxFilter::FgColor(FxColor::White),
            },
            &[sleep],
        );
        let bg = b.add(
            FxNodeKind::Filter {
                filter: FxFilter::BgColor(FxColor::Black),
            },
            &[glitch],
        );
        let freeze = b.add(FxNodeKind::FreezeAt { alpha: 0.0 }, &[shift]);
        let seq = b.add(FxNodeKind::Sequence, &[fg, bg, freeze]);
        let delay = b.add(FxNodeKind::Delay { ms: 1 }, &[seq]);
        let root = b.add(FxNodeKind::Repeat { times: 1 }, &[delay]);
        let mut g = b.build(root);
        bind(&mut g, sleep, "ms", ms);
        bind(&mut g, glitch, "ms", ms);
        bind(&mut g, delay, "ms", ms);
        bind(&mut g, root, "times", times);
        bind(&mut g, freeze, "alpha", alpha);
        bind(&mut g, shift, "hsl", hsl);
        bind(&mut g, fg, "color", colour);
        bind(&mut g, bg, "color", colour);
        g
    }

    #[test]
    fn numeric_hsl_and_filter_values_emit_lets_that_compile() {
        let g = numeric_valued_graph();
        let dsl = g.to_dsl().unwrap();
        assert!(
            dsl.starts_with(
                "let v1 = 300;\nlet v2 = 3;\nlet v3 = 0.5;\nlet v4 = [40.0, -0.5, 0.25];\nlet v5 = Color::Red;\n"
            ),
            "{dsl}"
        );
        for needle in [
            "fx::sleep(v1)",
            "fx::glitch(v1)",
            "fx::hsl_shift_fg(v4, (100, Linear))",
            ".with_filter(CellFilter::FgColor(v5))",
            ".with_filter(CellFilter::BgColor(v5))",
            "fx::freeze_at(v3, false, ",
            "fx::delay(\n        v1,",
            "RepeatMode::Times(v2)",
        ] {
            assert!(dsl.contains(needle), "{needle}\n{dsl}");
        }
        compiles(&dsl);
    }

    #[test]
    fn numeric_values_compile_in_each_slot_alone() {
        let g = numeric_valued_graph();
        let mut count = 0;
        for n in &g.nodes {
            for slot in n.params.keys() {
                let mut one = g.clone();
                for m in &mut one.nodes {
                    m.params.retain(|s, _| m.id == n.id && s == slot);
                }
                compiles(&one.to_dsl().unwrap());
                count += 1;
            }
        }
        assert_eq!(count, 8);
    }

    #[test]
    fn numeric_values_drive_duration_and_outline() {
        let g = numeric_valued_graph();
        // (sleep 300 + glitch 300 + shift 100 + delay 300) x 3.
        assert_eq!(g.duration_ms(), Some(3000));
        let outline = g.outline();
        assert_eq!(outline[0], "repeat x3");
        assert!(
            outline.iter().any(|l| l.contains("delay 300ms")),
            "{outline:?}"
        );
        assert!(
            outline.iter().any(|l| l.contains("filter fg Red")),
            "{outline:?}"
        );
        assert!(
            outline.iter().any(|l| l.contains("freeze_at at 0.5")),
            "{outline:?}"
        );
        assert!(
            outline
                .iter()
                .any(|l| l.contains("hsl_shift_fg h+40 s-0.5 l+0.25")),
            "{outline:?}"
        );
    }

    #[test]
    fn numeric_bindings_are_type_checked() {
        let base = numeric_valued_graph();
        let (ms, times, alpha, sleep, fg, root) = (1, 2, 3, 6, 9, base.root);
        let mut g = base.clone();
        bind(&mut g, root, "times", ms);
        assert_eq!(
            g.validate(),
            Err(FxGraphError::SlotType {
                node: root,
                slot: "times".into(),
                expected: FxValueType::Times,
                found: Some(FxValueType::Ms),
            })
        );
        let mut g = base.clone();
        bind(&mut g, sleep, "ms", alpha);
        assert!(matches!(
            g.validate(),
            Err(FxGraphError::SlotType {
                expected: FxValueType::Ms,
                found: Some(FxValueType::Alpha),
                ..
            })
        ));
        let mut g = base.clone();
        bind(&mut g, fg, "color", times);
        assert!(matches!(
            g.validate(),
            Err(FxGraphError::SlotType {
                expected: FxValueType::Color,
                ..
            })
        ));
        // A filter without a colour has no colour slot.
        let mut g = base;
        g.nodes.iter_mut().find(|n| n.id == fg).unwrap().kind = FxNodeKind::Filter {
            filter: FxFilter::Text,
        };
        assert_eq!(
            g.validate(),
            Err(FxGraphError::UnknownSlot {
                node: fg,
                slot: "color".into()
            })
        );
    }

    #[test]
    fn serde_round_trip_keeps_numeric_values() {
        let g = numeric_valued_graph();
        let json = serde_json::to_string(&g).unwrap();
        let back: FxGraph = serde_json::from_str(&json).unwrap();
        assert_eq!(back, g);
        assert_eq!(back.to_dsl().unwrap(), g.to_dsl().unwrap());
    }

    #[test]
    fn slot_values_cover_numbers_hsl_and_filter_colours() {
        let r = FxNodeKind::Repeat { times: 4 };
        assert_eq!(r.slot_value("times"), Some(FxValue::Times(4)));
        assert_eq!(
            r.with_slot_value("times", FxValue::Times(9)),
            FxNodeKind::Repeat { times: 9 }
        );
        // Same number, other type: refused.
        assert_eq!(r.with_slot_value("times", FxValue::Ms(9)), r);
        let f = FxNodeKind::Filter {
            filter: FxFilter::BgColor(FxColor::Blue),
        };
        assert_eq!(f.slot_value("color"), Some(FxValue::Color(FxColor::Blue)));
        assert_eq!(
            FxNodeKind::Filter {
                filter: FxFilter::All
            }
            .slot_value("color"),
            None
        );
        let h = FxNodeKind::Effect {
            effect: FxEffect::HslShiftFg {
                hsl: [1.0, 2.0, 3.0],
                timer: FxTimer::new(1, FxInterp::Linear),
            },
        };
        assert_eq!(h.slot_value("hsl"), Some(FxValue::Hsl([1.0, 2.0, 3.0])));
        assert_eq!(
            FxNodeKind::FreezeAt { alpha: 0.2 }.slot_value("alpha"),
            Some(FxValue::Alpha(0.2))
        );
    }

    #[test]
    fn slot_value_reads_and_writes_inline_values() {
        let k = FxNodeKind::Effect { effect: fade() };
        assert_eq!(k.slot_value("color"), Some(FxValue::Color(FxColor::Cyan)));
        let k2 = k.with_slot_value("color", FxValue::Color(FxColor::Red));
        assert_eq!(k2.slot_value("color"), Some(FxValue::Color(FxColor::Red)));
        // A value of the wrong type is ignored.
        assert_eq!(
            k.with_slot_value("color", FxValue::Motion(FxMotion::UpToDown)),
            k
        );
        assert_eq!(k.slot_value("motion"), None);
    }
}
