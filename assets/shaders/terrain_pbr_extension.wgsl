#import bevy_pbr::{
    pbr_bindings,
    pbr_functions::alpha_discard,
    pbr_fragment::pbr_input_from_standard_material,
};

#ifdef PREPASS_PIPELINE
#import bevy_pbr::{
    prepass_io::{VertexOutput, FragmentOutput},
    pbr_deferred_functions::deferred_output,
};
#else
#import bevy_pbr::{
    forward_io::{VertexOutput, FragmentOutput},
    pbr_functions,
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
    pbr_types::STANDARD_MATERIAL_FLAGS_UNLIT_BIT,
};
#endif

#ifdef MESHLET_MESH_MATERIAL_PASS
#import bevy_pbr::meshlet_visibility_buffer_resolve::resolve_vertex_output
#endif

struct TerrainMaterialExtension {
    uv_scale: f32,
    layer_count: u32,
    map_size: vec2<f32>,
    tile_size: f32,
    height_uv_scale: f32,
    height_world_scale: f32,
    cliff_blend_height: f32,
    wall_layer_index: u32,
    wall_enabled: u32,
    wall_has_normal: u32,
    wall_has_roughness: u32,
    _padding: vec2<f32>,
}

@group(2) @binding(100)
var<uniform> terrain_material_extension: TerrainMaterialExtension;

#ifdef TERRAIN_MATERIAL_EXTENSION_BASE_COLOR_ARRAY
@group(2) @binding(101)
var terrain_base_color_array: texture_2d_array<f32>;
@group(2) @binding(102)
var terrain_base_color_sampler: sampler;
#endif

#ifdef TERRAIN_MATERIAL_EXTENSION_NORMAL_ARRAY
@group(2) @binding(103)
var terrain_normal_array: texture_2d_array<f32>;
@group(2) @binding(104)
var terrain_normal_sampler: sampler;
#endif

#ifdef TERRAIN_MATERIAL_EXTENSION_ROUGHNESS_ARRAY
@group(2) @binding(105)
var terrain_roughness_array: texture_2d_array<f32>;
@group(2) @binding(106)
var terrain_roughness_sampler: sampler;
#endif

#ifdef TERRAIN_MATERIAL_EXTENSION_SPLAT_MAP
@group(2) @binding(107)
var terrain_splat_map: texture_2d<f32>;
@group(2) @binding(108)
var terrain_splat_sampler: sampler;
#endif

fn triplanar_sample(
    tex: texture_2d<f32>,
    samp: sampler,
    pos: vec3<f32>,
    norm: vec3<f32>,
    scale: f32
) -> vec4<f32> {
    let n = normalize(norm);
    let weights = abs(n) / (abs(n.x) + abs(n.y) + abs(n.z));

    let adjusted_y = pos.y * terrain_material_extension.height_uv_scale;

    // Wrap each projection into [0,1)
    let uv_x = fract(vec2<f32>(adjusted_y, pos.z) * scale);
    let uv_y = fract(pos.xz * scale);
    let uv_z = fract(vec2<f32>(pos.x, adjusted_y) * scale);

    let x_tex = textureSample(tex, samp, uv_x);
    let y_tex = textureSample(tex, samp, uv_y);
    let z_tex = textureSample(tex, samp, uv_z);

    return x_tex * weights.x + y_tex * weights.y + z_tex * weights.z;
}

#ifdef TERRAIN_MATERIAL_EXTENSION_BASE_COLOR_ARRAY
fn triplanar_sample_layer(
    tex: texture_2d_array<f32>,
    samp: sampler,
    pos: vec3<f32>,
    norm: vec3<f32>,
    scale: f32,
    layer: i32,
) -> vec4<f32> {
    let n = normalize(norm);
    let weights = abs(n) / (abs(n.x) + abs(n.y) + abs(n.z));

    let adjusted_y = pos.y * terrain_material_extension.height_uv_scale;

    let uv_x = fract(vec2<f32>(adjusted_y, pos.z) * scale);
    let uv_y = fract(pos.xz * scale);
    let uv_z = fract(vec2<f32>(pos.x, adjusted_y) * scale);

//    let layer_f = f32(layer);
//    let x_tex = textureSample(tex, samp, vec3<f32>(uv_x, layer_f));
//    let y_tex = textureSample(tex, samp, vec3<f32>(uv_y, layer_f));
//    let z_tex = textureSample(tex, samp, vec3<f32>(uv_z, layer_f));

    let x_tex = textureSample(tex, samp, uv_x, layer);
    let y_tex = textureSample(tex, samp, uv_y, layer);
    let z_tex = textureSample(tex, samp, uv_z, layer);

    return x_tex * weights.x + y_tex * weights.y + z_tex * weights.z;
}
#endif

