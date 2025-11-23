use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail, ensure};
use bevy::render::mesh::{Indices, Mesh, VertexAttributeValues};
use bevy::render::texture::Image;
use image::codecs::png::PngEncoder;
use image::{ColorType, ExtendedColorType, ImageEncoder};
use serde::Serialize;
use serde_json::json;
use zip::CompressionMethod;
use zip::write::FileOptions;

use crate::terrain;
use crate::terrain::splatmap;
use crate::texture::registry::TerrainTextureRegistry;
use crate::types::{TILE_SIZE, TileMap, TileType};

const VERTEX_BUFFER_TARGET: u32 = 34962;
const INDEX_BUFFER_TARGET: u32 = 34963;
const FLOAT_COMPONENT: u32 = 5126;
const UNSIGNED_INT_COMPONENT: u32 = 5125;

#[derive(Clone)]
pub struct TextureFileDescriptor {
    pub source_path: PathBuf,
}

#[derive(Clone)]
pub struct TextureExportDescriptor {
    pub tile_type: TileType,
    pub identifier: String,
    pub diffuse: TextureFileDescriptor,
    pub normal: Option<TextureFileDescriptor>,
    pub roughness: Option<TextureFileDescriptor>,
}

#[derive(Clone)]
pub struct WallTextureExportDescriptor {
    pub identifier: String,
    pub diffuse: TextureFileDescriptor,
    pub normal: Option<TextureFileDescriptor>,
    pub roughness: Option<TextureFileDescriptor>,
}

#[derive(Serialize)]
struct MetadataTextureEntry {
    id: String,
    diffuse: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    normal: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    roughness: Option<String>,
    splatmap_channel: usize,
}

#[derive(Serialize)]
struct MetadataWallTexture {
    id: String,
    diffuse: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    normal: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    roughness: Option<String>,
}

#[derive(Serialize)]
struct ExportMetadata {
    name: String,
    width: u32,
    height: u32,
    tile_size: f32,
    textures: Vec<MetadataTextureEntry>,
    splatmap: String,
    mesh: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    tilemap: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    wall_texture: Option<MetadataWallTexture>,
}

pub fn collect_texture_descriptors(
    map: &TileMap,
    registry: &TerrainTextureRegistry,
) -> Result<(
    Vec<TextureExportDescriptor>,
    Option<WallTextureExportDescriptor>,
)> {
    use std::collections::HashSet;

    let mut used: HashSet<TileType> = HashSet::new();
    for tile in &map.tiles {
        used.insert(tile.tile_type);
    }

    let mut descriptors = Vec::new();
    for tile_type in TileType::ALL {
        if !used.contains(&tile_type) {
            continue;
        }

        let entry = registry
            .get(tile_type)
            .ok_or_else(|| anyhow!("No terrain texture registered for {tile_type:?}"))?;

        let diffuse = TextureFileDescriptor {
            source_path: resolve_asset_path(&entry.diffuse_path)?,
        };

        let normal = match &entry.normal_path {
            Some(path) => Some(TextureFileDescriptor {
                source_path: resolve_asset_path(path)?,
            }),
            None => None,
        };

        let roughness = match &entry.roughness_path {
            Some(path) => Some(TextureFileDescriptor {
                source_path: resolve_asset_path(path)?,
            }),
            None => None,
        };

        descriptors.push(TextureExportDescriptor {
            tile_type,
            identifier: tile_type.identifier().to_string(),
            diffuse,
            normal,
            roughness,
        });
    }

    let wall_descriptor = registry
        .wall_texture()
        .map(|entry| -> Result<WallTextureExportDescriptor> {
            let diffuse = TextureFileDescriptor {
                source_path: resolve_asset_path(&entry.diffuse_path)?,
            };

            let normal = match &entry.normal_path {
                Some(path) => Some(TextureFileDescriptor {
                    source_path: resolve_asset_path(path)?,
                }),
                None => None,
            };

            let roughness = match &entry.roughness_path {
                Some(path) => Some(TextureFileDescriptor {
                    source_path: resolve_asset_path(path)?,
                }),
                None => None,
            };

            Ok(WallTextureExportDescriptor {
                identifier: entry.id.clone(),
                diffuse,
                normal,
                roughness,
            })
        })
        .transpose()?;

    Ok((descriptors, wall_descriptor))
}

