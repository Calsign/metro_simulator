use serde::{Deserialize, Serialize};
use uom::si::time::{day, hour, minute, second};
use uom::si::u64::Time;

use crate::engine::{Engine, Error};

#[enum_dispatch::enum_dispatch]
pub trait TriggerType: std::fmt::Debug + PartialEq + Eq + PartialOrd + Ord {
    fn execute(self, state: &mut Engine, time: u64);
}

// NOTE: all implementations of TriggerType must be listed here
#[enum_dispatch::enum_dispatch(TriggerType)]
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "type")]
#[non_exhaustive]
pub enum Trigger {
    Tick,
    UpdateTraffic,
    AgentPlanCommuteToWork,
    AgentPlanCommuteHome,
    AgentRouteStart,
    AgentRouteAdvance,
    #[cfg(debug_assertions)]
    DummyTrigger,
    #[cfg(debug_assertions)]
    DoublingTrigger,
}

// This is a common place to define triggers which produce important behavior.

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Tick {}

impl TriggerType for Tick {
    fn execute(self, engine: &mut Engine, time: u64) {
        // TODO: only re-run these when the underlying data updates
        engine.update_fields().unwrap();
        engine.state.update_collect_tiles().unwrap();

        // re-trigger every hour of simulated time
        engine
            .trigger_queue
            .push_rel(self, Time::new::<hour>(1).value);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct UpdateTraffic {}

impl TriggerType for UpdateTraffic {
    fn execute(self, engine: &mut Engine, time: u64) {
        // try to predict traffic 30 minutes in the future
        engine.update_route_weights(Time::new::<minute>(30).value);

        // re-trigger every hour of simulated time
        engine
            .trigger_queue
            .push_rel(self, engine.world_state_history.snapshot_period());
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct AgentPlanCommuteToWork {
    pub agent: u64,
}

impl TriggerType for AgentPlanCommuteToWork {
    fn execute(self, engine: &mut Engine, time: u64) {
        let agent = engine.agents.get(&self.agent).expect("missing agent");
        let id = agent.id;
        if let Some(workplace) = &agent.workplace {
            // morning commute to work

            let query_input = route::QueryInput {
                start: agent.housing,
                end: *workplace,
                car_config: Some(route::CarConfig::StartWithCar),
            };

            let start_time = engine.time_state.current_time + AgentRouteStart::DEADLINE;

            let receiver = engine.query_route_async(query_input);
            engine.trigger_queue.push(
                AgentRouteStart {
                    agent: id,
                    receiver: Some(RouteReceiver {
                        receiver: Box::new(receiver),
                    }),
                    route_type: agent::RouteType::CommuteToWork,
                    query_input,
                },
                start_time,
            );

            // come home from work after 8 hours
            // TODO: it would be better to use estimated time or something
            // we had this originally, but it's tougher with parallelism
            engine.trigger_queue.push(
                AgentPlanCommuteHome { agent: id },
                start_time as u64 + Time::new::<hour>(8).value,
            );
        }

        engine
            .trigger_queue
            .push_rel(self, Time::new::<day>(1).value);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct AgentPlanCommuteHome {
    pub agent: u64,
}

impl TriggerType for AgentPlanCommuteHome {
    fn execute(self, engine: &mut Engine, time: u64) {
        let agent = engine.agents.get(&self.agent).expect("missing agent");
        let id = agent.id;
        if let Some(workplace) = &agent.workplace {
            // commute back home from work

            let query_input = route::QueryInput {
                start: *workplace,
                end: agent.housing,
                // TODO: if a car is parked somewhere, account for it
                car_config: Some(route::CarConfig::StartWithCar),
            };

            let start_time = engine.time_state.current_time + AgentRouteStart::DEADLINE;

            let receiver = engine.query_route_async(query_input);
            engine.trigger_queue.push(
                AgentRouteStart {
                    agent: id,
                    receiver: Some(RouteReceiver {
                        receiver: Box::new(receiver),
                    }),
                    route_type: agent::RouteType::CommuteFromWork,
                    query_input,
                },
                start_time,
            );
        }
    }
}

#[derive(Debug, Clone, derivative::Derivative)]
#[derivative(PartialEq, Eq, PartialOrd, Ord)]
struct RouteReceiver {
    #[derivative(PartialEq = "ignore", PartialOrd = "ignore", Ord = "ignore")]
    receiver: Box<crossbeam::channel::Receiver<Result<Option<route::Route>, Error>>>,
}

// NOTE: if we are loading from a serialized copy, the spawned thread is dead, so we need to
// do a blocking compute from the query input.
#[derive(Debug, Clone, Serialize, Deserialize, derivative::Derivative)]
#[derivative(PartialEq, Eq, PartialOrd, Ord)]
pub struct AgentRouteStart {
    agent: u64,
    route_type: agent::RouteType,
    #[serde(skip)]
    receiver: Option<RouteReceiver>,
    #[derivative(PartialEq = "ignore", PartialOrd = "ignore", Ord = "ignore")]
    query_input: route::QueryInput,
}

impl AgentRouteStart {
    /// how long (in simulation time) we should wait before joining the calculation worker
    const DEADLINE: u64 = 5;
}

impl TriggerType for AgentRouteStart {
    fn execute(self, engine: &mut Engine, time: u64) {
        let route = match self.receiver {
            Some(receiver) => {
                // This blocks if the route has not been computed yet.
                // We can adjust how likely we are to block by twiddling the deadline.
                receiver
                    .receiver
                    .recv()
                    .expect("channel disconnected unexpectedly")
            }
            None => {
                // We don't have a receiver because the engine state was serialized between when the
                // route query was queued and now. The best we can do is compute the route here.
                engine.query_route(self.query_input)
            }
        };

        let agent = engine.agents.get_mut(&self.agent).expect("missing agent");

        if let agent::AgentState::Route(_) = agent.state {
            // the agent hasn't finished their previous route yet.
            // looks like they're sleeping at the office!
            return;
        }

        // TODO: if there's an error here we should probably do something about it
        if let Ok(Some(route)) = route {
            let route_state = agent::AgentRouteState::new(
                route,
                engine.time_state.current_time,
                self.route_type,
                &mut engine.world_state,
                &engine.state,
            );
            let next_trigger = route_state.next_trigger();
            agent.state = agent::AgentState::Route(route_state);

            if let Some(next_trigger) = next_trigger {
                engine
                    .trigger_queue
                    .push(AgentRouteAdvance { agent: self.agent }, next_trigger);
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct AgentRouteAdvance {
    agent: u64,
}

impl TriggerType for AgentRouteAdvance {
    fn execute(self, engine: &mut Engine, time: u64) {
        let agent = engine.agents.get_mut(&self.agent).expect("missing agent");
        match &mut agent.state {
            agent::AgentState::Route(route_state) => {
                route_state.advance(&mut engine.world_state, &engine.state);
                match route_state.next_trigger() {
                    Some(next_trigger) => {
                        assert!(next_trigger >= engine.time_state.current_time);
                        engine.trigger_queue.push(self, next_trigger);
                    }
                    None => {
                        agent.finish_route();
                    }
                }
            }
            _ => panic!("agent not in route state"),
        }
    }
}

// Sample trigger implementation, demonstrates a simple recurring trigger
#[cfg(debug_assertions)]
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct DummyTrigger {}

#[cfg(debug_assertions)]
impl TriggerType for DummyTrigger {
    fn execute(self, engine: &mut Engine, time: u64) {
        println!("executing {}", time);
        engine.trigger_queue.push_rel(self, 1);
    }
}

// Used for testing. Must be defined here since enum_dispatch doesn't support crossing crate
// boundaries.
#[cfg(debug_assertions)]
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct DoublingTrigger {}

#[cfg(debug_assertions)]
impl TriggerType for DoublingTrigger {
    fn execute(self, engine: &mut Engine, time: u64) {
        engine.trigger_queue.push_rel(DoublingTrigger {}, 1);
        engine.trigger_queue.push_rel(DoublingTrigger {}, 1);
    }
}
