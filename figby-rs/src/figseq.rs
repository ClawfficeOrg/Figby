use std::path::Path;

pub use ::figseq::*;

// ── bake: figmap → figseq ──────────────────────────────────────────────────

#[derive(Debug)]
pub enum BakeError {
    Figmap(crate::figmap::FigmapError),
    FigSeq(FigSeqError),
    /// The source document had no layers to flatten.
    NoLayers,
}

impl std::fmt::Display for BakeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BakeError::Figmap(e) => write!(f, "source figmap: {e}"),
            BakeError::FigSeq(e) => write!(f, "{e}"),
            BakeError::NoLayers => write!(f, "source figmap has no layers to bake"),
        }
    }
}

impl std::error::Error for BakeError {}

impl From<crate::figmap::FigmapError> for BakeError {
    fn from(e: crate::figmap::FigmapError) -> Self {
        BakeError::Figmap(e)
    }
}

impl From<FigSeqError> for BakeError {
    fn from(e: FigSeqError) -> Self {
        BakeError::FigSeq(e)
    }
}

/// Lighting parameters used when a figmap carries lights. These match
/// `LightingState::default()`; a figmap does not store them, so a bake uses the
/// defaults rather than inventing per-document values.
const BAKE_MAX_SHADOW_DISTANCE: u16 = 50;
const BAKE_HEIGHT_SCALE: f32 = 0.5;

/// Flatten a `.figmap` into a frame sequence.
///
/// This is the write half of the format and the reason it exists: a figmap
/// stores a *project*, and several things worth playing back have no editable
/// representation in one — most notably a lighting scene with no keyframes,
/// which `--play` on a figmap currently ignores. Baking resolves every frame to
/// the pixels the editor's exporter would produce (layers composited, Text-tool
/// blocks rasterized, lighting applied with the document's palette), so what
/// plays back is what you saw.
///
/// A figmap with no timeline frames bakes to a one-frame sequence.
///
/// `playback` carries the two end-of-playback behaviours a sequence can ask for.
/// Neither is inferable from a figmap, so the caller chooses them (`--loop` and
/// `--hold-last` on the CLI).
#[derive(Debug, Clone, Copy, Default)]
pub struct PlaybackIntent {
    pub loop_enabled: bool,
    pub hold_last_frame_cs: u16,
}

