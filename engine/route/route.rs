use std::collections::HashMap;
use std::iter::Zip;
use std::slice::Iter;

use once_cell::unsync::OnceCell;
use serde::{Deserialize, Serialize};

pub use spline_util::SplineVisitor;

use highway::{HighwaySegment, Highways};
use metro::MetroLine;

use crate::common::{CarConfig, Edge, Mode, Node, QueryInput};
use crate::traffic::WorldState;

#[derive(Debug, Copy, Clone, derive_more::Constructor, Serialize, Deserialize)]
pub struct RouteKey {
    pub position: (f32, f32),
    pub dist: f32,
    pub time: f32,
    pub mode: Mode,
}

impl splines::Interpolate<f32> for RouteKey {
    fn step(t: f32, threshold: f32, a: Self, b: Self) -> Self {
        unimplemented!()
    }

    fn lerp(t: f32, a: Self, b: Self) -> Self {
        Self {
            position: (
                f32::lerp(t, a.position.0, b.position.0),
                f32::lerp(t, a.position.1, b.position.1),
            ),
            dist: f32::lerp(t, a.dist, b.dist),
            time: f32::lerp(t, a.time, b.time),
            mode: a.mode,
        }
    }

    fn cosine(t: f32, a: Self, b: Self) -> Self {
        unimplemented!()
    }

    fn cubic_hermite(
        t: f32,
        x: (f32, Self),
        a: (f32, Self),
        b: (f32, Self),
        y: (f32, Self),
    ) -> Self {
        unimplemented!()
    }

    fn quadratic_bezier(t: f32, a: Self, u: Self, b: Self) -> Self {
        unimplemented!()
    }

    fn cubic_bezier(t: f32, a: Self, u: Self, v: Self, b: Self) -> Self {
        unimplemented!()
    }

    fn cubic_bezier_mirrored(t: f32, a: Self, u: Self, v: Self, b: Self) -> Self {
        unimplemented!()
    }
}

struct ConstructedSplines {
    keys: Vec<RouteKey>,
    total_dist: f32,
    total_time: f32,
}

#[derive(Debug, Clone)]
struct SplineData {
    spline: splines::Spline<f32, RouteKey>,
    total: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Route {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub query_input: QueryInput,
    pub cost: f32,
    pub bounds: quadtree::Rect,
    // NOTE: we store time and dist splines separately because dist spline is rarely used and this
    // saves a ton of memory.
    #[serde(skip)]
    time_spline: OnceCell<SplineData>,
    #[serde(skip)]
    dist_spline: OnceCell<SplineData>,
}

/**
 * Note: it is undefined behavior if the metro_lines and highways do not match those
 * used to construct the base graph from which this route was derived.
 */
pub struct SplineConstructionInput<'a, 'b, 'c> {
    pub metro_lines: &'a HashMap<u64, MetroLine>,
    pub highways: &'b Highways,
    pub state: &'c WorldState,
    pub tile_size: f64,
}

fn f64p_f32p((x, y): (f64, f64)) -> (f32, f32) {
    (x as f32, y as f32)
}

impl Route {
    pub fn new(nodes: Vec<Node>, edges: Vec<Edge>, cost: f32, query_input: QueryInput) -> Self {
        Self {
            bounds: spline_util::compute_bounds(&nodes, |node| node.location()),
            nodes,
            edges,
            cost,
            query_input,
            time_spline: OnceCell::new(),
            dist_spline: OnceCell::new(),
        }
    }

    /**
     * Iterate over ((start_node, end_node), edge) tuples.
     */
    pub fn iter(
        &self,
    ) -> Zip<itertools::TupleWindows<Iter<'_, Node>, (&Node, &Node)>, Iter<'_, Edge>> {
        use itertools::Itertools;
        assert!(self.nodes.len() == self.edges.len() + 1);
        return self.nodes.iter().tuple_windows().zip(self.edges.iter());
    }

    pub fn print(&self) {
        println!(
            "Route with cost {:.2}s ({:.2} minutes):",
            self.cost,
            self.cost / 60.0,
        );
        println!("  {}", self.nodes.first().expect("empty route"));
        for ((_, end), edge) in self.iter() {
            println!("    {}", edge);
            println!("  {}", end);
        }
    }

