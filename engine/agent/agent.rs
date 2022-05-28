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

    pub fn finish_route(&mut self) {
        match &self.state {
            AgentState::Route(AgentRouteState {
                route_type,
                phase: AgentRoutePhase::Finished { total_time },
                route,
                ..
            }) => {
                // TODO: do some fancy estimation instead of just using the previous time
                self.route_lengths.insert(*route_type, *total_time);
                self.state = AgentState::Tile(route.end());
            }
            _ => panic!("agent not in finished route state"),
        }
    }

    pub fn average_commute_length(&self) -> f32 {
        let sum = self.route_lengths[&RouteType::CommuteToWork]
            + self.route_lengths[&RouteType::CommuteFromWork];
        sum / 2.0
    }
}