pub fn export_package(
    output_path: &Path,
    map: TileMap,
    map_name: String,
    textures: Vec<TextureExportDescriptor>,
    wall_texture: Option<WallTextureExportDescriptor>,
    splat_png: Vec<u8>,
) -> Result<()> {
    if let Some(parent) = output_path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!("Failed to create export directory {}", parent.display())
            })?;
        }
    }

    let mesh = terrain::build_combined_mesh(&map);
    let mesh_bytes = mesh_to_glb(&mesh)?;

    let tilemap_json = serde_json::to_vec_pretty(&map)?;

    let (metadata, texture_files, wall_texture_metadata) =
        build_metadata_and_files(&textures, wall_texture)?;
    let metadata = ExportMetadata {
        name: map_name,
        width: map.width,
        height: map.height,
        tile_size: TILE_SIZE,
        textures: metadata,
        splatmap: "splatmap.png".to_string(),
        mesh: "mesh.glb".to_string(),
        tilemap: Some("tilemap.json".to_string()),
        wall_texture: wall_texture_metadata,
    };
    let metadata_json = serde_json::to_vec_pretty(&metadata)?;

    let file = File::create(output_path)
        .with_context(|| format!("Failed to create export file {}", output_path.display()))?;
    let mut zip = zip::ZipWriter::new(file);
    let options = FileOptions::default().compression_method(CompressionMethod::Deflated);

    zip.start_file("tilemap.json", options)?;
    zip.write_all(&tilemap_json)?;

    zip.start_file("mesh.glb", options)?;
    zip.write_all(&mesh_bytes)?;

    zip.start_file("splatmap.png", options)?;
    zip.write_all(&splat_png)?;

    zip.start_file("metadata.json", options)?;
    zip.write_all(&metadata_json)?;

    if !texture_files.is_empty() {
        zip.add_directory("textures/", options)?;
    }

    for (path, bytes) in texture_files {
        zip.start_file(path, options)?;
        zip.write_all(&bytes)?;
    }

    zip.finish()?;
    Ok(())
}

fn build_metadata_and_files(
    textures: &[TextureExportDescriptor],
    wall_texture: Option<WallTextureExportDescriptor>,
) -> Result<(
    Vec<MetadataTextureEntry>,
    Vec<(String, Vec<u8>)>,
    Option<MetadataWallTexture>,
)> {
    let mut metadata = Vec::new();
    let mut files = Vec::new();

    for descriptor in textures {
        validate_identifier(&descriptor.identifier)?;

        let diffuse_path = ingest_texture_file(
            &descriptor.identifier,
            "diffuse",
            &descriptor.diffuse,
            &mut files,
        )?;
        let normal_path = ingest_optional_texture_file(
            &descriptor.identifier,
            "normal",
            &descriptor.normal,
            &mut files,
        )?;
        let roughness_path = ingest_optional_texture_file(
            &descriptor.identifier,
            "roughness",
            &descriptor.roughness,
            &mut files,
        )?;

        metadata.push(MetadataTextureEntry {
            id: descriptor.identifier.clone(),
            diffuse: diffuse_path,
            normal: normal_path,
            roughness: roughness_path,
            splatmap_channel: descriptor.tile_type.as_index(),
        });
    }

    let wall_metadata = match wall_texture {
        Some(descriptor) => {
            validate_identifier(&descriptor.identifier)?;
            let diffuse = ingest_texture_file(
                &descriptor.identifier,
                "diffuse",
                &descriptor.diffuse,
                &mut files,
            )?;
            let normal = ingest_optional_texture_file(
                &descriptor.identifier,
                "normal",
                &descriptor.normal,
                &mut files,
            )?;
            let roughness = ingest_optional_texture_file(
                &descriptor.identifier,
                "roughness",
                &descriptor.roughness,
                &mut files,
            )?;

            Some(MetadataWallTexture {
                id: descriptor.identifier,
                diffuse,
                normal,
                roughness,
            })
        }
        None => None,
    };

    Ok((metadata, files, wall_metadata))
}

