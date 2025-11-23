use std::collections::HashMap;

use crate::types::{RampDirection, TILE_HEIGHT, TILE_SIZE, TileKind, TileMap, TileType};
use bevy::ecs::schedule::SystemSet;
use bevy::prelude::*;
use bevy::render::mesh::{Indices, Mesh};
use bevy::render::render_asset::RenderAssetUsages;
use bevy::render::render_resource::{
    AddressMode, FilterMode, PrimitiveTopology, SamplerDescriptor, TextureDimension, TextureFormat,
    TextureUsages,
};
use bevy::render::texture::{Image, ImageSampler};

pub const CORNER_NW: usize = 0;
pub const CORNER_NE: usize = 1;
pub const CORNER_SW: usize = 2;
pub const CORNER_SE: usize = 3;

#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone, Copy)]
pub enum TerrainMeshSet {
    Rebuild,
    Cleanup,
}

pub fn tile_corner_heights(map: &TileMap, x: u32, y: u32) -> [f32; 4] {
    let tile = map.get(x, y);
    let base = tile.elevation as f32 * TILE_HEIGHT;
    let mut corners = [base; 4];

    if tile.kind == TileKind::Ramp {
        let mut target = tile
            .ramp_direction
            .and_then(|dir| ramp_neighbor_height(map, x, y, dir, base).map(|h| (dir, h)));

        if target.is_none() {
            target = find_ramp_target(map, x, y, base);
        }

        if let Some((dir, neighbor_height)) = target {
            match dir {
                RampDirection::North => {
                    corners[CORNER_NW] = neighbor_height;
                    corners[CORNER_NE] = neighbor_height;
                }
                RampDirection::South => {
                    corners[CORNER_SW] = neighbor_height;
                    corners[CORNER_SE] = neighbor_height;
                }
                RampDirection::West => {
                    corners[CORNER_NW] = neighbor_height;
                    corners[CORNER_SW] = neighbor_height;
                }
                RampDirection::East => {
                    corners[CORNER_NE] = neighbor_height;
                    corners[CORNER_SE] = neighbor_height;
                }
            }
        }
    }

    corners
}

pub fn empty_mesh() -> Mesh {
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
}

pub fn build_map_meshes(map: &TileMap) -> HashMap<TileType, Mesh> {
    let mut buffers: HashMap<TileType, MeshBuffers> = HashMap::new();
    populate_mesh_buffers(map, Some(&mut buffers), None);
    buffers
        .into_iter()
        .map(|(tile_type, buffer)| (tile_type, buffer.into_mesh()))
        .collect()
}

pub fn build_combined_mesh(map: &TileMap) -> Mesh {
    let mut buffer = MeshBuffers::with_tile_types();
    populate_mesh_buffers(map, None, Some(&mut buffer));
    buffer.into_mesh()
}

fn populate_mesh_buffers(
    map: &TileMap,
    mut per_type: Option<&mut HashMap<TileType, MeshBuffers>>,
    mut combined: Option<&mut MeshBuffers>,
) {
    if map.width == 0 || map.height == 0 {
        return;
    }

    let mut corner_cache = vec![[0.0f32; 4]; (map.width * map.height) as usize];
    for y in 0..map.height {
        for x in 0..map.width {
            let idx = map.idx(x, y);
            corner_cache[idx] = tile_corner_heights(map, x, y);
        }
    }

    for y in 0..map.height {
        for x in 0..map.width {
            if let Some(buffers) = per_type.as_mut() {
                let tile_type = map.get(x, y).tile_type;
                let buffer = buffers.entry(tile_type).or_default();
                append_tile_geometry(map, &corner_cache, x, y, buffer, None);
            }

            if let Some(combined_buffer) = combined.as_mut() {
                let tile_layer = map.get(x, y).tile_type.as_index() as f32;

                // dbg!(map.get(x, y).tile_type);

                append_tile_geometry(map, &corner_cache, x, y, combined_buffer, Some(tile_layer));
            }
        }
    }
}

