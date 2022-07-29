use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use uom::si::time::{day, hour, minute};
use uom::si::u64::Time;

use crate::engine::{Engine, Error};

#[enum_dispatch::enum_dispatch]
pub trait TriggerType: std::fmt::Debug + PartialEq + Eq + PartialOrd + Ord {
    /// Execute the trigger. May schedule additional triggers, including the same trigger for a
    /// future re-execution.
    fn execute(self, state: &mut Engine, time: u64) -> Result<(), Error>;

    /// Optionally return a string with extra context for debugging purposes.
    fn debug_context(&self, state: &Engine) -> Option<String>;
}

// NOTE: all implementations of TriggerType must be listed here
#[allow(clippy::enum_variant_names)]
#[enum_dispatch::enum_dispatch(TriggerType)]
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, enum_kinds::EnumKind,
)]
#[enum_kind(
    TriggerKind,
    derive(
        PartialOrd,
        Ord,
        Serialize,
        Deserialize,
        enum_iterator::IntoEnumIterator
    )
)]
#[serde(tag = "type")]
#[non_exhaustive]
pub enum Trigger {
    UpdateFields,
    UpdateCollectTiles,
    UpdateTraffic,
    AgentPlanCommuteToWork,
    AgentPlanCommuteHome,
    AgentRouteStart,
    AgentRouteAdvance,
    AgentLifeDecisions,
    WorkplaceDecisions,
    DummyTrigger,
    DoublingTrigger,
}

// This is a common place to define triggers which produce important behavior.

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct UpdateFields {}

impl TriggerType for UpdateFields {
    fn execute(self, engine: &mut Engine, _time: u64) -> Result<(), Error> {
        // TODO: only re-run these when the underlying data updates
        engine.update_fields()?;

        // re-trigger every day of simulated time
        engine
            .trigger_queue
            .push_rel(self, Time::new::<day>(1).value);

        Ok(())
    }

