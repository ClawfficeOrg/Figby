//! The Props panel model: a node's parameters as a flat list of fields that
//! can be stepped (cycled or nudged) or typed (numbers). Pure: every change
//! returns a new `FxNodeKind`, which the overlay hands to
//! `GraphEditor::set_kind` so it is validated and undoable.
//!
//! Fields are produced by one visitor ([`visit`]), so the list a user sees
//! and the field a step changes can never disagree.

use super::graph::{
    FxColor, FxEffect, FxFilter, FxInterp, FxMotion, FxNodeKind, FxTarget, FxTimer, FxValue,
};

/// One editable field of a node.
#[derive(Debug, Clone, PartialEq)]
pub struct PropField {
    pub label: &'static str,
    /// The parameter slot this field belongs to, when a Value node can
    /// stand in for it (a timer's `ms` and `interp` both belong to `timer`).
    pub slot: Option<&'static str>,
    /// The current value, as shown.
    pub value: String,
    /// Whether the field takes a typed number.
    pub numeric: bool,
}

/// A mutable view of one field during a visit.
enum Prop<'a> {
    Effect(&'a mut FxEffect),
    Color(&'a mut FxColor),
    Interp(&'a mut FxInterp),
    Motion(&'a mut FxMotion),
    Target(&'a mut FxTarget),
    Filter(&'a mut FxFilter),
    Value(&'a mut FxValue),
    U32 {
        v: &'a mut u32,
        step: u32,
        min: u32,
        max: u32,
    },
    U16 {
        v: &'a mut u16,
        step: u16,
        max: u16,
    },
    F32 {
        v: &'a mut f32,
        step: f32,
        min: f32,
        max: f32,
    },
}

type Visitor<'f> = dyn FnMut(&'static str, Option<&'static str>, Prop<'_>) + 'f;

fn timer(t: &mut FxTimer, slot: Option<&'static str>, f: &mut Visitor<'_>) {
    f(
        "ms",
        slot,
        Prop::U32 {
            v: &mut t.ms,
            step: 50,
            min: 0,
            max: 600_000,
        },
    );
    f("interp", slot, Prop::Interp(&mut t.interp));
}

fn ms(v: &mut u32, f: &mut Visitor<'_>) {
    f(
        "ms",
        None,
        Prop::U32 {
            v,
            step: 50,
            min: 0,
            max: 600_000,
        },
    );
}

fn visit_value(v: &mut FxValue, slot: Option<&'static str>, f: &mut Visitor<'_>) {
    match v {
        FxValue::Color(c) => f("colour", slot, Prop::Color(c)),
        FxValue::Timer(t) => timer(t, slot, f),
        FxValue::Motion(m) => f("motion", slot, Prop::Motion(m)),
        FxValue::Target(t) => f("target", slot, Prop::Target(t)),
    }
}

/// Walk every field of `kind` in display order.
fn visit(kind: &mut FxNodeKind, f: &mut Visitor<'_>) {
    match kind {
        FxNodeKind::Effect { effect } => {
            f("effect", None, Prop::Effect(&mut *effect));
            match effect {
                FxEffect::FadeFrom { bg, fg, timer: t } | FxEffect::FadeTo { bg, fg, timer: t } => {
                    f("bg", Some("bg"), Prop::Color(bg));
                    f("fg", Some("fg"), Prop::Color(fg));
                    timer(t, Some("timer"), f);
                }
                FxEffect::FadeFromFg { color, timer: t }
                | FxEffect::FadeToFg { color, timer: t }
                | FxEffect::PaintFg { color, timer: t } => {
                    f("colour", Some("color"), Prop::Color(color));
                    timer(t, Some("timer"), f);
                }
                FxEffect::HslShiftFg { hsl, timer: t } => {
                    let [h, s, l] = hsl;
                    f(
                        "hue",
                        None,
                        Prop::F32 {
                            v: h,
                            step: 10.0,
                            min: -360.0,
                            max: 360.0,
                        },
                    );
                    f(
                        "sat",
                        None,
                        Prop::F32 {
                            v: s,
                            step: 0.1,
                            min: -1.0,
                            max: 1.0,
                        },
                    );
                    f(
                        "light",
                        None,
                        Prop::F32 {
                            v: l,
                            step: 0.1,
                            min: -1.0,
                            max: 1.0,
                        },
                    );
                    timer(t, Some("timer"), f);
                }
                FxEffect::Dissolve { timer: t } | FxEffect::Coalesce { timer: t } => {
                    timer(t, Some("timer"), f)
                }
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
                } => {
                    f("motion", Some("motion"), Prop::Motion(motion));
                    f(
                        "gradient",
                        None,
                        Prop::U16 {
                            v: gradient_len,
                            step: 1,
                            max: 200,
                        },
                    );
                    f(
                        "random",
                        None,
                        Prop::U16 {
                            v: randomness,
                            step: 1,
                            max: 200,
                        },
                    );
                    f("colour", Some("color"), Prop::Color(color));
                    timer(t, Some("timer"), f);
                }
                FxEffect::Sleep { ms: v } | FxEffect::Glitch { ms: v } => ms(v, f),
            }
        }
        FxNodeKind::Sequence | FxNodeKind::Parallel | FxNodeKind::PingPong => {}
        FxNodeKind::Repeat { times } => f(
            "times",
            None,
            Prop::U32 {
                v: times,
                step: 1,
                min: 1,
                max: 1000,
            },
        ),
        FxNodeKind::Delay { ms: v } => ms(v, f),
        FxNodeKind::ProlongStart { timer: t } | FxNodeKind::ProlongEnd { timer: t } => {
            timer(t, Some("timer"), f)
        }
        FxNodeKind::FreezeAt { alpha } => f(
            "alpha",
            None,
            Prop::F32 {
                v: alpha,
                step: 0.1,
                min: 0.0,
                max: 1.0,
            },
        ),
        FxNodeKind::Area { target } => f("target", Some("target"), Prop::Target(target)),
        FxNodeKind::Filter { filter } => {
            f("filter", None, Prop::Filter(&mut *filter));
            match filter {
                FxFilter::FgColor(c) | FxFilter::BgColor(c) => f("colour", None, Prop::Color(c)),
                FxFilter::Inner {
                    horizontal,
                    vertical,
                }
                | FxFilter::Outer {
                    horizontal,
                    vertical,
                } => {
                    f(
                        "horiz",
                        None,
                        Prop::U16 {
                            v: horizontal,
                            step: 1,
                            max: 100,
                        },
                    );
                    f(
                        "vert",
                        None,
                        Prop::U16 {
                            v: vertical,
                            step: 1,
                            max: 100,
                        },
                    );
                }
                FxFilter::All | FxFilter::Text | FxFilter::NonEmpty => {}
            }
        }
        FxNodeKind::Value { value } => {
            f("type", None, Prop::Value(&mut *value));
            visit_value(value, None, f);
        }
    }
}

/// The fields of `kind`, in display order.
pub fn fields(kind: &FxNodeKind) -> Vec<PropField> {
    let mut out = Vec::new();
    let mut k = kind.clone();
    visit(&mut k, &mut |label, slot, prop| {
        let (value, numeric) = describe(&prop);
        out.push(PropField {
            label,
            slot,
            value,
            numeric,
        });
    });
    out
}

/// `kind` with field `index` stepped by `delta` (cycled for choices,
/// nudged and clamped for numbers). `None` if there is no such field.
pub fn step(kind: &FxNodeKind, index: usize, delta: i32) -> Option<FxNodeKind> {
    change(kind, index, |prop| step_prop(prop, delta))
}

/// `kind` with numeric field `index` set to `value` (clamped). `None` if the
/// field does not exist or is not numeric.
pub fn set_number(kind: &FxNodeKind, index: usize, value: f64) -> Option<FxNodeKind> {
    if !value.is_finite() {
        return None;
    }
    change(kind, index, |prop| match prop {
        Prop::U32 { v, min, max, .. } => {
            *v = (value.round().max(0.0) as u64).clamp(u64::from(min), u64::from(max)) as u32;
            true
        }
        Prop::U16 { v, max, .. } => {
            *v = (value.round().max(0.0) as u64).min(u64::from(max)) as u16;
            true
        }
        Prop::F32 { v, min, max, .. } => {
            *v = (value as f32).clamp(min, max);
            true
        }
        _ => false,
    })
}

fn change(
    kind: &FxNodeKind,
    index: usize,
    op: impl FnOnce(Prop<'_>) -> bool,
) -> Option<FxNodeKind> {
    let mut k = kind.clone();
    let mut op = Some(op);
    let mut i = 0usize;
    let mut done = false;
    visit(&mut k, &mut |_, _, prop| {
        if i == index {
            if let Some(op) = op.take() {
                done = op(prop);
            }
        }
        i += 1;
    });
    done.then_some(k)
}

fn describe(prop: &Prop<'_>) -> (String, bool) {
    match prop {
        Prop::Effect(e) => (e.fn_name().to_string(), false),
        Prop::Color(c) => (c.name(), false),
        Prop::Interp(i) => (i.dsl_name().to_string(), false),
        Prop::Motion(m) => (m.dsl_name().to_string(), false),
        Prop::Target(t) => (t.ident(), false),
        Prop::Filter(fl) => (filter_name(**fl).to_string(), false),
        Prop::Value(v) => (v.ty().to_string(), false),
        Prop::U32 { v, .. } => (v.to_string(), true),
        Prop::U16 { v, .. } => (v.to_string(), true),
        Prop::F32 { v, .. } => (format!("{v:?}"), true),
    }
}

/// Position of `cur` in `all` moved by `delta`, wrapping. A value not in
/// the list moves to the first entry.
fn cycle<T: PartialEq + Copy>(all: &[T], cur: T, delta: i32) -> T {
    let n = all.len() as i64;
    match all.iter().position(|x| *x == cur) {
        Some(i) => all[(i as i64 + i64::from(delta)).rem_euclid(n) as usize],
        None => all[0],
    }
}

/// Index of `cur` among values compared by `same`, moved by `delta`.
fn cycle_by<T: Copy>(all: &[T], same: impl Fn(&T) -> bool, delta: i32) -> T {
    let n = all.len() as i64;
    match all.iter().position(same) {
        Some(i) => all[(i as i64 + i64::from(delta)).rem_euclid(n) as usize],
        None => all[0],
    }
}

const COLORS: &[FxColor] = &[
    FxColor::Black,
    FxColor::Red,
    FxColor::Green,
    FxColor::Yellow,
    FxColor::Blue,
    FxColor::Magenta,
    FxColor::Cyan,
    FxColor::Gray,
    FxColor::DarkGray,
    FxColor::White,
];

const MOTIONS: &[FxMotion] = &[
    FxMotion::LeftToRight,
    FxMotion::RightToLeft,
    FxMotion::UpToDown,
    FxMotion::DownToUp,
];

const TARGETS: &[FxTarget] = &[
    FxTarget::Screen,
    FxTarget::Canvas,
    FxTarget::Selection,
    FxTarget::Toolbox,
    FxTarget::SidePanel,
    FxTarget::Status,
    FxTarget::Layer(1),
    FxTarget::Layer(2),
    FxTarget::Layer(3),
    FxTarget::Layer(4),
    FxTarget::Layer(5),
    FxTarget::Layer(6),
    FxTarget::Layer(7),
    FxTarget::Layer(8),
    FxTarget::Layer(9),
];

fn filter_name(f: FxFilter) -> &'static str {
    match f {
        FxFilter::All => "all",
        FxFilter::Text => "text",
        FxFilter::NonEmpty => "non_empty",
        FxFilter::FgColor(_) => "fg_color",
        FxFilter::BgColor(_) => "bg_color",
        FxFilter::Inner { .. } => "inner",
        FxFilter::Outer { .. } => "outer",
    }
}

/// Every effect constructor, carrying over the timer and colour of `cur` so
/// switching effects keeps what the two have in common.
fn effect_choices(cur: &FxEffect) -> [FxEffect; 12] {
    let k = FxNodeKind::Effect {
        effect: cur.clone(),
    };
    let timer = match k.slot_value("timer") {
        Some(FxValue::Timer(t)) => t,
        _ => FxTimer::new(cur.duration_ms().min(600_000) as u32, FxInterp::Linear),
    };
    let color = match k.slot_value("color").or_else(|| k.slot_value("fg")) {
        Some(FxValue::Color(c)) => c,
        _ => FxColor::White,
    };
    let ms = timer.ms;
    [
        FxEffect::FadeFrom {
            bg: FxColor::Black,
            fg: color,
            timer,
        },
        FxEffect::FadeTo {
            bg: FxColor::Black,
            fg: color,
            timer,
        },
        FxEffect::FadeFromFg { color, timer },
        FxEffect::FadeToFg { color, timer },
        FxEffect::PaintFg { color, timer },
        FxEffect::HslShiftFg {
            hsl: [40.0, 0.0, 0.0],
            timer,
        },
        FxEffect::Dissolve { timer },
        FxEffect::Coalesce { timer },
        FxEffect::SweepIn {
            motion: FxMotion::LeftToRight,
            gradient_len: 10,
            randomness: 0,
            color,
            timer,
        },
        FxEffect::SweepOut {
            motion: FxMotion::LeftToRight,
            gradient_len: 10,
            randomness: 0,
            color,
            timer,
        },
        FxEffect::Sleep { ms },
        FxEffect::Glitch { ms },
    ]
}

fn step_prop(prop: Prop<'_>, delta: i32) -> bool {
    match prop {
        Prop::Effect(e) => {
            let name = e.fn_name();
            let all = effect_choices(e);
            let pos = all.iter().position(|x| x.fn_name() == name).unwrap_or(0);
            let n = all.len() as i64;
            *e = all[(pos as i64 + i64::from(delta)).rem_euclid(n) as usize].clone();
        }
        Prop::Color(c) => *c = cycle(COLORS, *c, delta),
        Prop::Interp(i) => *i = cycle(FxInterp::ALL, *i, delta),
        Prop::Motion(m) => *m = cycle(MOTIONS, *m, delta),
        Prop::Target(t) => *t = cycle(TARGETS, *t, delta),
        Prop::Filter(fl) => {
            let all = [
                FxFilter::All,
                FxFilter::Text,
                FxFilter::NonEmpty,
                FxFilter::FgColor(FxColor::White),
                FxFilter::BgColor(FxColor::Black),
                FxFilter::Inner {
                    horizontal: 1,
                    vertical: 1,
                },
                FxFilter::Outer {
                    horizontal: 1,
                    vertical: 1,
                },
            ];
            let name = filter_name(*fl);
            *fl = cycle_by(&all, |x| filter_name(*x) == name, delta);
        }
        Prop::Value(v) => {
            let all = [
                FxValue::Color(FxColor::White),
                FxValue::Timer(FxTimer::new(500, FxInterp::Linear)),
                FxValue::Motion(FxMotion::LeftToRight),
                FxValue::Target(FxTarget::Canvas),
            ];
            let ty = v.ty();
            *v = cycle_by(&all, |x| x.ty() == ty, delta);
        }
        Prop::U32 { v, step, min, max } => {
            let next = i64::from(*v) + i64::from(delta) * i64::from(step);
            *v = next.clamp(i64::from(min), i64::from(max)) as u32;
        }
        Prop::U16 { v, step, max } => {
            let next = i64::from(*v) + i64::from(delta) * i64::from(step);
            *v = next.clamp(0, i64::from(max)) as u16;
        }
        Prop::F32 { v, step, min, max } => {
            // Round to the step so repeated nudges do not drift (0.1 + 0.2).
            let next = ((*v + delta as f32 * step) / step).round() * step;
            *v = next.clamp(min, max);
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::fx_script::presets;

    fn fade() -> FxNodeKind {
        FxNodeKind::Effect {
            effect: FxEffect::FadeToFg {
                color: FxColor::Cyan,
                timer: FxTimer::new(500, FxInterp::QuadOut),
            },
        }
    }

    fn field(kind: &FxNodeKind, label: &str) -> usize {
        fields(kind)
            .iter()
            .position(|f| f.label == label)
            .unwrap_or_else(|| panic!("no field {label}"))
    }

    #[test]
    fn fields_list_parameters_with_slots() {
        let f = fields(&fade());
        let labels: Vec<_> = f
            .iter()
            .map(|f| (f.label, f.slot, f.value.as_str()))
            .collect();
        assert_eq!(
            labels,
            vec![
                ("effect", None, "fade_to_fg"),
                ("colour", Some("color"), "Cyan"),
                ("ms", Some("timer"), "500"),
                ("interp", Some("timer"), "QuadOut"),
            ]
        );
        assert!(f[2].numeric && !f[1].numeric);
    }

    #[test]
    fn step_cycles_choices_and_nudges_numbers() {
        let k = fade();
        let k = step(&k, field(&k, "colour"), 1).unwrap();
        assert_eq!(fields(&k)[1].value, "Gray");
        let k = step(&k, field(&k, "ms"), -1).unwrap();
        assert_eq!(fields(&k)[2].value, "450");
        let k = step(&k, field(&k, "interp"), 1).unwrap();
        assert_eq!(fields(&k)[3].value, "QuadInOut");
        // Gray is index 7; eight steps back wrap past Black to White.
        let mut k = k;
        for _ in 0..8 {
            k = step(&k, 1, -1).unwrap();
        }
        assert_eq!(fields(&k)[1].value, "White");
        assert_eq!(step(&k, 99, 1), None);
    }

    #[test]
    fn numbers_clamp_and_type() {
        let k = FxNodeKind::FreezeAt { alpha: 0.9 };
        let k = step(&k, 0, 1).unwrap();
        assert_eq!(k, FxNodeKind::FreezeAt { alpha: 1.0 });
        let k = step(&k, 0, 1).unwrap();
        assert_eq!(k, FxNodeKind::FreezeAt { alpha: 1.0 });
        assert_eq!(
            set_number(&k, 0, 0.25),
            Some(FxNodeKind::FreezeAt { alpha: 0.25 })
        );
        let r = FxNodeKind::Repeat { times: 2 };
        assert_eq!(
            set_number(&r, 0, 0.0),
            Some(FxNodeKind::Repeat { times: 1 })
        );
        assert_eq!(set_number(&r, 0, f64::NAN), None);
        // A choice field does not take a number.
        assert_eq!(set_number(&fade(), 0, 3.0), None);
    }

    #[test]
    fn effect_field_switches_constructor_and_keeps_timer() {
        let k = step(&fade(), 0, 1).unwrap();
        let FxNodeKind::Effect { effect } = &k else {
            panic!()
        };
        assert_eq!(effect.fn_name(), "paint_fg");
        assert_eq!(
            k.slot_value("timer"),
            Some(FxValue::Timer(FxTimer::new(500, FxInterp::QuadOut)))
        );
        // A full lap comes back to the same constructor and duration (the
        // colour and easing do not survive sleep and glitch, which have none).
        let mut k = fade();
        for _ in 0..12 {
            k = step(&k, 0, 1).unwrap();
        }
        let FxNodeKind::Effect { effect } = &k else {
            panic!()
        };
        assert_eq!(effect.fn_name(), "fade_to_fg");
        assert_eq!(effect.duration_ms(), 500);
    }

    #[test]
    fn filter_and_value_types_cycle_and_expose_sub_fields() {
        let k = FxNodeKind::Filter {
            filter: FxFilter::NonEmpty,
        };
        let k = step(&k, 0, 1).unwrap();
        assert_eq!(fields(&k).len(), 2, "fg_color has a colour field");
        let k = step(&k, 0, 2).unwrap();
        assert_eq!(
            fields(&k).iter().map(|f| f.label).collect::<Vec<_>>(),
            vec!["filter", "horiz", "vert"]
        );
        let v = FxNodeKind::Value {
            value: FxValue::Color(FxColor::Red),
        };
        let v = step(&v, 0, 1).unwrap();
        assert_eq!(
            fields(&v).iter().map(|f| f.label).collect::<Vec<_>>(),
            vec!["type", "ms", "interp"]
        );
    }

    #[test]
    fn every_field_of_every_preset_node_steps_to_a_valid_graph() {
        for p in presets::all() {
            for n in &p.graph.nodes {
                for i in 0..fields(&n.kind).len() {
                    for delta in [-1, 1] {
                        let k = step(&n.kind, i, delta).unwrap();
                        let mut g = p.graph.clone();
                        g.nodes.iter_mut().find(|m| m.id == n.id).unwrap().kind = k;
                        let dsl = g.to_dsl().unwrap();
                        let targets = crate::tui::fx_script::compile::FxTargets::uniform(
                            ratatui::layout::Rect::new(0, 0, 40, 10),
                            9,
                        );
                        crate::tui::fx_script::compile::compile(&dsl, &targets)
                            .unwrap_or_else(|e| panic!("{}: {e:?}\n{dsl}", p.name));
                    }
                }
            }
        }
    }
}
