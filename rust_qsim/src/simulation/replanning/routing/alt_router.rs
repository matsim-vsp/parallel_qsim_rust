use crate::simulation::id::Id;
use crate::simulation::replanning::routing::alt_landmark_data::AltLandmarkData;
use crate::simulation::replanning::routing::dijsktra::{Dijkstra, DijkstraActions, Distance};
use crate::simulation::replanning::routing::graph;
use crate::simulation::replanning::routing::graph::{ForwardBackwardGraph, LinkIndex, NodeIndex};
use crate::simulation::replanning::routing::least_cost_path_caluclator::{
    CustomQueryResult, Graph, IntNodeGraph, LeastCostPath, LeastCostPathCalculator,
    LeastCostPathRequest, Time, TravelDisutility,
};
use crate::simulation::scenario::network::{Link, Node};
use keyed_priority_queue::{Entry, KeyedPriorityQueue};
use ordered_float::OrderedFloat;
use std::cmp::Reverse;

/// Shorthand for `Reverse<OrderedFloat<f64>>`, i.e., an ordered float (implements Eq and Ord,
/// unlike f64) which is sorted in reverse order.
/// To be used in KeyedPriorityQueues in Dijkstra, since the queue prefers large values while we
/// prefer small values.
#[derive(Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct NodePriority {
    priority: Reverse<OrderedFloat<f64>>,
}

impl NodePriority {
    pub fn new(priority: f64) -> Self {
        NodePriority {
            priority: Reverse(OrderedFloat(priority)),
        }
    }

    pub fn get(&self) -> f64 {
        self.priority.0.into_inner()
    }
}

pub(crate) struct AltOptions {
    to_node: NodeIndex,
    parents: Vec<Option<NodeIndex>>,
}

impl DijkstraActions for AltOptions {
    fn reached_end(&self, current_node: NodeIndex) -> bool {
        self.to_node == current_node
    }
    fn set_parent_opt(&mut self, child: NodeIndex, parent: NodeIndex) {
        self.parents[child] = Some(parent);
    }
    fn get_parents_opt(&self) -> Option<Vec<Option<NodeIndex>>> {
        Some(self.parents.clone())
    }
}

/// Initialize the priority queue and distances vector for Dijkstra/A* search
pub(crate) fn create_initial_queue(
    node_count: usize,
    from: NodeIndex,
) -> (KeyedPriorityQueue<NodeIndex, NodePriority>, Vec<f64>) {
    let mut queue = KeyedPriorityQueue::new();
    let mut node_priorities = Vec::new();
    for node in 0..node_count {
        let node_index: NodeIndex = node;
        let node_priority = if node_index == from {
            NodePriority::new(0f64)
        } else {
            NodePriority::new(f64::INFINITY)
        };
        node_priorities.push(node_priority.0);
        queue.push(node_index, node_priority);
    }
    (queue, node_priorities)
}

pub struct AStarRouter<H: AStarHeuristic> {
    heuristic: H,
    travel_disutility: Box<dyn TravelDisutility>,
}

impl<H: AStarHeuristic> AStarRouter<H> {
    pub(crate) fn get_initial_queue(
        node_count: usize,
        from: NodeIndex,
    ) -> (KeyedPriorityQueue<NodeIndex, NodePriority>, Vec<f64>) {
        create_initial_queue(node_count, from)
    }

    fn extract_node_path(to: NodeIndex, parent: Vec<Option<NodeIndex>>) -> Vec<NodeIndex> {
        let mut node_path = Vec::new();
        let mut current = to;

        node_path.push(to);
        while let Some(father) = parent[current] {
            node_path.push(father);
            current = father;
        }

        node_path.reverse();

        node_path
    }