#ifdef TERRAIN_MATERIAL_EXTENSION_NORMAL_ARRAY
fn triplanar_sample_layer_normal(
    tex: texture_2d_array<f32>,
    samp: sampler,
    pos: vec3<f32>,
    norm: vec3<f32>,
    scale: f32,
    layer: i32,
) -> vec3<f32> {
    let n = normalize(norm);
    let weights = abs(n) / (abs(n.x) + abs(n.y) + abs(n.z));

    let adjusted_y = pos.y * terrain_material_extension.height_uv_scale;

    let uv_x = fract(vec2<f32>(adjusted_y, pos.z) * scale);
    let uv_y = fract(pos.xz * scale);
    let uv_z = fract(vec2<f32>(pos.x, adjusted_y) * scale);

    let sample_x = textureSample(tex, samp, uv_x, layer).xyz * 2.0 - vec3<f32>(1.0);
    let sample_y = textureSample(tex, samp, uv_y, layer).xyz * 2.0 - vec3<f32>(1.0);
    let sample_z = textureSample(tex, samp, uv_z, layer).xyz * 2.0 - vec3<f32>(1.0);

    var sign_x: f32;
    if (n.x >= 0.0) {
        sign_x = 1.0;
    } else {
        sign_x = -1.0;
    }

    var sign_y: f32;
    if (n.y >= 0.0) {
        sign_y = 1.0;
    } else {
        sign_y = -1.0;
    }

    var sign_z: f32;
    if (n.z >= 0.0) {
        sign_z = 1.0;
    } else {
        sign_z = -1.0;
    }

    let world_x = normalize(
        sample_x.x * vec3<f32>(0.0, sign_x, 0.0)
            + sample_x.y * vec3<f32>(0.0, 0.0, 1.0)
            + sample_x.z * vec3<f32>(sign_x, 0.0, 0.0),
    );
    let world_y = normalize(
        sample_y.x * vec3<f32>(sign_y, 0.0, 0.0)
            + sample_y.y * vec3<f32>(0.0, 0.0, 1.0)
            + sample_y.z * vec3<f32>(0.0, sign_y, 0.0),
    );
    let world_z = normalize(
        sample_z.x * vec3<f32>(sign_z, 0.0, 0.0)
            + sample_z.y * vec3<f32>(0.0, sign_z, 0.0)
            + sample_z.z * vec3<f32>(0.0, 0.0, sign_z),
    );

    return normalize(world_x * weights.x + world_y * weights.y + world_z * weights.z);
}
#endif

#ifdef TERRAIN_MATERIAL_EXTENSION_ROUGHNESS_ARRAY
fn triplanar_sample_layer_scalar(
    tex: texture_2d_array<f32>,
    samp: sampler,
    pos: vec3<f32>,
    norm: vec3<f32>,
    scale: f32,
    layer: i32,
) -> f32 {
    let n = normalize(norm);
    let weights = abs(n) / (abs(n.x) + abs(n.y) + abs(n.z));

    let adjusted_y = pos.y * terrain_material_extension.height_uv_scale;

    let uv_x = fract(vec2<f32>(adjusted_y, pos.z) * scale);
    let uv_y = fract(pos.xz * scale);
    let uv_z = fract(vec2<f32>(pos.x, adjusted_y) * scale);

    let sample_x = textureSample(tex, samp, uv_x, layer).g;
    let sample_y = textureSample(tex, samp, uv_y, layer).g;
    let sample_z = textureSample(tex, samp, uv_z, layer).g;

    return sample_x * weights.x + sample_y * weights.y + sample_z * weights.z;
}
#endif