pub fn bake_figmap(
    figmap_path: &Path,
    out_path: Option<&Path>,
    playback: PlaybackIntent,
) -> Result<FigSeqFile, BakeError> {
    use crate::tui::canvas::CanvasBuffer;
    use crate::tui::export::{self, LightingContext};
    use crate::tui::lighting::{self, LightingLut, Scene};
    use crate::tui::palette::build_rgb_to_swatch;
    use crate::tui::palette_editor;
    use crate::tui::timeline::TimelineState;

    let figmap = crate::figmap::load_figmap(figmap_path)?;
    let (mut layers, timeline, lights, mut palette, text_blocks) =
        crate::figmap::into_runtime(figmap);
    if layers.layers.is_empty() {
        return Err(BakeError::NoLayers);
    }

    let w = layers.layers[0].buffer.width();
    let h = layers.layers[0].buffer.height();

    // Bake persisted Text-tool blocks into the active layer, non-destructively
    // (the figmap on disk keeps them editable). Same rule as the exporter.
    if !text_blocks.is_empty() {
        let active = layers.active;
        for block in &text_blocks {
            block.bake_into(&mut layers.layers[active].buffer);
        }
    }

    // Rebuild the lighting LUT from the document's palette, with the artwork's
    // own colours added, so lit frames keep their hues exactly as the editor
    // shows them rather than snapping to the nearest swatch.
    palette_editor::sync_art_swatches(&mut palette, &layers);
    let swatch_data = palette_editor::lighting_swatch_data(&palette);
    let char_ramp = lighting::pick_char_ramp(layers.layers.iter().flat_map(|l| {
        let b = l.buffer();
        (0..b.height())
            .flat_map(move |y| (0..b.width()).filter_map(move |x| b.get(x, y).map(|c| c.ch)))
    }));
    let lut = LightingLut::from_swatches(&swatch_data, char_ramp);
    let rgb_to_swatch = build_rgb_to_swatch(
        &palette
            .iter()
            .map(|s| (s.name.clone(), s.hex.clone()))
            .collect::<Vec<_>>(),
    );

    let has_timeline = timeline.as_ref().is_some_and(|tl| !tl.frames.is_empty());

    if has_timeline {
        let tl = timeline.expect("checked above");
        let mut scene = Scene {
            lights: lights.clone(),
        };
        scene.ensure_ambient();

        let ts = TimelineState {
            fps: tl.fps,
            frames: tl.frames,
            layer_names: layers.layers.iter().map(|l| l.name.clone()).collect(),
            ..TimelineState::default()
        };

        let lighting_ctx = LightingContext {
            base_scene: &scene,
            light_keyframes: &tl.light_keyframes,
            lut: &lut,
            layer_stack: &layers,
            max_shadow_distance: BAKE_MAX_SHADOW_DISTANCE,
            height_scale: BAKE_HEIGHT_SCALE,
            palette_rgb_to_swatch: &rgb_to_swatch,
            swatch_data: &swatch_data,
        };

        let frames = export::capture_timeline_frames(&ts, &layers, w, h, Some(&lighting_ctx));
        let delays: Vec<u16> = ts.frames.iter().map(|f| f.delay).collect();
        let seq = FigSeqFile::new(
            w,
            h,
            frames,
            Some(&delays),
            playback.loop_enabled,
            playback.hold_last_frame_cs,
        );
        if let Some(p) = out_path {
            save_figseq(&seq, p)?;
        }
        Ok(seq)
    } else {
        // Static document: one frame, lighting applied if lights were saved.
        let mut buf: CanvasBuffer = layers.composite();
        if !lights.is_empty() {
            let mut scene = Scene {
                lights: lights.clone(),
            };
            scene.ensure_ambient();
            buf = crate::tui::components::canvas::shade_composited(
                &buf,
                &layers,
                &scene,
                &lut,
                BAKE_MAX_SHADOW_DISTANCE,
                BAKE_HEIGHT_SCALE,
                &rgb_to_swatch,
                &swatch_data,
            );
        }
        let cells: Vec<Vec<CanvasCell>> = (0..h)
            .map(|y| {
                (0..w)
                    .map(|x| buf.get(x, y).copied().unwrap_or_default())
                    .collect()
            })
            .collect();
        let seq = FigSeqFile::new(
            w,
            h,
            vec![cells],
            None,
            playback.loop_enabled,
            playback.hold_last_frame_cs,
        );
        if let Some(p) = out_path {
            save_figseq(&seq, p)?;
        }
        Ok(seq)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bake_flattens_a_static_figmap_to_one_frame() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src.figmap");
        let out = dir.path().join("out.figseq");
        crate::figmap::tests_support::write_static_figmap(&src, 6, 3);
        let seq = bake_figmap(&src, Some(&out), PlaybackIntent::default()).unwrap();
        assert_eq!(seq.frames.len(), 1);
        assert_eq!((seq.width, seq.height), (6, 3));
        let cells = seq.cells_for_playback();
        assert_eq!(cells[0].len(), 3);
        assert_eq!(cells[0][0].len(), 6);
        // Re-reading the written file must give back the same sequence.
        assert_eq!(load_figseq(&out).unwrap(), seq);
    }

    #[test]
    fn bake_preserves_the_artwork() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src.figmap");
        crate::figmap::tests_support::write_static_figmap(&src, 6, 3);
        let expect_cell = crate::figmap::load_figmap(&src).unwrap().layers[0]
            .buffer
            .get(1, 1)
            .copied()
            .unwrap();

        let seq = bake_figmap(&src, None, PlaybackIntent::default()).unwrap();
        let got = seq.cells_for_playback()[0][1][1];
        assert_eq!(got.ch, expect_cell.ch);
        assert_eq!(got.fg, expect_cell.fg);
    }

    #[test]
    fn bake_applies_a_static_light_scene() {
        // The reason --bake exists: `--play` on a figmap only shades when the
        // document has light *keyframes*, so a saved scene with none still plays
        // back unlit. Baked pixels must be lit.
        let dir = tempfile::tempdir().unwrap();
        let unlit = dir.path().join("unlit.figmap");
        let lit = dir.path().join("lit.figmap");
        crate::figmap::tests_support::write_static_figmap(&unlit, 6, 3);
        crate::figmap::tests_support::write_static_figmap_with_light(&lit, 6, 3);

        let plain = bake_figmap(&unlit, None, PlaybackIntent::default()).unwrap();
        let shaded = bake_figmap(&lit, None, PlaybackIntent::default()).unwrap();
        assert_eq!(
            plain.frames[0].chars, shaded.frames[0].chars,
            "lighting must not repaint the glyphs"
        );
        assert_ne!(
            plain.frames[0].attrs, shaded.frames[0].attrs,
            "a static light scene did not alter any cell colour"
        );
    }

    #[test]
    fn bake_reports_a_source_without_layers() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("empty.figmap");
        let layers = crate::tui::layers::LayerStack {
            layers: Vec::new(),
            active: 0,
            groups: Vec::new(),
            links: Vec::new(),
        };
        crate::figmap::save_figmap(&layers, None, &[], &[], &[], &src).unwrap();
        // save_figmap writes width/height 0 for an empty stack, which
        // load_figmap rejects before we ever get to the layer check.
        assert!(matches!(
            bake_figmap(&src, None, PlaybackIntent::default()),
            Err(BakeError::Figmap(_))
        ));
    }

    #[test]
    fn bake_records_the_requested_playback_intent() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src.figmap");
        crate::figmap::tests_support::write_static_figmap(&src, 4, 2);
        let seq = bake_figmap(
            &src,
            None,
            PlaybackIntent {
                loop_enabled: true,
                hold_last_frame_cs: 250,
            },
        )
        .unwrap();
        assert!(seq.loop_enabled);
        assert_eq!(seq.hold_last_frame_cs, 250);
        // The hold is not a frame delay: 250cs extra on top of the frames.
        assert_eq!(seq.total_duration_cs(), 250 + seq.frames[0].delay as u32);
    }
}
