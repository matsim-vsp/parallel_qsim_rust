use crate::simulation::id::Id;
use crate::simulation::replanning::routing::alt_router::{
    AStarHeuristic, AStarRouter, NodePriority,
};
use crate::simulation::replanning::routing::graph::NodeIndex;
use crate::simulation::replanning::routing::least_cost_path_caluclator::{
    IntNodeGraph, TravelDisutility,
};
use crate::simulation::replanning::routing::least_cost_path_caluclator::{
    LeastCostPathRequest, Time,
};
use crate::simulation::scenario::network::Node;
use crate::simulation::scenario::population::InternalPerson;
use crate::simulation::scenario::vehicles::InternalVehicle;
use derive_builder::Builder;
use keyed_priority_queue::Entry;
use tracing::warn;

// #[deprecated] // should use OrderedFloat, which is simply a float64 modified to implement Eq
// pub struct Distance(pub f64);
//
// // we have to implement PartialEq manually for Distance, since we need the Eq trait, i.e.,
// // reflexivity. Therefore, we treat two NaN distances as equal (while in general f64::NaN != f64::NaN)
// impl PartialEq for Distance {
//     fn eq(&self, other: &Self) -> bool {
//         match (self.0.is_nan(), other.0.is_nan()) {
//             (true, true) => true,                // both values NaN -> equal
//             (true, false) => false,              // left value NaN -> not equal
//             (false, true) => false,              // right value NaN -> not equal
//             (false, false) => self.0 == other.0, // compare normally
//         }
//     }
// }
//
// impl Eq for Distance {}
//
// impl PartialOrd for Distance {
//     fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
//         self.0.partial_cmp(&other.0) // None if one of the values is NaN
//     }
// }
//
// impl Ord for Distance {
//     fn cmp(&self, other: &Self) -> Ordering {
//         self.0
//             .partial_cmp(&other.0)
//             .unwrap_or(
//                 // both values NaN -> equal
//                 if self.0.is_nan() && other.0.is_nan() {
//                     Ordering::Equal
//                 }
//                 // left value NaN -> Greater, since NaN is bad, i.e., large distance
//                 else if self.0.is_nan() {
//                     Ordering::Greater
//                 }
//                 // right value NaN -> Less, since NaN is bad, i.e., large distance
//                 else {
//                     Ordering::Less
//                 },
//             )
//             .reverse() // reverse, since priority queue prefers large values
//     } // FIXME remove reverse and change the priority queue function instead
// } // TODO probably remove the entire Distance struct and use some crate
//
// impl Distance {
//     pub fn get(&self) -> f64 {
//         self.0
//     }
// }

pub(crate) trait DijkstraActions: Clone {
    fn reached_end(&self, current_node: NodeIndex) -> bool;
    fn set_parent_opt(&mut self, child: NodeIndex, parent: NodeIndex);
    // fn get_parents_opt(&self) -> Option<Vec<Option<NodeIndex>>>;
    /// Creates a Dijkstra result, the trait implementation chooses the result enum variant.
    /// Consumes self to allow moving values without cloning.
    /// This is okay, since the method is called when Dijkstra finishes.
    fn build_result(self, current_distance: Option<f64>, distances: Vec<f64>) -> DijkstraResult;
    fn get_to_node_opt(&self) -> NodeIdxOptions;
}

// #[derive(Debug, Clone)]
// pub(crate) enum GraphNodeOrLink {
//     Node(Id<Node>),
//     Link(Id<Link>),
// }
//
// impl From<Id<Node>> for GraphNodeOrLink {
//     fn from(id: Id<Node>) -> Self {
//         GraphNodeOrLink::Node(id)
//     }
// }
//
// impl From<Id<Link>> for GraphNodeOrLink {
//     fn from(id: Id<Link>) -> Self {
//         GraphNodeOrLink::Link(id)
//     }
// }

#[derive(Builder, Debug)]
pub(crate) struct DijkstraRequest<'a, H: AStarHeuristic, O: DijkstraActions> {
    heuristic: &'a H,
    travel_disutility: &'a Box<dyn TravelDisutility>,
    from: NodeIdxOptions,
    // to: Option<NodeIndex>,  // this is now part of the options, since it is only used in certain dijkstra cases (1to1)
    graph: &'a Box<dyn IntNodeGraph>,
    options: O,
    departure_time: Time,               // TODO set default to 0
    person: Option<&'a InternalPerson>, // TODO set default to none in builder
    vehicle: Option<&'a InternalVehicle>,
    backward: bool, // if true, uses the incoming edges (backward graph) when looking for neighbours
}

impl<'a, H: AStarHeuristic, O: DijkstraActions> DijkstraRequestBuilder<'a, H, O> {
    pub(crate) fn from_least_cost_path_request(
        &mut self,
        request: &LeastCostPathRequest<'a>,
    ) -> &mut Self {
        // convert "from"-link id to corresponding from-node id, and then to NodeIndex
        let from_idx = request
            .graph
            .get_node_idx_from_id(request.graph.get_end_node(request.from.clone()));

        self.graph(request.graph)
            .departure_time(request.departure_time)
            .person(request.person)
            .vehicle(request.vehicle)
            .from(NodeIdxOptions::One(from_idx))
    }
}

