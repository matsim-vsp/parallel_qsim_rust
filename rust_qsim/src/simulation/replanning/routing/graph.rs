use crate::simulation::id::Id;
use crate::simulation::replanning::routing::least_cost_path_caluclator::{Graph, IntNodeGraph};
use crate::simulation::scenario::network::{Link, Node};
use nohash_hasher::IntMap;
use std::collections::HashMap;
use std::fmt;

/// Error type for graph operations
#[derive(Debug, Clone, PartialEq)]
pub enum GraphError {
    LinkNotFound(Id<Link>),
    LinkIndexNotFound(LinkIndex),
    NodeNotFound(Id<Node>),
    NodeIndexNotFound(NodeIndex),
}

impl fmt::Display for GraphError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GraphError::LinkNotFound(link_id) => {
                write!(f, "There is no link with id {} in the graph.", link_id)
            }
            GraphError::NodeNotFound(node_id) => {
                write!(f, "There is no node with id {} in the graph.", node_id)
            }
            GraphError::LinkIndexNotFound(link_index) => {
                write!(
                    f,
                    "There is no link with index {} in the graph.",
                    link_index
                )
            }
            GraphError::NodeIndexNotFound(node_index) => {
                write!(
                    f,
                    "There is no node with index {} in the graph.",
                    node_index
                )
            }
        }
    }
}

impl std::error::Error for GraphError {}

#[derive(Clone, Debug, PartialEq)]
pub struct ForwardBackwardGraph {
    pub forward_graph: RoutingGraph,
    pub backward_graph: RoutingGraph,
    pub node_id_to_node: IntMap<Id<Node>, Node>,
    pub link_id_to_link: IntMap<Id<Link>, Link>,
}

impl ForwardBackwardGraph {
    pub fn new(
        forward_graph: RoutingGraph,
        backward_graph: RoutingGraph,
        node_id_to_node: IntMap<Id<Node>, Node>,
        link_id_to_link: IntMap<Id<Link>, Link>,
    ) -> Self {
        let graph = Self {
            forward_graph,
            backward_graph,
            node_id_to_node,
            link_id_to_link,
        };
        graph.validate_else_panic();
        graph
    }

    fn validate_else_panic(&self) {
        assert_eq!(
            self.forward_graph.head.len(),
            self.backward_graph.head.len()
        );
        // assert_eq!(
        //     self.forward_graph.travel_time.len(),
        //     self.backward_graph.travel_time.len()
        // );
        // assert_eq!(
        //     self.forward_graph.head.len(),
        //     self.backward_graph.travel_time.len()
        // );
        assert_eq!(
            self.forward_graph.first_out.len(),
            self.backward_graph.first_out.len()
        );
    }

    // pub fn get_forward_travel_time_by_link_id(&self, link_id: Id<Link>) -> Option<u32> {
    //     let index = self.forward_link_id_pos().get(&link_id);
    //
    //     //if index is None, then there is no link with link id in graph
    //     index.map(|i| {
    //         *self
    //             .forward_travel_time()
    //             .get(*i)
    //             .unwrap_or_else(|| panic!("There is no travel time for link {:?}", link_id))
    //     })
    // }
    pub fn forward_first_out(&self) -> &Vec<LinkIndex> {
        &self.forward_graph.first_out
    }

    pub fn backward_first_out(&self) -> &Vec<LinkIndex> {
        &self.backward_graph.first_out
    }

    pub fn forward_head(&self) -> &Vec<NodeIndex> {
        &self.forward_graph.head
    }

    pub fn backward_head(&self) -> &Vec<NodeIndex> {
        &self.backward_graph.head
    }

    // pub fn forward_travel_time(&self) -> &Vec<u32> {
    //     &self.forward_graph.travel_time
    // }

    pub fn forward_link_ids(&self) -> &Vec<Id<Link>> {
        &self.forward_graph.link_ids
    }

    pub fn backward_link_ids(&self) -> &Vec<Id<Link>> {
        &self.backward_graph.link_ids
    }

    pub fn forward_link_id_pos(&self) -> &IntMap<Id<Link>, LinkIndex> {
        &self.forward_graph.link_id_pos
    }

    #[cfg(test)]
    pub fn number_of_links(&self) -> usize {
        self.forward_graph.head.len()
    }

    pub fn get_node_id(&self, idx: NodeIndex) -> Id<Node> {
        self.forward_graph.node_id_by_index[idx].clone()
    }

    pub fn get_link_id(&self, idx: LinkIndex) -> Id<Link> {
        self.forward_graph.link_ids[idx].clone()
    }
}