const MAX_TERRAIN_LAYERS: u32 = 4u;

fn assign_weight(weights: vec4<f32>, index: u32, value: f32) -> vec4<f32> {
    var result = weights;
    switch index {
        case 0u: {
            result.x = value;
        }
        case 1u: {
            result.y = value;
        }
        case 2u: {
            result.z = value;
        }
        default: {
            result.w = value;
        }
    }
    return result;
}

fn weight_component(weights: vec4<f32>, index: u32) -> f32 {
    switch index {
        case 0u: {
            return weights.x;
        }
        case 1u: {
            return weights.y;
        }
        case 2u: {
            return weights.z;
        }
        default: {
            return weights.w;
        }
    }
}

fn world_to_splat_uv(world_position: vec3<f32>) -> vec2<f32> {
    let safe_tile = max(terrain_material_extension.tile_size, 0.0001);
    let safe_map = max(terrain_material_extension.map_size, vec2<f32>(1.0, 1.0));
    let tile_space = world_position.xz / safe_tile;
    let uv = tile_space / safe_map;
    return clamp(uv, vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 1.0));
}

fn clamp_layer_index(layer: i32, available_layers: u32) -> i32 {
    if (available_layers == 0u) {
        return 0;
    }
    let max_layer = i32(available_layers) - 1;
    return clamp(layer, 0, max_layer);
}

fn mask_float(condition: bool) -> f32 {
    if (condition) {
        return 1.0;
    }
    return 0.0;
}

fn decode_top_blend_mask(mask_bits: f32) -> vec4<f32> {
    let bits = max(i32(round(mask_bits)), 0);
    let north = mask_float((bits & 0x1) != 0);
    let south = mask_float((bits & 0x2) != 0);
    let west = mask_float((bits & 0x4) != 0);
    let east = mask_float((bits & 0x8) != 0);
    return vec4<f32>(north, south, west, east);
}

@fragment
fn fragment(
#ifdef MESHLET_MESH_MATERIAL_PASS
    @builtin(position) frag_coord: vec4<f32>,
#else
    in: VertexOutput,
    @builtin(front_facing) is_front: bool,
#endif
) -> FragmentOutput {
#ifdef MESHLET_MESH_MATERIAL_PASS
    let in = resolve_vertex_output(frag_coord);
    let is_front = true;
#endif

#ifdef VISIBILITY_RANGE_DITHER
    pbr_functions::visibility_range_dither(in.position, in.visibility_range_dither);
#endif

    // Build PBR input (uses mesh UVs initially)
    var pbr_input = pbr_input_from_standard_material(in, is_front);

    // Choose projection by dominant world normal axis
    let scale = terrain_material_extension.uv_scale;
    var base_color = vec4<f32>(pbr_input.material.base_color.rgb, 1.0);

#ifdef STANDARD_MATERIAL_BASE_COLOR_TEXTURE
    base_color = triplanar_sample(
        pbr_bindings::base_color_texture,
        pbr_bindings::base_color_sampler,
        pbr_input.world_position.xyz,
        pbr_input.world_normal.xyz,
        scale,
    );
#endif

#ifdef TERRAIN_MATERIAL_EXTENSION_SPLAT_MAP
    var weights = textureSample(
        terrain_splat_map,
        terrain_splat_sampler,
        world_to_splat_uv(pbr_input.world_position.xyz),
    );
#ifdef VERTEX_COLORS
    let is_top_face = abs(pbr_input.world_normal.y) >= 0.5;
    if (is_top_face && in.color.r < -1.5) {
        let mask = decode_top_blend_mask(in.color.g);
        if (mask.x < 0.5 || mask.y < 0.5 || mask.z < 0.5 || mask.w < 0.5) {
            let tile_size = max(terrain_material_extension.tile_size, 0.0001);
            let safe_map = max(terrain_material_extension.map_size, vec2<f32>(1.0, 1.0));
            let tile_space = pbr_input.world_position.xz / tile_size;
            let tile_base_f = floor(tile_space);
            let map_size_i = vec2<i32>(safe_map);
            let max_tile = max(map_size_i - vec2<i32>(1, 1), vec2<i32>(0, 0));
            let tile_base_i = clamp(vec2<i32>(tile_base_f), vec2<i32>(0, 0), max_tile);
            let tile_base = vec2<f32>(tile_base_i);
            let local = tile_space - tile_base;
            var adjusted = local;
            var needs_adjustment = false;

            if (mask.x < 0.5) {
                adjusted.y = max(adjusted.y, 0.5);
                needs_adjustment = true;
            }

            if (mask.y < 0.5) {
                adjusted.y = min(adjusted.y, 0.5);
                needs_adjustment = true;
            }

            if (mask.z < 0.5) {
                adjusted.x = max(adjusted.x, 0.5);
                needs_adjustment = true;
            }

            if (mask.w < 0.5) {
                adjusted.x = min(adjusted.x, 0.5);
                needs_adjustment = true;
            }

            if (needs_adjustment) {
                let sample_uv = (tile_base + adjusted) / safe_map;
                let clamped_uv = clamp(sample_uv, vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 1.0));
                weights = textureSampleLevel(
                    terrain_splat_map,
                    terrain_splat_sampler,
                    clamped_uv,
                    0.0,
                );
            }
        }
    }