fn append_tile_geometry(
    map: &TileMap,
    corner_cache: &[[f32; 4]],
    x: u32,
    y: u32,
    buffer: &mut MeshBuffers,
    tile_layer: Option<f32>,
) {
    let idx = map.idx(x, y);
    let corners = corner_cache[idx];
    let tile_kind = map.get(x, y).kind;
    let x0 = x as f32 * TILE_SIZE;
    let x1 = x0 + TILE_SIZE;
    let z0 = y as f32 * TILE_SIZE;
    let z1 = z0 + TILE_SIZE;

    let nw = Vec3::new(x0, corners[CORNER_NW], z0);
    let ne = Vec3::new(x1, corners[CORNER_NE], z0);
    let sw = Vec3::new(x0, corners[CORNER_SW], z1);
    let se = Vec3::new(x1, corners[CORNER_SE], z1);

    let top_height = max_corner_height(corners);

    let top_color_info = if tile_layer.is_some() {
        Some(tile_top_blend_mask(map, corner_cache, x, y, top_height))
    } else {
        None
    };

    buffer.push_quad(
        [nw, sw, se, ne],
        [[0.0, 0.0]; 4],
        tile_layer,
        top_height,
        top_color_info,
    );

    let (bnw, bne, north_neighbor_kind, north_bottom_layer) = if y > 0 {
        let neighbor_idx = map.idx(x, y - 1);
        let neighbor = corner_cache[neighbor_idx];
        let neighbor_tile = map.get(x, y - 1);
        (
            neighbor[CORNER_SW],
            neighbor[CORNER_SE],
            Some(neighbor_tile.kind),
            Some(neighbor_tile.tile_type.as_index() as f32),
        )
    } else {
        (0.0, 0.0, None, None)
    };
    let north_bottom_a_y = bnw.min(nw.y);
    let north_bottom_b_y = bne.min(ne.y);
    let north_bottom_a = Vec3::new(x0, north_bottom_a_y, z0);
    let north_bottom_b = Vec3::new(x1, north_bottom_b_y, z0);
    let north_bottom_info = north_bottom_layer.map(|layer| {
        let bottom_height = north_bottom_a_y.max(north_bottom_b_y);
        [layer, bottom_height, 0.0, 0.0]
    });
    let north_force_cliff = should_force_cliff_face(
        tile_kind,
        north_neighbor_kind,
        nw,
        ne,
        north_bottom_a,
        north_bottom_b,
    );
    buffer.add_side_face(
        nw,
        ne,
        north_bottom_a,
        north_bottom_b,
        RampDirection::North,
        tile_layer,
        nw.y.max(ne.y),
        north_bottom_info,
        north_force_cliff,
    );

    let (bsw, bse, south_neighbor_kind, south_bottom_layer) = if y + 1 < map.height {
        let neighbor_idx = map.idx(x, y + 1);
        let neighbor = corner_cache[neighbor_idx];
        let neighbor_tile = map.get(x, y + 1);
        (
            neighbor[CORNER_NW],
            neighbor[CORNER_NE],
            Some(neighbor_tile.kind),
            Some(neighbor_tile.tile_type.as_index() as f32),
        )
    } else {
        (0.0, 0.0, None, None)
    };
    let south_bottom_a_y = bse.min(se.y);
    let south_bottom_b_y = bsw.min(sw.y);
    let south_bottom_a = Vec3::new(x1, south_bottom_a_y, z1);
    let south_bottom_b = Vec3::new(x0, south_bottom_b_y, z1);
    let south_bottom_info = south_bottom_layer.map(|layer| {
        let bottom_height = south_bottom_a_y.max(south_bottom_b_y);
        [layer, bottom_height, 0.0, 0.0]
    });
    let south_force_cliff = should_force_cliff_face(
        tile_kind,
        south_neighbor_kind,
        se,
        sw,
        south_bottom_a,
        south_bottom_b,
    );
    buffer.add_side_face(
        se,
        sw,
        south_bottom_a,
        south_bottom_b,
        RampDirection::South,
        tile_layer,
        se.y.max(sw.y),
        south_bottom_info,
        south_force_cliff,
    );

    let (bnw, bsw, west_neighbor_kind, west_bottom_layer) = if x > 0 {
        let neighbor_idx = map.idx(x - 1, y);
        let neighbor = corner_cache[neighbor_idx];
        let neighbor_tile = map.get(x - 1, y);
        (
            neighbor[CORNER_NE],
            neighbor[CORNER_SE],
            Some(neighbor_tile.kind),
            Some(neighbor_tile.tile_type.as_index() as f32),
        )
    } else {
        (0.0, 0.0, None, None)
    };
    let west_bottom_a_y = bsw.min(sw.y);
    let west_bottom_b_y = bnw.min(nw.y);
    let west_bottom_a = Vec3::new(x0, west_bottom_a_y, z1);
    let west_bottom_b = Vec3::new(x0, west_bottom_b_y, z0);
    let west_bottom_info = west_bottom_layer.map(|layer| {
        let bottom_height = west_bottom_a_y.max(west_bottom_b_y);
        [layer, bottom_height, 0.0, 0.0]
    });
    let west_force_cliff = should_force_cliff_face(
        tile_kind,
        west_neighbor_kind,
        sw,
        nw,
        west_bottom_a,
        west_bottom_b,
    );
    buffer.add_side_face(
        sw,
        nw,
        west_bottom_a,
        west_bottom_b,
        RampDirection::West,
        tile_layer,
        sw.y.max(nw.y),
        west_bottom_info,
        west_force_cliff,
    );

    let (bne, bse, east_neighbor_kind, east_bottom_layer) = if x + 1 < map.width {
        let neighbor_idx = map.idx(x + 1, y);
        let neighbor = corner_cache[neighbor_idx];
        let neighbor_tile = map.get(x + 1, y);
        (
            neighbor[CORNER_NW],
            neighbor[CORNER_SW],
            Some(neighbor_tile.kind),
            Some(neighbor_tile.tile_type.as_index() as f32),
        )
    } else {
        (0.0, 0.0, None, None)
    };
    let east_bottom_a_y = bne.min(ne.y);
    let east_bottom_b_y = bse.min(se.y);
    let east_bottom_a = Vec3::new(x1, east_bottom_a_y, z0);
    let east_bottom_b = Vec3::new(x1, east_bottom_b_y, z1);
    let east_bottom_info = east_bottom_layer.map(|layer| {
        let bottom_height = east_bottom_a_y.max(east_bottom_b_y);
        [layer, bottom_height, 0.0, 0.0]
    });
    let east_force_cliff = should_force_cliff_face(
        tile_kind,
        east_neighbor_kind,
        ne,
        se,
        east_bottom_a,
        east_bottom_b,
    );
    buffer.add_side_face(
        ne,
        se,
        east_bottom_a,
        east_bottom_b,
        RampDirection::East,
        tile_layer,
        ne.y.max(se.y),
        east_bottom_info,
        east_force_cliff,
    );
}

