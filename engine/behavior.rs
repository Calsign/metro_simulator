use serde::{Deserialize, Serialize};
use uom::si::time::{day, hour, minute, second};
use uom::si::u64::Time;

use crate::state::State;

#[enum_dispatch::enum_dispatch]
pub trait TriggerType: PartialEq + Eq + PartialOrd + Ord {
    fn execute(self, state: &mut State, time: u64);
}

// NOTE: all implementations of TriggerType must be listed here
#[enum_dispatch::enum_dispatch(TriggerType)]
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "type")]
#[non_exhaustive]
pub enum Trigger {
    Tick,
    AgentStartDay,
    AgentRouteStart,
    AgentRouteEnd,
    #[cfg(debug_assertions)]
    DummyTrigger,
    #[cfg(debug_assertions)]
    DoublingTrigger,
}

// This is a common place to define triggers which produce important behavior.

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Tick {}

impl TriggerType for Tick {
    fn execute(self, state: &mut State, time: u64) {
        // TODO: only re-run these when the underlying data updates
        state.update_fields().unwrap();
        state.update_collect_tiles().unwrap();

        // re-trigger every hour of simulated time
        state
            .trigger_queue
            .push_rel(self, Time::new::<hour>(1).value);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct AgentStartDay {
    pub agent: u64,
}

impl TriggerType for AgentStartDay {
    fn execute(self, state: &mut State, time: u64) {
        let agent = state.agents.get(&self.agent).expect("missing agent");
        if let Some(workplace) = &agent.workplace {
            println!("plotting morning commute for {}", agent.id);
            // morning commute to work

            let housing = agent.housing;
            let workplace = *workplace;
            let id = agent.id;

            let start_time = state.time_state.current_time;
            if let Ok(Some(route)) = state.query_route(
                housing,
                workplace,
                Some(route::CarConfig::StartWithCar),
                start_time,
            ) {
                state.trigger_queue.push(
                    AgentRouteStart {
                        agent: id,
                        route: Box::new(route),
                    },
                    start_time,
                );
            }
        }

        state
            .trigger_queue
            .push_rel(self, Time::new::<day>(1).value);
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
    fn execute(self, state: &mut State, time: u64) {
        let total_time = self.route.total_time().ceil() as u64;

        let agent = state.agents.get_mut(&self.agent).expect("missing agent");
        agent.state = agent::AgentState::Route(*self.route);

        state
            .trigger_queue
            .push_rel(AgentRouteEnd { agent: self.agent }, total_time);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct AgentRouteEnd {
    agent: u64,
}

impl TriggerType for AgentRouteEnd {
    fn execute(self, state: &mut State, time: u64) {
        let agent = state.agents.get_mut(&self.agent).expect("missing agent");
        let dest = match &agent.state {
            agent::AgentState::Route(route) => route.start(),
            _ => panic!("agent in unexpected state: {:?}", agent.state),
        };
        agent.state = agent::AgentState::Tile(dest);
    }
}

// Sample trigger implementation, demonstrates a simple recurring trigger
#[cfg(debug_assertions)]
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct DummyTrigger {}

#[cfg(debug_assertions)]
impl TriggerType for DummyTrigger {
    fn execute(self, state: &mut State, time: u64) {
        println!("executing {}", time);
        state.trigger_queue.push_rel(self, 1);
    }
}

// Used for testing. Must be defined here since enum_dispatch doesn't support crossing crate
// boundaries.
#[cfg(debug_assertions)]
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct DoublingTrigger {}

#[cfg(debug_assertions)]
impl TriggerType for DoublingTrigger {
    fn execute(self, state: &mut State, time: u64) {
        state.trigger_queue.push_rel(DoublingTrigger {}, 1);
        state.trigger_queue.push_rel(DoublingTrigger {}, 1);
    }
}
