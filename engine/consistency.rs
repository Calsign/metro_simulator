use std::collections::HashMap;

use quadtree::VisitData;
use state::{BranchState, LeafState};

use crate::engine::Engine;
use crate::fields::FieldsState;

#[derive(thiserror::Error, Debug)]
pub enum ConsistencyError {
    #[error("Agent consistency error: {0}")]
    AgentError(String),
    #[error("Tile consistency error: {0}")]
    TileError(String),
    #[error("Traffic error: {0}")]
    TrafficError(String),
    #[error("Traffic errors: {0:?}")]
    TrafficErrors(Vec<String>),
    #[error("Parking error: {0}")]
    ParkingError(String),
    #[error("Parking errors: {0:?}")]
    ParkingErrors(Vec<String>),
}

impl Engine {
    /**
     * Returns an error if the internal data structures are in an inconsistent state. Calling this
     * is very expensive, so it should only be used for debugging, or if we are already panicking
     * due to a data inconsistency error.
     */
    pub fn consistency_check(&self) -> Result<(), ConsistencyError> {
        self.agent_housing_workplace_consistency_check()?;
        self.traffic_consistency_check()?;
        self.parking_consistency_check()?;
        Ok(())
    }

    fn agent_housing_workplace_consistency_check(&self) -> Result<(), ConsistencyError> {
        let mut find_agents = FindAgentVisitor {
            agents: &self.agents,
            housing: HashMap::new(),
            workplaces: HashMap::new(),
        };
        self.state.qtree.visit(&mut find_agents)?;

        for (id, agent) in &self.agents {
            if *id != agent.id {
                return Err(ConsistencyError::AgentError(format!(
                    "agent id does not match key in map: {} != {}",
                    *id, agent.id
                )));
            }

            let housing = self.state.qtree.get_leaf(agent.housing).map_err(|_| {
                ConsistencyError::TileError(format!(
                    "missing leaf (housing) at {:?} for agent {}; agent housing is at {:?}",
                    agent.housing,
                    agent.id,
                    find_agents.housing.get(&agent.id),
                ))
            })?;
            match &housing.tile {
                tiles::Tile::HousingTile(tiles::HousingTile { agents, .. }) => {
                    if !agents.contains(&agent.id) {
                        return Err(ConsistencyError::AgentError(format!(
                            "agent {} says {:?} is housing, but tile does not list agent; it has only {:?}; agent housing is at {:?}",
                            agent.id, agent.housing, agents, find_agents.housing.get(&agent.id)
                        )));
                    }
                }
                tile => return Err(ConsistencyError::AgentError(format!(
                    "expected housing at {:?} for agent {}, but found {:?}; agent housing is at {:?}",
                    agent.housing, agent.id, tile, find_agents.housing.get(&agent.id),
                ))),
            }

            if let Some(workplace_address) = agent.workplace {
                let workplace = self.state.qtree.get_leaf(workplace_address).map_err(|_| {
                    ConsistencyError::TileError(format!(
                        "missing leaf (workplace) at {:?} for agent {}; agent workplace is at {:?}",
                        workplace_address,
                        agent.id,
                        find_agents.workplaces.get(&agent.id),
                    ))
                })?;
                match &workplace.tile {
                    tiles::Tile::WorkplaceTile(tiles::WorkplaceTile { agents, .. }) => {
                        if !agents.contains(&agent.id) {
                            return Err(ConsistencyError::AgentError(format!(
                                "agent {} says {:?} is workplace, but tile does not list agent; it has only {:?}; agent workplace is at {:?}",
                                agent.id, workplace_address, agents, find_agents.workplaces.get(&agent.id),
                            )));
                        }
                    }
                    tile => return Err(ConsistencyError::AgentError(format!(
                        "expected workplace at {:?} for agent {}, but found {:?}; agent workplace is at {:?}",
                        workplace_address,
                        agent.id,
                        tile,
                        find_agents.workplaces.get(&agent.id),
                    ))),
                }
            }
        }

        // TODO: check consistency in the other direction

        Ok(())
    }