impl Graph for ForwardBackwardGraph {
    fn node(&self, id: Id<Node>) -> &Node {
        self.node_id_to_node.get(&id).unwrap()
    }

    fn edge(&self, id: Id<Link>) -> &Link {
        self.link_id_to_link.get(&id).unwrap()
    }

    fn outgoing_edges(&self, node: Id<Node>) -> &[Id<Link>] {
        let node_idx = self.forward_graph.node_index_by_id[&node];
        let link_indices =
            self.forward_first_out()[node_idx]..self.forward_first_out()[node_idx + 1];
        &self.forward_link_ids()[link_indices]
    }

    fn incoming_edges(&self, node: Id<Node>) -> &[Id<Link>] {
        let node_idx = self.backward_graph.node_index_by_id[&node];
        let link_indices =
            self.backward_first_out()[node_idx]..self.backward_first_out()[node_idx + 1];
        &self.backward_link_ids()[link_indices]
    }

    fn num_nodes(&self) -> usize {
        self.forward_first_out().len() - 1
    }

    fn get_end_node(&self, link_id: Id<Link>) -> Result<Id<Node>, GraphError> {
        let link_id_index = self
            .forward_link_id_pos()
            .get(&link_id)
            .ok_or_else(|| GraphError::LinkNotFound(link_id.clone()))?;

        let node_idx = self.forward_head().get(*link_id_index).unwrap().clone();
        Ok(self.forward_graph.node_id_by_index[node_idx].clone())
    }

    fn get_start_node(&self, link_id: Id<Link>) -> Result<Id<Node>, GraphError> {
        let link_id_index = self
            .forward_link_id_pos()
            .get(&link_id)
            .ok_or_else(|| GraphError::LinkNotFound(link_id.clone()))?;

        let mut result = None;

        for i in 0..self.forward_first_out().len() {
            if link_id_index >= self.forward_first_out().get(i).unwrap()
                && link_id_index < self.forward_first_out().get(i + 1).unwrap()
            {
                result = Some(i);
            }
        }

        let node_idx = result.ok_or_else(|| GraphError::LinkNotFound(link_id.clone()))?;
        Ok(self.forward_graph.node_id_by_index[node_idx].clone())
    }
    // fn clone_box(&self) -> Box<dyn Graph> {
    //     Box::new(self.clone())
    // }
}

impl IntNodeGraph for ForwardBackwardGraph {
    fn get_end_node_as_idx(&self, edge: LinkIndex) -> Result<NodeIndex, GraphError> {
        match self.forward_head().get(edge) {
            Some(node_idx) => Ok(*node_idx),
            None => Err(GraphError::LinkIndexNotFound(edge)),
        }
    }

    fn get_start_node_as_idx(&self, edge: LinkIndex) -> Result<NodeIndex, GraphError> {
        // Find which node owns this edge as an outgoing edge by searching first_out
        for i in 0..self.forward_first_out().len() - 1 {
            if edge >= self.forward_first_out()[i] && edge < self.forward_first_out()[i + 1] {
                return Ok(i);
            }
        }
        // No node was found to own this edge, so return an error
        Err(GraphError::LinkIndexNotFound(edge))
    }
    fn get_link_idx_from_id(&self, link_id: Id<Link>) -> LinkIndex {
        self.forward_link_id_pos()[&link_id]
    }
    fn get_node_idx_from_id(&self, node_id: Id<Node>) -> NodeIndex {
        self.forward_graph.node_index_by_id[&node_id]
    }
    fn get_link_from_idx(&self, idx: LinkIndex) -> &Link {
        // uses method from Graph trait to map link id to link
        self.edge(self.get_link_id_from_idx(idx))
    }
    fn get_node_from_idx(&self, idx: NodeIndex) -> &Node {
        // uses method from Graph trait to map node id to node
        self.node(self.get_node_id_from_idx(idx))
    }
    fn outgoing_edges_as_idx(&self, node: NodeIndex) -> Vec<LinkIndex> {
        (self.forward_first_out()[node]..self.forward_first_out()[node + 1]).collect()
    }
    fn incoming_edges_as_idx(&self, node: NodeIndex) -> Vec<LinkIndex> {
        // Get the link ids from backward structure
        let backward_range = self.backward_first_out()[node]..self.backward_first_out()[node + 1];
        let backward_link_ids: Vec<_> = backward_range
            .map(|i| &self.backward_link_ids()[i])
            .collect();

        // Map backward link ids back to forward indices
        backward_link_ids
            .iter()
            .map(|id| *self.forward_link_id_pos().get(id).unwrap())
            .collect()
    }
    fn get_link_id_from_idx(&self, idx: LinkIndex) -> Id<Link> {
        self.get_link_id(idx)
    }
    fn get_node_id_from_idx(&self, idx: NodeIndex) -> Id<Node> {
        self.get_node_id(idx)
    }
}

