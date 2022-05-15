use serde::{Deserialize, Serialize};
use uom::si::time::{day, hour, minute, second};
use uom::si::u64::Time;

use crate::engine::Engine;

#[enum_dispatch::enum_dispatch]
pub trait TriggerType: PartialEq + Eq + PartialOrd + Ord {
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
        engine.state.update_fields().unwrap();
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
            .push_rel(self, Time::new::<minute>(30).value);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct AgentPlanCommuteToWork {
    pub agent: u64,
}

impl TriggerType for AgentPlanCommuteToWork {
    fn execute(self, engine: &mut Engine, time: u64) {
        let agent = engine.agents.get(&self.agent).expect("missing agent");
        if let Some(workplace) = &agent.workplace {
            // morning commute to work

            let housing = agent.housing;
            let workplace = *workplace;
            let id = agent.id;

            let start_time = engine.time_state.current_time;
            if let Ok(Some(route)) =
                engine.query_route(housing, workplace, Some(route::CarConfig::StartWithCar))
            {
                // TODO: do something better than using the estimated time
                let estimated_total_time = route.total_cost();
                engine.trigger_queue.push(
                    AgentRouteStart {
                        agent: id,
                        route: Box::new(route),
                    },
                    start_time,
                );
                // come home from work after 8 hours
                engine.trigger_queue.push(
                    AgentPlanCommuteHome { agent: id },
                    start_time + estimated_total_time.ceil() as u64 + Time::new::<hour>(8).value,
                );
            }
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
        if let Some(workplace) = &agent.workplace {
            // commute back home from work

            let housing = agent.housing;
            let workplace = *workplace;
            let id = agent.id;

            let start_time = engine.time_state.current_time;
            if let Ok(Some(route)) = engine.query_route(
                workplace,
                housing,
                // TODO: if a car is parked somewhere, account for it
                Some(route::CarConfig::StartWithCar),
            ) {
                engine.trigger_queue.push(
                    AgentRouteStart {
                        agent: id,
                        route: Box::new(route),
                    },
                    start_time,
                );
            }
        }
    }
}

// NOTE: we intentionally don't compare the route.
// Hopefully this won't be an issue.
#[derive(Debug, Clone, Serialize, Deserialize, derivative::Derivative)]
#[derivative(PartialEq, Eq, PartialOrd, Ord)]
pub struct AgentRouteStart {
    agent: u64,
    #[derivative(PartialEq = "ignore", PartialOrd = "ignore", Ord = "ignore")]
    route: Box<route::Route>,
}

impl TriggerType for AgentRouteStart {
    fn execute(self, engine: &mut Engine, time: u64) {
        let agent = engine.agents.get_mut(&self.agent).expect("missing agent");
        let route_state = agent::AgentRouteState::new(
            *self.route,
            engine.time_state.current_time,
            &mut engine.route_state,
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

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct AgentRouteAdvance {
    agent: u64,
}

impl TriggerType for AgentRouteAdvance {
    fn execute(self, engine: &mut Engine, time: u64) {
        let agent = engine.agents.get_mut(&self.agent).expect("missing agent");
        match &mut agent.state {
            agent::AgentState::Route(route_state) => {
                route_state.advance(&mut engine.route_state, &engine.state);
                match route_state.next_trigger() {
                    Some(next_trigger) => {
                        engine.trigger_queue.push(self, next_trigger);
                    }
                    None => {
                        agent.state = agent::AgentState::Tile(route_state.route.end());
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
