use crate::base_graph::Graph;
use crate::common::{Edge, Error, Node};

#[derive(Debug)]
pub struct Route {
    pub edges: Vec<Edge>,
    pub cost: f64,
}

/**
 * A wrapper around graph that removes added nodes and edges when dropped.
 * Tracking added nodes and edges must be performed by the user.
 */
struct AugmentedGraph<'a> {
    graph: &'a mut Graph,
    new_nodes: Vec<petgraph::graph::NodeIndex>,
    new_edges: Vec<petgraph::graph::EdgeIndex>,
    base_nodes: usize,
    base_edges: usize,
}

impl<'a> AugmentedGraph<'a> {
    fn new(base_graph: &'a mut Graph) -> Self {
        Self {
            base_nodes: base_graph.graph.node_count(),
            base_edges: base_graph.graph.edge_count(),
            graph: base_graph,
            new_nodes: Vec::new(),
            new_edges: Vec::new(),
        }
    }
}

impl<'a> Drop for AugmentedGraph<'a> {
    fn drop(&mut self) {
        // NOTE: iterating in reverse order should unwind the graph in
        // such a way that no index swapping needs to take place
        for node in self.new_nodes.iter().rev() {
            self.graph.graph.remove_node(*node).unwrap();
        }
        for edge in self.new_edges.iter().rev() {
            self.graph.graph.remove_edge(*edge).unwrap();
        }

        // make sure we have removed all of the nodes and edges
        assert_eq!(self.graph.graph.node_count(), self.base_nodes);
        assert_eq!(self.graph.graph.edge_count(), self.base_edges);
    }
}

/**
 * Finds the best (lowest cost) route from `start` to `end` in `base_graph`.
 */
pub fn best_route(
    base_graph: &mut Graph,
    start: quadtree::Address,
    end: quadtree::Address,
    max_depth: u32,
) -> Result<Route, Error> {
    let mut graph = AugmentedGraph::new(base_graph);
    let inner = &mut graph.graph.graph;

    let start_node = inner.add_node(Node::StartNode { address: start });
    graph.new_nodes.push(start_node);
    let end_node = inner.add_node(Node::EndNode { address: end });
    graph.new_nodes.push(end_node);

    unimplemented!()
}
