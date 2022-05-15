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
    pub fn base_cost(&self) -> f64 {
        use Edge::*;
        let cost = match self {
            MetroSegment { time, .. } => *time,
            MetroEmbark {
                metro_line,
                station,
            } => EMBARK_TIME,
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
     * The time to traverse this edge under the congestion conditions given by the world state.
     */
    pub fn cost(&self, world_state: &WorldState) -> f64 {
        use Edge::*;
        let cost = match self {
            MetroSegment { time, .. } => *time,
            MetroEmbark {
                metro_line,
                station,
            } => {
                // TODO: properly account for train schedules
                EMBARK_TIME
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
