use crate::simulation::replanning::routing::a_star_router::{AStarHeuristic, ZeroHeuristic};
use crate::simulation::replanning::routing::graph::{GraphError, IndexableGraph, NodeIndex};
use crate::simulation::replanning::routing::least_cost_path_caluclator::TravelDisutility;
use crate::simulation::replanning::routing::least_cost_path_caluclator::{Disutility, TravelTime};
use crate::simulation::replanning::routing::least_cost_path_caluclator::{
    LeastCostPathRequest, Time,
};
use crate::simulation::scenario::population::InternalPerson;
use crate::simulation::scenario::vehicles::InternalVehicle;
use derive_builder::Builder;
use keyed_priority_queue::{Entry, KeyedPriorityQueue};
use ordered_float::OrderedFloat;
use std::cmp::Reverse;
use tracing::warn;

/// Specifies which heuristic to use for A* search
///
/// - `WithHeuristic(&'a H)`: Use the provided heuristic for One-to-One routing with A*
/// - `WithoutHeuristic`: Use zero heuristic (collapses A* to pure Dijkstra)
///   for One-to-Many landmark distance calculations
///     - this allows to run `a_star_core` without a `to`-node, since even when using
///     `ZeroHeuristic`, a node would have to be passed. But with this setting, `a_star_core` knows
///     not to call any Heuristic
#[derive(Clone, Debug)]
pub(crate) enum HeuristicMode<'a, H: AStarHeuristic = ZeroHeuristic> {
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

/// Shorthand for `Reverse<OrderedFloat<f64>>`, i.e., an ordered float (implements Eq and Ord,
/// unlike f64) which is sorted in reverse order.
/// To be used in KeyedPriorityQueues in A*, since the queue prefers large values while we
/// prefer small values.
#[derive(Eq, Ord, PartialEq, PartialOrd)]
struct NodePriority(Reverse<OrderedFloat<f64>>);

impl NodePriority {
    pub fn new(priority: f64) -> Self {
        NodePriority(Reverse(OrderedFloat(priority)))
    }

    pub fn get(&self) -> f64 {
        self.0.0.into_inner()
    }
}

/// Result of an A* run. Has two versions for different use cases (One2Many w/o parent tracking
/// and One2One with parent tracking)
pub(crate) enum AStarCoreResult {
    /// Distance (=travel disutility) from one node to all other nodes in the graph
    DistanceToAllWithoutParents(Vec<Disutility>),
    /// Shortest distance (=travel disutility) from one node to another, with the associated travel
    /// time and generated list of parents
    SingleDistWithParents(Disutility, Time, Vec<Option<NodeIndex>>),
}

/// Implementations of this trait represent different use cases of `a_star_core`.
/// In particular, they set whether the A* search is One2One or One2Many and whether parents are
/// tracked or not.
/// Specifically, the implementations decide:
/// - at every current node in the algorithm, whether it should stop, since it reached its goal
/// - upon reaching a node, whether its parent node should be tracked
/// - when the algorithm returns, what form the result should have (e.g. with or without parents)
pub(crate) trait AStarActions: Clone {
    /// Called by `a_star_core` at every visited node, the alg will return if it receives `true`
    fn reached_end(&self, current_node: NodeIndex) -> bool;
    /// Called by `a_star_core` when a node is reached, the implementation decides whether to store
    /// the information about the parent node, and if yes, how
    fn set_parent_opt(&mut self, child: NodeIndex, parent: NodeIndex);
    /// Creates a A* result, the trait implementation chooses the result enum variant.
    /// Consumes self to allow moving values without cloning.
    /// This is okay, since the method is called when A* finishes.
    fn build_result(
        self,
        current_distance: Option<f64>,
        current_travel_time: Option<Time>,
        distances: Vec<f64>,
    ) -> AStarCoreResult;
    /// Called by `a_star_core` to get the to-node, to be able to pass it to a heuristic
    fn get_to_node_opt(&self) -> Option<NodeIndex>;
}

