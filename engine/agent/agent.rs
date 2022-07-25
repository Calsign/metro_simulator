use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use uom::si::time::hour;
use uom::si::u64::Time;

use crate::agent_data::AgentData;
use crate::agent_route_state::{AgentRoutePhase, AgentRouteState, RouteType};
use crate::common::{agent_log, agent_log_timestamp, Error};

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
    parked_car: Option<quadtree::Address>,
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
            // by default, agents start out at home
            parked_car: Some(housing),
            route_lengths,
            state: AgentState::Tile(housing),
        }
    }

    pub fn begin_route<F: state::Fields>(
        &mut self,
        route: route::Route,
        start_time: u64,
        route_type: RouteType,
        world_state: &mut route::WorldStateImpl,
        state: &state::State<F>,
    ) -> Result<Option<u64>, Error> {
        let route_state = AgentRouteState::new(
            self.id,
            route,
            start_time,
            route_type,
            world_state,
            state,
            self.parked_car,
        )?;

        self.log(|| format!("route state: {:#?}", route_state));

        let next_trigger = route_state.next_trigger();
        self.state = AgentState::Route(route_state);
        Ok(next_trigger)
    }

    fn record_route_time(&mut self, route_type: RouteType, total_time: f32) {
        // TODO: do some fancy estimation instead of just using the previous time
        self.route_lengths.insert(route_type, total_time);
    }

    pub fn finish_route(&mut self) -> Result<(), Error> {
        let (route_type, total_time, route, parked_car) = match &self.state {
            AgentState::Route(AgentRouteState {
                route_type,
                phase: AgentRoutePhase::Finished { total_time },
                route,
                parked_car,
                ..
            }) => (*route_type, *total_time, route, *parked_car),
            _ => panic!("agent not in finished route state"),
        };

        self.state = AgentState::Tile(route.end());
        self.parked_car = parked_car;
        self.record_route_time(route_type, total_time);

        Ok(())
    }

    /// Teleport an agent home if they are at work and no route could be found for returning home.
    pub fn teleport_home(&mut self, world_state: &mut route::WorldStateImpl) -> Result<(), Error> {
        assert!(matches!(
            self.state,
            AgentState::Tile(_) | AgentState::Unknown
        ));

        if let Some(parked_car) = self.parked_car {
            world_state.decrement_parking(parked_car)?;
        }
        self.parked_car = Some(self.housing);
        world_state.increment_parking(self.housing)?;
        self.state = AgentState::Tile(self.housing);

        self.record_route_time(
            RouteType::CommuteFromWork,
            Time::new::<hour>(4).value as f32,
        );

        Ok(())
    }

    pub fn abort_route(&mut self, world_state: &mut route::WorldStateImpl) -> Result<(), Error> {
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
                parked_car,
                ..
            }) => {
                let route_type = *route_type;

                match *parked_car {
                    Some(parked_car) => {
                        self.log(|| "agent already parked");

                        self.parked_car = Some(parked_car);
                    }
                    None => {
                        // teleport car to the destination

                        let destination = route.end();

                        self.log(|| {
                            format!(
                                "agent currently driving; teleporting to destination: {:?}",
                                destination
                            )
                        });

                        world_state.increment_parking(destination)?;
                        self.parked_car = Some(destination);
                    }
                }

                // make sure to decrement the edge so that congestion totals are consistent
                let edge = &route.edges[*current_edge as usize];
                world_state.decrement_edge(edge)?;

                let total_time = current_edge_start + current_edge_total;
                self.record_route_time(route_type, total_time);
            }
            AgentState::Route(AgentRouteState {
                phase: AgentRoutePhase::Finished { .. },
                parked_car,
                ..
            }) => {
                self.log(|| "route already finished");

                // this is OK
                self.parked_car = *parked_car;
            }
            _ => panic!("agent not in in-progress route state"),
        }

        self.state = AgentState::Unknown;

        Ok(())
    }

    pub fn average_commute_length(&self) -> f32 {
        let sum = self.route_lengths[&RouteType::CommuteToWork]
            + self.route_lengths[&RouteType::CommuteFromWork];
        sum / 2.0
    }

    /// How happy this agent is with their current workplace.
    /// 0.0 means they want to quit immediately and 1.0 means they definitely don't want to leave.
    pub fn workplace_happiness_score(&self) -> Option<f32> {
        self.workplace.map(|_| {
            self.data
                .expected_workplace_happiness(self.average_commute_length())
        })
    }

    pub fn parked_car(&self) -> Option<quadtree::Address> {
        match self.state {
            AgentState::Route(AgentRouteState { parked_car, .. }) => parked_car,
            _ => self.parked_car,
        }
    }

    pub fn parked_car_mut(&mut self) -> &mut Option<quadtree::Address> {
        match &mut self.state {
            AgentState::Route(AgentRouteState { parked_car, .. }) => parked_car,
            _ => &mut self.parked_car,
        }
    }

    pub fn owns_car(&self) -> bool {
        self.parked_car().is_some()
            || match &self.state {
                AgentState::Route(AgentRouteState {
                    phase:
                        AgentRoutePhase::InProgress {
                            current_mode: route::Mode::Driving,
                            ..
                        },
                    ..
                }) => true,
                _ => false,
            }
    }

    pub fn log<F, S>(&self, msg: F)
    where
        F: Fn() -> S,
        S: Into<String>,
    {
        agent_log(self.id, msg);
    }

    pub fn log_timestamp<F, S>(&self, msg: F, timestamp: u64)
    where
        F: Fn() -> S,
        S: Into<String>,
    {
        agent_log_timestamp(self.id, msg, timestamp);
    }
}
