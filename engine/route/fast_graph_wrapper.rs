use std::collections::{HashMap, HashSet};

use fast_paths::{
    FastGraph, InputGraph, NodeId, Params, ParamsWithOrder, PathCalculator, ShortestPath, Weight,
};

use crate::common::QueryInput;
use crate::edge::Edge;
use crate::node::Node;
use crate::traffic::WorldState;

#[derive(derivative::Derivative)]
#[derivative(Debug)]
pub struct FastGraphWrapper {
    input: InputGraph,
    node_map: HashMap<NodeId, Node>,
    edge_map: HashMap<(NodeId, NodeId), Edge>,
    fast_graph: Option<FastGraph>,
    node_ordering: Option<Vec<NodeId>>,
    #[derivative(Debug = "ignore")]
    path_calculator: Option<PathCalculator>,
}

impl Clone for FastGraphWrapper {
    fn clone(&self) -> Self {
        Self {
            input: self.input.clone(),
            node_map: self.node_map.clone(),
            edge_map: self.edge_map.clone(),
            fast_graph: self.fast_graph.clone(),
            node_ordering: self.node_ordering.clone(),
            path_calculator: self
                .fast_graph
                .as_ref()
                .map(|g| fast_paths::create_calculator(&g)),
        }
    }
}

impl FastGraphWrapper {
    pub fn new() -> Self {
        Self {
            input: InputGraph::new(),
            node_map: HashMap::new(),
            edge_map: HashMap::new(),
            fast_graph: None,
            node_ordering: None,
            path_calculator: None,
        }
    }

    pub fn is_prepared(&self) -> bool {
        self.fast_graph.is_some()
    }

    pub fn add_node(&mut self, node: Node) -> NodeId {
        debug_assert!(!self.is_prepared());
        let id = self.node_map.len();
        self.node_map.insert(id, node);
        id
    }

    pub fn add_edge(&mut self, from: NodeId, to: NodeId, edge: Edge, state: &state::State) {
        debug_assert!(!self.is_prepared());
        debug_assert!(self.node_map.contains_key(&from));
        debug_assert!(self.node_map.contains_key(&to));
        // NOTE: fast_paths disallows negative weights...
        let weight = edge.base_cost(state) as Weight;
        debug_assert!(weight > 0, "base weight for {} -> {} is 0", from, to);
        self.input.add_edge(from, to, weight);
        self.edge_map.insert((from, to), edge);
    }

    pub fn get_node_map(&self) -> &HashMap<NodeId, Node> {
        &self.node_map
    }

    pub fn get_edge_map(&self) -> &HashMap<(NodeId, NodeId), Edge> {
        &self.edge_map
    }

    fn get_prepare_params() -> Params {
        Params::new(0.1, 10, 100, 100)
    }

    fn get_update_params() -> ParamsWithOrder {
        ParamsWithOrder::new(100)
    }

    pub fn prepare(&mut self) {
        debug_assert!(!self.is_prepared());
        self.input.freeze();
        // NOTE: the number of nodes may not align if there are orphaned nodes that don't belong to any edges
        // TODO: for some reason this assertion is failing
        // debug_assert_eq!(self.input.get_num_edges(), self.edge_map.len());
        let fast_graph = fast_paths::prepare_with_params(&self.input, &Self::get_prepare_params());
        let node_ordering = fast_graph.get_node_ordering();
        let path_calculator = fast_paths::create_calculator(&fast_graph);
        self.node_ordering = Some(node_ordering);
        self.fast_graph = Some(fast_graph);
        self.path_calculator = Some(path_calculator);
    }

    fn create_input(&self, world_state: &WorldState, state: &state::State) -> InputGraph {
        let mut input_graph = InputGraph::new();
        for ((from, to), edge) in self.edge_map.iter() {
            let weight = edge.cost(world_state, state, None) as Weight;
            assert!(weight > 0, "weight for {} -> {} is 0", from, to);
            input_graph.add_edge(*from, *to, weight);
        }
        input_graph.freeze();
        input_graph
    }

    pub fn update_weights(&mut self, world_state: &WorldState, state: &state::State) {
        debug_assert!(self.is_prepared());
        // TODO: Patch fast_paths to allow mutating the edge weights instead of constructing a new graph.
        // This might be nontrivial, but it seems like a worthwhile optimization.
        // At the very least, we should be able to pre-specify the number of nodes in the InputGraph.
        let input_graph = self.create_input(world_state, state);
        assert_eq!(
            input_graph.get_num_nodes(),
            self.node_ordering.as_ref().unwrap().len()
        );
        let node_ordering = &self.node_ordering.as_ref().unwrap();
        let fast_graph = fast_paths::prepare_with_order_with_params(
            &input_graph,
            node_ordering,
            &Self::get_update_params(),
        )
        .unwrap();
        let path_calculator = fast_paths::create_calculator(&fast_graph);
        self.fast_graph = Some(fast_graph);
        self.path_calculator = Some(path_calculator);
    }

    pub fn query(&mut self, source: NodeId, target: NodeId) -> Option<ShortestPath> {
        debug_assert!(self.is_prepared());
        let fast_graph = &self.fast_graph.as_ref().unwrap();
        let path_calculator = self.path_calculator.as_mut().unwrap();
        path_calculator.calc_path(fast_graph, source, target)
    }
}

// for compatibility with petgraph
impl FastGraphWrapper {
    pub fn node_weight(&self, node: NodeId) -> Option<&Node> {
        self.node_map.get(&node)
    }

    pub fn find_edge(&self, from: NodeId, to: NodeId) -> Option<(NodeId, NodeId)> {
        Some((from, to))
    }

    pub fn edge_weight(&self, edge: (NodeId, NodeId)) -> Option<&Edge> {
        self.edge_map.get(&edge)
    }

    pub fn node_count(&self) -> usize {
        self.node_map.len()
    }

    pub fn edge_count(&self) -> usize {
        self.edge_map.len()
    }
}