/// These objects represent the A* use case One2Many with no parent tracking.
/// That is, when they are called in `a_star_core` to determine whether the target node was reached,
/// they always return false, since this use case has no to-node.
/// When they are called in the context of parent tracking, they do nothing. When they are called
/// in the context of building the result, they return the distances from the from-node to all
/// other nodes in the graph.
#[derive(Clone)]
pub(crate) struct One2ManyNoParentsAStarActions;

impl AStarActions for One2ManyNoParentsAStarActions {
    fn set_parent_opt(&mut self, _child: NodeIndex, _parent: NodeIndex) {}
    fn reached_end(&self, _current_node: NodeIndex) -> bool {
        false
    }
    fn build_result(
        self,
        _current_distance: Option<f64>,
        _current_arrival_time: Option<Time>,
        distances: Vec<f64>,
    ) -> AStarCoreResult {
        AStarCoreResult::DistanceToAllWithoutParents(distances)
    }
    fn get_to_node_opt(&self) -> Option<NodeIndex> {
        None
    }
}

/// The A* use case "One to One with parent tracking", i.e., finding single shortest paths.
/// That is, when called in `a_star_core` to determine whether the target was reached, will return
/// true when the current node is the to-node.
/// When called in the context of parent tracking, will update the parents.
/// When called to build the A* result, will return the distance from the from-node to the to-node,
/// together with the parents vector, which can be used to extract the path.
#[derive(Clone)]
pub(crate) struct One2OneWithParentsAStarActions {
    to_node: NodeIndex,
    parents: Vec<Option<NodeIndex>>,
}

impl One2OneWithParentsAStarActions {
    pub fn new(to_node: NodeIndex, parents: Vec<Option<NodeIndex>>) -> Self {
        Self { to_node, parents }
    }
}

impl AStarActions for One2OneWithParentsAStarActions {
    fn reached_end(&self, current_node: NodeIndex) -> bool {
        self.to_node == current_node
    }
    fn set_parent_opt(&mut self, child: NodeIndex, parent: NodeIndex) {
        self.parents[child] = Some(parent);
    }

    // constructs a "single distance with parent tracking" result, containing the distance from the
    // from-node to the to-node and the tracked parents.
    // Consumes self, so parents can be moved without cloning
    fn build_result(
        self,
        current_distance: Option<Disutility>,
        current_travel_time: Option<Time>,
        _distances: Vec<f64>,
    ) -> AStarCoreResult {
        // note that current_distance and current_travel_time is given as an option, since the
        // trait also allows implementations of one2many, where only the distances vector is needed,
        // not current dist and current time.

        // But the below should not panic, since a_star_core only passes current_distance=None or
        // current_travel_time=None in the case where the entire queue has been visited and the
        // to_node has neither been found nor been determined to be unreachable, which only happens
        // in one2many (where no to-node exists)
        AStarCoreResult::SingleDistWithParents(
            current_distance.expect("A* use case 1to1 requires that current distance is given"),
            current_travel_time.expect("A* use case 1to1 requires that travel time is given"),
            self.parents,
        )
    }
    fn get_to_node_opt(&self) -> Option<NodeIndex> {
        Some(self.to_node)
    }
}

/// Request for A* runs. Contains
/// - data needed for calculation, that is the graph, the travel time and travel disutility
///     functions, the from-node, the departure time, the person and vehicle (if applicable)
/// - a `AStarActions` implementation that determines the use case (parent tracking or not,
///     one to many or not), which also contains the to-node when applicable
/// - the `HeuristicMode`: a heuristic to be used, or the information that none is to be used
/// - a bool specifying whether the search is to be performed forwards or backwards. In the
///     latter case, paths using incoming edges, i.e., paths leading going to the from-node,
///     are searched.
#[derive(Builder, Debug)]
pub(crate) struct AStarRequest<'a, H: AStarHeuristic, O: AStarActions> {
    heuristic_mode: HeuristicMode<'a, H>,
    travel_time: &'a dyn TravelTime, // used to update the time when a certain node is reached
    travel_disutility: &'a dyn TravelDisutility, // used as "distance" in A*, i.e., we find the shortest path wrt this
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

