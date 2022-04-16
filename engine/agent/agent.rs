use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentData {}

impl AgentData {
    pub fn new() -> Self {
        Self {}
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentState {
    /// Agent is currently at a tile with the given address.
    Tile(quadtree::Address),
    /// Agent is currently in transit along the given route.
    Route(route::Route),
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
