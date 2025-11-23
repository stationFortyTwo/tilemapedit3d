use bevy::asset::Asset;
use bevy::math::Vec2;
use bevy::pbr::{ExtendedMaterial, MaterialExtension, MaterialPipelineKey, StandardMaterial};
use bevy::prelude::*;
use bevy::reflect::TypePath;
use bevy::render::render_asset::RenderAssetUsages;
use bevy::render::render_resource::{
    AsBindGroup, Extent3d, RenderPipelineDescriptor, ShaderRef, ShaderType,
    SpecializedMeshPipelineError, TextureDimension, TextureFormat, TextureUsages,
    TextureViewDescriptor, TextureViewDimension,
};
use bevy::render::texture::{Image, ImageLoaderSettings};

use crate::types::TILE_SIZE;

pub type TerrainMaterial = ExtendedMaterial<StandardMaterial, TerrainMaterialExtension>;

#[derive(Debug, Clone)]
pub struct TerrainMaterialHandles {
    pub material: Handle<TerrainMaterial>,
    pub base_color: Handle<Image>,
    pub normal: Option<Handle<Image>>,
    pub roughness: Option<Handle<Image>>,
    pub dispersion: Option<Handle<Image>>,
}

const TILE_REPEAT: f32 = 4.0;

fn default_uv_scale() -> f32 {
    1.0 / (TILE_SIZE * TILE_REPEAT)
}

fn default_height_uv_scale() -> f32 {
    // Terrain vertices already encode their scaled world-space heights, so no
    // additional adjustment is needed when sampling triplanar textures.
    1.0
}

fn default_height_world_scale() -> f32 {
    // Seam and bottom heights stored in the mesh are expressed in world units.
    // Keeping this value at 1.0 preserves the editor's existing appearance
    // while satisfying the shader's expectations.
    1.0
}

#[derive(Clone, Copy, Debug, ShaderType)]
pub struct TerrainMaterialParams {
    pub uv_scale: f32,
    pub layer_count: u32,
    pub map_size: Vec2,
    pub tile_size: f32,
    pub height_uv_scale: f32,
    pub height_world_scale: f32,
    pub cliff_blend_height: f32,
    pub wall_layer_index: u32,
    pub wall_enabled: u32,
    pub wall_has_normal: u32,
    pub wall_has_roughness: u32,
    #[allow(dead_code)]
    pub _padding: Vec2,
}

impl Default for TerrainMaterialParams {
    fn default() -> Self {
        Self {
            uv_scale: default_uv_scale(),
            layer_count: 0,
            map_size: Vec2::splat(1.0),
            tile_size: TILE_SIZE,
            height_uv_scale: default_height_uv_scale(),
            height_world_scale: default_height_world_scale(),
            cliff_blend_height: 0.2,
            wall_layer_index: u32::MAX,
            wall_enabled: 0,
            wall_has_normal: 0,
            wall_has_roughness: 0,
            _padding: Vec2::ZERO,
        }
    }
}

#[derive(Asset, AsBindGroup, Debug, Clone, TypePath)]
pub struct TerrainMaterialExtension {
    #[uniform(100)]
    pub params: TerrainMaterialParams,

    #[texture(101, dimension = "2d_array")]
    #[sampler(102)]
    pub base_color_array: Option<Handle<Image>>,

    #[texture(103, dimension = "2d_array")]
    #[sampler(104)]
    pub normal_array: Option<Handle<Image>>,

    #[texture(105, dimension = "2d_array")]
    #[sampler(106)]
    pub roughness_array: Option<Handle<Image>>,

    #[texture(107, dimension = "2d")]
    #[sampler(108)]
    pub splat_map: Option<Handle<Image>>,
}

impl Default for TerrainMaterialExtension {
    fn default() -> Self {
        Self {
            params: TerrainMaterialParams::default(),
            base_color_array: None,
            normal_array: None,
            roughness_array: None,
            splat_map: None,
        }
    }
}

impl MaterialExtension for TerrainMaterialExtension {
    fn fragment_shader() -> ShaderRef {
        "shaders/terrain_pbr_extension.wgsl".into()
    }

    fn deferred_fragment_shader() -> ShaderRef {
        "shaders/terrain_pbr_extension.wgsl".into()
    }

