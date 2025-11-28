# Tilemap Edit 3D

A Bevy-powered tilemap editor that uses `bevy_egui` for its in-app UI. The app runs entirely on the CPU/GPU of the user's machine—no external services are required.

## Getting started

1. Ensure you have the Rust toolchain installed (Rust 1.75+ recommended for current Bevy releases).
2. Run the editor with:
   ```bash
   cargo run
   ```
3. The app starts with default lighting and plugins defined in `src/main.rs` and renders immediately without extra setup.

## How the project is wired

- **Application bootstrap** — `src/main.rs` wires Bevy's default plugins with the UI, texture, camera, controls, editor, runtime, and debug inspector plugins, then adds a directional light and grid rendering each frame.
- **Camera controls** — `src/controls.rs` handles WASD panning and mouse-wheel zoom for the orthographic camera while respecting Egui focus.
- **Editing state & tools** — `src/editor.rs` defines `EditorState`, the current tool selection (paint vs. ramp rotation), map data, hover gizmos, and the per-frame systems that rebuild meshes when the map changes.
- **UI & file operations** — `src/ui.rs` builds the toolbar, texture palette, and file dialogs for save/load/export using `rfd::AsyncFileDialog` and Bevy's async task pool.
- **Runtime rendering** — `src/runtime.rs` creates the live terrain entity, regenerates the combined mesh from `EditorState`, writes splat maps for texture blending, and keeps materials hidden until all assets load.
- **Core data types** — `src/types.rs` models tiles, ramps, tile types, and map dimensions, including helpers for indexing and constants for tile sizing.

## Contributing

We welcome contributions! A concise workflow:

1. Fork or branch from `work` and create a feature branch for your change.
2. Make focused commits with clear messages.
3. Run `cargo fmt` and `cargo clippy -- -D warnings` to keep formatting and lints clean.
4. Use `cargo run` to manually test interactions (painting, ramp rotation, loading/saving) before opening a PR.
5. Open a pull request describing the change and any relevant repro steps.

## Planned work

Use the checklist placeholders below when recording upcoming tasks:

- [ ] (add item)
- [ ] (add item)

## Additional tips

- Textures are registered during startup in `src/texture/registry.rs`; new terrain materials should be added there so both the editor preview and runtime renderer can access them.
- Grid rendering runs each frame via `grid_visual::draw_grid` from `src/grid_visual.rs`, controlled by the `show_grid` flag in `EditorState`.
- The default map dimensions and tile defaults live in `TileMap::new` inside `src/types.rs`, which initializes a 64×64 grid with grass floor tiles.
