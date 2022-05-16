use serde::{Deserialize, Serialize};

use crate::common::Mode;
use crate::node::Node;
use crate::route_key::RouteKey;
use crate::traffic::WorldState;

// time it takes to wait for a train, on average
// TODO: replace this with correct accounting for train schedules
pub const EMBARK_TIME: f64 = 480.0;
// time it takes to enter or leave a highway
pub const RAMP_TIME: f64 = 30.0;

#[non_exhaustive]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Edge {
    MetroSegment {
        metro_line: u64,
        time: f64,
        start: quadtree::Address,
        stop: quadtree::Address,
    },
    MetroEmbark {
        metro_line: u64,
        station: metro::Station,
    },
    MetroDisembark {
        metro_line: u64,
        station: metro::Station,
    },
    Highway {
        segment: u64,
        data: highway::HighwayData,
        time: f64,
    },
    HighwayRamp {
        position: (f64, f64),
    },
    ModeSegment {
        mode: Mode,
        distance: f64,
    },
    ModeTransition {
        from: Mode,
        to: Mode,
    },
}

fn u64_f64_point_dist(a: (f64, f64), (bx, by): (u64, u64)) -> f64 {
    use cgmath::MetricSpace;
    cgmath::Vector2::from(a).distance((bx as f64, by as f64).into())
}

impl Edge {
    /**
     * The time to traverse this edge in the absence of congestion, i.e. the idealized time.
     */
    pub fn base_cost(&self, state: &state::State) -> f64 {
        use Edge::*;
        let cost = match self {
            MetroSegment { time, .. } => *time,
            MetroEmbark {
                metro_line: metro_line_id,
                ..
            } => {
                let metro_line = state
                    .metro_lines
                    .get(metro_line_id)
                    .expect("missing metro line");
                metro_line.schedule.expected_waiting_time() as f64
            }
            MetroDisembark {
                metro_line,
                station,
            } => 0.0,
            Highway { time, .. } => *time,
            HighwayRamp { .. } => RAMP_TIME,
            ModeSegment { mode, distance } => distance / mode.linear_speed(),
            ModeTransition { .. } => 0.0,
        };
        f64::max(cost, 1.0)
    }

    /**
     * The time to traverse this edge under the congestion conditions given by the world state. If
     * the current time is specified, it is used to give a precise time cost where applicable (e.g.
     * for metro schedules). If it is not specified, the cost is instead an estimate.
     */
    pub fn cost(
        &self,
        world_state: &WorldState,
        state: &state::State,
        current_time: Option<u64>,
    ) -> f64 {
        use Edge::*;
        let cost = match self {
            MetroSegment { time, .. } => *time,
            MetroEmbark {
                metro_line: metro_line_id,
                station,
            } => {
                let metro_line = state
                    .metro_lines
                    .get(metro_line_id)
                    .expect("missing metro line");
                match current_time {
                    None => metro_line.schedule.expected_waiting_time() as f64,
                    Some(current_time) => {
                        let current_time_f64 = current_time as f64;
                        // TODO: agents for trains so that they can respond to congestion and get
                        // delayed and stuff
                        let station_time = metro_line
                            .get_splines()
                            .time_map
                            .get(&station.address)
                            .expect("station not found");
                        let departure = metro_line
                            .schedule
                            .next_departure((current_time_f64 - station_time).floor() as u64)
                            as f64
                            + station_time;
                        assert!(
                            departure > current_time_f64,
                            "{}, {}",
                            departure,
                            current_time
                        );
                        departure - current_time_f64
                    }
                }
            }
            MetroDisembark {
                metro_line,
                station,
            } => 0.0,
            Highway {
                segment: segment_id,
                ..
            } => {
                let travelers = world_state.get_highway_segment_travelers(*segment_id);
                let segment = state
                    .highways
                    .get_segment(*segment_id)
                    .expect("missing highway segment");

                segment.congested_travel_time(
                    state.config.min_tile_size,
                    state.config.people_per_sim,
                    travelers as u32,
                )
            }
            HighwayRamp { .. } => RAMP_TIME,
            ModeSegment { mode, distance } => distance / mode.linear_speed(),
            ModeTransition { .. } => 0.0,
        };
        f64::max(cost, 1.0)
    }