fn validate_identifier(identifier: &str) -> Result<()> {
    ensure!(
        identifier
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-'),
        "Texture identifier contains unsupported characters"
    );
    Ok(())
}

fn ingest_texture_file(
    identifier: &str,
    kind: &str,
    source: &TextureFileDescriptor,
    files: &mut Vec<(String, Vec<u8>)>,
) -> Result<String> {
    let target_path = texture_target_path(identifier, kind, &source.source_path);
    let bytes = std::fs::read(&source.source_path).with_context(|| {
        format!(
            "Failed to read {kind} texture for {identifier} from {}",
            source.source_path.display()
        )
    })?;
    files.push((target_path.clone(), bytes));
    Ok(target_path)
}

fn ingest_optional_texture_file(
    identifier: &str,
    kind: &str,
    source: &Option<TextureFileDescriptor>,
    files: &mut Vec<(String, Vec<u8>)>,
) -> Result<Option<String>> {
    match source {
        Some(descriptor) => ingest_texture_file(identifier, kind, descriptor, files).map(Some),
        None => Ok(None),
    }
}

fn texture_target_path(id: &str, kind: &str, source: &Path) -> String {
    let mut file_name = format!("{}_{}", id, kind);
    if let Some(ext) = source
        .extension()
        .and_then(|ext| ext.to_str())
        .filter(|s| !s.is_empty())
    {
        file_name.push('.');
        file_name.push_str(ext);
    }
    format!("textures/{file_name}")
}

fn resolve_asset_path(path: &str) -> Result<PathBuf> {
    let raw_path = Path::new(path);
    let resolved = if raw_path.is_absolute() {
        raw_path.to_path_buf()
    } else {
        Path::new("assets").join(raw_path)
    };

    if !resolved.exists() {
        bail!("Asset path not found: {}", resolved.display());
    }

    Ok(resolved)
}

pub fn encode_splatmap_png(image: &Image) -> Result<Vec<u8>> {
    ensure!(
        image.texture_descriptor.format == bevy::render::render_resource::TextureFormat::Rgba8Unorm,
        "Splatmap must be RGBA8 format for export"
    );

    let width = image.texture_descriptor.size.width;
    let height = image.texture_descriptor.size.height;
    let mut buffer = Vec::new();
    PngEncoder::new(&mut buffer).write_image(
        &image.data,
        width,
        height,
        ExtendedColorType::Rgba8,
    )?;
    Ok(buffer)
}

pub fn build_map_splatmap_png(map: &TileMap) -> Result<Vec<u8>> {
    let image = splatmap::create(map);
    encode_splatmap_png(&image)
}