fn find_ramp_target(map: &TileMap, x: u32, y: u32, base: f32) -> Option<(RampDirection, f32)> {
    let mut result: Option<(RampDirection, f32)> = None;
    for dir in RampDirection::ALL {
        if let Some(height) = ramp_neighbor_height(map, x, y, dir, base) {
            match &result {
                Some((_, existing)) if *existing <= height => {}
                _ => {
                    result = Some((dir, height));
                }
            }
        }
    }
    result
}

fn ramp_neighbor_height(
    map: &TileMap,
    x: u32,
    y: u32,
    dir: RampDirection,
    base: f32,
) -> Option<f32> {
    let (dx, dy) = dir.offset();
    let nx = x as i32 + dx;
    let ny = y as i32 + dy;
    if nx < 0 || ny < 0 {
        return None;
    }
    let (ux, uy) = (nx as u32, ny as u32);
    if ux >= map.width || uy >= map.height {
        return None;
    }
    let neighbor = map.get(ux, uy);
    let height = neighbor.elevation as f32 * TILE_HEIGHT;
    if height < base { Some(height) } else { None }
}

#[derive(Default)]
struct MeshBuffers {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    tile_layers: Option<Vec<[f32; 2]>>,
    colors: Option<Vec<[f32; 4]>>,
    indices: Vec<u32>,
    next_index: u32,
}

impl MeshBuffers {
    fn with_tile_types() -> Self {
        Self {
            tile_layers: Some(Vec::new()),
            colors: Some(Vec::new()),
            ..Default::default()
        }
    }

    fn push_quad(
        &mut self,
        verts: [Vec3; 4],
        tex: [[f32; 2]; 4],
        tile_layer: Option<f32>,
        seam_height: f32,
        bottom_layer: Option<[f32; 4]>,
    ) {
        push_quad(
            &mut self.positions,
            &mut self.normals,
            &mut self.uvs,
            self.tile_layers.as_mut(),
            self.colors.as_mut(),
            &mut self.indices,
            &mut self.next_index,
            verts,
            tex,
            tile_layer.map(|layer| [layer, seam_height]),
            bottom_layer,
        );
    }

    fn add_side_face(
        &mut self,
        top_a: Vec3,
        top_b: Vec3,
        bottom_a: Vec3,
        bottom_b: Vec3,
        direction: RampDirection,
        tile_layer: Option<f32>,
        seam_height: f32,
        bottom_info: Option<[f32; 4]>,
        force_cliff: bool,
    ) {
        add_side_face(
            &mut self.positions,
            &mut self.normals,
            &mut self.uvs,
            self.tile_layers.as_mut(),
            self.colors.as_mut(),
            &mut self.indices,
            &mut self.next_index,
            top_a,
            top_b,
            bottom_a,
            bottom_b,
            direction,
            tile_layer,
            seam_height,
            bottom_info,
            force_cliff,
        );
    }

