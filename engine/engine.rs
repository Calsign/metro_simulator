use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};

use serde::{Deserialize, Serialize};
use uom::si::time::hour;
use uom::si::u64::Time;

use crate::fields::{FieldsComputationData, FieldsState};
use crate::time_state::TimeState;
use crate::trigger::TriggerQueue;

/// number of times to record traffic history per day
pub const WORLD_STATE_HISTORY_SNAPSHOTS: usize = 48;

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
    #[error("Quadtree error: {0}")]
    QuadtreeError(#[from] quadtree::Error),
}

#[derive(Debug)]
pub struct BaseGraph {
    base_graph: once_cell::sync::OnceCell<route::Graph>,
    per_thread: thread_local::ThreadLocal<std::cell::RefCell<route::Graph>>,
}

impl Default for BaseGraph {
    fn default() -> Self {
        Self {
            base_graph: once_cell::sync::OnceCell::new(),
            per_thread: thread_local::ThreadLocal::new(),
        }
    }
}

impl Clone for BaseGraph {
    fn clone(&self) -> Self {
        Self {
            base_graph: self.base_graph.clone(),
            per_thread: thread_local::ThreadLocal::new(),
        }
    }
}

impl BaseGraph {
    pub fn construct_base_graph_filter<F: state::Fields>(
        state: &state::State<F>,
        metro_lines: Option<HashSet<u64>>,
        highway_segments: Option<HashSet<u64>>,
    ) -> Result<route::Graph, Error> {
        let graph = route::construct_base_graph(route::BaseGraphInput {
            state,
            filter_metro_lines: metro_lines,
            filter_highway_segments: highway_segments,
            add_inferred_edges: true,
            validate_highways: false,
        })?;
        Ok(graph)
    }

    pub fn construct_base_graph<F: state::Fields>(
        state: &state::State<F>,
    ) -> Result<route::Graph, Error> {
        Self::construct_base_graph_filter(state, None, None)
    }

    pub fn get_base_graph<F: state::Fields>(&self, state: &state::State<F>) -> &route::Graph {
        self.base_graph
            .get_or_init(|| Self::construct_base_graph(state).unwrap())
    }

    pub fn get_base_graph_mut<F: state::Fields>(
        &mut self,
        state: &state::State<F>,
    ) -> &mut route::Graph {
        // TODO: Annoying that we have to take the value out of the OnceCell and then put it back
        // in. Seems like there should be a get_or_init_mut function or equivalent.
        // NOTE: there is no race condition here because we have exclusive access to self
        let base_graph = self
            .base_graph
            .take()
            .unwrap_or_else(|| Self::construct_base_graph(state).unwrap());
        self.base_graph.set(base_graph).unwrap();
        self.base_graph.get_mut().unwrap()
    }

    /**
     * Call this every time the base graph is updated. This forces the thread-local copies to be
     * replaced, otherwise old versions will be used.
     */
    pub fn clear_thread_cache(&mut self) {
        self.per_thread.clear();
    }

    pub fn get_thread_base_graph(&self) -> std::cell::RefMut<route::Graph> {
        // TODO: This currently assumes that the base graph has been initialized, will panic if it
        // hasn't been. The alternative solution requires synchronizing the state across threads,
        // which seems not fun.
        self.per_thread
            .get_or(|| std::cell::RefCell::new(self.base_graph.get().unwrap().clone()))
            .borrow_mut()
    }

