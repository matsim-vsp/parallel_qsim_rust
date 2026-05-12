use crate::simulation::replanning::routing::alt_router::{
    AStarHeuristic, AStarRouter, NodePriority, ZeroHeuristic,
};
use crate::simulation::replanning::routing::graph::{GraphError, NodeIndex};
use crate::simulation::replanning::routing::least_cost_path_caluclator::{Disutility, TravelTime};
use crate::simulation::replanning::routing::least_cost_path_caluclator::{
    IndexableGraph, TravelDisutility,
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

/// Implementations of this trait are used in dijkstra to use the same algorithm for different use
/// cases.
/// Namely, the implementations can make the algorithm stop early when a certain node has been
/// reached, they handle parent tracking and they construct the result returned by Dijkstra.
/// Specifically, this allows the implementation to turn parent tracking on or off, and whether
/// to calculate distances to all nodes or just to one target node.
pub(crate) trait DijkstraActions: Clone {
    fn reached_end(&self, current_node: NodeIndex) -> bool;
    fn set_parent_opt(&mut self, child: NodeIndex, parent: NodeIndex);
    /// Creates a Dijkstra result, the trait implementation chooses the result enum variant.
    /// Consumes self to allow moving values without cloning.
    /// This is okay, since the method is called when Dijkstra finishes.
    fn build_result(
        self,
        current_distance: Option<f64>,
        current_travel_time: Option<Time>,
        distances: Vec<f64>,
    ) -> DijkstraResult;
    fn get_to_node_opt(&self) -> Option<NodeIndex>; // NodeIdxOptions;
}

/// Request for dijkstra runs. Contains
/// - data needed for calculation, that is the graph, the travel time and travel disutility
///     functions, the from-node, the departure time, the person and vehicle (if applicable)
/// - a `DijkstraActions` implementation that determines the use case (parent tracking or not,
///     one to many or not), which also contains the to-node when applicable
/// - the `HeuristicMode`: a heuristic to be used, or the information that none is to be used
/// - a bool specifying whether the search is to be performed forwards or backwards. In the
///     latter case, paths using incoming edges, i.e., paths leading going to the from-node,
///     are searched.
#[derive(Builder, Debug)]
pub(crate) struct DijkstraRequest<'a, H: AStarHeuristic, O: DijkstraActions> {
    heuristic_mode: HeuristicMode<'a, H>,
    travel_time: &'a dyn TravelTime, // used to update the time when a certain node is reached
    travel_disutility: &'a dyn TravelDisutility, // used as "distance" in dijkstra, i.e., we find the shortest path wrt this
    from: NodeIndex,
    // Note: the to-node is stored in the options, when applicable, since it is only used in certain use cases (1to1)
    graph: &'a dyn IndexableGraph,
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
    /// partially builds a dijkstra request using data from a given least cost path request
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

/// Result of a Dijkstra run. Has two versions for different use cases (One2Many w/o parent tracking
/// and One2One with parent tracking)
pub(crate) enum DijkstraResult {
    /// Distance (=travel disutility) from one node to all other nodes in the graph
    DistanceToAllWithoutParents(Vec<Disutility>),
    /// Shortest distance (=travel disutility) from one node to another, with the associated travel
    /// time and generated list of parents
    SingleDistWithParents(Disutility, Time, Vec<Option<NodeIndex>>),
}

/// Core Dijkstra implementation. Can be used to calculate distances from one to all other nodes
/// and from one to one node, with or without parent tracking, depending on the provided options.
/// This makes it usable both for calcualting landmark data as well as for AStar routing.
pub struct Dijkstra {}

impl Dijkstra {
    /// Core Dijkstra logic. Can be used with or without parent tracking, and for One2One or
    /// One2Many.
    /// Takes a `DijkstraRequest` containing all necessary data for the Dijkstra run.
    pub(crate) fn dijkstra_core<H: AStarHeuristic, O: DijkstraActions>(
        mut request: DijkstraRequest<H, O>,
    ) -> Result<DijkstraResult, GraphError> {
        let number_of_nodes = request.graph.num_nodes();

        let from_node = request.from;

        // TODO possibly rename distances? But it could also be okay because it's Dijstra terminology
        // initialize queue: from_node gets distance and priority 0, all others infinity
        let (mut queue, mut distances) =
            AStarRouter::<H>::get_initial_queue(number_of_nodes, from_node);

        // Initialize arrival_times to track when each node is reached
        let mut arrival_times = vec![f64::INFINITY as Time; number_of_nodes];
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
                    return Ok(request.options.build_result(
                        Some(current_distance),
                        Some(current_arrival_time),
                        distances,
                    ));
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
                    return Ok(request.options.build_result(
                        Some(nan_dist),
                        Some(current_arrival_time),
                        distances,
                    ));
                }
                _ => {}
            }

            // check if the target node has been reached, if applicable, in that case return early
            if request.options.reached_end(current_id) == true {
                // this chooses the correct result enum variant automatically
                return Ok(request.options.build_result(
                    Some(current_distance),
                    Some(current_arrival_time),
                    distances,
                ));
            }

            // if request.backward=true, we consider the incoming edges, to consider paths from
            // other nodes to the "from"-node
            let neighbour_edges = if request.backward {
                request.graph.incoming_edges_as_idx(current_id) // TODO check if the implementation of incoming_edges, get_link_from_idx and everything is correct, specifically, if the backward graph is treated correctly. The results are correct, but maybe two errors cancel out
            } else {
                request.graph.outgoing_edges_as_idx(current_id)
            };

            for i in neighbour_edges {
                //we need an update_or_insert + parent update here instead of push always.

                // When backward=true, incoming_edges return edges TO the current node,
                // so we need the start node to get the neighbours.
                // When backward=false, outgoing_edges return edges FROM the current node, so we
                // need the end node.
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
                                    // In WithoutHeuristic-mode, set heuristic to 0.0. This is the
                                    // case in For One-to-Many (landmark calculation).
                                    // This collapses A* to pure Dijkstra.
                                    // (We don't use the ZeroHeuristic.estimate function here, since
                                    // it would require unnecessary calls to the graph and in particular
                                    // that we pass a to-node, which doesn't exist in one-to-many).
                                    0.0
                                }
                            };

                            // update priority of the neighbour
                            e.set_priority(NodePriority::new(
                                neighbour_distance + heuristic_estimate,
                            ));
                        }
                        Entry::Vacant(_) => {
                            unreachable!()
                        }
                    }
                    // update parents if applicable
                    request.options.set_parent_opt(neighbour, current_id);
                }
            }
        }
        // will panic if options are AltOptions, since then, a current_distance and
        // current_arrival_time must be provided.
        // But this is okay, since we should not reach this point (all points in queue visited) in
        // this case: either, the to_node was reached and the function returned already, or the
        // to_node is unreachable, in which case, at some point the smallest distance in the queue
        // will be infinity or NaN and the function will also return.
        return Ok(request.options.build_result(None, None, distances));
    }
}

// Note: dijkstra_core is not tested here as of now, since it is implicitly tested by the tests of
// AStarRouter and AltLandmarkData, which use dijkstra_core for their implementations
// However, it might be good to add explicit tests for dijkstra_core at some point, to make sure
// that it works correctly in the various cases (1to1, 1tomany, with and without parents).