impl<'a, H: AStarHeuristic, O: AStarActions> AStarRequestBuilder<'a, H, O> {
    /// partially builds a A* request using data from a given least cost path request
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

/// Core A* logic.
/// Can be used to calculate distances from one to all other nodes
/// and from one to one node, with or without parent tracking, depending on the provided options.
/// This makes it usable both for calculating landmark data as well as for AStar routing.
/// Takes an `AStarRequest` containing all necessary data for the A* run.
pub(crate) fn a_star_core<H: AStarHeuristic, O: AStarActions>(
    mut request: AStarRequest<H, O>,
) -> Result<AStarCoreResult, GraphError> {
    let number_of_nodes = request.graph.num_nodes();

    let from_node = request.from;

    // TODO do we want the name "distances"? In our case, it is always a travel disutility, which would become clearer if we name it accordingly. On the other hand, distance is a common name for the values tracked in A*, even when they are not actual distances, so it is also fine to keep the name "distances".
    // initialize queue: from_node gets distance and priority 0, all others infinity
    let (mut queue, mut distances) = get_initial_queue(number_of_nodes, from_node);

    // Initialize arrival_times to track when each node is reached
    let mut arrival_times = vec![f64::INFINITY as Time; number_of_nodes];
    arrival_times[from_node] = request.departure_time;

    // Not initializing parents here, since they are contained in the options
    while let Some((current_id, _)) = queue.pop() {
        // distance from "from"-node to the current_id node
        let current_distance = distances[current_id];
        let current_arrival_time = arrival_times[current_id];

        // checking "unusual" values of current_distance
        // TODO is this the handling of unusual distance values that we want?
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
                warn!("Distance of negative infinity encountered in A*.");
            }
            nan_dist if nan_dist.is_nan() => {
                // The smallest value in queue is NaN, treated as worse than distance infinity
                warn!(
                    "Queue in A* only contains entries with distance NaN, which are\
                    treated as unreachable. Aborting A*."
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
            request.graph.incoming_edges_as_idx(current_id)
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

            let link_i = request.graph.get_link_from_idx(i)?;

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
                                        provided in AStarOptions.",
                                );
                                // panic is okay here, since it is a programming error if
                                // someone uses WithHeuristic but does not provide a to_node in
                                // the options

                                let to_node_id = request.graph.get_node_id_from_idx(to_node_idx)?;

                                h.estimate(
                                    request.graph,
                                    request.graph.get_node_id_from_idx(neighbour)?,
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
                        e.set_priority(NodePriority::new(neighbour_distance + heuristic_estimate));
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

/// Initialize the priority queue and distances vector for A* search. The from-node gets
/// priority and distance 0.0, all others infinity
fn get_initial_queue(
    node_count: usize,
    from: NodeIndex,
) -> (KeyedPriorityQueue<NodeIndex, NodePriority>, Vec<f64>) {
    // queue contains node indices and their priority (of type NodePriority, i.e., OrderedFloats
    // that are sorted in reverse order (=> queue prefers small numbers))
    let mut queue = KeyedPriorityQueue::new();

    // We will also return distances as f64 separately, since we need them as standard floats with
    // normal sorting.
    // Also, in A*, node priority and distance will not stay the same, since priorities also contain
    // the heuristic values.
    let mut distances = Vec::new();

    for node in 0..node_count {
        let node_index = node as NodeIndex;
        // the from node gets priority 0, all others infinity
        let node_priority = if node_index == from {
            NodePriority::new(0f64)
        } else {
            NodePriority::new(f64::INFINITY)
        };
        // track f64 distances
        distances.push(node_priority.get());
        // save entry to queue
        queue.push(node_index, node_priority);
    }
    (queue, distances)
}

// Note: a_star_core is not tested here as of now, since it is implicitly tested by the tests of
// AStarRouter and AltLandmarkData, which use a_star_core for their implementations
// However, it might be good to add explicit tests for a_star_core at some point, to make sure
// that it works correctly in the various cases (1to1, 1tomany, with and without parents).