    fn extract_link_path(
        to: NodeIndex,
        parent: Vec<Option<NodeIndex>>,
        graph: &(impl IntNodeGraph + ?Sized),
    ) -> Vec<Id<Link>> {
        let node_path = Self::extract_node_path(to, parent);

        let mut link_path = Vec::new();

        // look for link connecting node i and node i+1
        for i in 0..node_path.len() - 1 {
            let from_node = node_path[i];
            let to_node = node_path[i + 1];

            // go through outgoing edges of "from_node" and find the one that has to_node as head
            for j in graph.outgoing_edges_as_idx(from_node) {
                if graph.get_end_node_as_idx(j) == to_node {
                    // get actual Id<Link> of the link connecting from_node and to_node
                    link_path.push(graph.get_link_id_from_idx(j));
                    break;
                }
            }
        }
        link_path
    }
}

pub trait AStarHeuristic {
    fn estimate(&self, graph: &dyn IntNodeGraph, from: Id<Node>, to: Id<Node>) -> Time;
}

// with this, the A* collapses into Dijkstra
pub(crate) struct ZeroHeuristic;

impl AStarHeuristic for ZeroHeuristic {
    fn estimate(&self, graph: &dyn IntNodeGraph, from: Id<Node>, to: Id<Node>) -> Time {
        0.
    }
}

impl<H: AStarHeuristic> AStarRouter<H> {
    pub fn new(heuristic: H, travel_disutility: Box<dyn TravelDisutility>) -> Self {
        AStarRouter {
            heuristic,
            travel_disutility,
        }
    }
}

impl<H: AStarHeuristic> LeastCostPathCalculator for AStarRouter<H> {
    fn calc_route<G: IntNodeGraph>(
        &mut self,
        request: LeastCostPathRequest<G>,
    ) -> Option<LeastCostPath> {
        let from_node = request
            .graph
            .get_node_idx_from_id(request.graph.get_end_node(request.from));
        let to_node = request
            .graph
            .get_node_idx_from_id(request.graph.get_start_node(request.to));

        let mut parents = Vec::new();

        let distance_fn = |i: &Link| {
            self.travel_disutility(i, request.departure_time, request.person, request.vehicle)
        };

        (_, Some(parents)) = Dijkstra::dijkstra_core(
            self.heuristic,
            self.travel_disutility, // FIXME this shouldn't work
            request.from,
            request.to,
            request.graph,
            AltOptions { to_node, parents },
        );

        let number_of_nodes = request.graph.num_nodes();

        // TODO think about how I want the graph interface with the trait and so on
        // the algorithm should work with indices I guess, so that it is fast
        // but that means, if the request allows any dyn Graph, then the trait must somehow include
        // methods for that.
        // TODO continue by checking where the indices are actually used here

        // Note: possibly, one could define another trait (subtrait of Graph), specific to the
        // storage structure, so that the A* can rely on the methods of that trait, and then require that the graph in the request implements that trait.

        let (mut queue, mut distances) = Self::get_initial_queue(number_of_nodes, from_node);
        let mut parents: Vec<Option<NodeIndex>> = (0..number_of_nodes).map(|_| None).collect();

        while let Some((current_id, _)) = queue.pop() {
            // TODO should it still be called current distance? what do we want to name it?
            // TODO Note: it is in practive always a travel disutility, because that's what we require in the request.
            // However, it could be argued that distance is a good name because that's what is usually said in the context of A*
            let current_distance = distances[current_id];

            if current_distance == f64::INFINITY || current_distance == f64::NAN {
                // TODO do we want this? Or is there a better way?
                //The smallest value in queue was unreachable. So abort here.
                return None;
            }

            if current_id == to_node {
                return Some(LeastCostPath {
                    path: Self::extract_link_path(to_node, parents, request.graph),
                    travel_time: current_distance,
                });
            }

            // let begin_index_adjacent_nodes = request.graph.forward_graph.first_out[current_id];
            // let end_index_adjacent_nodes = request.graph.forward_graph.first_out[current_id + 1];

            // for i in begin_index_adjacent_nodes..end_index_adjacent_nodes {
            for i in request.graph.outgoing_edges_as_idx(current_id) {
                //we need an update_or_insert + parent update here instead of push always.

                // let neighbour = request.graph.forward_graph.head[i];
                let neighbour = request.graph.get_end_node_as_idx(i);

                if let Entry::Vacant(_) = queue.entry(neighbour) {
                    continue;
                }

                let link_id_i = request.graph.get_link_id_from_idx(i);
                let link_i = request.graph.edge(link_id_i);

                // TODO is it correct to use the departure time from the request here? -> NO!
                // or could it be later by now?
                let neighbour_distance = current_distance
                    + self.travel_disutility.travel_disutility(
                        link_i,
                        request.departure_time,
                        request.person,
                        request.vehicle,
                    ); // (request.graph.forward_graph.travel_time[i] as f64);

                if distances[neighbour] > neighbour_distance {
                    //perform update
                    distances[neighbour] = neighbour_distance;

                    match queue.entry(neighbour) {
                        Entry::Occupied(e) => {
                            e.set_priority(NodePriority::new(
                                neighbour_distance
                                    + self.heuristic.estimate(
                                        request.graph,
                                        request.graph.get_node_id_from_idx(neighbour),
                                        request.graph.get_node_id_from_idx(to_node),
                                    ), // TODO remove when sure that not needed: &self.landmark_data),
                            ));
                        }
                        Entry::Vacant(_) => {
                            unreachable!()
                        }
                    }

                    parents[neighbour] = Some(current_id);
                }
            }
        }
        // AltQueryResult::empty()
        None
    }
}

