use once_cell::unsync::OnceCell;

use quadtree::Quadtree;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

use crate::config::Config;
use crate::time_state::TimeState;
use crate::trigger::TriggerQueue;

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
    #[serde(skip)]
    pub collect_tiles: CollectTilesVisitor,
    #[serde(skip)]
    route_state: OnceCell<route::WorldState>,
    #[serde(skip)]
    base_route_graph: Option<route::Graph>,
    pub time_state: TimeState,
    pub agents: HashMap<u64, agent::Agent>,
    agent_counter: u64,
    pub trigger_queue: TriggerQueue,
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
            collect_tiles: CollectTilesVisitor::default(),
            time_state: TimeState::new(),
            route_state: OnceCell::new(),
            base_route_graph: None,
            agents: HashMap::new(),
            agent_counter: 0,
            trigger_queue: TriggerQueue::new(),
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

    pub fn add_agent(
        &mut self,
        data: agent::AgentData,
        housing: quadtree::Address,
        workplace: Option<quadtree::Address>,
    ) -> u64 {
        let id = self.agent_counter;
        self.agent_counter += 1;

        match self.qtree.get_leaf_mut(housing) {
            Ok(LeafState {
                tile: tiles::Tile::HousingTile(tiles::HousingTile { density, agents }),
                ..
            }) => {
                assert!(agents.len() < *density);
                agents.push(id);
            }
            Ok(tile) => panic!(
                "missing housing tile at {:?}, found tile: {:?}",
                housing, tile
            ),
            Err(err) => panic!(
                "missing housing tile at {:?}, no tile found, error: {:?}",
                housing, err
            ),
        };

        if let Some(workplace) = workplace {
            match self.qtree.get_leaf_mut(workplace) {
                Ok(LeafState {
                    tile: tiles::Tile::WorkplaceTile(tiles::WorkplaceTile { density, agents }),
                    ..
                }) => {
                    assert!(agents.len() < *density);
                    agents.push(id);
                }
                Ok(tile) => panic!(
                    "missing workplace tile at {:?}, found tile: {:?}",
                    workplace, tile
                ),
                Err(err) => panic!(
                    "missing workplace tile at {:?}, no tile found, error: {:?}",
                    workplace, err
                ),
            }
        }

        self.agents
            .insert(id, agent::Agent::new(id, data, housing, workplace));

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

    pub fn get_base_route_graph(&mut self) -> &mut route::Graph {
        // TODO: invalidate base graph when metros/highways change
        // also want to have separate instances per thread
        if let None = &self.base_route_graph {
            self.base_route_graph = Some(self.construct_base_route_graph().unwrap());
        }
        self.base_route_graph.as_mut().unwrap()
    }

    pub fn query_route(
        &mut self,
        start: quadtree::Address,
        end: quadtree::Address,
        car_config: Option<route::CarConfig>,
        start_time: u64,
    ) -> Result<Option<route::Route>, Error> {
        let query_input = route::QueryInput {
            start,
            end,
            car_config,
            start_time,
        };

        // TODO: borrowing issues, de-duplicate these
        let base_graph = {
            if let None = &self.base_route_graph {
                self.base_route_graph = Some(self.construct_base_route_graph().unwrap());
            }
            self.base_route_graph.as_mut().unwrap()
        };
        let state = &self.route_state.get_or_init(|| route::WorldState::new());

        Ok(route::best_route(base_graph, query_input, state)?)
    }

    pub fn get_route_state(&self) -> &route::WorldState {
        &self.route_state.get_or_init(|| route::WorldState::new())
    }

    pub fn get_spline_construction_input(&self) -> route::SplineConstructionInput {
        route::SplineConstructionInput {
            metro_lines: &self.metro_lines,
            highways: &self.highways,
            state: &self.get_route_state(),
            tile_size: self.config.min_tile_size as f64,
        }
    }

    /**
     * Only adds triggers for a freshly-generated state, so that we don't clobber triggers when
     * loading a map. We do this here so that we don't need to regenerate the map every time we
     * update the trigger queue. This should also make it easier to perform testing.
     */
    pub fn init_trigger_queue(&mut self) {
        if self.time_state.current_time == 0 {
            self.trigger_queue.push(crate::behavior::Tick {}, 0);
            for agent in self.agents.values() {
                self.trigger_queue
                    .push(crate::behavior::AgentStartDay { agent: agent.id }, 0);
            }
        }
    }

    pub fn update(&mut self, elapsed: f64) {
        self.time_state.update(elapsed);
        if !self.time_state.paused {
            self.advance_trigger_queue();
        }
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

#[derive(Debug, Clone, Default)]
pub struct CollectTilesVisitor {
    pub housing: Vec<quadtree::Address>,
    pub workplaces: Vec<quadtree::Address>,
}

impl CollectTilesVisitor {
    pub fn clear(&mut self) {
        self.housing.clear();
        self.workplaces.clear();
    }
}

impl quadtree::Visitor<BranchState, LeafState, Error> for CollectTilesVisitor {
    fn visit_branch_pre(
        &mut self,
        branch: &BranchState,
        data: &quadtree::VisitData,
    ) -> Result<bool, Error> {
        Ok(true)
    }

    fn visit_leaf(&mut self, leaf: &LeafState, data: &quadtree::VisitData) -> Result<(), Error> {
        use tiles::Tile::*;
        match leaf.tile {
            HousingTile(_) => self.housing.push(data.address),
            WorkplaceTile(_) => self.workplaces.push(data.address),
            _ => (),
        }
        Ok(())
    }

    fn visit_branch_post(
        &mut self,
        branch: &BranchState,
        data: &quadtree::VisitData,
    ) -> Result<(), Error> {
        Ok(())
    }
}

#[cfg(test)]
mod trigger_tests {
    use engine::behavior::*;
    use engine::config::Config;
    use engine::state::State;

    #[test]
    fn doubling_trigger() {
        let mut state = State::new(Config {
            max_depth: 3,
            people_per_sim: 1,
            min_tile_size: 1,
        });

        // NOTE: all triggers have to be defined in the same crate, so we define the trigger in trigger.rs.
        state.trigger_queue.push(DoublingTrigger {}, 1);

        state.time_state.playback_rate = 1;
        state.time_state.paused = false;

        for i in 1..=6 {
            state.update(1.0);
            // make sure it added itself back to the queue twice
            assert_eq!(state.trigger_queue.len(), 2_usize.pow(i));
        }
    }
}
