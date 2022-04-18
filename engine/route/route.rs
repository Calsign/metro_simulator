use std::collections::HashMap;
use std::iter::Zip;
use std::slice::Iter;

use once_cell::unsync::OnceCell;
use serde::{Deserialize, Serialize};

pub use spline_util::SplineVisitor;

use highway::{HighwaySegment, Highways};
use metro::MetroLine;

use crate::common::{Edge, Mode, Node, WorldState};

#[derive(Debug, Copy, Clone, derive_more::Constructor, Serialize, Deserialize)]
pub struct RouteKey {
    pub position: (f64, f64),
    pub dist: f64,
    pub time: f64,
    pub mode: Mode,
}

impl splines::Interpolate<f64> for RouteKey {
    fn step(t: f64, threshold: f64, a: Self, b: Self) -> Self {
        unimplemented!()
    }

    fn lerp(t: f64, a: Self, b: Self) -> Self {
        Self {
            position: (
                f64::lerp(t, a.position.0, b.position.0),
                f64::lerp(t, a.position.1, b.position.1),
            ),
            dist: f64::lerp(t, a.dist, b.dist),
            time: f64::lerp(t, a.time, b.time),
            mode: a.mode,
        }
    }

    fn cosine(t: f64, a: Self, b: Self) -> Self {
        unimplemented!()
    }

    fn cubic_hermite(
        t: f64,
        x: (f64, Self),
        a: (f64, Self),
        b: (f64, Self),
        y: (f64, Self),
    ) -> Self {
        unimplemented!()
    }

    fn quadratic_bezier(t: f64, a: Self, u: Self, b: Self) -> Self {
        unimplemented!()
    }

    fn cubic_bezier(t: f64, a: Self, u: Self, v: Self, b: Self) -> Self {
        unimplemented!()
    }

    fn cubic_bezier_mirrored(t: f64, a: Self, u: Self, v: Self, b: Self) -> Self {
        unimplemented!()
    }
}

#[derive(Debug, Clone)]
struct Splines {
    dist_spline: splines::Spline<f64, RouteKey>,
    time_spline: splines::Spline<f64, RouteKey>,
    total_dist: f64,
    total_time: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Route {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub cost: f64,
    pub start_time: u64,
    #[serde(skip)]
    splines: OnceCell<Splines>,
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

impl Route {
    pub fn new(nodes: Vec<Node>, edges: Vec<Edge>, cost: f64, start_time: u64) -> Self {
        Self {
            nodes,
            edges,
            cost,
            start_time,
            splines: OnceCell::new(),
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

    fn construct_splines(&self, input: &SplineConstructionInput) -> Splines {
        use cgmath::MetricSpace;
        use splines::{Interpolation, Key, Spline};

        let mut keys = Vec::new();

        let mut d = 0.0; // total distance
        let mut t = 0.0; // total elapsed time

        for ((start, end), edge) in self.iter() {
            let dt = edge.cost(input.state);
            // TODO: there may be some errors in dimensional analysis, i.e. meters vs coordinates
            let default_dd =
                cgmath::Vector2::from(start.location()).distance(end.location().into());
            let dd: f64;
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
                        let time = key.t - start_key.t;
                        let dist = key.value - start_key.value;

                        assert!(time >= 0.0, "{}", time);
                        assert!(dist >= 0.0, "{}", dist);

                        let location = metro_line
                            .get_splines()
                            .spline
                            .clamped_sample(key.value / input.tile_size)
                            .unwrap();
                        // TODO: it is probably insufficient to describe this as walking
                        keys.push(RouteKey::new(
                            location.into(),
                            d + dist,
                            t + time,
                            Mode::Walking,
                        ));
                    }

                    dd = stop_key.value - start_key.value;
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
                            key.value.into(),
                            d + key.t,
                            t + dt * key.t / segment.length(),
                            Mode::Driving,
                        ));
                    }
                    dd = segment.length();
                }
                Edge::HighwayRamp { .. } => {
                    dd = default_dd;
                }
                Edge::ModeSegment { mode, .. } => {
                    dd = default_dd;
                    keys.push(RouteKey::new(start.location(), d, t, *mode));
                    keys.push(RouteKey::new(end.location(), d + dd, t + dt, *mode));
                }
                Edge::ModeTransition { .. } => {
                    dd = default_dd;
                }
            }
            d += dd;
            t += dt;
        }

        Splines {
            dist_spline: Spline::from_vec(
                keys.iter()
                    .map(|key| Key::new(key.dist, key.clone(), Interpolation::Linear))
                    .collect(),
            ),
            time_spline: Spline::from_vec(
                keys.iter()
                    .map(|key| Key::new(key.time, key.clone(), Interpolation::Linear))
                    .collect(),
            ),
            total_dist: d,
            total_time: t,
        }
    }

    fn get_splines(&self, input: &SplineConstructionInput) -> &Splines {
        self.splines.get_or_init(|| self.construct_splines(input))
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
        let splines = self.get_splines(input);
        spline_util::visit_spline(
            self,
            &splines.dist_spline,
            splines.total_dist,
            visitor,
            step,
            rect,
            |key| key.position.into(),
        )
    }

    /**
     * Get the route key at the given time, relative to the start of the route.
     */
    pub fn sample_time(&self, time: f64, input: &SplineConstructionInput) -> Option<RouteKey> {
        let splines = self.get_splines(input);
        splines.time_spline.sample(time)
    }

    /**
     * Get the route key at the given time in engine time, i.e. subtract off this route's start
     * time.
     */
    pub fn sample_engine_time(
        &self,
        time: f64,
        input: &SplineConstructionInput,
    ) -> Option<RouteKey> {
        self.sample_time(time - self.start_time as f64, input)
    }

    pub fn total_dist(&self, input: &SplineConstructionInput) -> f64 {
        let splines = self.get_splines(input);
        splines.total_dist
    }

    pub fn total_time(&self, input: &SplineConstructionInput) -> f64 {
        let splines = self.get_splines(input);
        splines.total_dist
    }

    pub fn start(&self) -> quadtree::Address {
        *self.nodes.first().expect("empty route").address()
    }

    pub fn stop(&self) -> quadtree::Address {
        *self.nodes.last().expect("empty route").address()
    }
}
