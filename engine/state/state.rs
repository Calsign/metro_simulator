use quadtree::Quadtree;
use serde::{Deserialize, Serialize};

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
    // NOTE: i64 so that we can use i64::MIN to represent tiles that are part of the original map.
    pub creation_time: i64,
}

impl<F: Fields> Default for LeafState<F> {
    fn default() -> Self {
        Self {
            tile: tiles::EmptyTile {}.into(),
            fields: F::default(),
            creation_time: i64::MIN,
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
    pub railways: metro::Railways,
    pub highways: highway::Highways,
    pub metros: metro::Metros,
    #[serde(skip)]
    pub collect_tiles: CollectTilesVisitor,
}

impl<F: Fields> State<F> {
    pub fn new(config: Config) -> Self {
        let qtree = Quadtree::new(LeafState::default(), config.max_depth);
        Self {
            config,
            qtree,
            railways: metro::Railways::new(),
            highways: highway::Highways::new(),
            metros: metro::Metros::new(),
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

    /**
     * Insert a tile at the specified address, preserving the existing tile. Will split the tile if
     * needed, or possibly place the new tile in an adjacent empty tile instead.
     *
     * Returns the addresses of the existing tile (which may have been moved) and the new tile,
     * (existing_tile, new_tile). The existing tile may be None if it was empty and replaced, and
     * the new tile may be none if the qtree depth was too high for a new tile to be added.
     *
     * This function should not be used directly, instead use Engine::insert_tile.
     */
    pub fn insert_tile<R: rand::Rng>(
        &mut self,
        address: quadtree::Address,
        tile: tiles::Tile,
        current_time: i64,
        rng: &mut R,
    ) -> Result<(Option<quadtree::Address>, Option<quadtree::Address>), Error> {
        use itertools::Itertools;
        use rand::seq::SliceRandom;

        // TODO: this could be improved by moving instead of cloning
        let current_leaf = self.qtree.get_leaf(address)?.clone();

        if let tiles::Tile::EmptyTile(_) = current_leaf.tile {
            // replace the empty tile

            self.qtree.get_leaf_mut(address)?.tile = tile;
            Ok((None, Some(address)))
        } else {
            // split the tile

            // TODO: we should be able to place the new tile into an adjacent empty tile instead of
            // splitting every time

            // choose_multiple is without replacement, which is important
            let (current_quadrant, new_quadrant) = quadtree::QUADRANTS
                .choose_multiple(rng, 2)
                .collect_tuple()
                .unwrap();

            let empty_quadrants: [LeafState<F>; 4] = Default::default();
            let mut quad_map = quadtree::QuadMap::from(empty_quadrants);
            quad_map[*current_quadrant] = LeafState {
                tile: current_leaf.tile,
                fields: F::default(),
                creation_time: current_leaf.creation_time,
            };
            quad_map[*new_quadrant] = LeafState {
                tile,
                fields: F::default(),
                creation_time: current_time,
            };

            match self.qtree.split(
                address,
                BranchState {
                    fields: current_leaf.fields,
                },
                quad_map,
            ) {
                Ok(()) => Ok((
                    Some(address.child(*current_quadrant)),
                    Some(address.child(*new_quadrant)),
                )),
                // TODO: handle this case
                Err(quadtree::Error::MaxDepthExceeded(_)) => Ok((Some(address), None)),
                Err(err) => Err(err.into()),
            }
        }
    }

    pub fn apply_change_set(&mut self) {
        self.highways.apply_change_set();
        self.railways.apply_change_set();
    }

    pub fn advance_network_tombstones(&mut self) {
        self.highways.advance_tombstones();
        self.railways.advance_tombstones();
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
        _branch: &BranchState<F>,
        _data: &quadtree::VisitData,
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
        _branch: &BranchState<F>,
        _data: &quadtree::VisitData,
    ) -> Result<(), Error> {
        Ok(())
    }
}