pub(crate) enum DijkstraResult {
    DistanceToAllWithoutParents(Vec<f64>), // vector of "distances" from one node to all others
    // shortest distance from one node to another, with the generated list of parents
    SingleDistWithParents(f64, Vec<Option<NodeIndex>>),
}

// impl<'a, H: AStarHeuristic, O: DijkstraActions> DijkstraRequestBuilder<'a, H, O> {
//     pub(crate) fn from_landmark_request(
//         &mut self,
//         request: &LandmarkCreationRequest<'a>,
//     ) -> &mut Self {
//         self.travel_disutility(request.travel_disutility)
//             .graph(request.graph)
//             .departure_time(request.departure_time)
//             .person(request.person)
//             .vehicle(request.vehicle)
//     }
// }

// TODO is this defined in the right place? probably move to Graph
#[derive(Debug, Clone)]
pub enum NodeIdxOptions {
    One(NodeIndex), // one specific node in the graph
    Any,            // any node
}

impl NodeIdxOptions {
    pub(crate) fn get_node_or_panic(&self) -> NodeIndex {
        match self {
            NodeIdxOptions::One(node_idx) => *node_idx,
            NodeIdxOptions::Any => panic!("NodeIdxOptions::Any does not contain a specific node."),
        }
    }
}

// TODO is this defined in the right place? probably move to Graph
#[derive(Debug)]
pub enum NodeIdOptions {
    One(Id<Node>), // one specific node in the graph
    Any,           // any node
}

impl NodeIdOptions {
    pub(crate) fn get_node_or_panic(&self) -> Id<Node> {
        match self {
            NodeIdOptions::One(node_id) => node_id.clone(),
            NodeIdOptions::Any => panic!("NodeIdOptions::Any does not contain a specific node."),
        }
    }
}

/// Core Dijkstra implementation. Can be used to calculate distances from one to all other nodes
/// and from one to one node, with or without parent tracking, depending on the provided options.
/// This makes it usable both for calcualting landmark data as well as for AStar routing.
pub struct Dijkstra {}

impl Dijkstra {
    /// calculates the distance from one node to all other nodes in the graph (Dikstra)
    // pub(crate) fn distance_one_2_many(from: usize, graph: &RoutingGraph) -> Vec<u32> {
    //     let (mut queue, mut distances) =
    //         Dijkstra::get_initial_queue(graph.first_out.len() - 1, from);
    //
    //     while let Some((current_id, current_distance)) = queue.pop() {
    //         if current_distance.get() == u32::MAX {
    //             //The smallest value in queue was unreachable. So abort here.
    //             return distances;
    //         }
    //
    //         let begin_index_adjacent_nodes = graph.first_out[current_id];
    //         let end_index_adjacent_nodes = graph.first_out[current_id + 1];
    //
    //         for i in begin_index_adjacent_nodes..end_index_adjacent_nodes {
    //             //we need an update_or_insert + parent update here instead of push always.
    //             let neighbour = graph.head[i];
    //
    //             if let Entry::Vacant(_) = queue.entry(neighbour) {
    //                 continue;
    //             }
    //
    //             if queue.get_priority(&neighbour).unwrap().get()
    //                 > current_distance.get() + graph.travel_time[i]
    //             {
    //                 //perform update
    //                 match queue.entry(neighbour) {
    //                     Entry::Occupied(e) => {
    //                         e.set_priority(Distance(current_distance.get() + graph.travel_time[i]));
    //                     }
    //                     Entry::Vacant(_) => {
    //                         unreachable!();
    //                     }
    //                 }
    //                 //store in distance vec to return
    //                 distances[neighbour] = current_distance.get() + graph.travel_time[i];
    //             }
    //         }
    //     }
    //     distances
    // }