    /**
     * Interpolate the position along this edge at the given fraction of the total edge time.
     */
    pub fn interpolate_position(
        &self,
        state: &state::State,
        pred: &Node,
        succ: &Node,
        fraction: f32,
    ) -> (f32, f32) {
        match &self {
            Edge::MetroSegment {
                metro_line,
                start,
                stop,
                ..
            } => {
                let metro_line = state
                    .metro_lines
                    .get(&metro_line)
                    .expect("missing metro line");
                let dist_spline = &metro_line.get_splines().dist_spline;
                let position_spline = &metro_line.get_splines().spline;

                let start_index = *metro_line
                    .get_splines()
                    .index_map
                    .get(&start)
                    .expect("start index not found");
                let end_index = *metro_line
                    .get_splines()
                    .index_map
                    .get(&stop)
                    .expect("end index not found");

                let start_key = dist_spline.keys()[start_index];
                let end_key = dist_spline.keys()[end_index];

                let dist = dist_spline
                    .clamped_sample(fraction as f64 * (end_key.t - start_key.t) + start_key.t)
                    .expect("dist spline is empty");
                let position = position_spline
                    .clamped_sample(dist / state.config.min_tile_size as f64)
                    .expect("position spline is empty");
                (position.x as f32, position.y as f32)
            }
            Edge::Highway { segment, .. } => {
                let segment = state
                    .highways
                    .get_segment(*segment)
                    .expect("missing highway segment");

                let position = segment
                    .get_spline()
                    .clamped_sample(fraction as f64 * segment.length())
                    .expect("highway spline is empty");
                (position.x as f32, position.y as f32)
            }
            Edge::ModeSegment { .. } => {
                use cgmath::VectorSpace;
                cgmath::Vector2::from(pred.location_f32())
                    .lerp(succ.location_f32().into(), fraction)
                    .into()
            }
            Edge::MetroEmbark { .. }
            | Edge::MetroDisembark { .. }
            | Edge::ModeTransition { .. }
            | Edge::HighwayRamp { .. } => pred.location_f32(),
        }
    }

    /**
     * If this edge changes the mode of travel, the new mode. Otherwise, None.
     */
    pub fn mode_transition(&self) -> Option<Mode> {
        match self {
            Edge::ModeTransition { to, .. } => Some(*to),
            _ => None,
        }
    }

    pub fn is_jammed(&self, world_state: &WorldState, state: &state::State) -> bool {
        match self {
            Edge::Highway {
                segment: segment_id,
                ..
            } => {
                let travelers = world_state.get_highway_segment_travelers(*segment_id);
                let segment = state
                    .highways
                    .get_segment(*segment_id)
                    .expect("missing highway segment");
                segment.is_jammed(
                    state.config.min_tile_size,
                    state.config.people_per_sim,
                    travelers as u32,
                )
            }
            _ => false,
        }
    }
}

impl std::fmt::Display for Edge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use Edge::*;
        match self {
            MetroSegment {
                metro_line, time, ..
            } => write!(f, "metro:{}:{:.2}", metro_line, time),
            MetroEmbark {
                metro_line,
                station,
            } => write!(f, "embark:{}:{}", metro_line, station.name),
            MetroDisembark {
                metro_line,
                station,
            } => write!(f, "disembark:{}:{}", metro_line, station.name),
            Highway {
                segment,
                data,
                time,
            } => {
                let name = data.name.clone().unwrap_or("unknown".to_string());
                let refs = data.refs.join(";");
                write!(f, "highway:{}:{}:{}:{:.2}", segment, name, refs, time)
            }
            HighwayRamp { .. } => write!(f, "ramp"),
            ModeSegment { mode, distance } => {
                write!(
                    f,
                    "{}:{:.2}m:{:.2}s",
                    mode,
                    distance,
                    distance / mode.linear_speed(),
                )
            }
            ModeTransition { from, to } => write!(f, "{}->{}", from, to),
        }
    }
}