    fn specialize(
        _pipeline: &bevy::pbr::MaterialExtensionPipeline,
        descriptor: &mut bevy::render::render_resource::RenderPipelineDescriptor,
        _layout: &bevy::render::mesh::MeshVertexBufferLayoutRef,
        _key: bevy::pbr::MaterialExtensionKey<Self>,
    ) -> Result<(), bevy::render::render_resource::SpecializedMeshPipelineError> {
        descriptor.vertex.shader_defs.push("VERTEX_UVS_B".into());
        descriptor
            .fragment
            .as_mut()
            .unwrap()
            .shader_defs
            .push("VERTEX_UVS_B".into());

        if let Some(frag) = descriptor.fragment.as_mut() {
            frag.shader_defs.push("VERTEX_UVS_B".into());
            frag.shader_defs
                .push("TERRAIN_MATERIAL_EXTENSION_BASE_COLOR_ARRAY".into());

            frag.shader_defs
                .push("TERRAIN_MATERIAL_EXTENSION_NORMAL_ARRAY".into());

            frag.shader_defs
                .push("TERRAIN_MATERIAL_EXTENSION_ROUGHNESS_ARRAY".into());

            frag.shader_defs
                .push("TERRAIN_MATERIAL_EXTENSION_SPLAT_MAP".into());

            // frag.shader_defs.push("DEBUG_ROUGHNESS".into());
            // frag.shader_defs.push("DEBUG_NORMALS".into());
        }

        Ok(())
    }
}

// fn format_loaded_roughness_textures(
//     mut events: EventReader<AssetEvent<Image>>,
//     mut images: ResMut<Assets<Image>>,
//     terrain_handles: Res<TerrainMaterialHandles>,
// ) {
//     for event in events.read() {
//         if let AssetEvent::LoadedWithDependencies { id } = event {
//             // For each terrain roughness handle, compare its .id()
//             if let Some(rough_handle) = &terrain_handles.roughness {
//                 if rough_handle.id() == *id {
//                     if let Some(image) = images.get_mut(rough_handle) {
//                         if image.texture_descriptor.format == TextureFormat::Rgba8UnormSrgb {
//                             image.texture_descriptor.format = TextureFormat::Rgba8Unorm;
//                         }
//                         // else if image.texture_descriptor.format == TextureFormat::R8UnormSrgb {
//                         //     image.texture_descriptor.format = TextureFormat::R8Unorm;
//                         // }
//                     }
//                 }
//             }
//         }
//     }
// }

/// Load a terrain material and keep the individual texture handles around so they can be
/// reused for things like UI previews.
pub fn load_terrain_material(
    asset_server: &AssetServer,
    materials: &mut Assets<TerrainMaterial>,
    base_color: String,
    normal: Option<String>,
    roughness: Option<String>,
    dispersion: Option<String>,
) -> TerrainMaterialHandles {
    let base_color_handle: Handle<Image> = asset_server.load(base_color);

    // let normal_handle: Option<Handle<Image>> = normal.map(|path| asset_server.load(path));
    // let roughness_handle: Option<Handle<Image>> = roughness.map(|path| {
    //     asset_server.load_with_settings::<Image, _>(path, |settings: &mut ImageLoaderSettings| {
    //         settings.is_srgb = false; // force linear for roughness/metallic/AO
    //     })
    // });
    // let dispersion_handle: Option<Handle<Image>> = dispersion.map(|path| asset_server.load(path));

    let normal_handle: Option<Handle<Image>> = normal.map(|path| asset_server.load(path));
    let roughness_handle: Option<Handle<Image>> = roughness.map(|path| asset_server.load(path));
    let dispersion_handle: Option<Handle<Image>> = dispersion.map(|path| asset_server.load(path));

    info!("roughness_handle:");
    info!("{:?}", roughness_handle);

    let mut base_material = StandardMaterial {
        base_color_texture: Some(base_color_handle.clone()),
        normal_map_texture: normal_handle.clone(),
        metallic_roughness_texture: roughness_handle.clone(),
        ..default()
    };

    info!(
        "Roughness handle set? {:?}",
        base_material.metallic_roughness_texture.is_some()
    );

    base_material.metallic = 0.0;

    let material_handle = materials.add(TerrainMaterial {
        base: base_material,
        extension: TerrainMaterialExtension::default(),
    });

    TerrainMaterialHandles {
        material: material_handle,
        base_color: base_color_handle,
        normal: normal_handle,
        roughness: roughness_handle,
        dispersion: dispersion_handle,
    }
}