/// Heuristic that uses landmarks and triangle inequality to estimate
struct AltHeuristic {
    landmark_data: AltLandmarkData,
    // some internal state
}

impl AStarHeuristic for AltHeuristic {
    fn estimate(&self, graph: &dyn IntNodeGraph, from: Id<Node>, to: Id<Node>) -> Time {
        /* The ALT algorithm uses two lower bounds for each Landmark:
         * given: source node S, target node T, landmark L
         * then, due to the triangle inequality:
         *  1) ST + TL >= SL --> ST >= SL - TL (forward estimate)
         *  2) LS + ST >= LT --> ST >= LT - LS (backward estimate)
         * The algorithm is interested in the largest possible value of (SL-TL) and (LT-LS),
         * as this gives the closest approximation for the minimal travel time required to
         * go from S to T.
         */
        let from_idx = graph.get_node_idx_from_id(from);
        let to_idx = graph.get_node_idx_from_id(to);

        let mut h = 0;
        for l in self.landmark_data.travel_times_to_all() {
            let from_distance = l[from_idx]; // (SL,LS)
            let to_distance = l[to_idx]; // (LT,TL)

            let forward_estimate = from_distance.0 as i32 - to_distance.1 as i32;
            let backward_estimate = to_distance.0 as i32 - from_distance.1 as i32;

            h = h.max(forward_estimate.max(backward_estimate))
        }
        if h < 0 { 0 as Time } else { h as Time }
    }
}

#[derive(PartialEq, Debug)]
#[deprecated]
struct AltQueryResult {
    travel_time: Option<u32>,
    node_path: Option<Vec<usize>>,
}

#[deprecated]
impl AltQueryResult {
    fn empty() -> Self {
        Self {
            travel_time: None,
            node_path: None,
        }
    }

    fn node_path(self) -> Option<Vec<usize>> {
        self.node_path
    }
}

#[deprecated]
pub struct AltRouter {
    pub landmark_data: AltLandmarkData,
    pub current_graph: ForwardBackwardGraph,
    pub initial_graph: ForwardBackwardGraph,
}

impl AltRouter {
    pub fn new(graph: ForwardBackwardGraph) -> Self {
        let landmark_data = AltLandmarkData::new(&graph);
        AltRouter {
            landmark_data,
            current_graph: graph.clone(),
            initial_graph: graph,
        }
    }