#endif
#else
    var weights = vec4<f32>(0.0, 0.0, 0.0, 0.0);
#endif

    let available_layers = min(terrain_material_extension.layer_count, MAX_TERRAIN_LAYERS);
    var weight_total = weights.x + weights.y + weights.z + weights.w;

    if (weight_total <= 0.0001) {
        if (available_layers == 0u) {
            weights = vec4<f32>(1.0, 0.0, 0.0, 0.0);
        } else {
#ifdef VERTEX_UVS_B
            let fallback_source = in.uv_b.x;
#else
            let fallback_source = 0.0;
#endif
            let fallback_layer = clamp_layer_index(i32(round(fallback_source)), available_layers);
            weights = assign_weight(vec4<f32>(0.0, 0.0, 0.0, 0.0), u32(fallback_layer), 1.0);
        }
        weight_total = 1.0;
    } else {
        weights = weights / weight_total;
    }

#ifdef TERRAIN_MATERIAL_EXTENSION_BASE_COLOR_ARRAY
    if (available_layers > 0u) {
        var color_accum = vec3<f32>(0.0, 0.0, 0.0);
        var color_weight = 0.0;
        for (var layer = 0u; layer < available_layers; layer = layer + 1u) {
            let weight = weight_component(weights, layer);
            if (weight <= 0.0001) {
                continue;
            }

            let sampled = triplanar_sample_layer(
                terrain_base_color_array,
                terrain_base_color_sampler,
                pbr_input.world_position.xyz,
                pbr_input.world_normal.xyz,
                scale,
                i32(layer),
            );
            color_accum += sampled.rgb * weight;
            color_weight += weight;
        }

        if (color_weight > 0.0001) {
            base_color = vec4<f32>(color_accum / color_weight, 1.0);
        }
    }
#endif

#ifdef TERRAIN_MATERIAL_EXTENSION_NORMAL_ARRAY
    if (available_layers > 0u) {
        var normal_accum = vec3<f32>(0.0, 0.0, 0.0);
        var normal_weight = 0.0;
        for (var layer = 0u; layer < available_layers; layer = layer + 1u) {
            let weight = weight_component(weights, layer);
            if (weight <= 0.0001) {
                continue;
            }

            let world_normal = triplanar_sample_layer_normal(
                terrain_normal_array,
                terrain_normal_sampler,
                pbr_input.world_position.xyz,
                pbr_input.world_normal.xyz,
                scale,
                i32(layer),
            );
            normal_accum += world_normal * weight;
            normal_weight += weight;
        }

        if (normal_weight > 0.0001) {
            let blended_normal = normalize(normal_accum / normal_weight);
            pbr_input.N = blended_normal;
            pbr_input.clearcoat_N = blended_normal;
        }
    }