pub fn fix_roughness_images_on_load(
    mut events: EventReader<AssetEvent<Image>>,
    mut images: ResMut<Assets<Image>>,
    asset_server: Res<AssetServer>,
) {
    for (handle, image) in images.iter_mut() {
        // only touch roughness maps
        if let Some(path) = asset_server.get_path(handle) {
            if !path.path().to_string_lossy().contains("roughness") {
                continue;
            }
        }

        // if it's already fine, skip
        match image.texture_descriptor.format {
            TextureFormat::R8Unorm | TextureFormat::R32Float => continue,
            _ => {
                // Coerce if it's an RGBA EXR or something else heavy
                if image.texture_descriptor.format == TextureFormat::Rgba32Float {
                    if let Some(path) = asset_server.get_path(handle) {
                        info!("Inspecting roughness map @: {}", path.path().display());
                    }
                    info!("Found a roughness map with type TextureFormat::Rgba32Float");

                    // Downcast: grab red channel as f32 and rebuild buffer
                    let new_data: Vec<f32> = image
                        .data
                        .chunks_exact(16) // RGBA32F = 4 * f32 = 16 bytes
                        .map(|px| f32::from_le_bytes([px[0], px[1], px[2], px[3]])) // take red
                        .collect();

                    image.data = bytemuck::cast_slice(&new_data).to_vec();
                    image.texture_descriptor.format = TextureFormat::R32Float;
                    image.texture_descriptor.size.depth_or_array_layers = 1;
                    info!("Coerced roughness map {:?} to R32Float", handle);
                }
            }
        }
    }

    for event in events.read() {
        if let AssetEvent::LoadedWithDependencies { id } = event {
            if let Some(path) = asset_server.get_path(*id) {
                if path.path().to_string_lossy().contains("roughness") {
                    if let Some(image) = images.get_mut(*id) {
                        match image.texture_descriptor.format {
                            TextureFormat::Rgba8UnormSrgb => {
                                image.texture_descriptor.format = TextureFormat::Rgba8Unorm;

                                info!(
                                    "Roughness image format: {:?}",
                                    image.texture_descriptor.format
                                );
                                info!("First few bytes: {:?}", &image.data[..16]);
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    }
}

pub fn create_runtime_material(materials: &mut Assets<TerrainMaterial>) -> Handle<TerrainMaterial> {
    let base = StandardMaterial {
        base_color_texture: None,
        normal_map_texture: None,
        metallic_roughness_texture: None,
        occlusion_texture: None,
        perceptual_roughness: 1.0,
        metallic: 0.0,
        ..default()
    };

    materials.add(TerrainMaterial {
        base,
        extension: TerrainMaterialExtension::default(),
    })
}

pub fn create_texture_array_image(layers: &[&Image]) -> Option<Image> {
    if layers.is_empty() {
        return None;
    }

    let first = layers[0];
    let size = first.texture_descriptor.size;
    let format = first.texture_descriptor.format;
    let layer_size = first.data.len();

    if layer_size == 0 {
        return None;
    }

    let mut data = Vec::with_capacity(layer_size * layers.len());
    for image in layers {
        if image.texture_descriptor.size != size || image.texture_descriptor.format != format {
            return None;
        }
        data.extend_from_slice(&image.data);
    }

    let mut array_image = Image::new(
        Extent3d {
            width: size.width,
            height: size.height,
            depth_or_array_layers: layers.len() as u32,
        },
        TextureDimension::D2,
        data,
        format,
        RenderAssetUsages::default(),
    );
    array_image.texture_descriptor.mip_level_count = 1;
    array_image.texture_descriptor.usage = TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST;
    array_image.texture_view_descriptor = Some(TextureViewDescriptor {
        dimension: Some(TextureViewDimension::D2Array),
        ..Default::default()
    });

    Some(array_image)
}

pub(crate) fn ensure_image_uses_linear_format(image: &mut Image) -> bool {
    let current = image.texture_descriptor.format;
    let linear = linear_texture_format(current);
    if current == linear {
        return false;
    }

    image.texture_descriptor.format = linear;

    if let Some(view_descriptor) = image.texture_view_descriptor.as_mut() {
        if let Some(view_format) = view_descriptor.format {
            if view_format == current {
                view_descriptor.format = Some(linear);
            }
        }
    }

    true
}

pub(crate) fn linear_texture_format(format: TextureFormat) -> TextureFormat {
    match format {
        TextureFormat::Rgba8UnormSrgb => TextureFormat::Rgba8Unorm,
        other => other,
    }

    // format
}
