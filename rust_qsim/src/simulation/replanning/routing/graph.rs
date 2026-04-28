use crate::simulation::id::Id;
use crate::simulation::replanning::routing::least_cost_path_caluclator::{Graph, IntNodeGraph};
use crate::simulation::scenario::network::{Link, Node};
use nohash_hasher::IntMap;
use std::collections::HashMap;

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
        assert_eq!(
            self.forward_graph.travel_time.len(),
            self.backward_graph.travel_time.len()
        );
        assert_eq!(
            self.forward_graph.head.len(),
            self.backward_graph.travel_time.len()
        );
        assert_eq!(
            self.forward_graph.first_out.len(),
            self.backward_graph.first_out.len()
        );
    }

    pub fn get_forward_travel_time_by_link_id(&self, link_id: Id<Link>) -> Option<u32> {
        let index = self.forward_link_id_pos().get(&link_id);

        //if index is None, then there is no link with link id in graph
        index.map(|i| {
            *self
                .forward_travel_time()
                .get(*i)
                .unwrap_or_else(|| panic!("There is no travel time for link {:?}", link_id))
        })
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

    pub fn forward_travel_time(&self) -> &Vec<u32> {
        &self.forward_graph.travel_time
    }

    pub fn forward_link_ids(&self) -> &Vec<Id<Link>> {
        &self.forward_graph.link_ids
    }

    pub fn backward_link_ids(&self) -> &Vec<Id<Link>> {
        &self.backward_graph.link_ids
    }

    pub fn forward_link_id_pos(&self) -> &HashMap<Id<Link>, LinkIndex> {
        &self.forward_graph.link_id_pos
    }

    pub fn number_of_nodes(&self) -> usize {
        // TODO remove, since this is now part of the Graph Trait
        self.forward_graph.first_out.len() - 1
    }

    #[cfg(test)]
    pub fn number_of_links(&self) -> usize {
        self.forward_graph.head.len()
    }

    pub fn clone_with_new_travel_times_by_link(
        &self,
        new_travel_times_by_link: HashMap<Id<Link>, u32>,
    ) -> ForwardBackwardGraph {
        ForwardBackwardGraph {
            forward_graph: self
                .forward_graph
                .clone_with_new_travel_times_by_link(&new_travel_times_by_link),
            backward_graph: self
                .backward_graph
                .clone_with_new_travel_times_by_link(&new_travel_times_by_link),
            node_id_to_node: self.node_id_to_node.clone(),
            link_id_to_link: self.link_id_to_link.clone(),
        }
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

    fn get_end_node(&self, link_id: Id<Link>) -> Id<Node> {
        let link_id_index = self.forward_link_id_pos().get(&link_id).unwrap_or_else(|| {
            panic!(
                "There is no link with id {} in the current mode graph.",
                link_id
            )
        });

        let node_idx = self.forward_head().get(*link_id_index).unwrap().clone();
        self.forward_graph.node_id_by_index[node_idx].clone()
    }

    fn get_start_node(&self, link_id: Id<Link>) -> Id<Node> {
        let link_id_index = self.forward_link_id_pos().get(&link_id).unwrap_or_else(|| {
            panic!(
                "There is no link with id {} in the current mode graph.",
                link_id
            )
        });

        let mut result = None;

        for i in 0..self.forward_first_out().len() {
            if link_id_index >= self.forward_first_out().get(i).unwrap()
                && link_id_index < self.forward_first_out().get(i + 1).unwrap()
            {
                result = Some(i);
            }
        }

        self.forward_graph.node_id_by_index[result.unwrap()].clone()
    }
}

impl IntNodeGraph for ForwardBackwardGraph {
    fn get_end_node_as_idx(&self, edge: LinkIndex) -> NodeIndex {
        self.forward_head()[edge]
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
    // TODO keep this name? had to rename so name is different from the graph trait
    // TODO remove commented old versions when done
    pub(crate) first_out: Vec<LinkIndex>, // interpret this as a map NodeIndex->LinkIndex, where firstout[i] is the (Link)Index of the first outgoing link of node i, in 'head'
    pub(crate) node_index_by_id: IntMap<Id<Node>, NodeIndex>, // maps nodes to indices (in first_out, x, y etc.)
    pub(crate) node_id_by_index: Vec<Id<Node>>, // maps (Node)Indices (in first_out, x, y etc.) to nodes
    pub(crate) head: Vec<NodeIndex>, // heads are NodeIndices, that can be transformed to Id<Node> using node_id_by_index
    #[deprecated(note = "Travel time should not be part of the graph anymore.")]
    pub(crate) travel_time: Vec<u32>,
    pub(crate) link_ids: Vec<Id<Link>>, // Vec<u64>,
    pub(crate) x: Vec<f64>,
    pub(crate) y: Vec<f64>,
    pub(crate) link_id_pos: HashMap<Id<Link>, LinkIndex>, // HashMap<u64, usize>,  // TODO should this also be an IntMap?
}

impl RoutingGraph {
    #[cfg(test)]
    fn new(
        first_out: Vec<LinkIndex>,
        node_index_by_id: HashMap<Id<Node>, NodeIndex>,
        node_id_by_index: Vec<Id<Node>>,
        head: Vec<NodeIndex>,
        travel_time: Vec<u32>,
    ) -> RoutingGraph {
        RoutingGraph {
            first_out,
            node_index_by_id,
            node_id_by_index,
            head,
            travel_time,
            link_ids: vec![],
            x: vec![],
            y: vec![],
            link_id_pos: HashMap::new(),
        }
    }

    #[tracing::instrument(level = "trace", skip(new_travel_times_by_link))]
    pub fn clone_with_new_travel_times_by_link(
        &self,
        new_travel_times_by_link: &HashMap<Id<Link>, u32>,
    ) -> RoutingGraph {
        debug_assert_eq!(self.link_ids.len(), self.travel_time.len());

        let mut new_travel_time_vector = Vec::new();
        for (index, id) in self.link_ids.iter().enumerate() {
            new_travel_time_vector.push(
                *new_travel_times_by_link
                    .get(&id.clone())
                    .unwrap_or_else(|| self.travel_time.get(index).unwrap()),
            );
        }

        self.clone_with_new_travel_times(new_travel_time_vector)
    }

    #[tracing::instrument(level = "trace", skip(travel_times))]
    fn clone_with_new_travel_times(&self, travel_times: Vec<u32>) -> RoutingGraph {
        let mut result = self.clone();
        result.travel_time = travel_times;
        result
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use crate::simulation::config::{MetisOptions, PartitionMethod};
    use crate::simulation::id::Id;
    use crate::simulation::replanning::routing::graph::{ForwardBackwardGraph, RoutingGraph};
    use crate::simulation::replanning::routing::network_converter::NetworkConverter;
    use crate::simulation::scenario::network::Network;
    use macros::integration_test;
    use metis::option::IpType::Node;
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
                HashMap::from([
                    (Id::create("0"), 0),
                    (Id::create("1"), 1),
                    (Id::create("2"), 2),
                ]),
                vec![Id::create("0"), Id::create("1"), Id::create("2")],
                vec![0, 1, 2, 3, 4, 5],
                vec![1, 1, 1, 1, 1, 1],
            ),
            RoutingGraph::new(
                vec![0, 1, 2],
                HashMap::from([
                    (Id::create("0"), 0),
                    (Id::create("1"), 1),
                    (Id::create("2"), 2),
                ]),
                vec![Id::create("0"), Id::create("1"), Id::create("2")],
                vec![0, 1, 2, 3, 4],
                vec![1, 1, 1, 1, 1],
            ),
        );
    }

    #[test]
    fn test_graph_valid() {
        ForwardBackwardGraph::new(
            RoutingGraph::new(
                vec![0, 1, 2],
                HashMap::from([
                    (Id::create("0"), 0),
                    (Id::create("1"), 1),
                    (Id::create("2"), 2),
                ]),
                vec![Id::create("0"), Id::create("1"), Id::create("2")],
                vec![0, 1, 2, 3, 4, 5],
                vec![1, 1, 1, 1, 1, 1],
            ),
            RoutingGraph::new(
                vec![42, 43, 44],
                HashMap::from([
                    (Id::create("0"), 0),
                    (Id::create("1"), 1),
                    (Id::create("2"), 2),
                ]),
                vec![Id::create("0"), Id::create("1"), Id::create("2")],
                vec![8, 10, 12, 13, 14, 15],
                vec![1, 1, 1, 1, 1, 10],
            ),
        );
    }

    #[integration_test]
    fn clone_without_change() {
        let graph = get_triangle_test_graph();
        let new_graph = graph.clone_with_new_travel_times_by_link(HashMap::new());

        assert_eq!(graph, new_graph);
    }

    #[test]
    #[ignore] //ignored because we use a global ID store now and the internal IDs are not predictable anymore
    fn clone_with_change() {
        let mut graph = get_triangle_test_graph();
        let mut change = HashMap::new();
        change.insert(Id::create("5"), 42);
        let new_graph = graph.clone_with_new_travel_times_by_link(change);

        //change manually
        graph.forward_graph.travel_time[5] = 42;
        graph.backward_graph.travel_time[3] = 42;
        assert_eq!(graph, new_graph);
    }
}
