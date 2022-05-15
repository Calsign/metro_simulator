use serde::{Deserialize, Serialize};

use crate::agent_route_state::AgentRouteState;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentData {}

impl AgentData {
    pub fn new() -> Self {
        Self {}
    }
}

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
}

impl Agent {
    pub fn new(
        id: u64,
        data: AgentData,
        housing: quadtree::Address,
        workplace: Option<quadtree::Address>,
    ) -> Self {
        Self {
            id,
            data,
            housing,
            workplace,
            state: AgentState::Tile(housing),
        }
    }
}
