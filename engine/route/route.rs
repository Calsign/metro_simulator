use std::collections::HashMap;
use std::iter::Zip;
use std::slice::Iter;

use once_cell::unsync::OnceCell;

pub use spline_util::SplineVisitor;

use highway::{HighwaySegment, Highways};
use metro::MetroLine;

use crate::common::{Edge, Mode, Node, WorldState};

#[derive(Debug, Copy, Clone, derive_more::Constructor)]
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

#[derive(Debug)]
struct Splines {
    dist_spline: splines::Spline<f64, RouteKey>,
    time_spline: splines::Spline<f64, RouteKey>,
    total_dist: f64,
    total_time: f64,
}

#[derive(Debug)]
pub struct Route {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub cost: f64,
    splines: OnceCell<Splines>,
}

/**
 * Note: it is undefined behavior if the metro_lines and highways do not match those
 * used to construct the base graph from which this route was derived.
 */
pub struct SplineConstructionInput<'a, 'b, 'c> {
    metro_lines: &'a HashMap<u64, MetroLine>,
    highways: &'b Highways,
    state: &'c WorldState,
    tile_size: f64,
}

impl Route {
    pub fn new(nodes: Vec<Node>, edges: Vec<Edge>, cost: f64) -> Self {
        Self {
            nodes,
            edges,
            cost,
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
            match &edge {
                Edge::MetroSegment { metro_line, .. } => {
                    let metro_line = input
                        .metro_lines
                        .get(metro_line)
                        .expect("missing metro line");
                    let speed_keys =
                        metro::timing::speed_keys(metro_line.get_keys(), input.tile_size);
                    let dist_spline = metro::timing::dist_spline(&speed_keys);
                    for key in dist_spline.keys() {
                        let time = key.t;
                        let dist = key.value;
                        let location = metro_line.get_spline().clamped_sample(dist).unwrap();
                        // TODO: it is probably insufficient to describe this as walking
                        keys.push(RouteKey::new(location.into(), dist, time, Mode::Walking));
                    }
                }
                Edge::MetroEmbark { .. } | Edge::MetroDisembark { .. } => (),
                Edge::Highway { segment, .. } => {
                    let segment = input
                        .highways
                        .get_segment(*segment)
                        .expect("missing highway segment");
                    for key in segment.get_spline_keys() {
                        let total_time = highway::timing::travel_time(segment, input.tile_size);
                        keys.push(RouteKey::new(
                            key.value.into(),
                            d + key.t,
                            t + total_time * d / segment.length(),
                            Mode::Driving,
                        ));
                    }
                }
                Edge::ModeSegment { mode, .. } => {
                    keys.push(RouteKey::new(start.location(), d, t, *mode));
                    keys.push(RouteKey::new(end.location(), d, t, *mode));
                }
                Edge::ModeTransition { .. } => (),
            }
            t += edge.cost(input.state);
            d += cgmath::Vector2::from(start.location()).distance(end.location().into());
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

    pub fn sample_time(&self, time: f64, input: &SplineConstructionInput) -> Option<RouteKey> {
        let splines = self.get_splines(input);
        splines.time_spline.sample(time)
    }
}
