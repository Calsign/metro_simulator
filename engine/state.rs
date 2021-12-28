use crate::config::Config;
use quadtree::Quadtree;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),
    #[error("TOML serialization error: {0}")]
    TomlSerError(#[from] toml::ser::Error),
    #[error("TOML deserialization error: {0}")]
    TomlDeError(#[from] toml::de::Error),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Quadtree error: {0}")]
    QuadtreeError(#[from] quadtree::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchState {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeafState {
    pub tile: tiles::Tile,
}

impl LeafState {
    pub fn default() -> Self {
        Self {
            tile: tiles::EmptyTile {}.into(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct State {
    pub config: Config,
    pub qtree: Quadtree<BranchState, LeafState>,
    pub metro_lines: HashMap<u64, metro::MetroLine>,
}

impl State {
    pub fn new(config: Config) -> Self {
        let qtree = Quadtree::new(LeafState::default(), config.max_depth);
        Self {
            config,
            qtree,
            metro_lines: HashMap::new(),
        }
    }

    pub fn load(data: &str) -> Result<Self, Error> {
        Ok(serde_json::from_str(data)?)
    }

    pub fn load_file(path: &std::path::Path) -> Result<Self, Error> {
        Ok(Self::load(&std::fs::read_to_string(path)?)?)
    }

    pub fn dump(&self) -> Result<String, Error> {
        Ok(serde_json::to_string(self)?)
    }

    pub fn dump_file(&self, path: &std::path::Path) -> Result<(), Error> {
        Ok(std::fs::write(path, self.dump()?)?)
    }

    pub fn get_leaf_json<A: Into<quadtree::Address>>(&self, address: A) -> Result<String, Error> {
        Ok(serde_json::to_string(self.qtree.get_leaf(address)?)?)
    }

    pub fn set_leaf_json<A: Into<quadtree::Address>>(
        &mut self,
        address: A,
        json: &str,
    ) -> Result<(), Error> {
        let leaf = self.qtree.get_leaf_mut(address)?;
        *leaf = serde_json::from_str(json)?;
        Ok(())
    }
}