#endif

#ifdef TERRAIN_MATERIAL_EXTENSION_ROUGHNESS_ARRAY
    if (available_layers > 0u) {
        var roughness_accum = 0.0;
        var roughness_weight = 0.0;
        for (var layer = 0u; layer < available_layers; layer = layer + 1u) {
            let weight = weight_component(weights, layer);
            if (weight <= 0.0001) {
                continue;
            }

            let sampled = triplanar_sample_layer_scalar(
                terrain_roughness_array,
                terrain_roughness_sampler,
                pbr_input.world_position.xyz,
                pbr_input.world_normal.xyz,
                scale,
                i32(layer),
            );

            roughness_accum += sampled * weight;
            roughness_weight += weight;
        }

        if (roughness_weight > 0.0001) {
            let rough_min: f32 = 0.2;
            let rough_max: f32 = 0.9;
            let averaged = clamp(roughness_accum / roughness_weight, 0.0, 1.0);
            let remapped = mix(rough_min, rough_max, averaged);
            pbr_input.material.perceptual_roughness = clamp(remapped, 0.045, 1.0);
        }
    }
#endif

    if (abs(pbr_input.world_normal.y) < 0.5 && available_layers > 0u) {
#ifdef VERTEX_UVS_B
        let fallback_source = in.uv_b.x;
#else
        let fallback_source = 0.0;
#endif
        let top_layer_index = clamp_layer_index(i32(round(fallback_source)), available_layers);
        let seam_height = in.uv_b.y * terrain_material_extension.height_world_scale;
        let safe_blend = max(terrain_material_extension.cliff_blend_height, 0.0001);
        let top_delta = seam_height - pbr_input.world_position.y;
        var top_blend = clamp(1.0 - (top_delta / safe_blend), 0.0, 1.0);

        var bottom_blend = 0.0;
        var bottom_layer_index = top_layer_index;
        var has_bottom = false;
        var force_cliff = false;
#ifdef VERTEX_COLORS
        if (in.color.b > 0.5) {
            force_cliff = true;
        }
        if (in.color.r >= 0.0) {
            let candidate = clamp_layer_index(i32(round(in.color.r)), available_layers);
            bottom_layer_index = candidate;
            let bottom_height = in.color.g * terrain_material_extension.height_world_scale;
            let bottom_delta = pbr_input.world_position.y - bottom_height;
            bottom_blend = clamp(1.0 - (bottom_delta / safe_blend), 0.0, 1.0);
            has_bottom = true;
        }
#endif

        var cliff_weight = max(1.0 - top_blend - bottom_blend, 0.0);

        if (force_cliff) {
            top_blend = 0.0;
            bottom_blend = 0.0;
            cliff_weight = 1.0;
        }

        let wall_enabled = terrain_material_extension.wall_enabled == 1u;
        let wall_layer_index = i32(terrain_material_extension.wall_layer_index);
#ifdef TERRAIN_MATERIAL_EXTENSION_NORMAL_ARRAY
        let wall_has_normal_map = terrain_material_extension.wall_has_normal == 1u;
#endif
#ifdef TERRAIN_MATERIAL_EXTENSION_ROUGHNESS_ARRAY
        let wall_has_roughness_map = terrain_material_extension.wall_has_roughness == 1u;
#endif

#ifdef TERRAIN_MATERIAL_EXTENSION_BASE_COLOR_ARRAY
        var cliff_sample: vec4<f32>;
        if (wall_enabled) {
            cliff_sample = triplanar_sample_layer(
                terrain_base_color_array,
                terrain_base_color_sampler,
                pbr_input.world_position.xyz,
                pbr_input.world_normal.xyz,
                scale,
                wall_layer_index,
            );
        } else {
            cliff_sample = triplanar_sample_layer(
                terrain_base_color_array,
                terrain_base_color_sampler,
                pbr_input.world_position.xyz,
                pbr_input.world_normal.xyz,
                scale,
                top_layer_index,
            );
        }
        var color_accum = vec3<f32>(0.0);
        var color_weight = 0.0;

        if (cliff_weight > 0.0001) {
            color_accum += cliff_sample.rgb * cliff_weight;
            color_weight += cliff_weight;
        }

        if (top_blend > 0.0001) {
            let top_sample = triplanar_sample_layer(
                terrain_base_color_array,
                terrain_base_color_sampler,
                pbr_input.world_position.xyz,
                pbr_input.world_normal.xyz,
                scale,
                top_layer_index,
            );
            color_accum += top_sample.rgb * top_blend;
            color_weight += top_blend;
        }

        if (has_bottom && bottom_blend > 0.0001) {
            let bottom_sample = triplanar_sample_layer(
                terrain_base_color_array,
                terrain_base_color_sampler,
                pbr_input.world_position.xyz,
                pbr_input.world_normal.xyz,
                scale,
                bottom_layer_index,
            );
            color_accum += bottom_sample.rgb * bottom_blend;
            color_weight += bottom_blend;
        }

        if (color_weight > 0.0001) {
            base_color = vec4<f32>(color_accum / color_weight, 1.0);
        } else {
            base_color = vec4<f32>(cliff_sample.rgb, 1.0);
        }
#endif

#ifdef TERRAIN_MATERIAL_EXTENSION_NORMAL_ARRAY
        var cliff_normal: vec3<f32>;
        if (wall_enabled && wall_has_normal_map) {
            cliff_normal = triplanar_sample_layer_normal(
                terrain_normal_array,
                terrain_normal_sampler,
                pbr_input.world_position.xyz,
                pbr_input.world_normal.xyz,
                scale,
                wall_layer_index,
            );
        } else {
            cliff_normal = triplanar_sample_layer_normal(
                terrain_normal_array,
                terrain_normal_sampler,
                pbr_input.world_position.xyz,
                pbr_input.world_normal.xyz,
                scale,
                top_layer_index,
            );
        }
        var normal_accum = vec3<f32>(0.0);
        var normal_weight = 0.0;

        if (cliff_weight > 0.0001) {
            normal_accum += cliff_normal * cliff_weight;
            normal_weight += cliff_weight;
        }

        if (top_blend > 0.0001) {
            let top_normal = triplanar_sample_layer_normal(
                terrain_normal_array,
                terrain_normal_sampler,
                pbr_input.world_position.xyz,
                pbr_input.world_normal.xyz,
                scale,
                top_layer_index,
            );
            normal_accum += top_normal * top_blend;
            normal_weight += top_blend;
        }

        if (has_bottom && bottom_blend > 0.0001) {
            let bottom_normal = triplanar_sample_layer_normal(
                terrain_normal_array,
                terrain_normal_sampler,
                pbr_input.world_position.xyz,
                pbr_input.world_normal.xyz,
                scale,
                bottom_layer_index,
            );
            normal_accum += bottom_normal * bottom_blend;
            normal_weight += bottom_blend;
        }

        if (normal_weight > 0.0001) {
            let blended_normal = normalize(normal_accum / normal_weight);
            pbr_input.N = blended_normal;
            pbr_input.clearcoat_N = blended_normal;
        } else {
            pbr_input.N = cliff_normal;
            pbr_input.clearcoat_N = cliff_normal;
        }
#endif

#ifdef TERRAIN_MATERIAL_EXTENSION_ROUGHNESS_ARRAY
        var cliff_rough: f32;
        if (wall_enabled && wall_has_roughness_map) {
            cliff_rough = triplanar_sample_layer_scalar(
                terrain_roughness_array,
                terrain_roughness_sampler,
                pbr_input.world_position.xyz,
                pbr_input.world_normal.xyz,
                scale,
                wall_layer_index,
            );
        } else {
            cliff_rough = triplanar_sample_layer_scalar(
                terrain_roughness_array,
                terrain_roughness_sampler,
                pbr_input.world_position.xyz,
                pbr_input.world_normal.xyz,
                scale,
                top_layer_index,
            );
        }
        var roughness_accum = 0.0;
        var roughness_weight = 0.0;

        if (cliff_weight > 0.0001) {
            roughness_accum += cliff_rough * cliff_weight;
            roughness_weight += cliff_weight;
        }

        if (top_blend > 0.0001) {
            let top_rough = triplanar_sample_layer_scalar(
                terrain_roughness_array,
                terrain_roughness_sampler,
                pbr_input.world_position.xyz,
                pbr_input.world_normal.xyz,
                scale,
                top_layer_index,
            );
            roughness_accum += top_rough * top_blend;
            roughness_weight += top_blend;
        }

        if (has_bottom && bottom_blend > 0.0001) {
            let bottom_rough = triplanar_sample_layer_scalar(
                terrain_roughness_array,
                terrain_roughness_sampler,
                pbr_input.world_position.xyz,
                pbr_input.world_normal.xyz,
                scale,
                bottom_layer_index,
            );
            roughness_accum += bottom_rough * bottom_blend;
            roughness_weight += bottom_blend;
        }

        if (roughness_weight > 0.0001) {
            let rough_min: f32 = 0.2;
            let rough_max: f32 = 0.9;
            let averaged = clamp(roughness_accum / roughness_weight, 0.0, 1.0);
            let remapped = mix(rough_min, rough_max, averaged);
            pbr_input.material.perceptual_roughness = clamp(remapped, 0.045, 1.0);
        } else {
            let rough_min: f32 = 0.2;
            let rough_max: f32 = 0.9;
            let remapped = mix(rough_min, rough_max, clamp(cliff_rough, 0.0, 1.0));
            pbr_input.material.perceptual_roughness = clamp(remapped, 0.045, 1.0);
        }
#endif
    }

    pbr_input.material.base_color = alpha_discard(pbr_input.material, base_color);