    pub fn query_links(&self, from_link: u64, to_link: u64) -> CustomQueryResult {
        let travel_time;
        let result_edge_path;
        {
            let result = self.query(self.get_end_node(from_link), self.get_start_node(to_link));
            travel_time = result.travel_time;
            result_edge_path = result.node_path();
        }
        let edge_path = result_edge_path
            .map(|node_path| Self::get_edge_path(node_path, &self.current_graph))
            .map(|mut path| {
                //add from link at the beginning and to link at the end
                path.insert(0, from_link);
                path.push(to_link);
                path
            });

        CustomQueryResult {
            travel_time,
            path: edge_path,
        }
    }

    fn query(&self, from: usize, to: usize) -> AltQueryResult {
        let number_of_nodes = self.current_graph.forward_first_out().len() - 1;
        let (mut queue, mut distances) = Dijkstra::get_initial_queue(number_of_nodes, from);
        let mut parents: Vec<Option<usize>> = (0..number_of_nodes).map(|_| None).collect();

        while let Some((current_id, _)) = queue.pop() {
            let current_distance = distances[current_id];

            if current_distance == u32::MAX {
                //The smallest value in queue was unreachable. So abort here.
                return AltQueryResult::empty();
            }

            if current_id == to {
                return AltQueryResult {
                    travel_time: Some(current_distance),
                    node_path: Some(Self::extract_path(to, parents)),
                };
            }

            let begin_index_adjacent_nodes = self.current_graph.forward_graph.first_out[current_id];
            let end_index_adjacent_nodes =
                self.current_graph.forward_graph.first_out[current_id + 1];

            for i in begin_index_adjacent_nodes..end_index_adjacent_nodes {
                //we need an update_or_insert + parent update here instead of push always.
                let neighbour = self.current_graph.forward_graph.head[i];

                if let Entry::Vacant(_) = queue.entry(neighbour) {
                    continue;
                }

                let neighbour_distance =
                    current_distance + self.current_graph.forward_graph.travel_time[i];

                if distances[neighbour] > neighbour_distance {
                    //perform update
                    distances[neighbour] = neighbour_distance;

                    match queue.entry(neighbour) {
                        Entry::Occupied(e) => {
                            e.set_priority(Distance(
                                neighbour_distance
                                    + Self::heuristic(neighbour, to, &self.landmark_data),
                            ));
                        }
                        Entry::Vacant(_) => {
                            unreachable!()
                        }
                    }

                    parents[neighbour] = Some(current_id);
                }
            }
        }
        AltQueryResult::empty()
    }

    fn heuristic(node: usize, goal: usize, landmark_data: &AltLandmarkData) -> u32 {
        /* The ALT algorithm uses two lower bounds for each Landmark:
         * given: source node S, target node T, landmark L
         * then, due to the triangle inequality:
         *  1) ST + TL >= SL --> ST >= SL - TL (forward estimate)
         *  2) LS + ST >= LT --> ST >= LT - LS (backward estimate)
         * The algorithm is interested in the largest possible value of (SL-TL) and (LT-LS),
         * as this gives the closest approximation for the minimal travel time required to
         * go from S to T.
         */
        let mut h = 0;
        for l in landmark_data.travel_times_to_all() {
            let node_distance = l[node]; // (SL,LS)
            let goal_distance = l[goal]; // (LT,TL)

            let forward_estimate = node_distance.0 as i32 - goal_distance.1 as i32;
            let backward_estimate = goal_distance.0 as i32 - node_distance.1 as i32;

            h = h.max(forward_estimate.max(backward_estimate))
        }
        if h < 0 { 0 } else { h as u32 }
    }

    fn extract_path(to: usize, parent: Vec<Option<usize>>) -> Vec<usize> {
        let mut path = Vec::new();
        let mut current = to;

        path.push(to);
        while let Some(father) = parent[current] {
            path.push(father);
            current = father;
        }

        path.reverse();
        path
    }

