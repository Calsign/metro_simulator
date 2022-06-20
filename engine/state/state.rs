use quadtree::Quadtree;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};

use crate::config::Config;

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
    #[error("Config error: {0}")]
    ConfigError(#[from] crate::config::Error),
}

pub trait Fields: std::fmt::Debug + Default + Clone {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchState<F: Fields> {
    #[serde(skip)]
    pub fields: F,
}

impl<F: Fields> BranchState<F> {
    pub fn default() -> Self {
        Self {
            fields: F::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeafState<F: Fields> {
    pub tile: tiles::Tile,
    #[serde(skip)]
    pub fields: F,
}

impl<F: Fields> LeafState<F> {
    pub fn default() -> Self {
        Self {
            tile: tiles::EmptyTile {}.into(),
            fields: F::default(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum SerdeFormat {
    Json,
    Toml,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct State<F: Fields> {
    pub config: Config,
    pub qtree: Quadtree<BranchState<F>, LeafState<F>>,
    pub metro_lines: BTreeMap<u64, metro::MetroLine>,
    metro_line_counter: u64,
    pub highways: highway::Highways,
    #[serde(skip)]
    pub collect_tiles: CollectTilesVisitor,
}

impl<F: Fields> State<F> {
    pub fn new(config: Config) -> Self {
        let qtree = Quadtree::new(LeafState::default(), config.max_depth);
        Self {
            config,
            qtree,
            metro_lines: BTreeMap::new(),
            metro_line_counter: 0,
            highways: highway::Highways::new(),
            collect_tiles: CollectTilesVisitor::default(),
        }
    }

    pub fn update_collect_tiles(&mut self) -> Result<(), Error> {
        self.collect_tiles.clear();
        self.qtree.visit(&mut self.collect_tiles)?;
        Ok(())
    }

    pub fn get_leaf_data<A: Into<quadtree::Address>>(
        &self,
        address: A,
        format: SerdeFormat,
    ) -> Result<String, Error> {
        let leaf = self.qtree.get_leaf(address)?;
        Ok(match format {
            SerdeFormat::Json => serde_json::to_string(leaf)?,
            SerdeFormat::Toml => toml::to_string(leaf)?,
        })
    }

    pub fn set_leaf_data<A: Into<quadtree::Address>>(
        &mut self,
        address: A,
        data: &str,
        format: SerdeFormat,
    ) -> Result<(), Error> {
        let leaf = self.qtree.get_leaf_mut(address)?;
        let decoded = match format {
            SerdeFormat::Json => serde_json::from_str(data)?,
            SerdeFormat::Toml => toml::from_str(data)?,
        };
        *leaf = decoded;
        Ok(())
    }

    pub fn add_metro_line(
        &mut self,
        name: String,
        color: Option<metro::Color>,
        speed_limit: u32,
        keys: Option<Vec<metro::MetroKey>>,
    ) -> u64 {
        let id = self.metro_line_counter;
        self.metro_line_counter += 1;

        let color = match color {
            Some(color) => color,
            None => metro::DEFAULT_COLORS[id as usize % metro::DEFAULT_COLORS.len()].into(),
        };

        let mut metro_line = metro::MetroLine::new(
            id,
            color,
            speed_limit,
            name,
            self.config.min_tile_size as f64,
        );

        if let Some(keys) = keys {
            metro_line.set_keys(keys);
        }

        self.metro_lines.insert(id, metro_line);

        id
    }
}

#[derive(Debug, Clone, Default)]
pub struct CollectTilesVisitor {
    pub total: u64,
    pub housing: Vec<quadtree::Address>,
    pub workplaces: Vec<quadtree::Address>,
    pub vacant_housing: Vec<quadtree::Address>,
    pub vacant_workplaces: Vec<quadtree::Address>,
}

impl CollectTilesVisitor {
    pub fn clear(&mut self) {
        self.total = 0;
        self.housing.clear();
        self.workplaces.clear();
        self.vacant_housing.clear();
        self.vacant_workplaces.clear();
    }
}

impl<F: Fields> quadtree::Visitor<BranchState<F>, LeafState<F>, Error> for CollectTilesVisitor {
    fn visit_branch_pre(
        &mut self,
        branch: &BranchState<F>,
        data: &quadtree::VisitData,
    ) -> Result<bool, Error> {
        Ok(true)
    }

    fn visit_leaf(&mut self, leaf: &LeafState<F>, data: &quadtree::VisitData) -> Result<(), Error> {
        use tiles::Tile::*;
        self.total += 1;
        match &leaf.tile {
            HousingTile(tiles::HousingTile { density, agents }) => {
                self.housing.push(data.address);
                if &agents.len() < density {
                    self.vacant_housing.push(data.address);
                }
            }
            WorkplaceTile(tiles::WorkplaceTile { density, agents }) => {
                self.workplaces.push(data.address);
                if &agents.len() < density {
                    self.vacant_workplaces.push(data.address);
                }
            }
            _ => (),
        }
        Ok(())
    }

    fn visit_branch_post(
        &mut self,
        branch: &BranchState<F>,
        data: &quadtree::VisitData,
    ) -> Result<(), Error> {
        Ok(())
    }
}
