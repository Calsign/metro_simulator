use std::collections::HashMap;

use quadtree::{Address, VisitData};
use state::{BranchState, LeafState};

use crate::engine::{Engine, Error};
use crate::fields::FieldsState;

impl Engine {
    /**
     * Panics if the internal data structures are in an inconsistent state, ideally with a useful
     * error message. Calling this is very expensive, so it should only be used for debugging, or if
     * we are already panicking due to a data inconsistency error.
     */
    pub fn consistency_check(&self) {
        let mut find_agents = FindAgentVisitor {
            agents: &self.agents,
            housing: HashMap::new(),
            workplaces: HashMap::new(),
        };
        self.state.qtree.visit(&mut find_agents).unwrap();

        for (id, agent) in &self.agents {
            assert_eq!(*id, agent.id, "agent id does not match key in map");

            let housing = self
                .state
                .qtree
                .get_leaf(agent.housing)
                .unwrap_or_else(|_| {
                    panic!(
                        "missing leaf (housing) at {:?} for agent {}; agent housing is at {:?}",
                        agent.housing,
                        agent.id,
                        find_agents.housing.get(&agent.id),
                    )
                });
            match &housing.tile {
                tiles::Tile::HousingTile(tiles::HousingTile { agents, .. }) => {
                    assert!(
                        agents.contains(&agent.id),
                        "agent {} says {:?} is housing, but tile does not list agent; it has only {:?}; agent housing is at {:?}",
                        agent.id, agent.housing, agents, find_agents.housing.get(&agent.id),
                    );
                }
                tile => panic!(
                    "expected housing at {:?} for agent {}, but found {:?}; agent housing is at {:?}",
                    agent.housing, agent.id, tile, find_agents.housing.get(&agent.id),
                ),
            }

            if let Some(workplace_address) = agent.workplace {
                let workplace = self
                    .state
                    .qtree
                    .get_leaf(workplace_address)
                    .unwrap_or_else(|_| {
                        panic!(
                            "missing leaf (workplace) at {:?} for agent {}; agent workplace is at {:?}",
                            workplace_address, agent.id, find_agents.workplaces.get(&agent.id),
                        )
                    });
                match &workplace.tile {
                    tiles::Tile::WorkplaceTile(tiles::WorkplaceTile { agents, .. }) => {
                        assert!(
                            agents.contains(&agent.id),
                            "agent {} says {:?} is workplace, but tile does not list agent; it has only {:?}; agent workplace is at {:?}",
                            agent.id, workplace_address, agents, find_agents.workplaces.get(&agent.id),
                        );
                    }
                    tile => panic!(
                        "expected workplace at {:?} for agent {}, but found {:?}; agent workplace is at {:?}",
                        workplace_address,
                        agent.id,
                        tile,
                        find_agents.workplaces.get(&agent.id),
                    ),
                }
            }
        }

        // TODO: check consistency in the other direction
    }
}

#[derive(Debug, Clone)]
struct FindAgentVisitor<'a> {
    agents: &'a HashMap<u64, agent::Agent>,
    housing: HashMap<u64, quadtree::Address>,
    workplaces: HashMap<u64, quadtree::Address>,
}

impl<'a> quadtree::Visitor<BranchState<FieldsState>, LeafState<FieldsState>, Error>
    for FindAgentVisitor<'a>
{
    fn visit_branch_pre(
        &mut self,
        branch: &BranchState<FieldsState>,
        data: &VisitData,
    ) -> Result<bool, Error> {
        Ok(true)
    }

    fn visit_leaf(&mut self, leaf: &LeafState<FieldsState>, data: &VisitData) -> Result<(), Error> {
        match &leaf.tile {
            tiles::Tile::HousingTile(tiles::HousingTile { density, agents }) => {
                for agent in agents {
                    if let Some(existing) = self.housing.insert(*agent, data.address) {
                        panic!(
                            "two tiles are housing for agent {}: {:?} and {:?}; agent housing is {:?}",
                            agent, existing, data.address, self.agents.get(&agent).map(|a| a.housing)
                        );
                    }
                }
            }
            tiles::Tile::WorkplaceTile(tiles::WorkplaceTile { density, agents }) => {
                for agent in agents {
                    if let Some(existing) = self.workplaces.insert(*agent, data.address) {
                        panic!(
                            "two tiles are workplace for agent {}: {:?} and {:?}; agent workplace is {:?}",
                            agent, existing, data.address, self.agents.get(&agent).map(|a| a.workplace),
                        );
                    }
                }
            }
            _ => (),
        }
        Ok(())
    }

    fn visit_branch_post(
        &mut self,
        branch: &BranchState<FieldsState>,
        data: &VisitData,
    ) -> Result<(), Error> {
        Ok(())
    }
}