#ifdef PREPASS_PIPELINE
    let out = deferred_output(in, pbr_input);
#else
    var out: FragmentOutput;
    if (pbr_input.material.flags & STANDARD_MATERIAL_FLAGS_UNLIT_BIT) == 0u {
        out.color = apply_pbr_lighting(pbr_input);
    } else {
        out.color = pbr_input.material.base_color;
    }

    out.color = main_pass_post_lighting_processing(pbr_input, out.color);

#ifdef DEBUG_NORMALS
    out.color = vec4<f32>(
        0.5 * (pbr_input.N.x + 1.0),
        0.5 * (pbr_input.N.y + 1.0),
        0.5 * (pbr_input.N.z + 1.0),
        1.0
    );
#endif

//    out.color = vec4<f32>(in.uv_b.x / 10.0, in.uv_b.y, 0.0, 1.0);

#ifdef DEBUG_SPLAT
    let splat = textureSample(
        terrain_splat_map,
        terrain_splat_sampler,
        world_to_splat_uv(pbr_input.world_position.xyz),
    );
    out.color = vec4<f32>(splat.rgb, 1.0);
#endif

#ifdef DEBUG_LAYER3
    let test = triplanar_sample_layer(
        terrain_base_color_array,
        terrain_base_color_sampler,
        pbr_input.world_position.xyz,
        pbr_input.world_normal.xyz,
        terrain_material_extension.uv_scale,
        3   // test the rock layer index
    );
    out.color = vec4<f32>(test.rgb, 1.0);
#endif

#ifdef DEBUG_ROUGHNESS
if (terrain_material_extension.layer_count > 0u) {
    let max_layer = i32(terrain_material_extension.layer_count) - 1;
    #ifdef VERTEX_UVS_B
    let layer_source = in.uv_b.x;
    #else
    let layer_source = 0.0;
    #endif
    let layer_value = clamp(i32(round(layer_source)), 0, max_layer);

    // sample via triplanar (same path as shading)
    let sampled = triplanar_sample_layer_scalar(
        terrain_roughness_array,
        terrain_roughness_sampler,
        pbr_input.world_position.xyz,
        pbr_input.world_normal.xyz,
        terrain_material_extension.uv_scale,
        layer_value,
    );

    // show EXACT value Bevy reads: 0..1
    out.color = vec4<f32>(sampled, sampled, sampled, 1.0);
}
#endif


#endif'

    return out;
}