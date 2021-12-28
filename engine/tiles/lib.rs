use serde_derive::{Deserialize, Serialize};

#[enum_dispatch::enum_dispatch]
pub trait TileType {
    fn name(&self) -> &'static str;
}

#[enum_dispatch::enum_dispatch(TileType)]
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Tile {
    EmptyTile,
    HousingTile,
    WorkplaceTile,
    MetroStationTile,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmptyTile {}

impl TileType for EmptyTile {
    fn name(&self) -> &'static str {
        "empty"
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HousingTile {
    pub density: usize,
}

impl TileType for HousingTile {
    fn name(&self) -> &'static str {
        "housing"
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkplaceTile {
    pub density: usize,
}

impl TileType for WorkplaceTile {
    fn name(&self) -> &'static str {
        "workplace"
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetroStationTile {
    // Exact location of station within tile, relative to tile.
    pub x: u64,
    pub y: u64,
    // IDs to lookup metro lines. If there is more than one, then this is a transfer station.
    // May be empty because orphan stations are allowed, especially when constructing new lines.
    pub id: Vec<u64>,
}

impl TileType for MetroStationTile {
    fn name(&self) -> &'static str {
        "metro"
    }
}

#[cfg(test)]
mod tests {
    use tiles::*;

    #[test]
    fn enum_dispatch() {
        let tile = Tile::from(EmptyTile {});
        assert_eq!(tile.name(), "empty");
    }
}