fn mesh_to_glb(mesh: &Mesh) -> Result<Vec<u8>> {
    let positions = extract_vec3(mesh, Mesh::ATTRIBUTE_POSITION, "POSITION")?;
    let normals = extract_vec3(mesh, Mesh::ATTRIBUTE_NORMAL, "NORMAL")?;
    let texcoords = extract_vec2(mesh, Mesh::ATTRIBUTE_UV_0, "TEXCOORD_0")?;
    let texcoords1 = extract_optional_vec2(mesh, Mesh::ATTRIBUTE_UV_1)?;
    let colors = extract_optional_vec4(mesh, Mesh::ATTRIBUTE_COLOR)?;
    let indices = extract_indices(mesh)?;

    ensure!(
        positions.len() == normals.len(),
        "Mesh export requires matching position and normal counts"
    );
    ensure!(
        positions.len() == texcoords.len(),
        "Mesh export requires matching position and UV counts"
    );
    if let Some(ref uv1) = texcoords1 {
        ensure!(
            uv1.len() == positions.len(),
            "Secondary UV set must match vertex count"
        );
    }
    if let Some(ref cols) = colors {
        ensure!(
            cols.len() == positions.len(),
            "Vertex color count must match vertices"
        );
    }

    let mut writer = BufferWriter::default();
    let position_accessor = writer.push_vec3(&positions, true)?;
    let normal_accessor = writer.push_vec3(&normals, false)?;
    let tex_accessor = writer.push_vec2(&texcoords)?;
    let tex1_accessor = texcoords1
        .as_ref()
        .map(|uvs| writer.push_vec2(uvs))
        .transpose()?;
    let color_accessor = colors
        .as_ref()
        .map(|cols| writer.push_vec4(cols))
        .transpose()?;
    let index_accessor = writer.push_indices(&indices)?;

    let (mut bin, buffer_views, accessors) = writer.finish();

    let mut attributes = serde_json::Map::new();
    attributes.insert("POSITION".to_string(), json!(position_accessor));
    attributes.insert("NORMAL".to_string(), json!(normal_accessor));
    attributes.insert("TEXCOORD_0".to_string(), json!(tex_accessor));
    if let Some(accessor) = tex1_accessor {
        attributes.insert("TEXCOORD_1".to_string(), json!(accessor));
    }
    if let Some(accessor) = color_accessor {
        attributes.insert("COLOR_0".to_string(), json!(accessor));
    }

    let primitive = json!({
        "attributes": attributes,
        "indices": index_accessor,
        "mode": 4,
    });

    let root = json!({
        "asset": {
            "version": "2.0",
            "generator": "tilemapedit3d exporter",
        },
        "buffers": [{
            "byteLength": bin.len() as u64,
            "name": "TerrainBuffer",
        }],
        "bufferViews": buffer_views,
        "accessors": accessors,
        "meshes": [{
            "name": "Terrain",
            "primitives": [primitive],
        }],
        "nodes": [{
            "mesh": 0,
            "name": "Terrain",
        }],
        "scenes": [{
            "nodes": [0],
        }],
        "scene": 0,
    });

    let mut json_bytes = serde_json::to_vec(&root)?;
    pad_to_four(&mut json_bytes, b' ');
    pad_to_four(&mut bin, 0);

    let total_length = 12 + 8 + json_bytes.len() + 8 + bin.len();
    let mut glb = Vec::with_capacity(total_length);
    glb.extend_from_slice(&0x46546C67u32.to_le_bytes());
    glb.extend_from_slice(&2u32.to_le_bytes());
    glb.extend_from_slice(&(total_length as u32).to_le_bytes());

    glb.extend_from_slice(&(json_bytes.len() as u32).to_le_bytes());
    glb.extend_from_slice(&0x4E4F534Au32.to_le_bytes());
    glb.extend_from_slice(&json_bytes);

    glb.extend_from_slice(&(bin.len() as u32).to_le_bytes());
    glb.extend_from_slice(&0x004E4942u32.to_le_bytes());
    glb.extend_from_slice(&bin);

    Ok(glb)
}

fn pad_to_four(buffer: &mut Vec<u8>, pad: u8) {
    while buffer.len() % 4 != 0 {
        buffer.push(pad);
    }
}

fn extract_vec3(
    mesh: &Mesh,
    attribute: bevy::render::mesh::MeshVertexAttribute,
    name: &str,
) -> Result<Vec<[f32; 3]>> {
    let values = mesh
        .attribute(attribute)
        .ok_or_else(|| anyhow!("Mesh missing {name} attribute"))?;
    match values {
        VertexAttributeValues::Float32x3(data) => Ok(data.clone()),
        _ => bail!("Mesh attribute {name} has unsupported format"),
    }
}

