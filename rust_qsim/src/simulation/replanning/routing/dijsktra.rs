use crate::simulation::id::Id;
use crate::simulation::replanning::routing::alt_router::{
    AStarHeuristic, AStarRouter, NodePriority, ZeroHeuristic,
};
use crate::simulation::replanning::routing::graph::{GraphError, NodeIndex};
use crate::simulation::replanning::routing::least_cost_path_caluclator::TravelTime;
use crate::simulation::replanning::routing::least_cost_path_caluclator::{
    IntNodeGraph, TravelDisutility,
};
use crate::simulation::replanning::routing::least_cost_path_caluclator::{
    LeastCostPathRequest, Time,
};
use crate::simulation::scenario::population::InternalPerson;
use crate::simulation::scenario::vehicles::InternalVehicle;
use derive_builder::Builder;
use keyed_priority_queue::Entry;
use tracing::warn;

/// Specifies which heuristic to use for A* search
///
/// - `WithHeuristic(&'a H)`: Use the provided heuristic for One-to-One routing with A*
/// - `WithoutHeuristic`: Use zero heuristic (collapses A* to pure Dijkstra)
///   for One-to-Many landmark distance calculations
#[derive(Clone, Debug)]
pub enum HeuristicMode<'a, H: AStarHeuristic = ZeroHeuristic> {
    WithHeuristic(&'a H),
    WithoutHeuristic,
}

impl<'a, H: AStarHeuristic> HeuristicMode<'a, H> {
    pub fn with_heuristic(heuristic: &'a H) -> Self {
        HeuristicMode::WithHeuristic(heuristic)
    }
}

impl<'a> HeuristicMode<'a, ZeroHeuristic> {
    pub fn without_heuristic() -> Self {
        HeuristicMode::WithoutHeuristic
    }
}

pub(crate) trait DijkstraActions: Clone {
    fn reached_end(&self, current_node: NodeIndex) -> bool;
    fn set_parent_opt(&mut self, child: NodeIndex, parent: NodeIndex);
    // fn get_parents_opt(&self) -> Option<Vec<Option<NodeIndex>>>;
    /// Creates a Dijkstra result, the trait implementation chooses the result enum variant.
    /// Consumes self to allow moving values without cloning.
    /// This is okay, since the method is called when Dijkstra finishes.
    fn build_result(self, current_distance: Option<f64>, distances: Vec<f64>) -> DijkstraResult;
    fn get_to_node_opt(&self) -> Option<NodeIndex>; // NodeIdxOptions;
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
    heuristic_mode: HeuristicMode<'a, H>,
    travel_time: &'a dyn TravelTime, // used to update the time when a certain node is reached
    travel_disutility: &'a dyn TravelDisutility, // used as "distance" in dijkstra, i.e., we find the shortest path wrt this
    from: NodeIndex,                             // NodeIdxOptions,
    // to: Option<NodeIndex>,  // this is now part of the options, since it is only used in certain dijkstra cases (1to1)
    graph: &'a dyn IntNodeGraph,
    options: O,
    #[builder(default)]
    departure_time: Time,
    #[builder(default)]
    person: Option<&'a InternalPerson>,
    #[builder(default)]
    vehicle: Option<&'a InternalVehicle>,
    #[builder(default)]
    backward: bool, // if true, uses the incoming edges (backward graph) when looking for neighbours
}

impl<'a, H: AStarHeuristic, O: DijkstraActions> DijkstraRequestBuilder<'a, H, O> {
    pub(crate) fn from_least_cost_path_request(
        &mut self,
        request: &LeastCostPathRequest<'a>,
    ) -> Result<&mut Self, GraphError> {
        // convert "from"-link id to corresponding from-node id, and then to NodeIndex
        let from_node_id = request.graph.get_end_node(request.from.clone())?;

        let from_idx = request.graph.get_node_idx_from_id(from_node_id);

        Ok(self
            .graph(request.graph)
            .departure_time(request.departure_time)
            .person(request.person)
            .vehicle(request.vehicle)
            .from(from_idx))
    }
}

pub(crate) enum DijkstraResult {
    DistanceToAllWithoutParents(Vec<f64>), // vector of "distances" from one node to all others
    // shortest distance from one node to another, with the generated list of parents
    SingleDistWithParents(f64, Vec<Option<NodeIndex>>),
}

/// Core Dijkstra implementation. Can be used to calculate distances from one to all other nodes
/// and from one to one node, with or without parent tracking, depending on the provided options.
/// This makes it usable both for calcualting landmark data as well as for AStar routing.
pub struct Dijkstra {}