    fn debug_context(&self, _state: &Engine) -> Option<String> {
        None
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct UpdateCollectTiles {}

impl TriggerType for UpdateCollectTiles {
    fn execute(self, engine: &mut Engine, _time: u64) -> Result<(), Error> {
        engine.state.update_collect_tiles()?;

        // re-trigger every hour of simulated time
        engine
            .trigger_queue
            .push_rel(self, Time::new::<hour>(1).value);

        Ok(())
    }

    fn debug_context(&self, _state: &Engine) -> Option<String> {
        None
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct UpdateTraffic {}

impl TriggerType for UpdateTraffic {
    fn execute(self, engine: &mut Engine, _time: u64) -> Result<(), Error> {
        // try to predict traffic 30 minutes in the future
        engine.update_route_weights(Time::new::<minute>(30).value);

        // re-trigger every hour of simulated time
        engine
            .trigger_queue
            .push_rel(self, engine.world_state_history.snapshot_period());

        Ok(())
    }

    fn debug_context(&self, _state: &Engine) -> Option<String> {
        None
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct AgentPlanCommuteToWork {
    pub agent: u64,
}

impl TriggerType for AgentPlanCommuteToWork {
    fn execute(self, engine: &mut Engine, _time: u64) -> Result<(), Error> {
        let agent = engine.agents.get_mut(&self.agent).expect("missing agent");
        let id = agent.id;

        if let agent::AgentState::Route(_) = agent.state {
            agent.log_timestamp(|| "aborting route", engine.time_state.current_time);

            // the agent hasn't finished their previous route yet.
            agent.abort_route(&mut engine.world_state)?;
        }

        if let Some(workplace) = &agent.workplace {
            // morning commute to work

            agent.log_timestamp(
                || "planning commute to work",
                engine.time_state.current_time,
            );

            let query_input = route::QueryInput {
                start: agent.housing,
                end: *workplace,
                car_config: agent.parked_car().map(|_| route::CarConfig::StartWithCar),
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

        Ok(())
    }

    fn debug_context(&self, state: &Engine) -> Option<String> {
        Some(format!("{:#?}", state.agents.get(&self.agent)))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct AgentPlanCommuteHome {
    pub agent: u64,
}

impl TriggerType for AgentPlanCommuteHome {
    fn execute(self, engine: &mut Engine, _time: u64) -> Result<(), Error> {
        let agent = engine.agents.get_mut(&self.agent).expect("missing agent");
        let id = agent.id;

        if let agent::AgentState::Route(_) = agent.state {
            agent.log_timestamp(|| "aborting route", engine.time_state.current_time);

            // the agent hasn't finished their previous route yet.
            agent.abort_route(&mut engine.world_state)?;
        }

        if let Some(workplace) = &agent.workplace {
            // commute back home from work

            agent.log_timestamp(
                || "planning commute home from work",
                engine.time_state.current_time,
            );

            let query_input = route::QueryInput {
                start: *workplace,
                end: agent.housing,
                // if a car is parked somewhere, account for it
                car_config: agent
                    .parked_car()
                    .map(|address| route::CarConfig::CollectParkedCar { address }),
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

        Ok(())
    }

    fn debug_context(&self, state: &Engine) -> Option<String> {
        Some(format!("{:#?}", state.agents.get(&self.agent)))
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
    fn execute(self, engine: &mut Engine, _time: u64) -> Result<(), Error> {
        agent::agent_log_timestamp(
            self.agent,
            || "starting route",
            engine.time_state.current_time,
        );

        // This blocks if the route has not been computed yet.
        // We can adjust how likely we are to block by twiddling the deadline.
        let route = match self
            .receiver
            .and_then(|receiver| receiver.receiver.recv().ok())
        {
            Some(route) => route,
            None => {
                agent::agent_log_timestamp(
                    self.agent,
                    || "missing route receiver; performing blocking query",
                    engine.time_state.current_time,
                );

                // We don't have a receiver because the engine state was serialized between when the
                // route query was queued and now. The best we can do is compute the route here.
                engine.query_route(self.query_input)
            }
        };

        let agent = engine.agents.get_mut(&self.agent).expect("missing agent");

        if let agent::AgentState::Route(_) = agent.state {
            panic!("route should have been aborted before it was queued");
        }

        if let Some(route) = route.unwrap() {
            agent.log_timestamp(|| "route found; starting", engine.time_state.current_time);

            let next_trigger = agent.begin_route(
                route,
                engine.time_state.current_time,
                self.route_type,
                &mut engine.world_state,
                &engine.state,
            )?;

            if let Some(next_trigger) = next_trigger {
                engine
                    .trigger_queue
                    .push(AgentRouteAdvance { agent: self.agent }, next_trigger);
            }
        } else if let agent::RouteType::CommuteFromWork = self.route_type {
            agent.log_timestamp(
                || "no route found; teleporting home",
                engine.time_state.current_time,
            );

            // teleport the agent home
            agent.teleport_home(&mut engine.world_state)?;
        } else {
            agent.log_timestamp(
                || "no route found; staying put",
                engine.time_state.current_time,
            );
        }

        Ok(())
    }

    fn debug_context(&self, state: &Engine) -> Option<String> {
        Some(format!("{:#?}", state.agents.get(&self.agent)))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct AgentRouteAdvance {
    agent: u64,
}

impl TriggerType for AgentRouteAdvance {
    fn execute(self, engine: &mut Engine, _time: u64) -> Result<(), Error> {
        let agent = engine.agents.get_mut(&self.agent).expect("missing agent");

        agent.log_timestamp(|| "advancing", engine.time_state.current_time);

        if let agent::AgentState::Route(route_state) = &mut agent.state {
            route_state.advance(&mut engine.world_state, &engine.state)?;
            match route_state.next_trigger() {
                Some(next_trigger) => {
                    assert!(next_trigger >= engine.time_state.current_time);
                    engine.trigger_queue.push(self, next_trigger);
                }
                None => {
                    agent.log_timestamp(|| "finishing route", engine.time_state.current_time);
                    agent.finish_route()?;
                }
            }
        } else {
            // this route was aborted because it took too long
        }

        Ok(())
    }

    fn debug_context(&self, state: &Engine) -> Option<String> {
        Some(format!("{:#?}", state.agents.get(&self.agent)))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct AgentLifeDecisions {
    pub agent: u64,
}

impl AgentLifeDecisions {
    fn get_agent<'a>(&self, agents: &'a HashMap<u64, agent::Agent>) -> &'a agent::Agent {
        agents.get(&self.agent).expect("missing agent")
    }

    fn modify_agent<F>(&self, engine: &mut Engine, f: F)
    where
        F: FnOnce(&mut agent::Agent),
    {
        let agent = engine.agents.get_mut(&self.agent).expect("missing agent");
        f(agent);
    }

    fn maybe_quit_job(&self, engine: &mut Engine) {
        let agent = self.get_agent(&engine.agents);

        if let Some(workplace_happiness_score) = agent.workplace_happiness_score() {
            if workplace_happiness_score < 0.1 {
                if let Some(workplace) = agent.workplace {
                    let agent_id = agent.id;
                    match engine.state.qtree.get_leaf_mut(workplace) {
                        Ok(state::LeafState {
                            tile: tiles::Tile::WorkplaceTile(tiles::WorkplaceTile { agents, .. }),
                            ..
                        }) => {
                            agents.retain(|id| *id != agent_id);
                        }
                        _ => panic!("missing workplace or non-workplace tile"),
                    }
                    self.modify_agent(engine, |agent| agent.workplace = None);
                }
            }
        }
    }

    fn maybe_find_new_job(&self, engine: &mut Engine) {
        let agent = self.get_agent(&engine.agents);

        // NOTE: if this is slow, it should be easy to parallelize
        if agent.workplace.is_none() {
            use rand::seq::SliceRandom;

            // TODO: be smarter about picking workplace candidates; sampling the map at random will
            // lead to the majority being too far away
            let vacant = &engine.state.collect_tiles.vacant_workplaces[..];
            let best = vacant.choose_multiple(&mut engine.rng, 100).filter_map(|address| {
                // the CollectTilesVisitor could be out-of-date; make sure the information is still
                // valid
                match engine.state.qtree.get_leaf(*address) {
                    Ok(state::LeafState {
                        tile: tiles::Tile::WorkplaceTile(tiles::WorkplaceTile { density, agents }),
                        ..
                    }) => {
                        if agents.len() >= *density {
                            return None;
                        }
                    }
                    _ => return None,
                };

                // TODO: running a bunch of queries is too expensive

                // let query_input = route::QueryInput {
                //     start: agent.housing,
                //     end: *address,
                //     car_config: Some(route::CarConfig::StartWithCar),
                // };
                // // TODO: query for what congestion *would* be during normal commuting hours
                // // TODO: we don't need to construct the route, this is a significant source of
                // // wasted time
                // let route = engine.query_route(query_input).unwrap();
                // route.map(|route| (*address, route.total_cost()))

                use cgmath::MetricSpace;

                let (x1, y1) = agent.housing.to_xy_f64();
                let (x2, y2) = address.to_xy_f64();

                let dist_sq = cgmath::Vector2::from((x1, y1)).distance2((x2, y2).into());
                Some((*address, -dist_sq))
            }).max_by(|(_, score1), (_, score2)| score1.partial_cmp(score2).unwrap());
            if let Some((address, neg_dist_sq)) = best {
                // TODO: this is a gross approximation, would be better to actually compute the
                // route cost
                if (-neg_dist_sq).sqrt() < (agent.data.commute_length_tolerance() as f64) / 10.0 {
                    let agent_id = agent.id;
                    if match engine.state.qtree.get_leaf_mut(address) {
                        Ok(state::LeafState {
                            tile:
                                tiles::Tile::WorkplaceTile(tiles::WorkplaceTile { density, agents }),
                            ..
                        }) => {
                            if agents.len() < *density {
                                agents.push(agent_id);
                                true
                            } else {
                                false
                            }
                        }
                        _ => false,
                    } {
                        self.modify_agent(engine, |agent| agent.workplace = Some(address));
                    }
                }
            }
        }
    }
}

impl TriggerType for AgentLifeDecisions {
    fn execute(self, engine: &mut Engine, _time: u64) -> Result<(), Error> {
        self.maybe_quit_job(engine);
        self.maybe_find_new_job(engine);

        // TODO: a longer cadence would make sense, but doing this for testing purposes
        engine
            .trigger_queue
            .push_rel(self, Time::new::<day>(2).value);

        Ok(())
    }

    fn debug_context(&self, state: &Engine) -> Option<String> {
        Some(format!("{:#?}", state.agents.get(&self.agent)))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct WorkplaceDecisions {}

impl TriggerType for WorkplaceDecisions {
    fn execute(self, engine: &mut Engine, _time: u64) -> Result<(), Error> {
        let root_branch = engine.state.qtree.get_root_branch().unwrap();
        // this should be a reasonable number
        let new_workplaces = root_branch.fields.raw_demand.raw_workplace_demand.count / 100;

        for _ in 0..new_workplaces {
            let address = match engine
                .blurred_fields
                .workplace_demand
                .sample(&mut engine.rng, &engine.state.qtree)
            {
                Some(address) => address,
                None => {
                    // no valid distribution, so just give up
                    break;
                }
            };

            engine
                .insert_tile(
                    address,
                    tiles::Tile::WorkplaceTile(tiles::WorkplaceTile {
                        density: 1,
                        agents: vec![],
                    }),
                )
                .unwrap();
        }

        engine
            .trigger_queue
            .push_rel(self, Time::new::<day>(2).value);

        Ok(())
    }

    fn debug_context(&self, _state: &Engine) -> Option<String> {
        None
    }
}

// Sample trigger implementation, demonstrates a simple recurring trigger
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct DummyTrigger {}

impl TriggerType for DummyTrigger {
    fn execute(self, engine: &mut Engine, time: u64) -> Result<(), Error> {
        println!("executing {}", time);
        engine.trigger_queue.push_rel(self, 1);
        Ok(())
    }

    fn debug_context(&self, _state: &Engine) -> Option<String> {
        None
    }
}

// Used for testing. Must be defined here since enum_dispatch doesn't support crossing crate
// boundaries.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct DoublingTrigger {}

impl TriggerType for DoublingTrigger {
    fn execute(self, engine: &mut Engine, _time: u64) -> Result<(), Error> {
        engine.trigger_queue.push_rel(DoublingTrigger {}, 1);
        engine.trigger_queue.push_rel(DoublingTrigger {}, 1);
        Ok(())
    }

    fn debug_context(&self, _state: &Engine) -> Option<String> {
        None
    }
}
