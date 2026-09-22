# Figby Feature Inventory

Comprehensive list of all features in the figby codebase. ~324 distinct features across 30+ modules.

## Summary

| Category | Count | Tested |
|----------|-------|--------|
| CLI flags/modes | 36 | 32 |
| Font parsing + generation | 29 | 29 |
| Rendering + smushing | 17 | 17 |
| Image processing | 20 | 20 |
| GIF import | 10 | 9 |
| Export formats | 9 | 9 |
| Template engine | 18 | 18 |
| Input pipeline | 5 | 5 |
| Control file parser | 11 | 11 |
| Configuration | 5 | 5 |
| Safety/sanitization | 5 | 5 |
| Palette import | 8 | 8 |
| TUI core + canvas | 7 | 1 |
| Drawing tools | 11 | 0 |
| Layers + blend modes | 10 | 0 |
| Panels/UI widgets | 12 | 1 |
| Theme system | 3 | 0 |
| Keybindings | 5 | 0 |
| Undo system | 5 | 5 |
| Particle system | 19 | 19 |
| Lighting engine | 13 | 13 |
| Animation timeline | 15 | 15 |
| Animation player | 13 | 13 |
| Visual effects | 2 | 0 |
| Dialogs | 5 | 1 |
| File operations | 3 | 0 |
| Export (TUI) | 2 | 0 |
| Web mode | 5 | 5 |
| **TOTAL** | **~324** | **~264 (81%)** |

---

## 1. CLI Modes and Flags

| Flag | Feature | Description |
|------|---------|-------------|
| (default) | FIGlet text rendering | Render text to ASCII art using FIGfont/TLF fonts |
| `-A` | Command-line input | Read input from arguments instead of stdin |
| `-D` / `-E` | Deutsch mode | Enable/disable character remapping for umlauts/eszett |
| `-X` / `-L` / `-R` | Writing direction | Font default / forced LTR / forced RTL |
| `-x` / `-l` / `-c` / `-r` | Justification | Font default / left / center / right |
| `-p` / `-n` | Paragraph mode | Newlines become spaces (wrap text) |
| `-s` / `-k` / `-S` / `-o` / `-W` | Smushing modes | Kerning / force smush / overlap / full-width |
| `-t` | Terminal width | Use terminal width for output |
| `-w` | Output width | Set output width in columns (default 80) |
| `-m` | Smush mode | Numeric smush mode bitmask |
| `-f` | Font name | Select font by name |
| `-d` | Font directory | Set font search directory |
| `-C` | Control file | Load FIGlet .flc control file |
| `-I` | Info codes | Print version/fontdir/font/width/formats |
| `-N` | Disable multibyte | Disable multi-byte input processing |
| `-F` | Removed flag | Prints error (matches C FIGlet 2.2.5) |
| `--to-file` | File output | Write output to file |
| `--play` | GIF playback | Play animated GIF fullscreen in terminal |
| `--play-width` | Playback scaling | Scale playback to N terminal columns |
| `--loop` | Loop playback | Repeat until keypress |
| `--tui` | TUI editor | Launch interactive full-screen editor |
| `--tui-render-mode` | Render mode | Fast (always redraw) or dirty (on change) |
| `--render-template` / `-T` | Template rendering | Render a .ftmp template file |
| `--create-font` | Font generation | Generate FIGfont from system font by name |
| `--create-font-path` | Font generation | Generate FIGfont from TTF/OTF file |
| `--font-size` | Font size | Point size for --create-font (default 12.0) |
| `--create-font-charset` | Charset | Character set for font generation |
| `-i` / `--image` | Image input | Image file path(s) or URL(s) to convert |
| `--map` | Character map | Custom luminance→character mapping |
| `-b` / `--braille` | Braille output | Use braille characters instead of ASCII |
| `--color` | Color output | 24-bit ANSI color codes |
| `--grayscale` | Grayscale | Convert to grayscale before output |
| `--negative` | Invert | Invert image colors |
| `--dither` | Dithering | Floyd-Steinberg dithering for braille |
| `--width` / `--height` / `--dimensions` | Output size | Dimensions for image mode |
| `--flipX` / `--flipY` | Flip | Flip image horizontally/vertically |

## 2. Core Library Modules