fn extract_vec2(
    mesh: &Mesh,
    attribute: bevy::render::mesh::MeshVertexAttribute,
    name: &str,
) -> Result<Vec<[f32; 2]>> {
    let values = mesh
        .attribute(attribute)
        .ok_or_else(|| anyhow!("Mesh missing {name} attribute"))?;
    match values {
        VertexAttributeValues::Float32x2(data) => Ok(data.clone()),
        _ => bail!("Mesh attribute {name} has unsupported format"),
    }
}

fn extract_optional_vec2(
    mesh: &Mesh,
    attribute: bevy::render::mesh::MeshVertexAttribute,
) -> Result<Option<Vec<[f32; 2]>>> {
    let Some(values) = mesh.attribute(attribute) else {
        return Ok(None);
    };
    match values {
        VertexAttributeValues::Float32x2(data) => Ok(Some(data.clone())),
        _ => bail!("Mesh attribute has unsupported format"),
    }
}

fn extract_optional_vec4(
    mesh: &Mesh,
    attribute: bevy::render::mesh::MeshVertexAttribute,
) -> Result<Option<Vec<[f32; 4]>>> {
    let Some(values) = mesh.attribute(attribute) else {
        return Ok(None);
    };
    match values {
        VertexAttributeValues::Float32x4(data) => Ok(Some(data.clone())),
        _ => bail!("Mesh attribute has unsupported format"),
    }
}

fn extract_indices(mesh: &Mesh) -> Result<Vec<u32>> {
    let indices = mesh
        .indices()
        .ok_or_else(|| anyhow!("Mesh is missing triangle indices"))?;
    match indices {
        Indices::U16(data) => Ok(data.iter().map(|&value| value as u32).collect()),
        Indices::U32(data) => Ok(data.clone()),
    }
}

#[derive(Default)]
struct BufferWriter {
    data: Vec<u8>,
    buffer_views: Vec<serde_json::Value>,
    accessors: Vec<serde_json::Value>,
}

impl BufferWriter {
    fn align(&mut self) {
        while self.data.len() % 4 != 0 {
            self.data.push(0);
        }
    }

    fn push_vec3(&mut self, values: &[[f32; 3]], include_bounds: bool) -> Result<usize> {
        ensure!(!values.is_empty(), "Vector attribute cannot be empty");
        self.align();
        let byte_offset = self.data.len();
        let bytes = bytemuck::cast_slice(values);
        self.data.extend_from_slice(bytes);
        let byte_length = bytes.len();
        let view_index = self.buffer_views.len();
        let mut view = serde_json::Map::new();
        view.insert("buffer".into(), json!(0));
        if byte_offset != 0 {
            view.insert("byteOffset".into(), json!(byte_offset as u64));
        }
        view.insert("byteLength".into(), json!(byte_length as u64));
        view.insert("target".into(), json!(VERTEX_BUFFER_TARGET));
        self.buffer_views.push(serde_json::Value::Object(view));

        let accessor_index = self.accessors.len();
        let mut accessor = serde_json::Map::new();
        accessor.insert("bufferView".into(), json!(view_index));
        accessor.insert("componentType".into(), json!(FLOAT_COMPONENT));
        accessor.insert("count".into(), json!(values.len() as u64));
        accessor.insert("type".into(), serde_json::Value::String("VEC3".into()));
        if include_bounds {
            let (min, max) = bounds_vec3(values);
            accessor.insert("min".into(), json!(min));
            accessor.insert("max".into(), json!(max));
        }
        self.accessors.push(serde_json::Value::Object(accessor));
        Ok(accessor_index)
    }

