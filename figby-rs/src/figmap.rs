use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::palette_import::Swatch;
use crate::tui::layers::{Layer, LayerGroup, LayerLink, LayerStack};
use crate::tui::lighting::{Light, LightKeyframe};
use crate::tui::timeline::TimelineFrame;

const FIGMAP_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FigmapKind {
    Image,
    Animation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FigmapTimeline {
    pub fps: u8,
    pub loop_enabled: bool,
    pub frames: Vec<TimelineFrame>,
    #[serde(default)]
    pub light_keyframes: Vec<LightKeyframe>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FigmapFile {
    pub version: u32,
    pub kind: FigmapKind,
    pub width: u32,
    pub height: u32,
    pub layers: Vec<Layer>,
    pub groups: Vec<LayerGroup>,
    pub links: Vec<LayerLink>,
    pub active_layer: usize,
    pub timeline: Option<FigmapTimeline>,
    #[serde(default)]
    pub palette: Vec<Swatch>,
    #[serde(default)]
    pub lights: Vec<Light>,
}

#[derive(Debug)]
pub enum FigmapError {
    Io(std::io::Error),
    Json(serde_json::Error),
    InvalidVersion(u32),
    InvalidDimensions { width: u32, height: u32 },
    LayerCountMismatch { expected: usize, actual: usize },
}

impl std::fmt::Display for FigmapError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FigmapError::Io(e) => write!(f, "I/O error: {e}"),
            FigmapError::Json(e) => write!(f, "JSON error: {e}"),
            FigmapError::InvalidVersion(v) => {
                write!(
                    f,
                    "Unsupported figmap version {v} (expected {FIGMAP_VERSION})"
                )
            }
            FigmapError::InvalidDimensions { width, height } => {
                write!(f, "Invalid dimensions: {width}x{height}")
            }
            FigmapError::LayerCountMismatch { expected, actual } => {
                write!(
                    f,
                    "Timeline frame layer count {actual} does not match document layers {expected}"
                )
            }
        }
    }
}

impl std::error::Error for FigmapError {}

impl From<std::io::Error> for FigmapError {
    fn from(e: std::io::Error) -> Self {
        FigmapError::Io(e)
    }
}

impl From<serde_json::Error> for FigmapError {
    fn from(e: serde_json::Error) -> Self {
        FigmapError::Json(e)
    }
}

/// Save a document state as a `.figmap` file.
pub fn save_figmap(
    layers: &LayerStack,
    timeline: Option<&FigmapTimeline>,
    lights: &[Light],
    palette: &[Swatch],
    path: &Path,
) -> Result<(), FigmapError> {
    let width = layers
        .layers
        .first()
        .map(|l| l.buffer.width() as u32)
        .unwrap_or(0);
    let height = layers
        .layers
        .first()
        .map(|l| l.buffer.height() as u32)
        .unwrap_or(0);

    let kind = if timeline.is_some() && !timeline.unwrap().frames.is_empty() {
        FigmapKind::Animation
    } else {
        FigmapKind::Image
    };

    let figmap = FigmapFile {
        version: FIGMAP_VERSION,
        kind,
        width,
        height,
        layers: layers.layers.clone(),
        groups: layers.groups.clone(),
        links: layers.links.clone(),
        active_layer: layers.active,
        timeline: timeline.cloned(),
        palette: palette.to_vec(),
        lights: lights.to_vec(),
    };

    let json = serde_json::to_string_pretty(&figmap)?;
    std::fs::write(path, json)?;
    Ok(())
}

/// Load a `.figmap` file into document state.
pub fn load_figmap(path: &Path) -> Result<FigmapFile, FigmapError> {
    let data = std::fs::read_to_string(path)?;
    let figmap: FigmapFile = serde_json::from_str(&data)?;

    if figmap.version != FIGMAP_VERSION {
        return Err(FigmapError::InvalidVersion(figmap.version));
    }
    if figmap.width == 0 || figmap.height == 0 {
        return Err(FigmapError::InvalidDimensions {
            width: figmap.width,
            height: figmap.height,
        });
    }

    // Validate timeline frame layer counts match document layer count
    let layer_count = figmap.layers.len();
    if let Some(ref timeline) = figmap.timeline {
        for frame in &timeline.frames {
            if frame.document_state.len() != layer_count {
                return Err(FigmapError::LayerCountMismatch {
                    expected: layer_count,
                    actual: frame.document_state.len(),
                });
            }
        }
    }

    Ok(figmap)
}