    fn into_mesh(self) -> Mesh {
        let mut mesh = empty_mesh();
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, self.positions);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, self.uvs);
        if let Some(layers) = self.tile_layers {
            if !layers.is_empty() {
                mesh.insert_attribute(Mesh::ATTRIBUTE_UV_1, layers);
            }
        }
        if let Some(colors) = self.colors {
            if !colors.is_empty() {
                mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
            }
        }
        if !self.indices.is_empty() {
            mesh.insert_indices(Indices::U32(self.indices));
        }
        mesh
    }
}

fn push_quad(
    positions: &mut Vec<[f32; 3]>,
    normals: &mut Vec<[f32; 3]>,
    uvs: &mut Vec<[f32; 2]>,
    tile_layers: Option<&mut Vec<[f32; 2]>>,
    colors: Option<&mut Vec<[f32; 4]>>,
    indices: &mut Vec<u32>,
    next_index: &mut u32,
    verts: [Vec3; 4],
    tex: [[f32; 2]; 4],
    tile_info: Option<[f32; 2]>,
    color_info: Option<[f32; 4]>,
) {
    push_triangle(
        positions, normals, uvs, indices, next_index, verts[0], verts[1], verts[2], tex[0], tex[1],
        tex[2],
    );
    push_triangle(
        positions, normals, uvs, indices, next_index, verts[0], verts[2], verts[3], tex[0], tex[2],
        tex[3],
    );

    if let Some(layers) = tile_layers {
        for _ in 0..6 {
            layers.push(tile_info.unwrap_or([0.0, 0.0]));
        }
    }

    if let Some(colors) = colors {
        let value = color_info.unwrap_or([-1.0, 0.0, 0.0, 0.0]);
        for _ in 0..6 {
            colors.push(value);
        }
    }
}

fn push_triangle(
    positions: &mut Vec<[f32; 3]>,
    normals: &mut Vec<[f32; 3]>,
    uvs: &mut Vec<[f32; 2]>,
    indices: &mut Vec<u32>,
    next_index: &mut u32,
    a: Vec3,
    b: Vec3,
    c: Vec3,
    ta: [f32; 2],
    tb: [f32; 2],
    tc: [f32; 2],
) {
    let normal = (b - a).cross(c - a).normalize_or_zero();
    positions.push(a.to_array());
    positions.push(b.to_array());
    positions.push(c.to_array());
    normals.push(normal.to_array());
    normals.push(normal.to_array());
    normals.push(normal.to_array());
    uvs.push(ta);
    uvs.push(tb);
    uvs.push(tc);
    indices.extend_from_slice(&[*next_index, *next_index + 1, *next_index + 2]);
    *next_index += 3;
}

fn tile_top_blend_mask(
    map: &TileMap,
    corner_cache: &[[f32; 4]],
    x: u32,
    y: u32,
    top_height: f32,
) -> [f32; 4] {
    const HEIGHT_EPSILON: f32 = 0.01;

    let mut mask_bits: u32 = 0;

    if y > 0 {
        let neighbor = corner_cache[map.idx(x, y - 1)];
        if (top_height - max_corner_height(neighbor)).abs() < HEIGHT_EPSILON {
            mask_bits |= 0b0001;
        }
    }

    if y + 1 < map.height {
        let neighbor = corner_cache[map.idx(x, y + 1)];
        if (top_height - max_corner_height(neighbor)).abs() < HEIGHT_EPSILON {
            mask_bits |= 0b0010;
        }
    }

    if x > 0 {
        let neighbor = corner_cache[map.idx(x - 1, y)];
        if (top_height - max_corner_height(neighbor)).abs() < HEIGHT_EPSILON {
            mask_bits |= 0b0100;
        }
    }

    if x + 1 < map.width {
        let neighbor = corner_cache[map.idx(x + 1, y)];
        if (top_height - max_corner_height(neighbor)).abs() < HEIGHT_EPSILON {
            mask_bits |= 0b1000;
        }
    }

    [-2.0, mask_bits as f32, 0.0, 0.0]
}

fn max_corner_height(corners: [f32; 4]) -> f32 {
    corners
        .into_iter()
        .fold(f32::NEG_INFINITY, |acc, value| acc.max(value))
}