| Module | Feature | Description |
|--------|---------|-------------|
| `atomic_io` | Crash-safe writes | Atomic file replacement via temp + rename |
| `bounded_io` | Bounded reads | 32MB font cap, 8MB text cap |
| `config` | TOML config | $XDG_CONFIG_HOME/figby/config.toml |
| `control` | Control files | FIGlet .flc parser + ISO-2022 state machine |
| `font` | Font parser | FIGfont/TLF parser + ZIP font support |
| `font_gen` | Font generation | TTF/OTF → FIGfont conversion |
| `gif_import` | GIF import | Animated GIF with compositing + scaling |
| `image_input` | Image processing | Image → ASCII/braille/RGB matrix |
| `input` | Input pipeline | Deutsch, DBCS, HZ, UTF-8 encoding |
| `output` | Export | PNG/TXT/GIF/APNG/ANSI export |
| `palette_import` | Palette import | Paletty/ASE/WezTerm/Native palettes |
| `render` | Rendering | Kerning, smushing, justification |
| `sanitize` | Sanitization | Strip control chars, bidi marks |
| `smush` | Smushing engine | H1-H6 horizontal, V1-V5 vertical rules |
| `template` | Templates | .ftmp template engine with layers |
| `tui` | TUI | Full-screen editor (ratatui-based) |
| `web` | WASM | Browser-based demo (ratzilla) |

## 3. Rendering Engine

| Function | Description |
|----------|-------------|
| `lookup_char` | Character lookup with fallback |
| `calc_smush_amount` | Maximum overlap for LTR/RTL |
| `add_char` | Append character with kerning/smushing |
| `split_line` | Word-break line splitting |
| `render_line` | Hardblank replacement + justification |
| `render_string` | Full FIGlet pipeline for a string |

## 4. Font Parsing

| Feature | Description |
|---------|-------------|
| FIGcharacter | Character glyph with rows, width |
| FIGfont | Full font with hardblank, height, layout |
| FLF/TLF detection | Format auto-detection |
| Header parsing | All FIGfont header fields |
| Char data parsing | 95 ASCII + 7 Deutsch chars |
| Code-tagged chars | Hex/decimal codepoints, skip -1 |
| ZIP font support | Load from ZIP archives |
| Size limits | 10MB ZIP entry, 32MB file cap |
| Serde | Serialize/deserialize fonts |

## 5. Font Generation

| Feature | Description |
|---------|-------------|
| `system_font_to_figfont` | Generate from system font name |
| `font_file_to_figfont` | Generate from .ttf/.otf path |
| `list_system_fonts` | Enumerate system font families |
| `list_monospace_fonts` | Filter to monospace only |
| `resolve_charset` | 12 built-in charsets |
| Smooth charset | Antialiased edge characters |
| Braille charset | U+2800-U+28FF by dot count |
| Block elements | U+2580-U+259F by luminance |
| Box drawing | U+2500-U+257F + geometric |
| Dithered | ░▒▓ characters |
| Geometric | Squares, triangles, diamonds |
| Point size clamping | Min 4.0, max 200.0 |

## 6. Smushing Engine

