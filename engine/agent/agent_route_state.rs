use serde::{Deserialize, Serialize};

use uom::si::time::minute;
use uom::si::u64::Time;

use crate::common::{agent_log, Error};

#[derive(
    Debug,
    Copy,
    Clone,
    Serialize,
    Deserialize,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    enum_iterator::IntoEnumIterator,
)]
pub enum RouteType {
    CommuteToWork,
    CommuteFromWork,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentRoutePhase {
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

// TODO: The duplication of id and parked_car here is kind of bad. These associated functions should
// really be moved to Agent.
#[derive(Clone, Serialize, Deserialize)]
pub struct AgentRouteState {
    pub id: u64,
    /// the route the agent is following
    pub route: route::Route,
    /// the simulation time at which the agent started following the route
    pub start_time: u64,
    pub route_type: RouteType,
    pub phase: AgentRoutePhase,
    pub parked_car: Option<quadtree::Address>,
}

// Route output is really unwieldy by default. This cleans it up a bit.
impl std::fmt::Debug for AgentRouteState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentRouteState")
            .field("route", &"<elided>")
            .field(
                "route.edges[current_edge]",
                &match self.phase {
                    AgentRoutePhase::InProgress { current_edge, .. } => {
                        Some(self.route.edges.get(current_edge as usize))
                    }
                    AgentRoutePhase::Finished { .. } => None,
                },
            )
            .field(
                "route.edges[current_edge + 1]",
                &match self.phase {
                    AgentRoutePhase::InProgress { current_edge, .. } => {
                        Some(self.route.edges.get(current_edge as usize + 1))
                    }
                    AgentRoutePhase::Finished { .. } => None,
                },
            )
            .field("route.start()", &self.route.start())
            .field("route.end()", &self.route.end())
            .field("route.edges.len()", &self.route.edges.len())
            .field("route.query_input", &self.route.query_input)
            .field("start_time", &self.start_time)
            .field("route_type", &self.route_type)
            .field("phase", &self.phase)
            .field("parked_car", &self.parked_car)
            .finish_non_exhaustive()
    }
}

impl AgentRouteState {
    pub fn new<F: state::Fields>(
        id: u64,
        route: route::Route,
        start_time: u64,
        route_type: RouteType,
        world_state: &mut route::WorldStateImpl,
        state: &state::State<F>,
        parked_car: Option<quadtree::Address>,
    ) -> Result<Self, Error> {
        assert_eq!(route.nodes.len(), route.edges.len() + 1);
        let mut ret = Self {
            id,
            start_time,
            phase: match route.edges.first() {
                Some(first) => {
                    world_state.increment_edge(first, state)?;

                    AgentRoutePhase::InProgress {
                        current_edge: 0,
                        current_edge_start: 0.0,
                        current_edge_total: first.cost(world_state, state, Some(start_time)) as f32,
                        current_mode: first.mode_transition().unwrap_or(route.start_mode),
                    }
                }
                None => AgentRoutePhase::Finished { total_time: 0.0 },
            },
            route_type,
            route,
            parked_car,
        };

        // maybe adjust parked car
        if let Some(first) = ret.route.edges.first() {
            Self::handle_parking(id, &mut ret.parked_car, first)?;
        }

        Ok(ret)
    }

    fn handle_parking(
        id: u64,
        parked_car: &mut Option<quadtree::Address>,
        edge: &route::Edge,
    ) -> Result<(), Error> {
        match edge {
            route::Edge::ModeTransition {
                from: route::Mode::Walking,
                to: route::Mode::Driving,
                address,
                ..
            } => {
                agent_log(id, || format!("un-parking at {:?}", address));

                // NOTE: we might not have parked_car == Some(address) because the map is dynamic
                assert!(parked_car.is_some());
                *parked_car = None;
            }
            route::Edge::ModeTransition {
                from: route::Mode::Driving,
                to: route::Mode::Walking,
                address,
                ..
            } => {
                agent_log(id, || format!("parking at {:?}", address));

                assert_eq!(*parked_car, None);
                *parked_car = Some(*address);
            }
            _ => (),
        }
        Ok(())
    }

    /**
     * Advance the agent to the next edge in the route. This should only be done each time the
     * simulation time has passed the value of next_trigger.
     */
    pub fn advance<F: state::Fields>(
        &mut self,
        world_state: &mut route::WorldStateImpl,
        state: &state::State<F>,
    ) -> Result<(), Error> {
        match self.phase {
            AgentRoutePhase::InProgress {
                current_edge,
                current_edge_start,
                current_edge_total,
                current_mode,
            } => {
                let old_edge = &self.route.edges[current_edge as usize];
                world_state.decrement_edge(old_edge, state)?;

                let new_edge_index = current_edge + 1;
                self.phase = if new_edge_index as usize == self.route.edges.len() {
                    AgentRoutePhase::Finished {
                        total_time: current_edge_start + current_edge_total,
                    }
                } else {
                    let new_edge = &self.route.edges[new_edge_index as usize];

                    if new_edge.is_jammed(world_state, state) {
                        agent_log(self.id, || "jammed; restarting current edge");

                        // the next edge is jammed, so we can't advance!
                        world_state.increment_edge(old_edge, state)?;

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
                        world_state.increment_edge(new_edge, state)?;

                        // maybe adjust parked car
                        Self::handle_parking(self.id, &mut self.parked_car, new_edge)?;

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

        Ok(())
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
                assert!(current_edge_start >= 0.0);
                assert!(current_edge_total >= 0.0);
                let extra_time = (current_edge_start + current_edge_total).ceil() as u64;
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
    pub fn sample<F: state::Fields>(
        &self,
        current_time: u64,
        state: &state::State<F>,
    ) -> Option<route::RouteKey> {
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