fn add_side_face(
    positions: &mut Vec<[f32; 3]>,
    normals: &mut Vec<[f32; 3]>,
    uvs: &mut Vec<[f32; 2]>,
    tile_layers: Option<&mut Vec<[f32; 2]>>,
    colors: Option<&mut Vec<[f32; 4]>>,
    indices: &mut Vec<u32>,
    next_index: &mut u32,
    top_a: Vec3,
    top_b: Vec3,
    bottom_a: Vec3,
    bottom_b: Vec3,
    direction: RampDirection,
    tile_layer: Option<f32>,
    seam_height: f32,
    bottom_info: Option<[f32; 4]>,
    force_cliff: bool,
) {
    const EPS: f32 = 1e-4;
    if (top_a.y - bottom_a.y).abs() < EPS && (top_b.y - bottom_b.y).abs() < EPS {
        return;
    }

    let (verts, tex) = match direction {
        RampDirection::North => ([top_a, top_b, bottom_b, bottom_a], [[0.0, 0.0]; 4]),
        RampDirection::South => ([top_a, top_b, bottom_b, bottom_a], [[0.0, 0.0]; 4]),
        RampDirection::West => ([top_a, top_b, bottom_b, bottom_a], [[0.0, 0.0]; 4]),
        RampDirection::East => ([top_a, top_b, bottom_b, bottom_a], [[0.0, 0.0]; 4]),
    };

    let mut color_info = bottom_info;
    if let Some(info) = color_info.as_mut() {
        info[2] = if force_cliff { 1.0 } else { 0.0 };
    } else if force_cliff {
        color_info = Some([-1.0, 0.0, 1.0, 0.0]);
    }

    push_quad(
        positions,
        normals,
        uvs,
        tile_layers,
        colors,
        indices,
        next_index,
        verts,
        tex,
        tile_layer.map(|layer| [layer, seam_height]),
        color_info,
    );
}

fn should_force_cliff_face(
    tile_kind: TileKind,
    neighbor_kind: Option<TileKind>,
    top_a: Vec3,
    top_b: Vec3,
    bottom_a: Vec3,
    bottom_b: Vec3,
) -> bool {
    const EPS: f32 = 1e-4;
    if tile_kind != TileKind::Ramp && neighbor_kind != Some(TileKind::Ramp) {
        return false;
    }

    let drop_a = top_a.y - bottom_a.y;
    let drop_b = top_b.y - bottom_b.y;

    drop_a > EPS || drop_b > EPS
}

pub mod splatmap {
    use super::*;
    use bevy::render::render_asset::RenderAssetUsages;
    use bevy::render::render_resource::Extent3d;
    use bevy::render::texture::{ImageAddressMode, ImageFilterMode, ImageSamplerDescriptor};

    const CHANNELS: usize = 4;

    pub fn create(map: &TileMap) -> Image {
        let extent = extent_from_map(map);
        let mut image = Image::new_fill(
            extent,
            TextureDimension::D2,
            &[0u8; CHANNELS],
            TextureFormat::Rgba8Unorm,
            RenderAssetUsages::default(),
        );
        configure_image(&mut image);
        write(map, &mut image);
        image
    }

    pub fn write(map: &TileMap, image: &mut Image) {
        let extent = extent_from_map(map);
        if image.texture_descriptor.size != extent
            || image.texture_descriptor.format != TextureFormat::Rgba8Unorm
        {
            *image = create(map);
            return;
        }

        configure_image(image);

        let width = extent.width as usize;
        let required_len = width * (extent.height as usize) * CHANNELS;
        if image.data.len() != required_len {
            image.data.resize(required_len, 0);
        }

        if map.width == 0 || map.height == 0 {
            image.data.fill(0);
            return;
        }

        for y in 0..map.height as usize {
            for x in 0..map.width as usize {
                let mut pixel = [0u8; CHANNELS];
                let tile = map.get(x as u32, y as u32);
                let layer = tile.tile_type.as_index();
                if layer < CHANNELS {
                    pixel[layer] = 255;
                }

                let idx = (y * width + x) * CHANNELS;
                image.data[idx..idx + CHANNELS].copy_from_slice(&pixel);
            }
        }
    }

    fn extent_from_map(map: &TileMap) -> Extent3d {
        Extent3d {
            width: map.width.max(1),
            height: map.height.max(1),
            depth_or_array_layers: 1,
        }
    }

    fn configure_image(image: &mut Image) {
        image.texture_descriptor.mip_level_count = 1;
        image.texture_descriptor.usage = TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST;
        image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
            mag_filter: ImageFilterMode::Linear,
            min_filter: ImageFilterMode::Linear,
            mipmap_filter: ImageFilterMode::Nearest,
            address_mode_u: ImageAddressMode::ClampToEdge,
            address_mode_v: ImageAddressMode::ClampToEdge,
            ..Default::default()
        });
    }
}
