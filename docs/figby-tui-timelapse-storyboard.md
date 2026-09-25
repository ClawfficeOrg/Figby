# Figby TUI Timelapse Storyboard

## Goal

Produce a chaptered, start-to-finish timelapse showing real work inside Figby's full TUI. This is a build-along, not a finished-art slideshow, synthetic terminal animation, or CLI-only playback demo. Viewers should see the UI actions that create each document and see every save/reopen/playback succeed.

Core constraint: recording begins before creating the first document. Each chapter ends with a saved artifact that becomes the next chapter's starting point.

## Deliverables

Store all files under `assets/e2e-art/timelapse/`:

- `01-space-scene.cast` and `.gif`
- `02-ship-animation.cast` and `.gif`
- `03-effects-finale.cast` and `.gif`
- Saved `.figmap` checkpoints for each chapter, with distinct names.
- Driver/config files used to reproduce the MCP-controlled sessions.

Never overwrite existing files unless user explicitly approves. Do not write to `assets/e2e-art/recordings/e2e-art.cast` or `.gif`; those are a separate, previously approved recording.

## Chapter 1: Space Scene From Scratch

Start from Figby welcome screen and record the entire app workflow.

1. Create a new image document from the File menu; choose deliberate dimensions large enough for composition, e.g. 80x30 or 96x32.
2. Add and name a background layer. Select Fill, choose near-black/navy background color, fill canvas.
3. Select Brush. Change brush character to `*`; demonstrate character picker, foreground color selection, and brush size.
4. Build star field in visible passes: sparse bright stars, dim distant stars, colored clusters. Show tool/color changes in palette and status.
5. Demonstrate Braille tool for a nebula or dense star texture. Use a distinct color, then return to normal brush.
6. Add separate planet layer(s). Use brush/fill and geometric tools for planet disks, rings, moon/crater highlights. Keep shapes visibly authored through UI actions, not imported from a finished map.
7. Add a separate title/text layer. Use Figlet text tool with an actual FIGfont to place `FIGBY`; style it with a contrasting color and glow/highlight marks.
8. Show Layers drawer with meaningful layer names/order. Save first checkpoint, e.g. `space-scene-01.figmap`, via Figby Save as Figmap. Capture save completion and reopen confirmation if feasible.

End state: coherent space scene with a legible Figlet `FIGBY`, distinct layers, and clear evidence of brush, fill, palette, Braille, geometric, and text tools.

## Chapter 2: Ship And Keyframe Animation

Start by opening Chapter 1 checkpoint in the TUI. Save As a new version (do not replace chapter 1), e.g. `space-scene-02-animation.figmap`.

1. Add/name ship layer; construct first ship from visible brush/line/polygon/fill operations. Add windows/engine detail on separate layer if practical.
2. Add a point light and use Lighting UI to show its position/intensity/color. Keep ambient + point lights; save a light setup that makes ship readable.
3. Show timeline. Capture initial frame/keyframe with ship at left edge; create later frame/keyframe with ship crossing toward right edge using layer move/selection tools. Make movement evident at both endpoints.
4. Set easing/tween options intentionally; use tween to create intermediate frames. Make keyframe markers and frame count visible in timeline.
5. Run in-place playback with full Figby chrome visible. Confirm progress advances through multiple frames and ship actually moves across screen. Stop playback and inspect endpoint.
6. Save as another checkpoint, e.g. `space-scene-02-animation.figmap`.

Important: test current Figby playback/keyframe semantics first. If timeline UI or keyframes fail, stop and fix/document the app issue before recording. Never silently replace intended movement with pre-rendered duplicate frames.

## Chapter 3: Effects Finale

Open Chapter 2 checkpoint and Save As `space-scene-03-effects.figmap`.

1. Add emitter at ship engine and configure particle stream (character, color, spawn rate, lifetime, velocity/spread). Show particles emitting from ship during playback.
2. Add a second layer/object for another ship or moving satellite; demonstrate rotate and selection/move tools.
3. Add further keyframes for object rotation/position and light movement/intensity. Tween the new motion and test playback.
4. Add a Figlet text reveal/entrance or title movement as another layer/keyframe track. Keep title readable throughout.
5. Final playback should show moving ship, exhaust particles, light sweep, and animated Figlet title with timeline visible where layout permits.
6. Save final checkpoint; show saved filename and a composed final hold.

## Capture / MCP Workflow

Use configured local Figby MCP server. Start with `launch` at fixed recording dimensions, normally 140x50, and verify snapshot reports same dimensions. Launch from welcome; do not open a figmap as a substitute for Chapter 1 creation.

At each meaningful action:

- Use `snapshot` (text + PNG) to inspect actual app before/after.
- Use `wait_for_text` for modal completion, save status, timeline/playback markers.
- Verify active mode before using mode-dependent shortcuts.
- Prefer literal printable key path for letters; key tool maps `T` literally. Use named `Space`, `Enter`, `Escape`, arrows, and documented control names.
- Mouse action coordinates are zero-based and must be calculated from latest snapshot at same PTY dimensions.
- Record live PTY output, not pane snapshots stitched with duplicate frames or synthetic terminal art.
- Use unique recording names and record at PTY size matching cast metadata. Capture UI state often enough to show timelapse progress, but no more than ~1 second between visible changes.

Recommended recording cadence: human-readable 10–25 second chapters after tasteful acceleration, 140x50 or larger. Preserve normal pauses for menus, color changes, saves, and final playback. Do not use 50x–100x that makes tool use impossible to follow.

## Verification Gate Before Sharing

For every chapter, inspect actual decoded frames (contact sheet or image viewer) and confirm:

- Correct full interface, no split/overlaid pane, clipping, or dimension mismatch.
- Cursor/tool changes and artwork progress are visible; no blank canvas during claimed drawing.
- Figlet title is real Figby text output and legible.
- Distinct layers and timeline/keyframe state match chapter intent.
- Playback visibly advances and intended objects move; effects appear in correct location.
- Save dialog closes successfully and file exists/loads.
- GIF dimensions, frame count, and duration are reasonable; playback speed remains watchable.
- Earlier checkpoint files and approved `assets/e2e-art/recordings/e2e-art.cast` / `.gif` remain unchanged.

If a tool path fails, pause recording, inspect snapshot, fix driver/app workflow, and rerun in a fresh filename. Never claim visual review based solely on metadata or text markers.

## Known Risks / First Checks

- tmux/crossterm can flatten control keys and mouse protocols; MCP layer should be exercised independently for each required key/button before full recording.
- Figby keyboard brush paint was previously observed to move cursor/register undo without visible canvas writes in some states. Use actual snapshot verification after first stroke; investigate app issue if reproduced.
- Mode controls are context-sensitive: Image Editor `Ctrl+O` opens image formats; Font Editor Overview `Ctrl+O` is figmap-capable. Menus are safer if shortcut path is ambiguous.
- `T` can mean timeline or Tween depending modifier/menu context. Verify overlay text; use Animation menu Play/Pause or direct timeline controls when needed.
- Keyframe scene schema must use supported easing variants only: `Linear`, `EaseIn`, `EaseOut`, `Bounce`.
- The old demo generator can create fixtures, but fixtures are checkpoints/supporting inputs only; never use them to fake Chapter 1's live drawing process.
