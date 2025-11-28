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

- [ ] Allow adding new tiles (not just editing existing ones) so the overall map can grow or shrink.
- [ ] Enable importing custom textures without modifying source code.
- [ ] Support user/org-specific utilities (e.g., scenario triggers, spawn points, neutrals, or other hooks).

## Current caveats

- The splatmap is limited to four channels (`Rgba8Unorm`), so painting with more than four textures can produce unexpected blends. Extending this to more layers is a stretch goal that will require reworking texture allocation across the map. The current splatmap generation lives in `src/terrain.rs` under `splatmap` functions and is bound in `src/runtime.rs` when the terrain mesh is refreshed.
- Roughness maps are loaded and sampled in the terrain shader, but their results have not been fully verified yet. See the roughness accumulation paths in `assets/shaders/terrain_pbr_extension.wgsl` for the current implementation.
- Only a single cliff texture (wall layer) is supported right now. Extending wall variety is planned once a preferred approach is chosen.

## How the splatmap works

1. **Generation** — The CPU builds the `Rgba8Unorm` splatmap from the map grid in `src/terrain.rs` (`splatmap::create` and `splatmap::write`), assigning one channel per `TileType` index. The runtime registers the resulting texture handle in `src/runtime.rs` so the renderer can sample it when rebuilding terrain meshes.
2. **Sampling in the shader** — The fragment shader converts world space to splat UVs in `world_to_splat_uv`, samples the splat texture (`textureSampleLevel`) around each tile to derive normalized weights, and falls back to vertex UVs if no weights are present. This logic lives in `assets/shaders/terrain_pbr_extension.wgsl` near the weight normalization loop (see the section where `weights` is divided by `weight_total`).
3. **Applying layers and cliffs** — The same shader triplanar-samples base color, normals, and roughness for each weighted layer. Cliff handling happens later in the file around the computation of `cliff_weight`: when cliffs are enabled it uses `wall_layer_index` for the cliff sample; otherwise it reuses the top layer. Blending between cliff, top, and optional bottom layers is done in that block before the final PBR lighting call.

## Additional tips

- Textures are registered during startup in `src/texture/registry.rs`; new terrain materials should be added there so both the editor preview and runtime renderer can access them.
- Grid rendering runs each frame via `grid_visual::draw_grid` from `src/grid_visual.rs`, controlled by the `show_grid` flag in `EditorState`.
- The default map dimensions and tile defaults live in `TileMap::new` inside `src/types.rs`, which initializes a 64×64 grid with grass floor tiles.
