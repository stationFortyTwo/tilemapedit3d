use bincode::{Decode, Encode};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Debug, Encode, Decode)]
pub enum TileKind {
    Floor,
    Ramp,
}

#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Debug, Encode, Decode)]
pub enum RampDirection {
    North,
    East,
    South,
    West,
}

impl RampDirection {
    pub const ALL: [RampDirection; 4] = [
        RampDirection::North,
        RampDirection::East,
        RampDirection::South,
        RampDirection::West,
    ];

    pub fn next(self) -> RampDirection {
        match self {
            RampDirection::North => RampDirection::East,
            RampDirection::East => RampDirection::South,
            RampDirection::South => RampDirection::West,
            RampDirection::West => RampDirection::North,
        }
    }

    pub fn offset(self) -> (i32, i32) {
        match self {
            RampDirection::North => (0, -1),
            RampDirection::East => (1, 0),
            RampDirection::South => (0, 1),
            RampDirection::West => (-1, 0),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, Encode, Decode, PartialEq, Eq, Hash)]
pub enum TileType {
    Grass,
    Dirt,
    #[serde(alias = "Cliff")]
    Sand,
    Rock,
}

impl TileType {
    pub const ALL: [TileType; 4] = [
        TileType::Grass,
        TileType::Dirt,
        TileType::Sand,
        TileType::Rock,
    ];

    pub fn as_index(self) -> usize {
        match self {
            TileType::Grass => 0,
            TileType::Dirt => 1,
            TileType::Sand => 2,
            TileType::Rock => 3,
        }
    }

    pub fn identifier(self) -> &'static str {
        match self {
            TileType::Grass => "grass",
            TileType::Dirt => "dirt",
            TileType::Sand => "sand",
            TileType::Rock => "rock",
        }
    }
}

impl Default for TileType {
    fn default() -> Self {
        TileType::Grass
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Encode, Decode)]
pub struct Tile {
    pub kind: TileKind,
    pub tile_type: TileType,
    pub x: u32,
    pub y: u32,
    pub elevation: i8, // can be negative for underwater, or positive for cliffs
    #[serde(default)]
    pub ramp_direction: Option<RampDirection>,
}

#[derive(Serialize, Deserialize, Debug, Encode, Decode, Clone)]
pub struct TileMap {
    pub width: u32,
    pub height: u32,
    pub tiles: Vec<Tile>, // row-major
}

impl TileMap {
    pub fn new(w: u32, h: u32) -> Self {
        Self {
            width: w,
            height: h,
            tiles: (0..w * h)
                .map(|_| Tile {
                    kind: TileKind::Floor,
                    tile_type: TileType::default(),
                    elevation: 0,
                    x: 0,
                    y: 0,
                    ramp_direction: None,
                })
                .collect(),
        }
    }
    pub fn idx(&self, x: u32, y: u32) -> usize {
        (y * self.width + x) as usize
    }
    pub fn get(&self, x: u32, y: u32) -> &Tile {
        &self.tiles[self.idx(x, y)]
    }
    pub fn set(&mut self, x: u32, y: u32, t: Tile) {
        let i = self.idx(x, y);
        self.tiles[i] = t;
    }
}

pub const TILE_SIZE: f32 = 2.0; // world units per tile
pub const ELEVATION_FRACTION: f32 = 0.4; // fraction of tile width per elevation step
pub const TILE_HEIGHT: f32 = TILE_SIZE * ELEVATION_FRACTION; // height per elevation step