    fn construct_splines(&self, input: &SplineConstructionInput) -> ConstructedSplines {
        use cgmath::MetricSpace;

        let mut keys = Vec::new();

        let mut d: f32 = 0.0; // total distance
        let mut t: f32 = 0.0; // total elapsed time

        for ((start, end), edge) in self.iter() {
            let dt = edge.cost(input.state) as f32;
            // TODO: there may be some errors in dimensional analysis, i.e. meters vs coordinates
            let start_location = f64p_f32p(start.location());
            let end_location = f64p_f32p(end.location());
            let default_dd =
                cgmath::Vector2::from(start_location).distance(end_location.into()) as f32;
            let dd: f32;
            match &edge {
                Edge::MetroSegment {
                    metro_line,
                    start,
                    stop,
                    ..
                } => {
                    let metro_line = input
                        .metro_lines
                        .get(metro_line)
                        .expect("missing metro line");
                    let dist_spline = &metro_line.get_splines().dist_spline;

                    let start_index = *metro_line
                        .get_splines()
                        .index_map
                        .get(start)
                        .expect("start index not found");
                    let stop_index = *metro_line
                        .get_splines()
                        .index_map
                        .get(stop)
                        .expect("end index not found");

                    let start_key = dist_spline.keys()[start_index];
                    let stop_key = dist_spline.keys()[stop_index];

                    for key in &dist_spline.keys()[start_index..=stop_index] {
                        let time = (key.t - start_key.t) as f32;
                        let dist = (key.value - start_key.value) as f32;

                        assert!(time >= 0.0, "{}", time);
                        assert!(dist >= 0.0, "{}", dist);

                        let location = metro_line
                            .get_splines()
                            .spline
                            .clamped_sample(key.value / input.tile_size)
                            .unwrap();
                        // TODO: it is probably insufficient to describe this as walking
                        keys.push(RouteKey::new(
                            f64p_f32p(location.into()),
                            d + dist,
                            t + time,
                            Mode::Walking,
                        ));
                    }

                    dd = (stop_key.value - start_key.value) as f32;
                }
                Edge::MetroEmbark { .. } | Edge::MetroDisembark { .. } => {
                    dd = default_dd;
                }
                Edge::Highway { segment, .. } => {
                    let segment = input
                        .highways
                        .get_segment(*segment)
                        .expect("missing highway segment");
                    for key in segment.get_spline_keys() {
                        keys.push(RouteKey::new(
                            f64p_f32p(key.value.into()),
                            d + key.t as f32,
                            t + dt * (key.t / segment.length()) as f32,
                            Mode::Driving,
                        ));
                    }
                    dd = segment.length() as f32;
                }
                Edge::HighwayRamp { .. } => {
                    dd = default_dd;
                }
                Edge::ModeSegment { mode, .. } => {
                    dd = default_dd;
                    keys.push(RouteKey::new(start_location, d, t, *mode));
                    keys.push(RouteKey::new(end_location, d + dd, t + dt, *mode));
                }
                Edge::ModeTransition { .. }
                // | Edge::ParkCarSegment {}
                => {
                    dd = default_dd;
                }
            }
            d += dd;
            t += dt;
        }

        ConstructedSplines {
            keys,
            total_dist: d,
            total_time: t,
        }
    }

    fn construct_time_spline(&self, input: &SplineConstructionInput) -> SplineData {
        use splines::{Interpolation, Key, Spline};
        let constructed = self.construct_splines(input);
        SplineData {
            spline: Spline::from_vec(
                constructed
                    .keys
                    .iter()
                    .map(|key| Key::new(key.time, key.clone(), Interpolation::Linear))
                    .collect(),
            ),
            total: constructed.total_time,
        }
    }

    fn construct_dist_spline(&self, input: &SplineConstructionInput) -> SplineData {
        use splines::{Interpolation, Key, Spline};
        let constructed = self.construct_splines(input);
        SplineData {
            spline: Spline::from_vec(
                constructed
                    .keys
                    .iter()
                    .map(|key| Key::new(key.dist, key.clone(), Interpolation::Linear))
                    .collect(),
            ),
            total: constructed.total_dist,
        }
    }

    fn get_time_spline(&self, input: &SplineConstructionInput) -> &SplineData {
        self.time_spline
            .get_or_init(|| self.construct_time_spline(input))
    }

    fn get_dist_spline(&self, input: &SplineConstructionInput) -> &SplineData {
        self.dist_spline
            .get_or_init(|| self.construct_dist_spline(input))
    }

    pub fn visit_spline<V, E>(
        &self,
        visitor: &mut V,
        step: f64,
        rect: &quadtree::Rect,
        input: &SplineConstructionInput,
    ) -> Result<(), E>
    where
        V: SplineVisitor<Route, RouteKey, E>,
    {
        let spline = self.get_dist_spline(input);
        spline_util::visit_spline(
            self,
            &spline.spline,
            spline.total,
            visitor,
            step,
            rect,
            |key| key.position.into(),
        )
    }

    /**
     * Get the route key at the given time, relative to the start of the route.
     */
    pub fn sample_time(&self, time: f32, input: &SplineConstructionInput) -> Option<RouteKey> {
        let spline = self.get_time_spline(input);
        spline.spline.sample(time)
    }

    /**
     * Get the route key at the given time in engine time, i.e. subtract off this route's start
     * time.
     */
    pub fn sample_engine_time(
        &self,
        time: f32,
        input: &SplineConstructionInput,
    ) -> Option<RouteKey> {
        self.sample_time(time - self.query_input.start_time as f32, input)
    }

    pub fn total_dist(&self, input: &SplineConstructionInput) -> f32 {
        let spline = self.get_dist_spline(input);
        spline.total
    }

    pub fn total_time(&self) -> f32 {
        self.cost
    }

    pub fn start(&self) -> quadtree::Address {
        self.query_input.start
    }

    pub fn stop(&self) -> quadtree::Address {
        self.query_input.end
    }
}