| Rule | Description |
|------|-------------|
| H1 | Equal character smushing |
| H2 | Underscore + hierarchy |
| H3 | Bracket/brace/paren hierarchy |
| H4 | Bracket pair `[]{}()` → `\|` |
| H5 | BigX: `/+\` → `\|`, `\+/` → `Y`, `>+<` → `X` |
| H6 | Hardblank pair smushing |
| KERN | Kerning only (no smushing) |
| Universal overlap | Fallback when no rule matches |
| RTL support | Right-to-left smushing |
| V1-V5 | Vertical smushing rules |

## 7. Image Processing

| Feature | Description |
|---------|-------------|
| `load_luminance_matrix` | Image → grayscale |
| `load_rgb_matrix` | Image → RGB pixels |
| `bilinear_resize` | Bilinear resize |
| `luminance_to_ascii` | Luminance → ASCII chars |
| `luminance_to_braille` | 2×4 blocks → braille |
| `color_matrix_to_ascii` | RGB → colored ASCII |
| `floyd_steinberg_dither` | Floyd-Steinberg dithering |
| `apply_grayscale` | BT.709 grayscale |
| `apply_negative` | Color inversion |
| `apply_brightness` | Brightness adjustment |
| `apply_contrast` | Contrast adjustment |
| Image limits | 16K max dims, 256MB alloc |

## 8. GIF Import

| Feature | Description |
|---------|-------------|
| `import_gif` | Full compositing import |
| `import_gif_scaled` | Scaled import (FitWidth/FitBox/Exact) |
| Disposal methods | Background/Previous/Any |
| Per-frame palettes | Local palette support |
| Transparency | Transparent pixel handling |
| Loop count | Finite/infinite preservation |
| Frame delays | Per-frame timing preservation |
| Size limits | 1M cells/frame, 10K max frames |

## 9. Export Formats

| Format | Function | Description |
|--------|----------|-------------|
| PNG | `export_cells_to_png` | 8×16 bitmap font rasterization |
| PNG+alpha | `export_cells_to_png_with_alpha` | Transparent space cells |
| TXT | `export_cells_to_txt` | Plain text (strips colors) |
| ANSI | `export_cells_to_ansi` | 24-bit ANSI escape codes |
| ANSI script | `export_cells_to_ansi_multi` | Self-playing shell script |
| GIF | `export_cells_to_gif` | Animated GIF with variable delays |
| APNG | `export_cells_to_apng` | Animated PNG with loop count |

## 10. Template Engine

| Feature | Description |
|---------|-------------|
| `.ftmp` parsing | TOML frontmatter + body |
| Canvas settings | Width, height, margin, padding |
| Variable bindings | Text, font, position, z-order |
| `{{varname}}` | Variable substitution |
| `{{img:...}}` | Inline image rendering |
| `{{date:...}}` | Date formatting |
| Z-order rendering | Layers sorted by z-index |
| Overlap modes | "overwrite" and "flow" |
| Multi-font | Each layer can use different font |
| Border effects | Border width around content |
| Drop shadow | Shadow offset |
| Security | Command sub rejection, path containment |

## 11. Input Pipeline

| Feature | Description |
|---------|-------------|
| `deutsch_reroute` | ASCII→Deutsch mapping (7 chars) |
| `read_utf8_char` | Full UTF-8 (1-6 bytes, overlong rejection) |
| `read_dbcs_char` | Double-byte character set |
| `read_hz_char` | HZ (GB2312) encoding |
| `HZState` | HZ mode state tracking |

## 12. Control File Parser

| Feature | Description |
|---------|-------------|
| `read_control` | Parse .flc control files |
| `remap_char` | Character remapping |
| ISO-2022 | Full state machine (SO/SI/SS2/SS3/ESC) |
| Translate commands | `t` command remapping |
| Freeze commands | `f` command block separation |
| Multibyte modes | DBCS, UTF-8, HZ, JIS |
| Charset definitions | g0/g1/g2/g3 with 94/96 variants |
| Command count cap | MAX_CONTROL_COMMANDS = 4096 |

## 13. Safety Modules

| Module | Feature |
|--------|---------|
| `sanitize` | Strip control chars, C1 bytes, bidi marks, zero-width |
| `bounded_io` | Read with byte cap |
| `atomic_io` | Crash-safe file write + symlink protection |

## 14. Palette Import

| Format | Description |
|--------|-------------|
| Paletty JSON | `[{"hex":"#RRGGBB","name":"..."}]` |
| Adobe ASE | Binary RGB + Gray |
| WezTerm JSON | Terminal color scheme |
| Windows Terminal JSON | Terminal color scheme |
| Built-in palettes | Grayscale, Primary, Warm, Cool |

## 15. TUI Application

### Drawing Tools
Brush, Eraser, Fill, Line, Selection, Spray, Text, Eyedropper, Move, Rotate

### Layers
Visibility, lock, opacity, blend modes (Normal/Multiply/Overlay/Screen/Add/Subtract), groups, linking, masks, rename, reorder, merge down, shadow casting

### UI Widgets
Side panel, color palette, toolbox, font editor, image editor, status bar, menu bar, theme system, keybindings (60+ bindings across 9 scopes)

### Undo System
Configurable undo/redo, layer-aware, batch mode, configurable limit

## 16. Particle System

| Feature | Description |
|---------|-------------|
| Spawn rate | Particles-per-second with accumulator |
| Lifetime | Min/max range per particle |
| Velocity | Min/max per axis |
| Acceleration | Constant per axis |
| Spread angle | Directional randomization |
| Emission shapes | Point, CircleRadius, RectWH |
| Edge modes | Bounce, Wrap, Despawn |
| Layer collision | Bounce off occupied cells |
| Color/opacity | Per-particle RGB + alpha |
| Blend mode | Per-particle blend mode |
| Keyframe tracks | Color/size/character/opacity over lifetime |
| On-death bursts | Spawn secondary particles (non-recursive) |
| Bake frames | Generate animation frames |
| Config panel | 19-field interactive editor |

## 17. Lighting Engine

| Feature | Description |
|---------|-------------|
| Normal3 | Packed 3D normal (i8 ×3) |
| NormalMap | Per-cell normal grid |
| Scene | Light collection |
| Ambient light | Flat intensity, no direction |
| Directional light | Lambertian + shadow |
| Point light | Attenuation (constant/linear/quadratic) |
| `shade_canvas` | Per-cell luminance computation |
| `cast_shadow` | DDA ray marching |
| Normal map generation | Sobel kernel from heightfield |
| LightingLut | 256-entry shadow→lit ramp per swatch |
| Multi-swatch LUT | Multiple color ramps |
| Specular | Blinn-Phong per-swatch |

## 18. Animation Timeline

| Feature | Description |
|---------|-------------|
| Frame storage | Thumbnails + pixel buffers |
| Frame operations | Add, insert, remove, duplicate, reorder |
| Per-layer keyframes | Position, opacity, blend mode |
| Keyframe interpolation | Linear lerp between keyframes |
| Tween generation | EaseIn/EaseOut/Bounce easing |
| Transport bar | Play/Pause/Stop/Loop |
| Per-frame delays | Variable timing per frame |

## 19. Animation Player

| Feature | Description |
|---------|-------------|
| Playback | Play/pause/seek/speed/loop |
| Variable delays | Per-frame hold times |
| Speed control | 0.25x to 4.0x |
| Keyboard controls | Space, arrows, L, Esc/q |
| Progress bar | Visual progress + counter |
| Fullscreen | Terminal-session capture |
| Raw ANSI | Bypass ratatui diffing |

## 20. Web Mode (WASM)

| Feature | Description |
|---------|-------------|
| Browser demo | ratzilla-based |
| Embedded fonts | standard, banner, big |
| Real-time rendering | Type → see output live |
| Font list | Scrollable selector |

---

## ASCII Cinema Recordings (Planned)

Candidates for documentation demos:

| Demo | What it shows | Command |
|------|---------------|---------|
| Basic rendering | Core FIGlet output | `figby -f standard "Hello"` |
| Font comparison | Different fonts side by side | Multiple `-f` flags |
| Kerning vs smushing | `-k` vs `-s` vs `-W` | Compare outputs |
| Image to ASCII | Photo conversion | `figby -i photo.jpg` |
| Braille art | High-res braille output | `figby -b -i photo.jpg` |
| Color ASCII | ANSI color output | `figby --color -i photo.jpg` |
| Lighting | Animated light sweep | GIF export (built) |
| Particles | Fire/rain/snow | GIF export (built) |
| Templates | Multi-layer compositions | `.ftmp` rendering |
| Font generation | TTF → FIGlet | `--create-font-path` |
| Control files | Character remapping | `-C uskata.flc` |
| Deutsch mode | Umlaut rendering | `-D` flag |

## Light Keyframing System (Planned)

### Problem

The lighting state is a single global (`LightingState` on `TuiApp`). Timeline frames only save pixel buffers + layer transforms. No lighting data is captured or restored per frame. This means animated lighting (moving lights, flickering, color shifts) is impossible.

### Design

Add per-frame light keyframes to the timeline, following the existing `LayerKeyframe` pattern.

#### New Types

```rust
// Per-light keyframe at a specific timeline frame
#[derive(Debug, Clone, Copy)]
pub struct LightKeyframe {
    pub position: (f32, f32, f32),  // Point position or Directional direction
    pub intensity: f32,
    pub color: Rgb,
}
```

#### TimelineFrame Addition

```rust
// Added to TimelineFrame
pub light_keyframes: Vec<Vec<Option<LightKeyframe>>>,
// Structure: light_keyframes[frame_idx][light_idx] = Option<LightKeyframe>
```

#### Commit/Restore Flow

1. `commit_current_timeline_frame` snapshots `lighting.scene` into the frame
2. `load_current_timeline_frame` restores the scene from the frame
3. On frame navigation, scene is captured before switch, restored after

#### Interpolation

When playing back between two keyframes:
- Find nearest previous/next keyframe with lighting data
- Lerp `position`, `intensity`, `color` by interpolation factor `t`
- Use interpolated scene for `shade_composited` call

```rust
fn interpolate_light(a: &Light, b: &Light, t: f32) -> Light {
    // Lerp position (Point/Directional)
    // Lerp intensity
    // Lerp color channels
    // Keep attenuation from nearest keyframe
}
```

#### Render Pipeline Change

In `mod.rs`, replace:
```rust
if let Some(ref scene) = self.lighting.scene {
```
With:
```rust
let effective_scene = interpolate_scene(
    &self.lighting.scene,
    &self.animation.timeline_state,
    self.animation.timeline_state.current_frame,
);
if let Some(ref scene) = effective_scene {
```

#### Keybindings

| Key | Action |
|-----|--------|
| `K` | Set light keyframe at current frame |
| `Shift+K` | Clear light keyframe at current frame |
| `L` | Toggle light keyframe visibility on timeline |

#### Particle + Light Coupling

After basic keyframing works:
- Attach a light to the particle emitter position
- Each frame: `light.position = (emitter_x, emitter_y, z_offset)`
- Enables "fire with glow", "sparkler trail with light" effects
- Store offset in `ParticleSystem` config

### Files to Modify

| File | Change |
|------|--------|
| `timeline.rs` | Add `LightKeyframe`, add field to `TimelineFrame`, interpolation |
| `app_state.rs` | Snapshot/restore scene in commit/load |
| `mod.rs` | Render with interpolated scene |
| `lighting.rs` | `interpolate_light()` helper |
| `dispatch.rs` | Keybinding for set/clear keyframe |
| `particles.rs` | Optional: emitter-attached light |