    // TODO put parameters into a request struct
    // TODO use builder (derive(builder))
    pub(crate) fn dijkstra_core<
        H: AStarHeuristic,
        // T: TravelDisutility,
        // G: IntNodeGraph + Sized,
        O: DijkstraActions,
    >(
        mut request: DijkstraRequest<H, O>, // heuristic: H,
                                            // travel_disutility: T,
                                            // from: Id<Link>,
                                            // to: Option<Id<Link>>,
                                            // graph: &G,
                                            // options: O,
                                            // departure_time: Time,            // TODO set default to 0
                                            // person: Option<&InternalPerson>, // TODO set default to none in builder
                                            // vehicle: Option<&InternalVehicle>,
    ) -> DijkstraResult {
        let number_of_nodes = request.graph.num_nodes();

        let from_node = request.from.get_node_or_panic();

        // // if "to" is None, leave it as such, else if "to" is a link, get the corresponding node, else use node directly. Convert to NodeIndex.
        // let to_node = request.to.map(|to| match to {
        //     GraphNodeOrLink::Node(node_id) => request.graph.get_node_idx_from_id(node_id),
        //     GraphNodeOrLink::Link(link_id) => request
        //         .graph
        //         .get_node_idx_from_id(request.graph.get_start_node(link_id)),
        // });

        // let from_node = request
        //     .graph
        //     .get_node_idx_from_id(request.graph.get_end_node(request.from));
        // FIXME what is supposed to happen with the to-node here? what should happen in the case when it is None? i.e., to many?
        // let to_node = request
        //     .graph
        //     .get_node_idx_from_id(request.graph.get_start_node(request.to));

        // TODO possibly rename distances to priorities?
        // TODO is it reasonable to take the distances separately as f64? wouldn't it be nicer to extract them from the queue?
        // check why is it even done in this way
        let (mut queue, mut distances) =
            AStarRouter::<H>::get_initial_queue(number_of_nodes, from_node);
        // Not initializing parents here, since they are contained in the options
        while let Some((current_id, _)) = queue.pop() {
            let current_distance = distances[current_id];

            // checking "unusual" values of current_distance // TODO is this the handling that we want?
            match current_distance {
                f64::INFINITY => {
                    //The smallest value in queue was unreachable. So abort here.

                    // this chooses the correct result enum variant automatically
                    return request
                        .options
                        .build_result(Some(current_distance), distances);
                }
                f64::NEG_INFINITY => {
                    warn!("Distance of negative infinity encountered in dijkstra.");
                }
                nan_dist if nan_dist.is_nan() => {
                    // The smallest value in queue is NaN, treated as worse than distance infinity
                    warn!(
                        "Queue in dijkstra only contains entries with distance NaN, which are\
                    treated as unreachable. Aborting dijkstra."
                    );
                    return request.options.build_result(Some(nan_dist), distances);
                }
                _ => {}
            }

            if request.options.reached_end(current_id) == true {
                // this chooses the correct result enum variant automatically
                return request
                    .options
                    .build_result(Some(current_distance), distances);
            }

            // if request.backward=true, we consider the incoming edges, i.e., the path from other nodes to the "from"-node
            let neighbour_edges = if request.backward {
                request.graph.incoming_edges_as_idx(current_id)
            } else {
                request.graph.outgoing_edges_as_idx(current_id)
            };

            for i in neighbour_edges {
                //we need an update_or_insert + parent update here instead of push always.

                // let neighbour = request.graph.forward_graph.head[i];
                let neighbour = request.graph.get_end_node_as_idx(i);

                if let Entry::Vacant(_) = queue.entry(neighbour) {
                    continue;
                }

                let link_i = request.graph.get_link_from_idx(i);

                // TODO is it correct to use the departure time from the request here? -> NO!
                // or could it be later by now?
                let neighbour_distance = current_distance
                    + request.travel_disutility.travel_disutility(
                        link_i,
                        request.departure_time,
                        request.person,
                        request.vehicle,
                    );

                if distances[neighbour] > neighbour_distance {
                    //perform update
                    distances[neighbour] = neighbour_distance;

                    match queue.entry(neighbour) {
                        Entry::Occupied(e) => {
                            // TODO could this be skipped, and I just use NodeIdOptions or smth?
                            // if to_node index present, convert to to_node id. Else keep "Any".
                            let to_node_id_opt = match request.options.get_to_node_opt() {
                                NodeIdxOptions::One(to_node) => {
                                    NodeIdOptions::One(request.graph.get_node_id_from_idx(to_node))
                                }
                                NodeIdxOptions::Any => NodeIdOptions::Any,
                            };

                            // set priority to distance to neighbour + heuristic from there to the to_node
                            e.set_priority(NodePriority::new(
                                neighbour_distance
                                    + request.heuristic.estimate(
                                        request.graph.as_ref(),
                                        NodeIdOptions::One(
                                            request.graph.get_node_id_from_idx(neighbour),
                                        ),
                                        to_node_id_opt,
                                    ),
                            ));
                        }
                        Entry::Vacant(_) => {
                            unreachable!()
                        }
                    }
                    request.options.set_parent_opt(neighbour, current_id);
                }
            }
        }
        // will panic if options are AltOptions, since then, a current_distance must be provided.
        // But this is okay, since we should not reach this point (all points in queue visited) in
        // this case: either, the to_node was reached and the function returned already, or the
        // to_node is unreachable, in which case, at some point the smallest distance in the queue
        // will be infinity or NaN and the function will also return.
        return request.options.build_result(None, distances);
    }

    // pub fn get_initial_queue(
    //     node_count: usize,
    //     from: usize,
    // ) -> (KeyedPriorityQueue<usize, Distance>, Vec<u32>) {
    //     let mut queue = KeyedPriorityQueue::new();
    //     let mut distances = Vec::new();
    //     for i in 0..node_count {
    //         let distance = if i == from {
    //             //update start node
    //             Distance(0)
    //         } else {
    //             Distance(u32::MAX)
    //         };
    //         distances.push(distance.0);
    //         queue.push(i, distance);
    //     }
    //     (queue, distances)
    // }
}
