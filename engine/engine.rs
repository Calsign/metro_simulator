use once_cell::unsync::OnceCell;

use quadtree::Quadtree;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use uom::si::time::hour;
use uom::si::u64::Time;

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
    #[error("State error: {0}")]
    StateError(#[from] state::Error),
    #[error("Route error: {0}")]
    RouteError(#[from] route::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Engine {
    pub state: state::State,
    #[serde(skip)]
    route_state: route::WorldState,
    #[serde(skip)]
    base_route_graph: Option<route::Graph>,
    pub time_state: TimeState,
    pub agents: HashMap<u64, agent::Agent>,
    agent_counter: u64,
    pub trigger_queue: TriggerQueue,
}

impl Engine {
    pub fn new(config: state::Config) -> Self {
        Self {
            state: state::State::new(config),
            time_state: TimeState::new(),
            route_state: route::WorldState::new(),
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

    pub fn add_agent(
        &mut self,
        data: agent::AgentData,
        housing: quadtree::Address,
        workplace: Option<quadtree::Address>,
    ) -> u64 {
        let id = self.agent_counter;
        self.agent_counter += 1;

        match self.state.qtree.get_leaf_mut(housing) {
            Ok(state::LeafState {
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
            match self.state.qtree.get_leaf_mut(workplace) {
                Ok(state::LeafState {
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
            state: &self.state,
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

        Ok(route::best_route(
            base_graph,
            query_input,
            &self.route_state,
        )?)
    }

    fn construct_route_state(&self) -> route::WorldState {
        route::WorldState::from_routes(self.agents.values().filter_map(
            |agent| match &agent.state {
                agent::AgentState::Route(route) => Some(route),
                _ => None,
            },
        ))
    }

    /**
     * Re-compute the current route state. This should be called periodically.
     */
    pub fn update_route_state(&mut self) {
        let route_state = self.construct_route_state();
        // TODO: only do this lazily?
        self.get_base_route_graph().update_weights(&route_state);
        self.route_state = route_state;
    }

    /**
     * Returns the current route state.
     */
    pub fn get_route_state(&self) -> &route::WorldState {
        &self.route_state
    }

    /**
     * Returns a prediction of what the route state will be at the provided time.
     */
    pub fn predict_route_state(&self, time: u64) -> &route::WorldState {
        // TODO: actually predict route state
        &self.route_state
    }

    pub fn get_spline_construction_input(&self) -> route::SplineConstructionInput {
        route::SplineConstructionInput {
            metro_lines: &self.state.metro_lines,
            highways: &self.state.highways,
            state: &self.route_state,
            tile_size: self.state.config.min_tile_size as f64,
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
                // start the day at 8 am
                self.trigger_queue.push(
                    crate::behavior::AgentPlanCommuteToWork { agent: agent.id },
                    Time::new::<hour>(8).value,
                );
            }
        }
    }

    pub fn update(&mut self, elapsed: f64, time_budget: f64) {
        // try to jump forward an amount dictated by the playback rate
        let rate_step = (self.time_state.playback_rate as f64 * elapsed) as u64;
        // if we have recently skipped forward, try to catch up to the skip target time
        let target_step = (self.time_state.target_time as i64 - self.time_state.current_time as i64)
            .max(0) as u64;

        let time_step = if self.time_state.paused {
            // allow skipping to work even if we are paused
            target_step
        } else {
            // always advance at least one interval if unpaused
            // NOTE: enforces a minimum playback rate equal to the frame rate
            rate_step.max(target_step).max(1)
        };

        if time_step > 0 {
            self.advance_trigger_queue(time_step, time_budget);
        }
    }
}

#[cfg(test)]
mod trigger_tests {
    use crate::behavior::*;
    use crate::Engine;

    #[test]
    fn doubling_trigger() {
        let mut engine = Engine::new(state::Config {
            max_depth: 3,
            people_per_sim: 1,
            min_tile_size: 1,
        });

        // NOTE: all triggers have to be defined in the same crate, so we define the trigger in trigger.rs.
        engine.trigger_queue.push(DoublingTrigger {}, 1);

        engine.time_state.playback_rate = 1;
        engine.time_state.paused = false;

        for i in 1..=6 {
            engine.update(1.0, f64::INFINITY);
            // make sure it added itself back to the queue twice
            assert_eq!(engine.trigger_queue.len(), 2_usize.pow(i));
        }
    }
}
