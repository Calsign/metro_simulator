use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::agent_data::AgentData;
use crate::agent_route_state::{AgentRoutePhase, AgentRouteState, RouteType};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentState {
    /// agent is currently at a tile with the given address.
    Tile(quadtree::Address),
    /// agent is currently in transit along the given route.
    Route(AgentRouteState),
    /// agent state is currently unknown
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Agent {
    pub id: u64,
    pub data: AgentData,
    pub housing: quadtree::Address,
    pub workplace: Option<quadtree::Address>,
    pub state: AgentState,
    /// estimate of commute duration, in seconds
    pub route_lengths: HashMap<RouteType, f32>,
}

impl Agent {
    pub fn new(
        id: u64,
        data: AgentData,
        housing: quadtree::Address,
        workplace: Option<quadtree::Address>,
    ) -> Self {
        use enum_iterator::IntoEnumIterator;
        let mut route_lengths = HashMap::new();
        for route_type in RouteType::into_enum_iter() {
            route_lengths.insert(route_type, 0.0);
        }
        Self {
            id,
            data,
            housing,
            workplace,
            route_lengths,
            state: AgentState::Tile(housing),
        }
    }

    fn record_route_time(&mut self, route_type: RouteType, total_time: f32) {
        // TODO: do some fancy estimation instead of just using the previous time
        self.route_lengths.insert(route_type, total_time);
    }

    pub fn finish_route(&mut self) {
        let (route_type, total_time, route) = match &self.state {
            AgentState::Route(AgentRouteState {
                route_type,
                phase: AgentRoutePhase::Finished { total_time },
                route,
                ..
            }) => (*route_type, *total_time, route),
            _ => panic!("agent not in finished route state"),
        };

        self.state = AgentState::Tile(route.end());
        self.record_route_time(route_type, total_time);
    }

    pub fn abort_route<F: state::Fields>(
        &mut self,
        world_state: &mut route::WorldStateImpl,
        state: &state::State<F>,
    ) {
        match &self.state {
            AgentState::Route(AgentRouteState {
                route_type,
                phase:
                    AgentRoutePhase::InProgress {
                        current_edge,
                        current_edge_start,
                        current_edge_total,
                        ..
                    },
                route,
                ..
            }) => {
                let route_type = *route_type;

                // make sure to decrement the edge so that congestion totals are consistent
                let edge = &route.edges[*current_edge as usize];
                world_state.decrement_edge(edge, state);

                let total_time = current_edge_start + current_edge_total;
                self.record_route_time(route_type, total_time);
            }
            AgentState::Route(AgentRouteState {
                phase: AgentRoutePhase::Finished { .. },
                ..
            }) => {
                // this is OK
            }
            _ => panic!("agent not in in-progress route state"),
        }

        self.state = AgentState::Unknown;
    }

    pub fn average_commute_length(&self) -> f32 {
        let sum = self.route_lengths[&RouteType::CommuteToWork]
            + self.route_lengths[&RouteType::CommuteFromWork];
        sum / 2.0
    }
}