    pub fn get_stats(&self) -> Option<route::BaseGraphStats> {
        self.base_graph.get().map(|g| g.get_stats())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Engine {
    pub state: state::State<FieldsState>,
    pub world_state: route::WorldStateImpl,
    pub world_state_history: route::WorldStateHistory,
    #[serde(skip)]
    pub base_graph: Arc<RwLock<BaseGraph>>,
    pub time_state: TimeState,
    pub agents: HashMap<u64, agent::Agent>,
    agent_counter: u64,
    pub trigger_queue: TriggerQueue,
    #[serde(skip, default = "Engine::create_thread_pool")]
    pub(crate) thread_pool: threadpool::ThreadPool,
    #[serde(skip)]
    pub(crate) blurred_fields: crate::field_update::BlurredFields,
}

impl Engine {
    pub fn new(config: state::Config) -> Self {
        Self {
            state: state::State::new(config),
            world_state: route::WorldStateImpl::new(),
            world_state_history: route::WorldStateHistory::new(WORLD_STATE_HISTORY_SNAPSHOTS),
            base_graph: Arc::new(RwLock::new(BaseGraph::default())),
            time_state: TimeState::new(),
            agents: HashMap::new(),
            agent_counter: 0,
            trigger_queue: TriggerQueue::new(),
            thread_pool: Self::create_thread_pool(),
            blurred_fields: Default::default(),
        }
    }

    fn create_thread_pool() -> threadpool::ThreadPool {
        let parallelism = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1);
        threadpool::ThreadPool::new(parallelism)
    }

    pub fn set_num_threads(&mut self, num_threads: usize) {
        self.thread_pool.set_num_threads(num_threads);
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

    /**
     * When a tile moves, there are some relations, such as agents, that need to be updated
     * accordingly. Call this to patch the tile at a given address.
     */
    pub fn patch_tile(&mut self, address: quadtree::Address) -> Result<(), Error> {
        match &mut self.state.qtree.get_leaf_mut(address)?.tile {
            tiles::Tile::HousingTile(tiles::HousingTile { agents, .. }) => {
                for agent_id in agents {
                    self.agents
                        .get_mut(&agent_id)
                        .expect("missing agent")
                        .housing = address;
                }
            }
            tiles::Tile::WorkplaceTile(tiles::WorkplaceTile { agents, .. }) => {
                for agent_id in agents {
                    self.agents
                        .get_mut(&agent_id)
                        .expect("missing agent")
                        .workplace = Some(address);
                }
            }
            _ => (),
        }

        Ok(())
    }

    /**
     * Forwards to State::insert_tile, but takes care of calling patch_tile. This should always be
     * used instead of calling insert_tile in State directly.
     */
    pub fn insert_tile(
        &mut self,
        address: quadtree::Address,
        tile: tiles::Tile,
    ) -> Result<(Option<quadtree::Address>, Option<quadtree::Address>), Error> {
        let new_addresses =
            self.state
                .insert_tile(address, tile, self.time_state.current_time as i64)?;
        if let (Some(existing_tile), _) = new_addresses {
            self.patch_tile(existing_tile)?;
        }
        Ok(new_addresses)
    }

    pub fn query_route(
        &self,
        query_input: route::QueryInput,
    ) -> Result<Option<route::Route>, Error> {
        // TODO: using the thread local mechanism isn't necessary here, but currently
        // route::best_route is written to accept RefMut so we have to do this
        let base_graph = self.base_graph.write().unwrap();
        // TODO: this is necessary to make sure the base graph is constructed
        let _ = base_graph.get_base_graph(&self.state);
        Ok(route::best_route(
            base_graph.get_thread_base_graph(),
            query_input,
        )?)
    }

    /**
     * Performs the same work as query_route, but passes the work off to a thread pool which sends
     * the route response on a channel to the returned reciever when it finishes.
     */
    pub fn query_route_async(
        &self,
        query_input: route::QueryInput,
    ) -> crossbeam::channel::Receiver<Result<Option<route::Route>, Error>> {
        let (sender, receiver) = crossbeam::channel::bounded(1);

        let base_graph = self.base_graph.clone();

        self.thread_pool.execute(move || {
            let base_graph = base_graph.read().unwrap();
            let route = route::best_route(base_graph.get_thread_base_graph(), query_input);
            sender.send(route.map_err(|e| e.into())).unwrap();
        });

        receiver
    }

    pub fn query_isochrone(
        &self,
        focus: quadtree::Address,
        mode: route::Mode,
    ) -> Result<route::Isochrone, Error> {
        let base_graph = self.base_graph.write().unwrap();
        // TODO: this is necessary to make sure the base graph is constructed
        let _ = base_graph.get_base_graph(&self.state);
        Ok(route::calculate_isochrone(
            base_graph.get_thread_base_graph(),
            focus,
            mode,
        )?)
    }

    pub fn query_isochrone_map(
        &self,
        focus: quadtree::Address,
        mode: route::Mode,
    ) -> Result<route::IsochroneMap, Error> {
        let isochrone = self.query_isochrone(focus, mode)?;
        Ok(route::calculate_isochrone_map(
            isochrone,
            &self.state.qtree,
            &self.state.config,
            crate::field_update::BLOCK_SIZE,
        )?)
    }

    /**
     * Re-compute the weights on the fast graph used for querying routes.
     * This makes newly computed routes use the predicted traffic conditions.
     * Horizon is how far in the future we should try to predict.
     */
    pub fn update_route_weights(&mut self, horizon: u64) {
        // update history so that future predictions will use the new data
        // NOTE: this must happen on the correct cycle, otherwise this will panic
        self.world_state_history
            .take_snapshot(&self.world_state, self.time_state.current_time);

        // predict future traffic
        let predicted_state = &self
            .world_state_history
            .get_predictor(self.time_state.current_time + horizon);

        // TODO: invalidate base graph when metros/highways change
        let mut base_graph = self.base_graph.write().unwrap();
        base_graph
            .get_base_graph_mut(&self.state)
            .update_weights(predicted_state, &self.state);
        // force the thread-local copies to be invalidated
        base_graph.clear_thread_cache();
    }

    /**
     * Only adds triggers for a freshly-generated state, so that we don't clobber triggers when
     * loading a map. We do this here so that we don't need to regenerate the map every time we
     * update the trigger queue. This should also make it easier to perform testing.
     */
    pub fn init_trigger_queue(&mut self) {
        if self.time_state.current_time == 0 {
            self.trigger_queue.push(crate::behavior::UpdateFields {}, 0);
            self.trigger_queue
                .push(crate::behavior::UpdateCollectTiles {}, 0);
            self.trigger_queue
                .push(crate::behavior::UpdateTraffic {}, 0);
            for agent in self.agents.values() {
                self.trigger_queue
                    .push(crate::behavior::AgentLifeDecisions { agent: agent.id }, 0);

                // start the day at 8 am
                self.trigger_queue.push(
                    crate::behavior::AgentPlanCommuteToWork { agent: agent.id },
                    Time::new::<hour>(8).value,
                );
            }
            self.trigger_queue
                .push(crate::behavior::WorkplaceDecisions {}, 0);
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
        engine
            .trigger_queue
            .push(crate::behavior::DoublingTrigger {}, 1);

        engine.time_state.playback_rate = 1;
        engine.time_state.paused = false;

        for i in 1..=6 {
            engine.update(1.0, f64::INFINITY);
            // make sure it added itself back to the queue twice
            assert_eq!(engine.trigger_queue.len(), 2_usize.pow(i));
        }
    }
}
