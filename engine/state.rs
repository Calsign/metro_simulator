use crate::config::Config;
use quadtree::Quadtree;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

#[derive(Debug, Clone, serde_derive::Serialize, serde_derive::Deserialize)]
pub struct BranchState {}

#[derive(Debug, Clone, serde_derive::Serialize, serde_derive::Deserialize)]
pub struct LeafState {
    pub tile: Box<dyn tiles::Tile>,
}

impl LeafState {
    pub fn default() -> Self {
        Self {
            tile: Box::new(tiles::EmptyTile {}),
        }
    }
}

#[derive(Debug, serde_derive::Serialize, serde_derive::Deserialize)]
pub struct State {
    pub qtree: Quadtree<BranchState, LeafState>,
    pub config: Config,
}

impl State {
    pub fn new(config: Config) -> Self {
        return Self {
            qtree: Quadtree::new(LeafState::default(), config.max_depth),
            config,
        };
    }

    pub fn load(data: &str) -> Result<Self, Error> {
        return Ok(serde_json::from_str(data)?);
    }

    pub fn load_file(path: &std::path::Path) -> Result<Self, Error> {
        return Ok(Self::load(&std::fs::read_to_string(path)?)?);
    }

    pub fn dump(&self) -> Result<String, Error> {
        return Ok(serde_json::to_string(self)?);
    }

    pub fn dump_file(&self, path: &std::path::Path) -> Result<(), Error> {
        return Ok(std::fs::write(path, self.dump()?)?);
    }
}
