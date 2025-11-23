use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat as F;
use bevy::render::texture::Image;

pub struct ImageInspectorPlugin;

impl Plugin for ImageInspectorPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, inspect_loaded_images);
    }
}

fn inspect_loaded_images(
    assets: Res<Assets<Image>>,
    mut events: EventReader<AssetEvent<Image>>,
    asset_server: Res<AssetServer>,
) {
    for event in events.read() {
        if let AssetEvent::LoadedWithDependencies { id } = event {
            if let Some(image) = assets.get(*id) {
                info!("--- Loaded Image ---");

                if let Some(path) = asset_server.get_path(*id) {
                    // info!("Path: {}", path.path().display());

                    let path_str = path.path().display().to_string();
                    info!("Path: {}", path_str);

                    info!("Size: {:?}", image.texture_descriptor.size);
                    info!("Format: {:?}", image.texture_descriptor.format);
                    info!("Usage: {:?}", image.texture_descriptor.usage);
                    info!("Mip levels: {}", image.texture_descriptor.mip_level_count);
                    info!(
                        "Array layers: {}",
                        image.texture_descriptor.array_layer_count()
                    );
                    info!("Dimension: {:?}", image.texture_descriptor.dimension);

                    // check sRGB
                    let srgb = image.texture_descriptor.format.is_srgb();
                    info!("is_srgb: {}", srgb);

                    // preview a few bytes
                    let bytes = &image.data[..16.min(image.data.len())];
                    info!("First few bytes: {:?}", bytes);

                    log_channel_ranges(&path_str, image);
                }
            }
        }
    }
}

fn log_channel_ranges(path: &str, image: &Image) {
    if matches!(
        image.texture_descriptor.format,
        F::Rgba8Unorm | F::Rgba8UnormSrgb
    ) {
        let (mut rmin, mut rmax) = (u8::MAX, 0u8);
        let (mut gmin, mut gmax) = (u8::MAX, 0u8);
        let (mut bmin, mut bmax) = (u8::MAX, 0u8);

        for px in image.data.chunks_exact(4) {
            let (r, g, b) = (px[0], px[1], px[2]);
            rmin = rmin.min(r);
            rmax = rmax.max(r);
            gmin = gmin.min(g);
            gmax = gmax.max(g);
            bmin = bmin.min(b);
            bmax = bmax.max(b);
        }

        info!("Channel ranges for {}:", path);
        info!("  R: {rmin}..{rmax}");
        info!("  G: {gmin}..{gmax}");
        info!("  B: {bmin}..{bmax}");
    }
}
