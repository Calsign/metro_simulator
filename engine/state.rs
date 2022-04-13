use crate::config::Config;
use quadtree::Quadtree;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

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
    #[error("Route error: {0}")]
    RouteError(#[from] route::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchState {
    #[serde(skip)]
    pub fields: fields::FieldsState,
}

impl BranchState {
    pub fn default() -> Self {
        Self {
            fields: fields::FieldsState::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeafState {
    pub tile: tiles::Tile,
    #[serde(skip)]
    pub fields: fields::FieldsState,
}

impl LeafState {
    pub fn default() -> Self {
        Self {
            tile: tiles::EmptyTile {}.into(),
            fields: fields::FieldsState::new(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum SerdeFormat {
    Json,
    Toml,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct State {
    pub config: Config,
    pub qtree: Quadtree<BranchState, LeafState>,
    pub metro_lines: HashMap<u64, metro::MetroLine>,
    metro_line_counter: u64,
    pub highways: highway::Highways,
}

impl State {
    pub fn new(config: Config) -> Self {
        let qtree = Quadtree::new(LeafState::default(), config.max_depth);
        Self {
            config,
            qtree,
            metro_lines: HashMap::new(),
            metro_line_counter: 0,
            highways: highway::Highways::new(),
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

    pub fn update_fields(&mut self) -> Result<(), Error> {
        let mut fold = UpdateFieldsFold {};
        let _ = self.qtree.fold_mut(&mut fold)?;
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
        keys: Option<Vec<metro::MetroKey>>,
    ) -> u64 {
        let id = self.metro_line_counter;
        self.metro_line_counter += 1;

        let color = match color {
            Some(color) => color,
            None => metro::DEFAULT_COLORS[id as usize % metro::DEFAULT_COLORS.len()].into(),
        };

        let mut metro_line =
            metro::MetroLine::new(id, color, name, self.config.min_tile_size as f64);

        if let Some(keys) = keys {
            metro_line.set_keys(keys);
        }

        self.metro_lines.insert(id, metro_line);

        id
    }

    pub fn construct_base_route_graph_filter(
        &self,
        metro_lines: Option<HashSet<u64>>,
        highway_segments: Option<HashSet<u64>>,
    ) -> Result<route::Graph, Error> {
        let graph = route::construct_base_graph(route::BaseGraphInput {
            metro_lines: &self.metro_lines,
            highways: &self.highways,
            tile_size: self.config.min_tile_size as f64,
            max_depth: self.qtree.max_depth(),
            filter_metro_lines: metro_lines,
            filter_highway_segments: highway_segments,
            add_inferred_edges: true,
            validate_highways: false,
        })?;
        Ok(graph)
    }

    pub fn construct_base_route_graph(&self) -> Result<route::Graph, Error> {
        self.construct_base_route_graph_filter(None, None)
    }
}

struct UpdateFieldsFold {}

impl quadtree::MutFold<BranchState, LeafState, (bool, fields::FieldsState), Error>
    for UpdateFieldsFold
{
    fn fold_leaf(
        &mut self,
        leaf: &mut LeafState,
        data: &quadtree::VisitData,
    ) -> Result<(bool, fields::FieldsState), Error> {
        let changed = leaf.fields.compute_leaf(&leaf.tile, data);
        Ok((changed, leaf.fields.clone()))
    }

    fn fold_branch(
        &mut self,
        branch: &mut BranchState,
        children: &quadtree::QuadMap<(bool, fields::FieldsState)>,
        data: &quadtree::VisitData,
    ) -> Result<(bool, fields::FieldsState), Error> {
        let changed = children.values().iter().any(|(c, _)| *c);
        if changed {
            // only recompute branch if at least one of the children changed
            let fields = children.clone().map_into(&|(_, f)| f);
            branch.fields.compute_branch(&fields, data);
        }
        Ok((changed, branch.fields.clone()))
    }
}
