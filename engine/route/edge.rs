use serde::{Deserialize, Serialize};

use crate::common::Mode;
use crate::node::Node;
use crate::traffic::WorldState;

// time it takes to enter or leave a highway
pub const RAMP_TIME: f64 = 30.0;

#[non_exhaustive]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Edge {
    MetroSegment {
        metro_line: metro::MetroLineHandle,
        oriented_segment: metro::OrientedSegment,
        time: f64,
        start: quadtree::Address,
        stop: quadtree::Address,
    },
    MetroEmbark {
        metro_line: metro::MetroLineHandle,
        station: metro::Station,
    },
    MetroDisembark {
        metro_line: metro::MetroLineHandle,
        station: metro::Station,
    },
    Highway {
        segment: network::SegmentHandle,
        data: highway::HighwaySegment,
        time: f64,
    },
    HighwayRamp {
        position: (f64, f64),
    },
    ModeSegment {
        mode: Mode,
        distance: f64,
        start: (f64, f64),
        stop: (f64, f64),
    },
    ModeTransition {
        from: Mode,
        to: Mode,
        address: quadtree::Address,
    },
}

impl Edge {
    /**
     * The time to traverse this edge in the absence of congestion, i.e. the idealized time.
     */
    pub fn base_cost<F: state::Fields>(&self, state: &state::State<F>) -> f64 {
        use Edge::*;
        let cost = match self {
            MetroSegment { time, .. } => *time,
            MetroEmbark {
                metro_line: metro_line_id,
                ..
            } => {
                let metro_line = state.metros.metro_line(*metro_line_id);
                metro_line.data.schedule.expected_waiting_time() as f64
            }
            MetroDisembark { .. } => 0.0,
            Highway { time, .. } => *time,
            HighwayRamp { .. } => RAMP_TIME,
            ModeSegment { mode, distance, .. } => distance / mode.linear_speed(),
            ModeTransition { .. } => 0.0,
        };
        f64::max(cost, 1.0)
    }

    /**
     * The time to traverse this edge under the congestion conditions given by the world state. If
     * the current time is specified, it is used to give a precise time cost where applicable (e.g.
     * for metro schedules). If it is not specified, the cost is instead an estimate.
     */
    pub fn cost<W: WorldState, F: state::Fields>(
        &self,
        world_state: &W,
        state: &state::State<F>,
        current_time: Option<u64>,
    ) -> f64 {
        use Edge::*;
        let cost = match self {
            MetroSegment { time, .. } => *time,
            MetroEmbark {
                metro_line: metro_line_id,
                ..
            } => {
                let metro_line = state.metros.metro_line(*metro_line_id);
                match current_time {
                    None => metro_line.data.schedule.expected_waiting_time() as f64,
                    Some(_current_time) => {
                        // let current_time_f64 = current_time as f64;
                        // // TODO: agents for trains so that they can respond to congestion and get
                        // // delayed and stuff
                        // let station_time = metro_line
                        //     .get_splines()
                        //     .time_map
                        //     .get(&station.address)
                        //     .expect("station not found");
                        // let departure = metro_line
                        //     .schedule
                        //     .next_departure((current_time_f64 - station_time).floor() as u64)
                        //     as f64
                        //     + station_time;
                        // assert!(
                        //     departure > current_time_f64,
                        //     "{}, {}",
                        //     departure,
                        //     current_time
                        // );
                        // departure - current_time_f64

                        // TODO: re-implement the new way
                        metro_line.data.schedule.expected_waiting_time() as f64
                    }
                }
            }
            MetroDisembark { .. } => 0.0,
            Highway {
                segment: segment_id,
                ..
            } => {
                use highway::timing::HighwayTiming;

                let travelers = world_state.get_highway_segment_travelers(*segment_id);
                let segment = state.highways.segment(*segment_id);

                segment.congested_travel_time(
                    state.config.min_tile_size,
                    state.config.people_per_sim,
                    travelers,
                )
            }
            HighwayRamp { .. } => RAMP_TIME,
            ModeSegment {
                mode,
                distance,
                start,
                stop,
            } => {
                let base_travel_time = distance / mode.linear_speed();
                match mode {
                    Mode::Driving => {
                        let travelers =
                            world_state.get_local_road_travelers(*start, *stop, *distance);
                        let travel_time = crate::local_traffic::congested_travel_time(
                            base_travel_time,
                            &state.config,
                            travelers,
                        );
                        assert!(
                            (0.0..=highway::timing::MAX_CONGESTED_TIME).contains(&travel_time),
                            "{}",
                            travel_time
                        );
                        travel_time
                    }
                    _ => base_travel_time,
                }
            }
            ModeTransition { .. } => 0.0,
        };
        f64::max(cost, 1.0)
    }

