use crate::types::TileMap;
use bincode::{config, decode_from_slice, encode_to_vec};
use std::path::Path;

const KEY: u8 = 0xAA;

fn obfuscate(data: &mut [u8]) {
    for b in data.iter_mut() {
        *b ^= KEY;
    }
}

pub fn save_map(path: impl AsRef<Path>, map: &TileMap) -> anyhow::Result<()> {
    // pick a config (matches old bincode defaults)
    let cfg = config::standard();
    let mut bytes = encode_to_vec(map, cfg)?;
    obfuscate(&mut bytes);
    std::fs::write(path, bytes)?;
    Ok(())
}

pub fn load_map(path: impl AsRef<Path>) -> anyhow::Result<TileMap> {
    let mut bytes = std::fs::read(path)?;
    obfuscate(&mut bytes);
    let cfg = config::standard();
    let (map, _len): (TileMap, usize) = decode_from_slice(&bytes, cfg)?;
    Ok(map)
}
