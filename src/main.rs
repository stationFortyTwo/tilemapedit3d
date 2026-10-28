mod camera;
mod controls;
mod debug;
mod editor;
mod export;
mod grid_visual;
#[cfg(test)]
mod headless_smoke;
mod io;
mod runtime;
mod terrain;
mod texture;
mod types;
mod ui;

use crate::debug::asset::image_inspector::ImageInspectorPlugin;
use crate::texture::material;
use bevy::prelude::*;
use bevy_egui::EguiPlugin;
use camera::CameraPlugin;
use controls::ControlsPlugin;
use editor::EditorPlugin;
use runtime::RuntimePlugin;
use texture::TexturePlugin;
use ui::UiPlugin;

fn main() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins);
    add_editor(&mut app);
    app.run();
}

/// Everything the editor adds on top of Bevy's own plugins.
fn add_editor(app: &mut App) {
    app.add_plugins(EguiPlugin::default())
        .configure_sets(
            Update,
            terrain::TerrainMeshSet::Rebuild.before(terrain::TerrainMeshSet::Cleanup),
        )
        .add_plugins((
            TexturePlugin,
            CameraPlugin,
            ControlsPlugin,
            EditorPlugin,
            RuntimePlugin,
            UiPlugin,
            ImageInspectorPlugin,
        ))
        .add_systems(Startup, setup_light)
        .add_systems(Update, grid_visual::draw_grid);
    // .add_systems(Update, material::fix_roughness_images_on_load)
}

fn setup_light(mut commands: Commands) {
    commands.spawn((
        DirectionalLight {
            illuminance: 20_000.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -1.2, -0.8, 0.0)),
    ));
}
