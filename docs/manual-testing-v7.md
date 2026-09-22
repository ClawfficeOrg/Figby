# Latest Manual Tests

## UI

[x] where did brush options go to? Only see layers in sidebar, that should also have another panel for tool options, like brush size, shape, etc, lighting options, text options, etc. — root cause was two bugs: the tab bar's width math always rendered blank labels, and the Layers panel's Left/Right handling always swallowed the tab-cycle keys. Also added a real Lighting props entry (previously fell through to a generic keybind list).
[x] there are no keyboard shortcuts shown in the menus — every menu item now shows its shortcut, right-aligned.
[x] there's no "New" option when in image editor — File > New Image (Ctrl+N), reuses the existing New Image dialog.
[x] there's no "new font from system" or "New font from file" option in the menus — both added to the File menu; "from system" required a new picker dialog since it was previously only a CLI flag.
[x] timeline doesn't seem to be clickabe except right after importing a gif — click-to-seek implemented; it was never wired to mouse input at all, import timing was a red herring.
[x] timeline doesn't seem to be scrollable? — mouse-wheel scroll implemented (Shift+scroll for the layer rows).
[x] Layers in sidepanel has light blue background and white text, hard to read — wired up the theme's already-defined (but unused) dark-navy LayerTheme instead of reusing the menu's bright-highlight colors.
[x] Layers sidebar doesn't have any tool buttons like add layer, delete layer — added a New/Duplicate/Delete/Group/Link button row.
[x] Layers don't have layer groups — groups were already fully implemented (Ctrl+G, collapse/expand); this was a discoverability gap, now fixed by the above.
[x] Layers can't be linked — new feature: linked layers sync visibility + lock state, mirrors the existing group data model.
[x] Can't find anywhere to toggle loop animation — now a persistent 🔁 button in the transport bar; also now defaults from the imported GIF's own loop setting instead of always starting off.
[x] should have a transport controls section for animations with icons for play pause, etc — persistent, mouse-clickable ▶/⏸/⏹/🔁 bar in the timeline toolbar row (previously static text); also fixed a latent bug where clicks during playback leaked through to canvas drawing.

## pt deux

[ ] So when i import an animated gif there's an error and the dialog box doesn't close (Error: Cannot read file: stream did not contain valid UTF-8) but it loads fine i just have to close the dialog with escape
[ ] quitting dialog asking to save isn't sized correctly, it cuts off, and it doesn't accept mouse input
[ ] the playback of an animation is still not correct behavior. Right now there's some weird separation of playing and the timeline - they should be intrinsically connected. When I pause playback, we're still at frame 0 even if i pause in the middle of playback. play should be advancing the frame counter and displaying the frame it's on. pause stops the playback and stops at the right frame, like every other app in history.
[ ] there needs to be an overhaul of keyboard shortcuts. there are too many overlaps. frame advance arrows don't work because that's also the command to change sidebar focus. make some of these, like change sidebar panel to alt arrows
[ ] brush props don't seem to be changeable outside of \
[ ] there's a huge problem editing a frame, changing frames, change back - all edits are gone essentially the entire animation editor is useless
[ ] files are HUGE. that mod.rs file is like 6k lines wtf can we plan splitting that up into some smaller files? state management on the whole looks so bolted on in different places, like amateur hour.
[ ] half the props panels for tools are just the old toolbox? i don't even know what the point is here.
[ ] particle effects... where's the inertia? vector of travel? need collision layers, particles should have their own timelines at some point, a big change i'm sure.
[ ] lighting makes no sense


## Part Twah

[x] create font should have a dialog box with options, same as cli, font size, character set, etc. — both "New Font from System" and "New Font from File" now have a Charset field (12 presets, matching CLI's --create-font-charset); file import also gained a size field (previously hardcoded).
[x] return shouldn't be "play" it should be spacebar — playback now starts on Space; Enter remains the keyboard-paint key for Brush/Eraser/Line/Fill/Spray.
[x] file dialogs SUCK: mouse doesn't work, you have to unintuitively hit tab to open a folder, delete stuff to go up a folder? there's no .. to go up? or right arrow should open as well, left should go up a directory, return should only be available if a file is openable/selectable. also the font import doesn't show zip files i thought we added zip support. — mouse click/scroll support added, .. now listed in every directory, Left/Right navigate, Enter only finalizes on selectable entries, and font import now shows/browses .zip bundles the same way Open does.
[x] default brush size should really be 1
[x] save dialog also sucks bad for a lot of the same reasons open file dialog sucks, needs an edit field for input of the file name, and the file picker just chooses destination folder, default for a font creation is the original font name + .flf — filename is now a separate field from the directory browser (Tab switches focus), defaulting to the font's current name + .flf.
[x] other file formats should probably be figmap for a graphic/animation. — `.figmap` shipped (todo-v8 8.6, all checked): JSON project format with layers/timeline/lights/palette, round-trip tested, Open/Save dialogs + File > Save as Figmap + CLI `--play` support.
[x] there probably should a unique tool for a brush that uses the braille dots for subcell accuracy? — new standalone Braille tool ('k'), keyboard-driven sub-cell dot cursor (mouse can only address whole cells).
[ ] instead of the little strip for "image editor"" and "font editor" under the menu bar that should be a document tab strip. you can have multiple files open, both files and images, so you can edit a font in one tab and be creating a banner in another. i'm sure this isn't a small change. — scoped as its own architectural milestone (docs/todo-v8.md 8.5), not yet implemented.