impl Dijkstra {
    /// TODO needs docstring
    pub(crate) fn dijkstra_core<H: AStarHeuristic, O: DijkstraActions>(
        mut request: DijkstraRequest<H, O>,
    ) -> Result<DijkstraResult, GraphError> {
        let number_of_nodes = request.graph.num_nodes();

        let from_node = request.from; // request.from.get_node_or_panic();

        // TODO possibly rename distances? But it could also be okay because it's Dijstra terminology
        let (mut queue, mut distances) =
            AStarRouter::<H>::get_initial_queue(number_of_nodes, from_node);

        // Initialize arrival_times to track when each node is reached
        let mut arrival_times = vec![request.departure_time; number_of_nodes];
        arrival_times[from_node] = request.departure_time;

        // Not initializing parents here, since they are contained in the options
        while let Some((current_id, _)) = queue.pop() {
            // distance from "from"-node to the current_id node
            let current_distance = distances[current_id];
            let current_arrival_time = arrival_times[current_id];

            // checking "unusual" values of current_distance // TODO is this the handling that we want?
            match current_distance {
                f64::INFINITY => {
                    //The smallest value in queue was unreachable. So abort here.

                    // this chooses the correct result enum variant automatically
                    return Ok(request
                        .options
                        .build_result(Some(current_distance), distances));
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
                    return Ok(request.options.build_result(Some(nan_dist), distances));
                }
                _ => {}
            }

            if request.options.reached_end(current_id) == true {
                // this chooses the correct result enum variant automatically
                return Ok(request
                    .options
                    .build_result(Some(current_distance), distances));
            }

            // if request.backward=true, we consider the incoming edges, i.e., the path from other nodes to the "from"-node
            let neighbour_edges = if request.backward {
                request.graph.incoming_edges_as_idx(current_id)
            } else {
                request.graph.outgoing_edges_as_idx(current_id)
            };

            for i in neighbour_edges {
                //we need an update_or_insert + parent update here instead of push always.

                // When backward=true, incoming_edges return edges TO the current node,
                // so we need the start node. When backward=false, outgoing_edges return
                // edges FROM the current node, so we need the end node.
                let neighbour = if request.backward {
                    request.graph.get_start_node_as_idx(i)
                } else {
                    request.graph.get_end_node_as_idx(i)
                }?;

                if let Entry::Vacant(_) = queue.entry(neighbour) {
                    continue;
                }

                let link_i = request.graph.get_link_from_idx(i);

                // Use the actual arrival time at the current node, not the departure time from the request
                let neighbour_distance = current_distance
                    + request.travel_disutility.travel_disutility(
                        link_i,
                        current_arrival_time,
                        request.person,
                        request.vehicle,
                    );

                if distances[neighbour] > neighbour_distance {
                    //perform update
                    distances[neighbour] = neighbour_distance;

                    // Calculate arrival time at the neighbour node
                    let link_travel_time = request.travel_time.travel_time(
                        link_i,
                        current_arrival_time,
                        request.person,
                        request.vehicle,
                    );
                    arrival_times[neighbour] = current_arrival_time + link_travel_time;

                    match queue.entry(neighbour) {
                        Entry::Occupied(e) => {
                            // Compute heuristic estimate based on the heuristic mode
                            let heuristic_estimate = match &request.heuristic_mode {
                                HeuristicMode::WithHeuristic(h) => {
                                    // For One-to-One routing, use the provided heuristic
                                    let to_node_idx = request.options.get_to_node_opt().expect(
                                        "Heuristic mode is WithHeuristic, but no to_node \
                                        provided in Dijkstra options.",
                                    );
                                    // panic is okay here, since it is a programming error if
                                    // someone uses WithHeuristic but does not provide a to_node in
                                    // the options

                                    let to_node_id =
                                        request.graph.get_node_id_from_idx(to_node_idx);

                                    h.estimate(
                                        request.graph,
                                        request.graph.get_node_id_from_idx(neighbour),
                                        to_node_id,
                                    )
                                }
                                HeuristicMode::WithoutHeuristic => {
                                    // For One-to-Many (landmark calculation), use zero heuristic
                                    // This collapses A* to pure Dijkstra
                                    // (We don't use the ZeroHeuristic.estimate function here, since
                                    // it would require unnecessary calls to the graph and in particular
                                    // that we pass a to-node, which doesn't exist in one-to-many).
                                    0.0
                                }
                            };

                            e.set_priority(NodePriority::new(
                                neighbour_distance + heuristic_estimate,
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
        return Ok(request.options.build_result(None, distances));
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

// Note: dijkstra_core is not tested here as of now, since it is implicitly tested by the tests of
// AStarRouter and AltLandmarkData, which use dijkstra_core for their implementations
// However, it might be good to add explicit tests for dijkstra_core at some point, to make sure
// that it works correctly in the various cases (1to1, 1tomany, with and without parents).