    pub fn update(&mut self, new_graph: ForwardBackwardGraph) {
        self.current_graph = new_graph;
    }

    fn get_end_node(&self, link_id: u64) -> usize {
        let link_id_index = *self
            .current_graph
            .forward_link_id_pos()
            .get(&link_id)
            .unwrap_or_else(|| {
                panic!(
                    "There is no link with id {} in the current mode graph.",
                    link_id
                )
            });
        *self
            .current_graph
            .forward_head()
            .get(link_id_index)
            .unwrap()
    }

    fn get_start_node(&self, link_id: u64) -> usize {
        let link_id_index = *self
            .current_graph
            .forward_link_id_pos()
            .get(&link_id)
            .unwrap_or_else(|| {
                panic!(
                    "There is no link with id {} in the current mode graph.",
                    link_id
                )
            });

        let mut result = None;
        for i in 0..self.current_graph.forward_first_out().len() {
            if link_id_index >= *self.current_graph.forward_first_out().get(i).unwrap()
                && link_id_index < *self.current_graph.forward_first_out().get(i + 1).unwrap()
            {
                result = Some(i);
            }
        }

        result.unwrap()
    }

    pub fn current_graph(&self) -> &ForwardBackwardGraph {
        &self.current_graph
    }

    pub fn get_initial_travel_time(&self, link_id: u64) -> Option<u32> {
        self.initial_graph
            .get_forward_travel_time_by_link_id(link_id)
    }

    pub fn get_current_travel_time(&self, link_id: u64) -> Option<u32> {
        self.current_graph
            .get_forward_travel_time_by_link_id(link_id)
    }

    fn get_edge_path(path: Vec<usize>, graph: &ForwardBackwardGraph) -> Vec<u64> {
        let mut res = Vec::new();
        let mut last_node: Option<usize> = None;
        for node in path {
            match last_node {
                None => last_node = Some(node),
                Some(n) => {
                    let first_out_index = *graph.forward_first_out().get(n).unwrap();
                    let last_out_index = graph.forward_first_out().get(n + 1).unwrap() - 1;
                    res.push(Self::find_edge_id_of_outgoing(
                        first_out_index,
                        last_out_index,
                        node,
                        graph,
                    ));
                    last_node = Some(node)
                }
            }
        }
        res
    }

    fn find_edge_id_of_outgoing(
        first_out_index: usize,
        last_out_index: usize,
        next_node: usize,
        graph: &ForwardBackwardGraph,
    ) -> u64 {
        assert!(
            last_out_index as i64 - first_out_index as i64 >= 0,
            "No outgoing edges!"
        );
        let mut result = None;
        for i in first_out_index..=last_out_index {
            if *graph.forward_head().get(i).unwrap() == next_node {
                result = Some(*graph.forward_link_ids().get(i).unwrap());
                break;
            }
        }
        result.expect("No outgoing edge found!")
    }

    fn distance_one_2_many(
        from: usize,
        graph: &crate::simulation::replanning::routing::graph::RoutingGraph,
    ) -> Vec<u32> {
        let (mut queue, mut distances) = get_initial_queue(graph.first_out.len() - 1, from);

        while let Some((current_id, current_distance)) = queue.pop() {
            if current_distance.get() == u32::MAX {
                //The smallest value in queue was unreachable. So abort here.
                return distances;
            }

            let begin_index_adjacent_nodes = graph.first_out[current_id];
            let end_index_adjacent_nodes = graph.first_out[current_id + 1];

            for i in begin_index_adjacent_nodes..end_index_adjacent_nodes {
                //we need an update_or_insert + parent update here instead of push always.
                let neighbour = graph.head[i];

                if let Entry::Vacant(_) = queue.entry(neighbour) {
                    continue;
                }

                if queue.get_priority(&neighbour).unwrap().get()
                    > current_distance.get() + graph.travel_time[i]
                {
                    //perform update
                    match queue.entry(neighbour) {
                        Entry::Occupied(e) => {
                            e.set_priority(Distance(current_distance.get() + graph.travel_time[i]));
                        }
                        Entry::Vacant(_) => {
                            unreachable!();
                        }
                    }
                    //store in distance vec to return
                    distances[neighbour] = current_distance.get() + graph.travel_time[i];
                }
            }
        }
        distances
    }