/// Convert a loaded `FigmapFile` into runtime `LayerStack` + optional timeline state.
pub fn into_runtime(
    figmap: FigmapFile,
) -> (LayerStack, Option<FigmapTimeline>, Vec<Light>, Vec<Swatch>) {
    let layer_count = figmap.layers.len();
    let layers = LayerStack {
        layers: figmap.layers,
        active: figmap.active_layer.min(layer_count.saturating_sub(1)),
        groups: figmap.groups,
        links: figmap.links,
    };

    (layers, figmap.timeline, figmap.lights, figmap.palette)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::canvas::{CanvasBuffer, CanvasCell};
    use crate::tui::layers::Layer;
    use ratatui::style::Color;

    fn make_test_layers() -> LayerStack {
        let mut layer = Layer::new(4, 2, "Test".to_string());
        layer.buffer.set(
            0,
            0,
            CanvasCell {
                ch: 'X',
                fg: Some(Color::Red),
                bg: None,
                height: None,
            },
        );
        LayerStack {
            layers: vec![layer],
            active: 0,
            groups: Vec::new(),
            links: Vec::new(),
        }
    }

    #[test]
    fn figmap_roundtrip_image() {
        let layers = make_test_layers();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.figmap");

        save_figmap(&layers, None, &[], &[], &path).unwrap();
        let loaded = load_figmap(&path).unwrap();

        assert_eq!(loaded.version, 1);
        assert_eq!(loaded.kind, FigmapKind::Image);
        assert_eq!(loaded.width, 4);
        assert_eq!(loaded.height, 2);
        assert_eq!(loaded.layers.len(), 1);
        assert_eq!(loaded.layers[0].name, "Test");
        assert!(loaded.timeline.is_none());
    }

    #[test]
    fn figmap_roundtrip_animation() {
        let layers = make_test_layers();
        let frame = TimelineFrame {
            thumbnail: vec![vec![' '; 4]; 2],
            has_keyframe: false,
            label: "Frame 1".to_string(),
            delay: 10,
            document_state: vec![CanvasBuffer::new(4, 2)],
            layer_keyframes: vec![None],
        };
        let timeline = FigmapTimeline {
            fps: 12,
            loop_enabled: true,
            frames: vec![frame],
            light_keyframes: Vec::new(),
        };

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test_anim.figmap");

        save_figmap(&layers, Some(&timeline), &[], &[], &path).unwrap();
        let loaded = load_figmap(&path).unwrap();

        assert_eq!(loaded.kind, FigmapKind::Animation);
        let t = loaded.timeline.unwrap();
        assert_eq!(t.fps, 12);
        assert!(t.loop_enabled);
        assert_eq!(t.frames.len(), 1);
        assert_eq!(t.frames[0].delay, 10);
    }

    #[test]
    fn figmap_rejects_bad_version() {
        let json = r#"{"version":99,"kind":"Image","width":1,"height":1,"layers":[],"groups":[],"links":[],"active_layer":0}"#;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bad.figmap");
        std::fs::write(&path, json).unwrap();
        let result = load_figmap(&path);
        assert!(matches!(result, Err(FigmapError::InvalidVersion(99))));
    }

    #[test]
    fn figmap_rejects_zero_dimensions() {
        let json = r#"{"version":1,"kind":"Image","width":0,"height":0,"layers":[],"groups":[],"links":[],"active_layer":0}"#;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("zero.figmap");
        std::fs::write(&path, json).unwrap();
        let result = load_figmap(&path);
        assert!(matches!(
            result,
            Err(FigmapError::InvalidDimensions {
                width: 0,
                height: 0
            })
        ));
    }
}