    /**
     * Interpolate the position along this edge at the given fraction of the total edge time.
     */
    pub fn interpolate_position<F: state::Fields>(
        &self,
        state: &state::State<F>,
        pred: &Node,
        succ: &Node,
        fraction: f32,
    ) -> (f32, f32) {
        match &self {
            Edge::MetroSegment {
                metro_line: metro_line_id,
                oriented_segment,
                ..
            } => {
                use metro::RailwayTiming;

                let metro_line = state.metros.metro_line(*metro_line_id);
                let segment = state.railways.segment(oriented_segment.segment);
                let dist = segment
                    .railway_dist_spline(
                        metro_line.data.speed_limit,
                        state.config.min_tile_size as f64,
                        &state.railways,
                    )
                    .clamped_sample(oriented_segment.maybe_reversed_fraction(fraction) as f64)
                    .unwrap_or(0.0);
                let position = segment
                    .spline()
                    .clamped_sample(dist / state.config.min_tile_size as f64)
                    .expect("empty spline");

                (position.x as f32, position.y as f32)
            }
            Edge::Highway { segment, .. } => {
                let segment = state.highways.segment(*segment);

                let position = segment
                    .spline()
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

    pub fn is_jammed<W: WorldState, F: state::Fields>(
        &self,
        world_state: &W,
        state: &state::State<F>,
    ) -> bool {
        match self {
            Edge::Highway {
                segment: segment_id,
                ..
            } => {
                use highway::timing::HighwayTiming;

                let travelers = world_state.get_highway_segment_travelers(*segment_id);
                let segment = state.highways.segment(*segment_id);
                segment.is_jammed(
                    state.config.min_tile_size,
                    state.config.people_per_sim,
                    travelers,
                )
            }
            Edge::ModeSegment {
                mode: Mode::Driving,
                distance,
                start,
                stop,
            } => {
                let travelers = world_state.get_local_road_travelers(*start, *stop, *distance);
                crate::local_traffic::is_jammed(&state.config, travelers)
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
                metro_line,
                oriented_segment,
                time,
                ..
            } => write!(
                f,
                "metro:{:?}:{:?}:{:.2}",
                metro_line, oriented_segment, time
            ),
            MetroEmbark {
                metro_line,
                station,
            } => write!(f, "embark:{:?}:{}", metro_line, station.name),
            MetroDisembark {
                metro_line,
                station,
            } => write!(f, "disembark:{:?}:{}", metro_line, station.name),
            Highway {
                segment,
                data,
                time,
            } => {
                let name = data.name.clone().unwrap_or_else(|| "unknown".to_string());
                let refs = data.refs.join(";");
                write!(
                    f,
                    "highway:{}:{}:{}:{:.2}",
                    segment.inner(),
                    name,
                    refs,
                    time
                )
            }
            HighwayRamp { .. } => write!(f, "ramp"),
            ModeSegment { mode, distance, .. } => {
                write!(
                    f,
                    "{}:{:.2}m:{:.2}s",
                    mode,
                    distance,
                    distance / mode.linear_speed(),
                )
            }
            ModeTransition { from, to, address } => {
                let (x, y) = address.to_xy_f64();
                write!(f, "{}->{}:({:.1}, {:.1})", from, to, x, y)
            }
        }
    }
}