    pub fn get_initial_queue(
        nodes: Vec<Id<Node>>, // : usize, TODO remove commented
        from: Id<Node>,       // usize,
    ) -> (KeyedPriorityQueue<Id<Node>, NodePriority>, Vec<u32>) {
        let mut queue = KeyedPriorityQueue::new();
        let mut distances = Vec::new();
        for node in nodes {
            // in 0..node_count {
            let distance = if node == from {
                //update start node
                NodePriority(0)
                // Distance(0)
            } else {
                Distance(u32::MAX)
            };
            distances.push(distance.0);
            queue.push(node, distance);
        }
        (queue, distances)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::path::PathBuf;

    use crate::simulation::config::{MetisOptions, PartitionMethod};
    use crate::simulation::id::Id;
    use crate::simulation::replanning::routing::alt_router::{AltQueryResult, AltRouter};
    use crate::simulation::replanning::routing::graph::tests::get_triangle_test_graph;
    use crate::simulation::replanning::routing::network_converter::NetworkConverter;
    use crate::simulation::scenario::network::Network;
    use crate::simulation::scenario::vehicles::Garage;
    use crate::simulation::scenario::vehicles::InternalVehicleType;

    fn query_and_check(
        router: &AltRouter,
        from: usize,
        to: usize,
        expected_travel_time: Option<u32>,
        expected_path: Option<Vec<usize>>,
    ) {
        let result = router.query(from, to);
        assert_eq!(
            result,
            AltQueryResult {
                travel_time: expected_travel_time,
                node_path: expected_path,
            }
        )
    }

    #[test]
    #[ignore] //ignored because we use a global ID store now and the internal IDs are not predictable anymore
    fn test_simple_alt_routing() {
        let graph = get_triangle_test_graph();
        let router = AltRouter::new(graph);

        query_and_check(&router, 2, 1, Some(6), Some(vec![2, 3, 1]));
        query_and_check(&router, 3, 2, Some(3), Some(vec![3, 1, 2]));
        query_and_check(&router, 2, 3, Some(4), Some(vec![2, 3]));
        query_and_check(&router, 0, 1, None, None);
    }

    #[test]
    #[ignore] //ignored because we use a global ID store now and the internal IDs are not predictable anymore
    fn test_mode_alt_routing() {
        let network = Network::from_file(
            "./assets/adhoc_routing/no_updates/network.xml",
            1,
            &PartitionMethod::Metis(MetisOptions::default()),
        );
        let garage = Garage::from_file(&PathBuf::from(
            "./assets/adhoc_routing/no_updates/vehicles.xml",
        ));

        let graph_by_vehicle_type =
            NetworkConverter::convert_network_with_vehicle_types(&network, &garage.vehicle_types);

        let bike_id = &Id::<InternalVehicleType>::get_from_ext("bike");
        let car_id = &Id::<InternalVehicleType>::get_from_ext("car");

        let router_by_vehicle_type = graph_by_vehicle_type
            .into_iter()
            .map(|(id, g)| (id, AltRouter::new(g)))
            .collect::<HashMap<_, _>>();

        // check routing for bike
        query_and_check(
            router_by_vehicle_type.get(bike_id).unwrap(),
            0,
            5,
            Some(280),
            Some(vec![0, 1, 2, 3, 4, 5]),
        );

        // check routing for car
        query_and_check(
            router_by_vehicle_type.get(car_id).unwrap(),
            0,
            5,
            Some(120),
            Some(vec![0, 1, 6, 4, 5]),
        )
    }
}