    fn push_vec2(&mut self, values: &[[f32; 2]]) -> Result<usize> {
        ensure!(!values.is_empty(), "Vector attribute cannot be empty");
        self.align();
        let byte_offset = self.data.len();
        let bytes = bytemuck::cast_slice(values);
        self.data.extend_from_slice(bytes);
        let byte_length = bytes.len();
        let view_index = self.buffer_views.len();
        let mut view = serde_json::Map::new();
        view.insert("buffer".into(), json!(0));
        if byte_offset != 0 {
            view.insert("byteOffset".into(), json!(byte_offset as u64));
        }
        view.insert("byteLength".into(), json!(byte_length as u64));
        view.insert("target".into(), json!(VERTEX_BUFFER_TARGET));
        self.buffer_views.push(serde_json::Value::Object(view));

        let accessor_index = self.accessors.len();
        let mut accessor = serde_json::Map::new();
        accessor.insert("bufferView".into(), json!(view_index));
        accessor.insert("componentType".into(), json!(FLOAT_COMPONENT));
        accessor.insert("count".into(), json!(values.len() as u64));
        accessor.insert("type".into(), serde_json::Value::String("VEC2".into()));
        self.accessors.push(serde_json::Value::Object(accessor));
        Ok(accessor_index)
    }

    fn push_vec4(&mut self, values: &[[f32; 4]]) -> Result<usize> {
        ensure!(!values.is_empty(), "Vector attribute cannot be empty");
        self.align();
        let byte_offset = self.data.len();
        let bytes = bytemuck::cast_slice(values);
        self.data.extend_from_slice(bytes);
        let byte_length = bytes.len();
        let view_index = self.buffer_views.len();
        let mut view = serde_json::Map::new();
        view.insert("buffer".into(), json!(0));
        if byte_offset != 0 {
            view.insert("byteOffset".into(), json!(byte_offset as u64));
        }
        view.insert("byteLength".into(), json!(byte_length as u64));
        view.insert("target".into(), json!(VERTEX_BUFFER_TARGET));
        self.buffer_views.push(serde_json::Value::Object(view));

        let accessor_index = self.accessors.len();
        let mut accessor = serde_json::Map::new();
        accessor.insert("bufferView".into(), json!(view_index));
        accessor.insert("componentType".into(), json!(FLOAT_COMPONENT));
        accessor.insert("count".into(), json!(values.len() as u64));
        accessor.insert("type".into(), serde_json::Value::String("VEC4".into()));
        self.accessors.push(serde_json::Value::Object(accessor));
        Ok(accessor_index)
    }

    fn push_indices(&mut self, values: &[u32]) -> Result<usize> {
        ensure!(!values.is_empty(), "Index buffer cannot be empty");
        self.align();
        let byte_offset = self.data.len();
        let bytes = bytemuck::cast_slice(values);
        self.data.extend_from_slice(bytes);
        let byte_length = bytes.len();
        let view_index = self.buffer_views.len();
        let mut view = serde_json::Map::new();
        view.insert("buffer".into(), json!(0));
        if byte_offset != 0 {
            view.insert("byteOffset".into(), json!(byte_offset as u64));
        }
        view.insert("byteLength".into(), json!(byte_length as u64));
        view.insert("target".into(), json!(INDEX_BUFFER_TARGET));
        self.buffer_views.push(serde_json::Value::Object(view));

        let accessor_index = self.accessors.len();
        let mut accessor = serde_json::Map::new();
        accessor.insert("bufferView".into(), json!(view_index));
        accessor.insert("componentType".into(), json!(UNSIGNED_INT_COMPONENT));
        accessor.insert("count".into(), json!(values.len() as u64));
        accessor.insert("type".into(), serde_json::Value::String("SCALAR".into()));
        self.accessors.push(serde_json::Value::Object(accessor));
        Ok(accessor_index)
    }

    fn finish(self) -> (Vec<u8>, Vec<serde_json::Value>, Vec<serde_json::Value>) {
        (self.data, self.buffer_views, self.accessors)
    }
}

fn bounds_vec3(values: &[[f32; 3]]) -> ([f32; 3], [f32; 3]) {
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    for value in values {
        for i in 0..3 {
            min[i] = min[i].min(value[i]);
            max[i] = max[i].max(value[i]);
        }
    }
    (min, max)
}