    fn traffic_consistency_check(&self) -> Result<(), ConsistencyError> {
        // re-construct traffic state so that we can compare to real world state
        let mut world_state_comparison = route::WorldStateImpl::new(&self.state.config);

        for agent in self.agents.values() {
            if let agent::AgentState::Route(agent::AgentRouteState {
                route,
                phase: agent::AgentRoutePhase::InProgress { current_edge, .. },
                ..
            }) = &agent.state
            {
                world_state_comparison
                    .increment_edge_no_parking(route.edges.get(*current_edge as usize).ok_or_else(
                        || {
                            ConsistencyError::TrafficError(format!(
                                "route edge out of bounds: {}",
                                current_edge
                            ))
                        },
                    )?)
                    .expect("should be impossible");
            }
        }

        let traffic_errs = self.world_state.check_same_traffic(&world_state_comparison);
        if traffic_errs.len() > 0 {
            return Err(ConsistencyError::TrafficErrors(traffic_errs));
        }

        Ok(())
    }

    fn parking_consistency_check(&self) -> Result<(), ConsistencyError> {
        // re-construct parking state so that we can compare to real world state
        let mut world_state_comparison = route::WorldStateImpl::new(&self.state.config);

        for agent in self.agents.values() {
            let parked_car = agent.parked_car();

            // make sure current mode of travel is consistent
            if let agent::AgentState::Route(agent::AgentRouteState {
                phase: agent::AgentRoutePhase::InProgress { current_mode, .. },
                ..
            }) = &agent.state
            {
                if parked_car.is_some() {
                    if *current_mode == route::Mode::Driving {
                        return Err(ConsistencyError::ParkingError(format!(
                            "car is parked but still driving"
                        )));
                    }
                } else {
                    // TODO: we will need to update this check once not all agents have cars
                    if *current_mode != route::Mode::Driving {
                        return Err(ConsistencyError::ParkingError(format!(
                            "car isn't parked but {} instead of driving",
                            *current_mode
                        )));
                    }
                }
            }

            if let Some(parked_car) = parked_car {
                // add parking to re-constructed parking state
                world_state_comparison
                    .increment_parking(parked_car)
                    .expect("should be impossible");
            }
        }

        let parking_errs = self.world_state.check_same_parking(&world_state_comparison);
        if parking_errs.len() > 0 {
            return Err(ConsistencyError::ParkingErrors(parking_errs));
        }

        Ok(())
    }
}

#[derive(Debug, Clone)]
struct FindAgentVisitor<'a> {
    agents: &'a HashMap<u64, agent::Agent>,
    housing: HashMap<u64, quadtree::Address>,
    workplaces: HashMap<u64, quadtree::Address>,
}

impl<'a> quadtree::Visitor<BranchState<FieldsState>, LeafState<FieldsState>, ConsistencyError>
    for FindAgentVisitor<'a>
{
    fn visit_branch_pre(
        &mut self,
        _branch: &BranchState<FieldsState>,
        _data: &VisitData,
    ) -> Result<bool, ConsistencyError> {
        Ok(true)
    }

    fn visit_leaf(
        &mut self,
        leaf: &LeafState<FieldsState>,
        data: &VisitData,
    ) -> Result<(), ConsistencyError> {
        match &leaf.tile {
            tiles::Tile::HousingTile(tiles::HousingTile { density, agents }) => {
                if agents.len() > *density {
                    return Err(ConsistencyError::TileError(format!(
                        "housing tile at {:?} has too many agents; density: {}, agents: {:?}",
                        data.address, density, agents
                    )));
                }
                for agent in agents {
                    if let Some(existing) = self.housing.insert(*agent, data.address) {
                        return Err(ConsistencyError::TileError(format!(
                            "two tiles are housing for agent {}: {:?} and {:?}; agent housing is {:?}",
                            agent, existing, data.address, self.agents.get(&agent).map(|a| a.housing)
                        )));
                    }
                }
            }
            tiles::Tile::WorkplaceTile(tiles::WorkplaceTile { density, agents }) => {
                if agents.len() > *density {
                    return Err(ConsistencyError::TileError(format!(
                        "workplace tile at {:?} has too many agents; density: {}, agents: {:?}",
                        data.address, density, agents
                    )));
                }
                for agent in agents {
                    if let Some(existing) = self.workplaces.insert(*agent, data.address) {
                        return Err(ConsistencyError::TileError(format!(
                            "two tiles are workplace for agent {}: {:?} and {:?}; agent workplace is {:?}",
                            agent, existing, data.address, self.agents.get(&agent).map(|a| a.workplace),
                        )));
                    }
                }
            }
            _ => (),
        }
        Ok(())
    }

    fn visit_branch_post(
        &mut self,
        _branch: &BranchState<FieldsState>,
        _data: &VisitData,
    ) -> Result<(), ConsistencyError> {
        Ok(())
    }
}
