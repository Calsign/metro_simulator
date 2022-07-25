use once_cell::unsync::OnceCell;
use serde::{Deserialize, Serialize};

pub use spline_util::SplineVisitor;

use crate::common::{Error, Mode, QueryInput};
use crate::edge::Edge;
use crate::node::Node;
use crate::route_key::RouteKey;

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
    pub start_mode: Mode,
    pub end_mode: Mode,
    // NOTE: we store time and dist splines separately because dist spline is rarely used and this
    // saves a ton of memory.
    #[serde(skip)]
    time_spline: OnceCell<SplineData>,
    #[serde(skip)]
    dist_spline: OnceCell<SplineData>,
}

fn f64p_f32p((x, y): (f64, f64)) -> (f32, f32) {
    (x as f32, y as f32)
}

impl Route {
    pub fn new(
        nodes: Vec<Node>,
        edges: Vec<Edge>,
        cost: f32,
        query_input: QueryInput,
        start_mode: Mode,
        end_mode: Mode,
    ) -> Self {
        Self {
            bounds: spline_util::compute_bounds(&nodes, |node| node.location()),
            nodes,
            edges,
            cost,
            query_input,
            start_mode,
            end_mode,
            time_spline: OnceCell::new(),
            dist_spline: OnceCell::new(),
        }
    }

    fn verify_node_edge_count(&self) {
        assert!(
            self.nodes.len() == self.edges.len() + 1,
            "nodes: {}, edges: {}",
            self.nodes.len(),
            self.edges.len()
        );
    }

    /**
     * Iterate over ((start_node, end_node), edge) tuples.
     */
    pub fn iter(&self) -> impl Iterator<Item = ((&Node, &Node), &Edge)> {
        use itertools::Itertools;
        self.verify_node_edge_count();
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

    fn construct_splines<F: state::Fields>(&self, state: &state::State<F>) -> ConstructedSplines {
        use cgmath::MetricSpace;

        let mut keys = Vec::new();

        let mut d: f32 = 0.0; // total distance
        let mut t: f32 = 0.0; // total elapsed time

        for ((start, end), edge) in self.iter() {
            let dt = edge.base_cost(state) as f32;
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
                    let metro_line = state
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
                            .clamped_sample(key.value / state.config.min_tile_size as f64)
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
                    let segment = state
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
                Edge::ModeTransition { .. } => {
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

    fn construct_time_spline<F: state::Fields>(&self, state: &state::State<F>) -> SplineData {
        use splines::{Interpolation, Key, Spline};
        let constructed = self.construct_splines(state);
        SplineData {
            spline: Spline::from_vec(
                constructed
                    .keys
                    .iter()
                    .map(|key| Key::new(key.time, *key, Interpolation::Linear))
                    .collect(),
            ),
            total: constructed.total_time,
        }
    }

    fn construct_dist_spline<F: state::Fields>(&self, state: &state::State<F>) -> SplineData {
        use splines::{Interpolation, Key, Spline};
        let constructed = self.construct_splines(state);
        SplineData {
            spline: Spline::from_vec(
                constructed
                    .keys
                    .iter()
                    .map(|key| Key::new(key.dist, *key, Interpolation::Linear))
                    .collect(),
            ),
            total: constructed.total_dist,
        }
    }

    fn get_time_spline<F: state::Fields>(&self, state: &state::State<F>) -> &SplineData {
        self.time_spline
            .get_or_init(|| self.construct_time_spline(state))
    }

    fn get_dist_spline<F: state::Fields>(&self, state: &state::State<F>) -> &SplineData {
        self.dist_spline
            .get_or_init(|| self.construct_dist_spline(state))
    }

    pub fn visit_spline<V, E, F: state::Fields>(
        &self,
        visitor: &mut V,
        step: f64,
        rect: &quadtree::Rect,
        state: &state::State<F>,
    ) -> Result<(), E>
    where
        V: SplineVisitor<Route, RouteKey, E>,
    {
        let spline = self.get_dist_spline(state);
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
    pub fn sample_time<F: state::Fields>(
        &self,
        time: f32,
        state: &state::State<F>,
    ) -> Option<RouteKey> {
        let spline = self.get_time_spline(state);
        spline.spline.sample(time)
    }

    pub fn total_dist<F: state::Fields>(&self, state: &state::State<F>) -> f32 {
        let spline = self.get_dist_spline(state);
        spline.total
    }

    pub fn total_cost(&self) -> f32 {
        self.cost
    }

    pub fn start(&self) -> quadtree::Address {
        self.query_input.start
    }

    pub fn end(&self) -> quadtree::Address {
        self.query_input.end
    }

    pub fn join(mut first: Self, mut second: Self) -> Self {
        assert_eq!(first.end(), second.start());

        // NOTE: We will end up with some extra endpoint nodes in the middle of the route. This
        // should be fine? We don't currently interpret endpoint nodes as anything special.

        // join together with an edge based on the mode
        if first.end_mode == second.end_mode {
            first.edges.push(Edge::ModeSegment {
                mode: first.end_mode,
                distance: 0.0,
                start: first.end().to_xy_f64(),
                stop: second.start().to_xy_f64(),
            });
        } else {
            first.edges.push(Edge::ModeTransition {
                from: first.end_mode,
                to: second.start_mode,
                address: first.end(),
            });
        }

        first.nodes.append(&mut second.nodes);
        first.edges.append(&mut second.edges);

        Self {
            nodes: first.nodes,
            edges: first.edges,
            query_input: QueryInput {
                start: first.query_input.start,
                end: second.query_input.end,
                car_config: first.query_input.car_config,
            },
            cost: first.cost + second.cost,
            bounds: first.bounds.and(&second.bounds),
            start_mode: first.start_mode,
            end_mode: second.end_mode,
            time_spline: OnceCell::new(),
            dist_spline: OnceCell::new(),
        }
    }

    pub fn patch_tile(
        &mut self,
        from: quadtree::Address,
        to: quadtree::Address,
    ) -> Result<(), Error> {
        // TODO: It could make sense to store which nodes and edges need to be patched when the
        // route is constructed. But on average, each route will be patched much less than once, so
        // it doesn't make sense to make that optimization yet.

        for node in self.nodes.iter_mut() {
            match node {
                Node::Endpoint { address } | Node::Parking { address } if *address == from => {
                    *address = to
                }
                _ => (),
            }
        }
        for edge in self.edges.iter_mut() {
            match edge {
                Edge::ModeTransition { address, .. } if *address == from => *address = to,
                Edge::MetroEmbark { station, .. } | Edge::MetroDisembark { station, .. }
                    if station.address == from =>
                {
                    station.address = to
                }
                _ => (),
            }
        }

        if self.query_input.start == from {
            self.query_input.start = to;
        }
        if self.query_input.end == from {
            self.query_input.end = to;
        }

        Ok(())
    }
}
