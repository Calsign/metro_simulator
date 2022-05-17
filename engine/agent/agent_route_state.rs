use serde::{Deserialize, Serialize};

use uom::si::time::minute;
use uom::si::u64::Time;

#[derive(Debug, Clone, Serialize, Deserialize)]
enum AgentRoutePhase {
    InProgress {
        /// the index of the current edge within the route
        current_edge: u32,
        /// the simulation time that the current edge started, relative to start_time; subsecond
        /// precision
        current_edge_start: f32,
        /// the total simulation time that the current edge will take; subsecond precision
        current_edge_total: f32,
        /// the current mode of transport
        current_mode: route::Mode,
    },
    Finished {
        /// total time taken for the agent to finish the route; subsecond precision
        total_time: f32,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRouteState {
    /// the route the agent is following
    pub route: route::Route,
    /// the simulation time at which the agent started following the route
    pub start_time: u64,
    phase: AgentRoutePhase,
}

impl AgentRouteState {
    pub fn new(
        route: route::Route,
        start_time: u64,
        world_state: &mut route::WorldStateImpl,
        state: &state::State,
    ) -> Self {
        assert_eq!(route.nodes.len(), route.edges.len() + 1);
        Self {
            start_time,
            phase: match route.edges.first() {
                Some(first) => {
                    world_state.increment_edge(first, state);

                    AgentRoutePhase::InProgress {
                        current_edge: 0,
                        current_edge_start: 0.0,
                        current_edge_total: first.cost(world_state, state, Some(start_time)) as f32,
                        current_mode: route.start_mode,
                    }
                }
                None => AgentRoutePhase::Finished { total_time: 0.0 },
            },
            route,
        }
    }

    /**
     * Advance the agent to the next edge in the route. This should only be done each time the
     * simulation time has passed the value of next_trigger.
     */
    pub fn advance(&mut self, world_state: &mut route::WorldStateImpl, state: &state::State) {
        match self.phase {
            AgentRoutePhase::InProgress {
                current_edge,
                current_edge_start,
                current_edge_total,
                current_mode,
            } => {
                let old_edge = &self.route.edges[current_edge as usize];
                world_state.decrement_edge(old_edge, state);

                let new_edge_index = current_edge + 1;
                self.phase = if new_edge_index as usize == self.route.edges.len() {
                    AgentRoutePhase::Finished {
                        total_time: current_edge_start + current_edge_total,
                    }
                } else {
                    let new_edge = &self.route.edges[new_edge_index as usize];

                    if new_edge.is_jammed(world_state, state) {
                        // the next edge is jammed, so we can't advance!
                        world_state.increment_edge(old_edge, state);

                        // wait five minutes before trying to advance
                        // TODO: would be better to wait precisely until the first car leaves the
                        // next edge
                        let wait = Time::new::<minute>(5).value as f32;

                        AgentRoutePhase::InProgress {
                            current_edge,
                            current_edge_start,
                            current_edge_total: current_edge_total + wait,
                            current_mode,
                        }
                    } else {
                        world_state.increment_edge(new_edge, state);

                        let start_time = current_edge_start + current_edge_total;

                        let cost =
                            new_edge.cost(world_state, state, Some(start_time.floor() as u64))
                                as f32;
                        assert!(cost >= 0.0);

                        AgentRoutePhase::InProgress {
                            current_edge: new_edge_index,
                            current_edge_start: start_time,
                            current_edge_total: cost,
                            current_mode: new_edge.mode_transition().unwrap_or(current_mode),
                        }
                    }
                };
            }
            AgentRoutePhase::Finished { .. } => panic!("cannot advance a finished AgentRouteState"),
        }
    }

    /**
     * If not finished, returns the next simulation time at which advance should be called.
     * If finished, returns None.
     */
    pub fn next_trigger(&self) -> Option<u64> {
        match self.phase {
            AgentRoutePhase::InProgress {
                current_edge_start,
                current_edge_total,
                ..
            } => {
                // NOTE: trigger at the first second after the end of this segment
                let extra_time = (current_edge_start + current_edge_total).ceil() as u64;
                assert!(extra_time >= 0);
                Some(self.start_time + extra_time)
            }
            AgentRoutePhase::Finished { .. } => None,
        }
    }

    /**
     * Whether the agent has finished its route.
     */
    pub fn finished(&self) -> bool {
        match self.phase {
            AgentRoutePhase::InProgress { .. } => false,
            AgentRoutePhase::Finished { .. } => true,
        }
    }

    /**
     * Sample the route key of the agent along this route at the given time.
     *
     * TODO: distance is currently unsupported and just returns 0.
     * It isn't too hard to support distance, but it isn't clear that its worth the overhead,
     * and we don't currently use it anywhere.
     */
    pub fn sample(&self, current_time: u64, state: &state::State) -> Option<route::RouteKey> {
        match self.phase {
            AgentRoutePhase::InProgress {
                current_edge,
                current_edge_start,
                current_edge_total,
                current_mode,
            } => {
                let relative_time =
                    current_time as f32 - current_edge_start - self.start_time as f32;

                let edge = &self.route.edges[current_edge as usize];
                let pred = &self.route.nodes[current_edge as usize];
                let succ = &self.route.nodes[(current_edge + 1) as usize];

                let position = edge.interpolate_position(
                    state,
                    pred,
                    succ,
                    relative_time / current_edge_total,
                );

                Some(route::RouteKey {
                    position,
                    dist: 0.0,
                    time: current_edge_start + relative_time,
                    mode: current_mode,
                })
            }
            AgentRoutePhase::Finished { .. } => None,
        }
    }
}
