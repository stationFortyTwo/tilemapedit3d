use bevy::pbr::MaterialPlugin;
use bevy::prelude::*;

pub mod material;
pub mod registry;

pub struct TexturePlugin;

impl Plugin for TexturePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<material::TerrainMaterial>::default())
            .init_resource::<registry::TerrainTextureRegistry>();
    }
}
