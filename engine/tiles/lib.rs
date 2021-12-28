#[enum_dispatch::enum_dispatch]
pub trait TileType {
    fn name(&self) -> &'static str;
}

#[enum_dispatch::enum_dispatch(TileType)]
#[derive(Clone, Debug, serde_derive::Serialize, serde_derive::Deserialize)]
pub enum Tile {
    EmptyTile,
    HousingTile,
    WorkplaceTile,
}

#[derive(Debug, Clone, serde_derive::Serialize, serde_derive::Deserialize)]
pub struct EmptyTile {}

impl TileType for EmptyTile {
    fn name(&self) -> &'static str {
        "empty"
    }
}

#[derive(Debug, Clone, serde_derive::Serialize, serde_derive::Deserialize)]
pub struct HousingTile {
    pub density: usize,
}

impl TileType for HousingTile {
    fn name(&self) -> &'static str {
        "housing"
    }
}

#[derive(Debug, Clone, serde_derive::Serialize, serde_derive::Deserialize)]
pub struct WorkplaceTile {
    pub density: usize,
}

impl TileType for WorkplaceTile {
    fn name(&self) -> &'static str {
        "workplace"
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