pub type NodeIndex = usize;

pub type LinkIndex = usize;

#[derive(Clone, Debug, PartialEq)]
#[allow(dead_code)]
pub struct RoutingGraph {
    pub(crate) first_out: Vec<LinkIndex>, // interpret this as a map NodeIndex->LinkIndex, where firstout[i] is the (Link)Index of the first outgoing link of node i, in 'head'
    pub(crate) node_index_by_id: IntMap<Id<Node>, NodeIndex>, // maps nodes to indices (in first_out, x, y etc.)
    pub(crate) node_id_by_index: Vec<Id<Node>>, // maps (Node)Indices (in first_out, x, y etc.) to nodes
    pub(crate) head: Vec<NodeIndex>, // heads are NodeIndices, that can be transformed to Id<Node> using node_id_by_index
    pub(crate) link_ids: Vec<Id<Link>>,
    pub(crate) x: Vec<f64>,
    pub(crate) y: Vec<f64>,
    pub(crate) link_id_pos: IntMap<Id<Link>, LinkIndex>,
}

impl RoutingGraph {
    #[cfg(test)]
    fn new(
        first_out: Vec<LinkIndex>,
        node_index_by_id: IntMap<Id<Node>, NodeIndex>,
        node_id_by_index: Vec<Id<Node>>,
        head: Vec<NodeIndex>,
    ) -> RoutingGraph {
        RoutingGraph {
            first_out,
            node_index_by_id,
            node_id_by_index,
            head,
            // travel_time,
            link_ids: vec![],
            x: vec![],
            y: vec![],
            link_id_pos: IntMap::default(),
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use crate::simulation::config::{MetisOptions, PartitionMethod};
    use crate::simulation::id::Id;
    use crate::simulation::replanning::routing::graph::GraphError;
    use crate::simulation::replanning::routing::graph::{ForwardBackwardGraph, RoutingGraph};
    use crate::simulation::replanning::routing::least_cost_path_caluclator::{Graph, IntNodeGraph};
    use crate::simulation::replanning::routing::network_converter::NetworkConverter;
    use crate::simulation::scenario::network::{Network, Node};
    use macros::integration_test;
    use nohash_hasher::IntMap;
    use std::collections::HashMap;

    pub fn get_triangle_test_graph() -> ForwardBackwardGraph {
        let network = Network::from_file(
            "./assets/routing_tests/triangle-network.xml",
            1,
            &PartitionMethod::Metis(MetisOptions::default()),
        );
        NetworkConverter::convert_network(&network, None)
    }

    #[integration_test]
    #[should_panic]
    fn test_graph_not_valid() {
        ForwardBackwardGraph::new(
            RoutingGraph::new(
                vec![0, 1, 2],
                IntMap::from_iter([
                    (Id::create("0"), 0),
                    (Id::create("1"), 1),
                    (Id::create("2"), 2),
                ]),
                vec![Id::create("0"), Id::create("1"), Id::create("2")],
                vec![0, 1, 2, 3, 4, 5],
            ),
            RoutingGraph::new(
                vec![0, 1, 2],
                IntMap::from_iter([
                    (Id::create("0"), 0),
                    (Id::create("1"), 1),
                    (Id::create("2"), 2),
                ]),
                vec![Id::create("0"), Id::create("1"), Id::create("2")],
                vec![0, 1, 2, 3, 4],
            ),
            IntMap::default(),
            IntMap::default(),
        );
    }

    #[integration_test]
    fn test_graph_valid() {
        ForwardBackwardGraph::new(
            RoutingGraph::new(
                vec![0, 1, 2],
                IntMap::from_iter([
                    (Id::create("0"), 0),
                    (Id::create("1"), 1),
                    (Id::create("2"), 2),
                ]),
                vec![Id::create("0"), Id::create("1"), Id::create("2")],
                vec![0, 1, 2, 3, 4, 5],
            ),
            RoutingGraph::new(
                vec![42, 43, 44],
                IntMap::from_iter([
                    (Id::create("0"), 0),
                    (Id::create("1"), 1),
                    (Id::create("2"), 2),
                ]),
                vec![Id::create("0"), Id::create("1"), Id::create("2")],
                vec![8, 10, 12, 13, 14, 15],
            ),
            IntMap::default(),
            IntMap::default(),
        );
    }

    /// test outgoing_edges
    #[test]
    fn test_outgoing_edges() {
        let graph = get_triangle_test_graph();

        let node_id = Id::create("1");
        let outgoing_edges = graph.outgoing_edges(node_id);
        // verify that the found edges are the true ones for this specific graph
        assert_eq!(outgoing_edges, [Id::create("1"), Id::create("2")]);
    }

    /// test incoming_edges
    #[test]
    fn test_incoming_edges() {
        let graph = get_triangle_test_graph();

        let node_id = Id::create("3");
        let outgoing_edges = graph.incoming_edges(node_id);
        // verify that the found edges are the true ones for this specific graph
        assert_eq!(outgoing_edges, [Id::create("2"), Id::create("4")]);
    }

    /// Test if outgoing_edges_as_idx yields results consistent with outgoing_edges
    #[test]
    fn test_outgoing_edges_as_idx() {
        let graph = get_triangle_test_graph();

        let node_id = Id::<Node>::create("1");
        let node_idx = graph.get_node_idx_from_id(node_id.clone());

        let outgoing_indices = graph.outgoing_edges_as_idx(node_idx);
        let outgoing_ids = graph.outgoing_edges(node_id);

        // Lengths should be equal
        assert_eq!(
            outgoing_indices.len(),
            outgoing_ids.len(),
            "outgoing_edges_as_idx and outgoing_edges should have same length"
        );
        for (i, index) in outgoing_indices.iter().enumerate() {
            // Check whether the link id associated with each LinkIndex returned from
            // outgoing_edges_as_idx() matches the link ids returned from outgoing_edges()
            assert_eq!(graph.get_link_id(*index), outgoing_ids[i]);
        }
    }

    /// Test if incoming_edges_as_idx gives results consistent with incoming_edges
    #[test]
    fn test_incoming_edges_as_idx() {
        let graph = get_triangle_test_graph();

        let node_id = Id::<Node>::create("2");
        let node_idx = graph.get_node_idx_from_id(node_id.clone());

        let incoming_indices = graph.incoming_edges_as_idx(node_idx);
        let incoming_ids = graph.incoming_edges(node_id);

        // Lengths should be equal
        assert_eq!(
            incoming_indices.len(),
            incoming_ids.len(),
            "incoming_edges_as_idx and incoming_edges should have same length"
        );
        for (i, index) in incoming_indices.iter().enumerate() {
            // Check whether the link id associated with each LinkIndex returned from
            // incoming_edges_as_idx() matches the link ids returned from incoming_edges()
            assert_eq!(graph.get_link_id(*index), incoming_ids[i]);
        }
    }

    /// Test get_end_node with valid link
    #[test]
    fn test_get_end_node_valid_link() {
        let graph = get_triangle_test_graph();

        // Test with various valid links that exist in the graph
        let outgoing_links = graph.outgoing_edges(Id::<Node>::create("1"));
        assert!(
            !outgoing_links.is_empty(),
            "Node 1 should have outgoing links"
        );
        let true_end_nodes = vec![Id::<Node>::create("2"), Id::<Node>::create("3")];

        for (link_id, true_end_node) in outgoing_links.iter().zip(true_end_nodes) {
            let result = graph.get_end_node(link_id.clone());
            assert!(result.is_ok(), "get_end_node should succeed for valid link");
            let end_node = result.unwrap();
            // End node should match the expected node
            assert_eq!(end_node, true_end_node);
        }
    }

    /// Test get_end_node with invalid link (GraphError)
    #[test]
    fn test_get_end_node_invalid_link_returns_error() {
        let graph = get_triangle_test_graph();

        let invalid_link_id = Id::create("nonexistent_link");
        let result = graph.get_end_node(invalid_link_id.clone());

        assert!(
            result.is_err(),
            "get_end_node should return error for invalid link"
        );
        match result.unwrap_err() {
            GraphError::LinkNotFound(_) => {} // Expected
            _ => panic!("Expected LinkNotFound error"),
        }
    }

    /// Test get_start_node with valid link
    #[test]
    fn test_get_start_node_valid_link() {
        let graph = get_triangle_test_graph();

        let outgoing_links = graph.outgoing_edges(Id::<Node>::create("1"));
        assert!(!outgoing_links.is_empty());

        for link_id in outgoing_links {
            let result = graph.get_start_node(link_id.clone());
            assert!(
                result.is_ok(),
                "get_start_node should succeed for valid link"
            );
            let start_node = result.unwrap();
            assert_eq!(start_node, Id::<Node>::create("1"));
        }
    }

    /// Test get_start_node with invalid link
    #[test]
    fn test_get_start_node_invalid_link_returns_error() {
        let graph = get_triangle_test_graph();

        let invalid_link_id = Id::create("nonexistent_link");
        let result = graph.get_start_node(invalid_link_id.clone());

        assert!(
            result.is_err(),
            "get_start_node should return error for invalid link"
        );
    }

    /// Test roundtrip: Link ID -> Index -> Link ID should be equal
    #[test]
    fn test_link_id_to_idx_roundtrip() {
        let graph = get_triangle_test_graph();

        let outgoing_links = graph.outgoing_edges(Id::<Node>::create("1"));
        for link_id in outgoing_links {
            // Link ID -> Link Index
            let link_idx = graph.get_link_idx_from_id(link_id.clone());
            // Link Index -> Link ID
            let link_id_roundtrip = graph.get_link_id_from_idx(link_idx);

            assert_eq!(
                *link_id, link_id_roundtrip,
                "Link ID roundtrip should be idempotent"
            );
        }
    }

    /// Test roundtrip: Node ID -> Index -> Node ID should be equal
    #[test]
    fn test_node_id_to_idx_roundtrip() {
        let graph = get_triangle_test_graph();

        let node_id = Id::<Node>::create("1");
        // Node ID -> Node Index
        let node_idx = graph.get_node_idx_from_id(node_id.clone());
        // Node Index -> Node ID
        let node_id_roundtrip = graph.get_node_id_from_idx(node_idx);

        assert_eq!(
            node_id, node_id_roundtrip,
            "Node ID roundtrip should be idempotent"
        );
    }

    #[test]
    fn test_get_end_node_as_idx_valid_edges() {
        let graph = get_triangle_test_graph();
        let true_end_node_indices = vec![vec![], vec![2, 3], vec![2, 3], vec![1, 2]];

        // For all outgoing edges, get_end_node_as_idx should not panic, since they are all valid (exist in the graph)
        for node_idx in 0..graph.num_nodes() {
            let outgoing_edge_indices = graph.outgoing_edges_as_idx(node_idx);
            for (j, edge_idx) in outgoing_edge_indices.iter().enumerate() {
                let end_node = graph.get_end_node_as_idx(*edge_idx).unwrap(); // Should not panic
                assert_eq!(
                    end_node, true_end_node_indices[node_idx][j],
                    "End node is incorrect, expected {}, got {}",
                    true_end_node_indices[node_idx][j], end_node
                );
            }
        }
    }

    #[test]
    fn test_get_end_node_as_idx_invalid_edge() {
        let graph = get_triangle_test_graph();
        assert_eq!(
            graph.get_end_node_as_idx(999),
            Err(GraphError::LinkIndexNotFound(999)),
            "get_end_node_as_idx should return LinkIndexNotFound error for invalid edge index"
        );
    }

    #[test]
    fn test_get_start_node_as_idx_invalid_edge() {
        let graph = get_triangle_test_graph();
        assert_eq!(
            graph.get_start_node_as_idx(999),
            Err(GraphError::LinkIndexNotFound(999)),
            "get_start_node_as_idx should return LinkIndexNotFound error for invalid edge index"
        );
    }

    #[test]
    fn test_get_start_node_as_idx() {
        let graph = get_triangle_test_graph();

        // For all outgoing edges, get_start_node_as_idx should not panic
        for node_idx in 0..graph.num_nodes() {
            let outgoing_edge_indices = graph.outgoing_edges_as_idx(node_idx);
            for edge_idx in outgoing_edge_indices {
                let start_node = graph.get_start_node_as_idx(edge_idx).unwrap(); // Should not panic
                assert_eq!(
                    start_node, node_idx,
                    "Start node should match the source node"
                );
            }
        }
    }
}
//
// #[derive(Debug, Clone)]
// pub enum NodeIdxOptions {
//     One(NodeIndex), // one specific node in the graph
//     Any,            // any node
// }
//
// impl NodeIdxOptions {
//     pub(crate) fn get_node_or_panic(&self) -> NodeIndex {
//         match self {
//             NodeIdxOptions::One(node_idx) => *node_idx,
//             NodeIdxOptions::Any => panic!("NodeIdxOptions::Any does not contain a specific node."),
//         }
//     }
// }
//
// #[derive(Debug)]
// pub enum NodeIdOptions {
//     One(Id<Node>), // one specific node in the graph
//     Any,           // any node
// }
//
// impl NodeIdOptions {
//     pub(crate) fn get_node_or_panic(&self) -> Id<Node> {
//         match self {
//             NodeIdOptions::One(node_id) => node_id.clone(),
//             NodeIdOptions::Any => panic!("NodeIdOptions::Any does not contain a specific node."),
//         }
//     }
// }
