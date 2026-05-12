use crate::simulation::id::Id;
use crate::simulation::replanning::routing::least_cost_path_caluclator::{Graph, IndexableGraph};
use crate::simulation::scenario::network::{Link, Node};
use nohash_hasher::IntMap;
use std::fmt;

/// Error type for graph operations. Has variants for when node ids or indices, or link ids or
/// indices, are passed to be used in some operation but are not found in the graph on which the
/// operation takes place.
#[derive(Debug, Clone, PartialEq)]
pub enum GraphError {
    LinkIdNotFound(Id<Link>),
    LinkIndexNotFound(LinkIndex),
    NodeIdNotFound(Id<Node>),
    NodeIndexNotFound(NodeIndex),
}

impl fmt::Display for GraphError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GraphError::LinkIdNotFound(link_id) => {
                write!(f, "There is no link with id {} in the graph.", link_id)
            }
            GraphError::NodeIdNotFound(node_id) => {
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

/// An implementation of `Graph` and `IndexableGraph`, i.e., a directed graph with network node ids
/// as nodes, and network link ids as edges.
/// The implementation works by storing two directed `RoutingGraph`s internally, a forward and a
/// backward graph, with flipped directions on the edges, respectively. The forward graph has the
/// true directions and the backward graph the reversed directions of the edges.
/// This allows easy retrieval of incoming edges, even though the internally stored graphs are
/// stored in a way optimized for retrieving outgoing edges.
/// Also contains maps from node and link ids to the actual network nodes and links.
/// TODO add info about forward and backward graph/labeling/indexing of the edges etc, once I have figured that out
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
        // ensure forward and backward graphs have same amount of edges
        assert_eq!(
            self.forward_graph.head.len(),
            self.backward_graph.head.len()
        );
        // ensure forward and backward graphs have same amount of nodes
        assert_eq!(
            self.forward_graph.first_out.len(),
            self.backward_graph.first_out.len()
        );
    }

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

    // TODO is this correct? it is tested, but doesn't seem to be used anywhere else
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
        // TODO this shoudl be the get_link_idx_from_id function actually
        let link_id_index = self
            .forward_link_id_pos()
            .get(&link_id)
            .ok_or_else(|| GraphError::LinkIdNotFound(link_id.clone()))?;

        let node_idx = self.forward_head().get(*link_id_index).unwrap().clone();
        Ok(self.forward_graph.node_id_by_index[node_idx].clone())
    }

    fn get_start_node(&self, link_id: Id<Link>) -> Result<Id<Node>, GraphError> {
        // TODO should be get_link_idx_from_id
        let link_id_index = self
            .forward_link_id_pos()
            .get(&link_id)
            .ok_or_else(|| GraphError::LinkIdNotFound(link_id.clone()))?;

        let mut result = None;

        // FIXME this can't be the optimal way to handle this, when we in fact have a backward graph!
        for i in 0..self.forward_first_out().len() {
            if link_id_index >= self.forward_first_out().get(i).unwrap()
                && link_id_index < self.forward_first_out().get(i + 1).unwrap()
            {
                result = Some(i);
            }
        }

        let node_idx = result.ok_or_else(|| GraphError::LinkIdNotFound(link_id.clone()))?;
        Ok(self.forward_graph.node_id_by_index[node_idx].clone())
    }
}

impl IndexableGraph for ForwardBackwardGraph {
    fn get_end_node_as_idx(&self, edge: LinkIndex) -> Result<NodeIndex, GraphError> {
        match self.forward_head().get(edge) {
            Some(node_idx) => Ok(*node_idx),
            None => Err(GraphError::LinkIndexNotFound(edge)),
        }
    }

    fn get_start_node_as_idx(&self, edge: LinkIndex) -> Result<NodeIndex, GraphError> {
        // FIXME again, I can't imagine this to be optimal when we have a backward graph?
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
        // TODO check if this function makes sense as well
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

/// An index of a node in an `IndexableGraph`
pub type NodeIndex = usize;

/// An index of a link in an `IndexableGraph`
pub type LinkIndex = usize;

/// A directed graph with network node ids as nodes, and network link ids as edges, used for routing
/// on the network.
///
/// Stored in a way optimized for retrieving outgoing edges. Namely:
/// - `first_out` is a vector such that `first_out[i]` is the index of the first outgoing edge of
///     node i in `head`, and `first_out[i+1]` is the index of the first outgoing edge of node i+1,
///     so that the outgoing edges of node i are exactly those in
///     `head[first_out[i]..first_out[i+1]]`.
/// - `head` is a vector such that `head[j]` is the node index of the end node of the edge with
///     index j
/// The graph also contains vectors and maps allowing to go from node index to network node id, and
/// from link index to network link id, and vice versa.
/// Also contains x and y coordinates, that are not used in routing.
#[derive(Clone, Debug, PartialEq)]
#[allow(dead_code)]
pub struct RoutingGraph {
    /// a vector such that `first_out[i]` is the index of the first outgoing edge of
    /// node i in `head`, and `first_out[i+1]` is the index of the first outgoing edge of node i+1,
    /// so that the outgoing edges of node i are exactly those in
    /// `head[first_out[i]..first_out[i+1]]`.
    pub(crate) first_out: Vec<LinkIndex>,
    pub(crate) node_index_by_id: IntMap<Id<Node>, NodeIndex>, // maps node ids to indices (in first_out, x, y etc.)
    pub(crate) node_id_by_index: Vec<Id<Node>>, // maps (Node)Indices (in first_out, x, y etc.) to node ids
    /// a vector such that `head[j]` is the node index of the end node of the edge with index j
    pub(crate) head: Vec<NodeIndex>,
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
    use crate::simulation::replanning::routing::least_cost_path_caluclator::{
        Graph, IndexableGraph,
    };
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
        let incoming_edges = graph.incoming_edges(node_id);
        // verify that the found edges are the true ones for this specific graph
        assert_eq!(incoming_edges, [Id::create("2"), Id::create("4")]);
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
            GraphError::LinkIdNotFound(_) => {} // Expected
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
