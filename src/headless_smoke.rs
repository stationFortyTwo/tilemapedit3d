//! Boots the whole editor with no window and checks that the terrain material's
//! shader compiles and renders. Needs a GPU, so it is `#[ignore]`d by default:
//! `cargo test -- --ignored`.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use bevy::asset::RenderAssetUsages;
use bevy::camera::{ImageRenderTarget, RenderTarget};
use bevy::prelude::*;
use bevy::render::render_resource::{
    CachedPipelineState, Extent3d, PipelineCache, PipelineDescriptor, TextureDimension,
    TextureFormat, TextureUsages,
};
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use bevy::render::{Render, RenderApp};
use bevy::shader::{Shader, ShaderCacheError};
use bevy::window::ExitCondition;

use crate::editor::EditorState;
use crate::runtime::RuntimeTerrainVisual;
use crate::types::{RampDirection, Tile, TileKind, TileType};

#[derive(Default)]
struct PipelineReport {
    terrain_ok: bool,
    errors: Vec<String>,
    seen: Vec<String>,
}

#[test]
#[ignore = "needs a GPU; run with `cargo test -- --ignored`"]
fn editor_renders_terrain_headless() {
    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: None,
                exit_condition: ExitCondition::DontExit,
                ..default()
            })
            .disable::<bevy::winit::WinitPlugin>()
            .disable::<bevy::render::pipelined_rendering::PipelinedRenderingPlugin>(),
    );
    crate::add_editor(&mut app);

    let terrain_shader: Handle<Shader> = app
        .world()
        .resource::<AssetServer>()
        .load("shaders/terrain_pbr_extension.wesl");
    let terrain_shader_id = terrain_shader.id();

    let report = Arc::new(Mutex::new(PipelineReport::default()));
    let render_report = report.clone();
    app.sub_app_mut(RenderApp)
        .add_systems(Render, move |cache: Res<PipelineCache>| {
            let mut report = render_report.lock().unwrap();
            for pipeline in cache.pipelines() {
                let (label, is_terrain) = match &pipeline.descriptor {
                    PipelineDescriptor::RenderPipelineDescriptor(d) => (
                        d.label.clone().unwrap_or_default().to_string(),
                        d.fragment
                            .as_ref()
                            .is_some_and(|f| f.shader.id() == terrain_shader_id),
                    ),
                    _ => continue,
                };
                if let PipelineDescriptor::RenderPipelineDescriptor(d) = &pipeline.descriptor {
                    let state = match &pipeline.state {
                        CachedPipelineState::Ok(_) => "ok".to_string(),
                        CachedPipelineState::Err(e) => format!("err {e}"),
                        CachedPipelineState::Queued => "queued".to_string(),
                        CachedPipelineState::Creating(_) => "creating".to_string(),
                    };
                    let line = format!(
                        "{label} frag={:?} {state}",
                        d.fragment.as_ref().map(|f| f.shader.id())
                    );
                    if !report.seen.contains(&line) {
                        report.seen.push(line);
                    }
                }
                match &pipeline.state {
                    CachedPipelineState::Ok(_) if is_terrain => report.terrain_ok = true,
                    // Not-yet-loaded shaders are retried by the cache; only
                    // real compile errors count.
                    CachedPipelineState::Err(
                        err @ (ShaderCacheError::ProcessShaderError(_)
                        | ShaderCacheError::CreateShaderModule(_)),
                    ) => {
                        let line = format!("{label}: {err}");
                        if !report.errors.contains(&line) {
                            report.errors.push(line);
                        }
                    }
                    _ => {}
                }
            }
        });

    app.finish();
    app.cleanup();

    // Startup spawns the editor camera aimed at the (absent) window; point it
    // at an offscreen image instead.
    app.update();

    // Paint a plateau with a ramp and a sunken patch in view, so cliff walls,
    // ramps and several splat layers are all drawn.
    {
        let mut state = app.world_mut().resource_mut::<EditorState>();
        let mut put = |x: u32, y: u32, kind, tile_type, elevation, ramp_direction| {
            state.map.set(
                x,
                y,
                Tile {
                    kind,
                    tile_type,
                    x,
                    y,
                    elevation,
                    ramp_direction,
                },
            );
        };
        for y in 0..4 {
            for x in 0..4 {
                put(x, y, TileKind::Floor, TileType::Rock, 2, None);
            }
        }
        put(
            4,
            1,
            TileKind::Ramp,
            TileType::Dirt,
            2,
            Some(RampDirection::East),
        );
        put(4, 2, TileKind::Ramp, TileType::Dirt, 2, None);
        for y in 5..8 {
            for x in 5..8 {
                put(x, y, TileKind::Floor, TileType::Sand, -1, None);
            }
        }
        state.map_dirty = true;
    }
    let target = {
        let mut images = app.world_mut().resource_mut::<Assets<Image>>();
        let mut image = Image::new_fill(
            Extent3d {
                width: 1280,
                height: 720,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            &[0; 4],
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::default(),
        );
        image.texture_descriptor.usage |= TextureUsages::RENDER_ATTACHMENT
            | TextureUsages::COPY_SRC
            | TextureUsages::TEXTURE_BINDING;
        images.add(image)
    };
    let camera = app
        .world_mut()
        .query_filtered::<Entity, With<Camera3d>>()
        .single(app.world())
        .expect("editor spawns one 3D camera");
    app.world_mut()
        .entity_mut(camera)
        .insert(RenderTarget::Image(ImageRenderTarget {
            handle: target.clone(),
            scale_factor: 1.0,
        }));

    let deadline = Instant::now() + Duration::from_secs(30);
    let mut terrain_visible = false;
    loop {
        app.update();
        assert!(
            app.should_exit().is_none(),
            "the app asked to exit (render error?)"
        );
        let world = app.world_mut();
        let entity = world.resource::<RuntimeTerrainVisual>().entity;
        terrain_visible |= world.get::<Visibility>(entity) == Some(&Visibility::Visible);

        let report = report.lock().unwrap();
        assert!(
            report.errors.is_empty(),
            "pipelines failed: {:#?}",
            report.errors
        );
        if terrain_visible && report.terrain_ok {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "timed out: terrain textures ready = {terrain_visible}, terrain pipeline compiled = {}; shader {terrain_shader_id:?}; seen {:#?}",
            report.terrain_ok,
            report.seen
        );
        drop(report);
        std::thread::sleep(Duration::from_millis(5));
    }

    // Keep a picture of the frame for a human to look at.
    let shot = std::env::temp_dir().join("tilemapedit3d-headless-smoke.png");
    let _ = std::fs::remove_file(&shot);
    for _ in 0..5 {
        app.update();
    }
    app.world_mut()
        .spawn(Screenshot::image(target))
        .observe(save_to_disk(shot.clone()));
    let deadline = Instant::now() + Duration::from_secs(30);
    while !shot.exists() {
        assert!(Instant::now() < deadline, "screenshot was never written");
        app.update();
        std::thread::sleep(Duration::from_millis(5));
    }
    println!("headless frame: {}", shot.display());
}
