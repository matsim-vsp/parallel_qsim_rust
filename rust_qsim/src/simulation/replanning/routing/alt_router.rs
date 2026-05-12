use crate::simulation::id::Id;
use crate::simulation::replanning::routing::alt_landmark_data::AltLandmarkData;
use crate::simulation::replanning::routing::dijsktra::{
    Dijkstra, DijkstraActions, DijkstraRequestBuilder, DijkstraResult, HeuristicMode,
};
use crate::simulation::replanning::routing::graph::{GraphError, NodeIndex};
use crate::simulation::replanning::routing::least_cost_path_caluclator::{
    IntNodeGraph, LeastCostPath, LeastCostPathCalculator, LeastCostPathRequest, Time,
    TravelDisutility, TravelTime,
};
use crate::simulation::scenario::network::{Link, Node};
use keyed_priority_queue::KeyedPriorityQueue;
use ordered_float::OrderedFloat;
use std::cmp::Reverse;
use tracing::warn;
// use serde::de::Unexpected::Option;

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

#[derive(Clone)]
pub(crate) struct AltOptions {
    to_node: NodeIndex,
    parents: Vec<Option<NodeIndex>>,
}

impl AltOptions {
    pub fn new(to_node: NodeIndex, parents: Vec<Option<NodeIndex>>) -> Self {
        Self { to_node, parents }
    }
}

impl DijkstraActions for AltOptions {
    fn reached_end(&self, current_node: NodeIndex) -> bool {
        self.to_node == current_node
    }
    fn set_parent_opt(&mut self, child: NodeIndex, parent: NodeIndex) {
        self.parents[child] = Some(parent);
    }

    // Consumes self, so parents can be moved without cloning
    fn build_result(self, current_distance: Option<f64>, _distances: Vec<f64>) -> DijkstraResult {
        DijkstraResult::SingleDistWithParents(
            current_distance.expect("Dijkstra 1to1 requires that current distance is given"),
            self.parents,
        )
    }
    fn get_to_node_opt(&self) -> Option<NodeIndex> {
        // NodeIdxOptions {
        Some(self.to_node) // NodeIdxOptions::One(self.to_node)
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
        node_priorities.push(node_priority.get());
        queue.push(node_index, node_priority);
    }
    (queue, node_priorities)
}
impl Clone for Box<dyn TravelTime> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}
impl Clone for Box<dyn TravelDisutility> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

pub struct AStarRouter<H: AStarHeuristic> {
    heuristic: H,
    travel_time: Box<dyn TravelTime>,
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
        graph: &dyn IntNodeGraph,
    ) -> Result<Vec<Id<Link>>, GraphError> {
        let node_path = Self::extract_node_path(to, parent);

        let mut link_path = Vec::new();

        // look for link connecting node i and node i+1
        for i in 0..node_path.len() - 1 {
            let from_node = node_path[i];
            let to_node = node_path[i + 1];

            // go through outgoing edges of "from_node" and find the one that has to_node as head
            for j in graph.outgoing_edges_as_idx(from_node) {
                if graph.get_end_node_as_idx(j)? == to_node {
                    // get actual Id<Link> of the link connecting from_node and to_node
                    link_path.push(graph.get_link_id_from_idx(j));
                    break;
                }
            }
        }
        Ok(link_path)
    }
}

pub trait AStarHeuristic: Clone {
    fn estimate(&self, graph: &dyn IntNodeGraph, from: Id<Node>, to: Id<Node>) -> Time;
}

// with this, the A* collapses into Dijkstra
#[derive(Clone)]
pub(crate) struct ZeroHeuristic;

impl AStarHeuristic for ZeroHeuristic {
    fn estimate(&self, _graph: &dyn IntNodeGraph, _from: Id<Node>, _to: Id<Node>) -> Time {
        0.
    }
}

impl<H: AStarHeuristic> AStarRouter<H> {
    pub fn new(
        heuristic: H,
        travel_time: Box<dyn TravelTime>,
        travel_disutility: Box<dyn TravelDisutility>,
    ) -> Self {
        AStarRouter {
            heuristic,
            travel_time,
            travel_disutility,
        }
    }
}

impl<H: AStarHeuristic> LeastCostPathCalculator for AStarRouter<H> {
    fn calc_route(&self, request: LeastCostPathRequest) -> Option<LeastCostPath> {
        // convert given "to" link id to node id, by looking for the start node of the link
        let to_node_id = match request.graph.get_start_node(request.to.clone()).ok() {
            Some(node_id) => node_id, // the link was found as expected
            None => {
                // if the to link is not in the graph, we cannot calculate a path, so return None
                warn!(
                    "To link {} not found in graph, cannot calculate path",
                    request.to
                );
                return None;
            }
        };

        let to_node_idx = request.graph.get_node_idx_from_id(to_node_id);

        let mut parents = vec![None; request.graph.num_nodes()];
        let distance_to_goal: f64;

        let dijkstra_request = match DijkstraRequestBuilder::default()
            // copies graph, from, to, departure time, person, vehicle values from the lcp request
            .from_least_cost_path_request(&request) {
            Ok(builder) => builder,  // if succesful, continue
            Err(err) => {  // else, likely the given from- or to-links do not exist
                warn!("Error building Dijkstra request from least cost path request: {}, cannot calculate path", err);
                return None;
            }
        }
            // continue building
            .heuristic_mode(HeuristicMode::with_heuristic(&self.heuristic))
            .travel_time(self.travel_time.as_ref())
            .travel_disutility(self.travel_disutility.as_ref())
            .options(AltOptions::new(to_node_idx, parents))
            .build()
            .unwrap();

        (distance_to_goal, parents) = match Dijkstra::dijkstra_core(dijkstra_request) {
            Ok(DijkstraResult::SingleDistWithParents(current_distance, parents)) => {
                (current_distance, parents)
            }
            Err(e) => {
                warn!("Error during Dijkstra: {} cannot calculate path.", e);
                return None;
            }
            _ => panic!("Dijkstra with AltOptions should return SingleDistWithParents result"),
        };

        // if the returned distance to the target is infinity, it is unreachable, so we return None
        if distance_to_goal == f64::INFINITY || distance_to_goal.is_nan() {
            return None;
        }

        // TODO it's correct that we return the link path here? and not node path as apparently before?
        // NOTE: it's both in Java, do we need that? Nodes are of course easy to get when you have the graph
        let link_path = match Self::extract_link_path(to_node_idx, parents, request.graph) {
            Ok(link_path) => link_path,
            Err(err) => {
                warn!(
                    "Error extracting link path from node path: {}, cannot calculate path",
                    err
                );
                return None;
            }
        };

        return Some(LeastCostPath {
            path: link_path,
            travel_time: distance_to_goal,
        });
    }
}

/// Heuristic that uses landmarks and triangle inequality to estimate
#[derive(Clone, Debug)]
pub(crate) struct AltHeuristic {
    landmark_data: AltLandmarkData,
    // some internal state
}

impl AltHeuristic {
    pub fn new(landmark_data: AltLandmarkData) -> Self {
        AltHeuristic { landmark_data }
    }
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

        let mut h: f64 = 0.0;
        for (_landmark_idx, lm_travel_disutility) in self
            .landmark_data
            .travel_disutilities_to_all()
            .iter()
            .enumerate()
        {
            let from_distance = lm_travel_disutility[from_idx]; // (SL,LS)
            let to_distance = lm_travel_disutility[to_idx]; // (LT,TL)

            let forward_estimate = from_distance.0 - to_distance.1;
            let backward_estimate = to_distance.0 - from_distance.1;

            h = h.max(forward_estimate.max(backward_estimate))
        }

        let result = if h < 0.0 { 0.0 as Time } else { h as Time };

        result
    }
}

#[cfg(test)]
mod tests {
    use crate::simulation::replanning::routing::alt_router::{
        AStarHeuristic, AltHeuristic, ZeroHeuristic,
    };
    use crate::simulation::replanning::routing::least_cost_path_caluclator::TravelDisutility;
    use crate::simulation::replanning::routing::least_cost_path_caluclator::TravelTime;
    use crate::simulation::replanning::routing::least_cost_path_caluclator::Utility;
    use crate::simulation::scenario::population::InternalPerson;

    use crate::simulation::replanning::routing::least_cost_path_caluclator::Time;
    use crate::simulation::replanning::routing::least_cost_path_caluclator::{
        FreeSpeedTravelTimeAndDisutility, LeastCostPathCalculator,
    };

    use crate::simulation::config::{MetisOptions, PartitionMethod};
    use crate::simulation::id::Id;
    use crate::simulation::replanning::routing::alt_landmark_data::AltLandmarkData;
    use crate::simulation::replanning::routing::alt_router::AStarRouter;
    use crate::simulation::replanning::routing::graph::tests::get_triangle_test_graph;
    use crate::simulation::replanning::routing::least_cost_path_caluclator::{
        IntNodeGraph, LeastCostPath, LeastCostPathRequestBuilder,
    };
    use crate::simulation::replanning::routing::network_converter::NetworkConverter;
    use crate::simulation::scenario::network::{Link, Network, Node};
    use crate::simulation::scenario::vehicles::InternalVehicleType;
    use crate::simulation::scenario::vehicles::{Garage, InternalVehicle};
    use macros::integration_test;
    use std::collections::HashMap;
    use std::path::PathBuf;

    /// Runs an A* least cost path run based on the given input and compares to expected output.
    fn calc_route_and_check<H: AStarHeuristic>(
        router: &AStarRouter<H>, // &AltRouter,
        graph: &dyn IntNodeGraph,
        from: &str, // usize,
        to: &str,   // usize,
        vehicle: Option<&InternalVehicle>,
        expected_travel_time: Option<Time>,
        expected_path: Option<Vec<&str>>,
    ) {
        let request = LeastCostPathRequestBuilder::default()
            .graph(graph)
            .from(Id::create(from))
            .to(Id::create(to))
            .vehicle(vehicle)
            .build()
            .unwrap();

        let result = router.calc_route(request);
        let expected_result = match (expected_travel_time, expected_path) {
            (Some(expected_travel_time), Some(expected_path)) => Some(LeastCostPath {
                travel_time: expected_travel_time,
                path: expected_path
                    .iter()
                    .map(|link_id_str| Id::create(link_id_str))
                    .collect(),
            }),
            (None, None) => None,
            (None, Some(_)) | (Some(_), None) => panic!(
                "Expected travel time and expected path \
            should either both be None or both be Some"
            ),
        };

        assert_eq!(
            result,
            expected_result // AltQueryResult {
                            //     travel_time: expected_travel_time,
                            //     node_path: expected_path,
                            // }
        )
    }

    // Define a time-dependent travel disutility for testing
    // Disutility = (freespeed) travel_time * (1 + 10 * departure time)
    // Very high increase in disutility with time, to ensure that we see a difference also for
    // very short routes, such as in the triangle test graph
    #[derive(Clone, Debug)]
    struct TimeDependentDisutility;

    impl TravelDisutility for TimeDependentDisutility {
        fn travel_disutility(
            &self,
            link: &Link,
            departure_time: Time,
            _person: Option<&InternalPerson>,
            vehicle: Option<&InternalVehicle>,
        ) -> Utility {
            // Get base travel time using free speed
            let free_speed_calc = FreeSpeedTravelTimeAndDisutility;
            let travel_time = free_speed_calc.travel_time(link, departure_time, None, vehicle);

            // Apply time-dependent factor: increases with time (minimal congestion at time 0)
            let time_factor = 1.0 + 10.0 * departure_time;

            travel_time * time_factor
        }

        fn clone_box(&self) -> Box<dyn TravelDisutility> {
            Box::new(self.clone())
        }
    }

    /// simple test of Dijkstra (ALT with zero heuristic) and free speed travel disutility
    #[test]
    // #[ignore] //ignored because we use a global ID store now and the internal IDs are not predictable anymore // TODO I don't understand this message really. Seems to work for me?
    fn test_simple_alt_routing() {
        let graph = get_triangle_test_graph();

        let router = AStarRouter::new(
            ZeroHeuristic,
            Box::new(FreeSpeedTravelTimeAndDisutility),
            Box::new(FreeSpeedTravelTimeAndDisutility),
        );

        calc_route_and_check(
            &router,
            &graph,
            "1",
            "2",
            None, // vehicle
            Some(6.0),
            Some(vec!["4", "5"]),
        ); // previously, the node path was returned, which is [2, 3, 1]
        calc_route_and_check(
            &router,
            &graph,
            "2",
            "3",
            None, // vehicle
            Some(3.0),
            Some(vec!["5", "1"]),
        ); // previously, the node path was returned, which is [3, 1, 2]
        calc_route_and_check(
            &router,
            &graph,
            "1",
            "5",
            None, // vehicle
            Some(4.0),
            Some(vec!["4"]),
        ); // previously, the node path was returned, which is [2, 3]
    }

    #[integration_test]
    // #[ignore] //ignored because we use a global ID store now and the internal IDs are not predictable anymore // TODO see above, is this still valid?
    fn test_mode_alt_routing() {
        let network = Network::from_file(
            "./assets/adhoc_routing/no_updates/network.xml",
            1,
            &PartitionMethod::Metis(MetisOptions::default()),
        );
        let mut garage = Garage::from_file(&PathBuf::from(
            "./assets/adhoc_routing/no_updates/vehicles.xml",
        ));

        let bike_type_id = &Id::<InternalVehicleType>::get_from_ext("bike");
        let car_type_id = &Id::<InternalVehicleType>::get_from_ext("car");

        // Add vehicles for each vehicle type (since the garage file only contains vehicle types)
        garage.add_veh_by_type(
            &Id::create("bike_person"), // create some person
            bike_type_id,               // vehicle type
        );
        garage.add_veh_by_type(&Id::create("car_person"), car_type_id);

        let graph_by_vehicle_type =
            NetworkConverter::convert_network_with_vehicle_types(&network, &garage.vehicle_types);

        let router_by_vehicle_type = graph_by_vehicle_type
            .iter()
            .map(|(id, g)| {
                let landmark_data = AltLandmarkData::new(g).unwrap();

                (
                    id,
                    AStarRouter::new(
                        // create new heuristic, based on the graph for the vehicle type. TODO note: so far, the travel time for the landmarks is truly freespeed, while previously, it was maxspeed = min(max_v, freespeed).
                        AltHeuristic::new(landmark_data),
                        // ZeroHeuristic,
                        Box::new(FreeSpeedTravelTimeAndDisutility),
                        Box::new(FreeSpeedTravelTimeAndDisutility),
                    ),
                )
            })
            .collect::<HashMap<_, _>>();

        // check routing for bike
        let bike_vehicle_id = garage.veh_id(
            &Id::create("bike_person"),
            &Id::<InternalVehicleType>::get_from_ext("bike"),
        );

        calc_route_and_check(
            router_by_vehicle_type.get(bike_type_id).unwrap(),
            graph_by_vehicle_type.get(bike_type_id).unwrap(),
            "link0", // 0,
            "link4", // 5,
            garage.vehicles.get(&bike_vehicle_id),
            Some(240.0),                           // Some(280.0),
            Some(vec!["link1", "link2", "link3"]), // Some(vec![0, 1, 2, 3, 4, 5]),
        );

        // check routing for car
        let car_vehicle_id = garage.veh_id(
            &Id::create("car_person"),
            &Id::<InternalVehicleType>::get_from_ext("car"),
        );

        calc_route_and_check(
            router_by_vehicle_type.get(car_type_id).unwrap(),
            graph_by_vehicle_type.get(car_type_id).unwrap(),
            "link0", // 0,
            "link4", // 5,
            garage.vehicles.get(&car_vehicle_id),
            Some(100.0),
            Some(vec!["link5", "link6"]), // Some(vec![0, 1, 6, 4, 5]),
        )
    }

    /// Test that ALT heuristic and zero heuristic find the same optimal path
    #[test]
    fn test_alt_vs_zero_heuristic_same_result() {
        let graph = get_triangle_test_graph();

        // Router with zero heuristic (pure Dijkstra)
        let zero_router = AStarRouter::new(
            ZeroHeuristic,
            Box::new(FreeSpeedTravelTimeAndDisutility),
            Box::new(FreeSpeedTravelTimeAndDisutility),
        );

        // Router with ALT heuristic
        let landmark_data = AltLandmarkData::new(&graph).unwrap();
        let alt_router = AStarRouter::new(
            AltHeuristic::new(landmark_data),
            Box::new(FreeSpeedTravelTimeAndDisutility),
            Box::new(FreeSpeedTravelTimeAndDisutility),
        );

        // Both should find the same optimal path
        let request_zero = LeastCostPathRequestBuilder::default()
            .graph(&graph)
            .from(Id::create("1"))
            .to(Id::create("2"))
            .build()
            .unwrap();

        let request_alt = LeastCostPathRequestBuilder::default()
            .graph(&graph)
            .from(Id::create("1"))
            .to(Id::create("2"))
            .build()
            .unwrap();

        let zero_result = zero_router.calc_route(request_zero);
        let alt_result = alt_router.calc_route(request_alt);

        assert_eq!(
            zero_result, alt_result,
            "ALT and ZeroHeuristic should find the same optimal path"
        );
    }

    /// Test routing when start and destination are the same (zero distance)
    #[test]
    fn test_same_start_and_destination() {
        let graph = get_triangle_test_graph();
        let router = AStarRouter::new(
            ZeroHeuristic,
            Box::new(FreeSpeedTravelTimeAndDisutility),
            Box::new(FreeSpeedTravelTimeAndDisutility),
        );

        let request = LeastCostPathRequestBuilder::default()
            .graph(&graph)
            .from(Id::create("1")) // link 1 ends in node 2
            .to(Id::create("4")) // link 4 starts in node 2
            .build()
            .unwrap();

        let result = router.calc_route(request);

        // Route from node to itself should have zero distance and empty path, since we are routing
        // from node 2 to node 2
        assert!(result.is_some());
        let path = result.unwrap();
        assert_eq!(path.travel_time, 0.0);
        assert!(path.path.is_empty());
    }

    /// Test time-dependent routing: different departure times produce different routes
    /// when travel disutility varies with time (e.g., congestion patterns)
    #[test]
    fn test_time_dependent_routing() {
        let graph = get_triangle_test_graph();

        // Create a router with time-independent disutility
        // disutility = freespeed travel_time
        let router_time_indep = AStarRouter::new(
            ZeroHeuristic,
            Box::new(FreeSpeedTravelTimeAndDisutility),
            Box::new(FreeSpeedTravelTimeAndDisutility),
        );

        // Create a router with time-dependent disutility
        // disutility = freespeed travel_time * (1 + 0.5 * sin(departure_time / 3600))
        // Note that at departure_time=0, the disutility coincides with the time-independent router
        // from above.
        // Therefore, if both routers start at the same time, if they return different disutilities,
        // this implies that time-dependent routing is working
        let router_time_dep = AStarRouter::new(
            ZeroHeuristic,
            Box::new(FreeSpeedTravelTimeAndDisutility),
            Box::new(TimeDependentDisutility),
        );

        // Route at time 0.0
        let request = LeastCostPathRequestBuilder::default()
            .graph(&graph)
            .from(Id::create("1"))
            .to(Id::create("2"))
            .departure_time(0.0)
            .build()
            .unwrap();

        let result_time_indep = router_time_indep.calc_route(request.clone());
        let result_time_dep = router_time_dep.calc_route(request);

        let disutility_time_indep = result_time_indep.unwrap().travel_time;
        let disutility_time_dep = result_time_dep.unwrap().travel_time; // TODO note: the field travel time here is acually the disutility, this should be fixed/renamed

        let ratio = disutility_time_indep / disutility_time_dep;
        dbg!(ratio);
        assert!(
            ratio < 1.0,
            "Ratio of time independent disutility to time dependent disutility should be less \
            than 1.0, since the time dependent disutility increases with time, but got {}",
            ratio
        );
    }

    /// Test routing with non-existing or disconnected links, should return None
    #[test]
    fn test_nonexisting_or_disconnected_links() {
        let network = Network::from_file(
            "./assets/adhoc_routing/no_updates/network.xml",
            1,
            &PartitionMethod::Metis(MetisOptions::default()),
        );
        let graph = NetworkConverter::convert_network(&network, None);

        let router = AStarRouter::new(
            ZeroHeuristic,
            Box::new(FreeSpeedTravelTimeAndDisutility),
            Box::new(FreeSpeedTravelTimeAndDisutility),
        );

        // Verify the behaviour when the from-link or to-link doesn't exist, and when they exist but
        // are not connected

        let nonexisting_from_link_request = LeastCostPathRequestBuilder::default()
            .graph(&graph)
            .from(Id::create("link100"))
            .to(Id::create("link4")) // Non-existent node ID
            .build()
            .unwrap();
        let nonexisting_to_link_request = LeastCostPathRequestBuilder::default()
            .graph(&graph)
            .from(Id::create("link0"))
            .to(Id::create("link999")) // Non-existent node ID
            .build()
            .unwrap();
        let unreachable_request = LeastCostPathRequestBuilder::default()
            .graph(&graph)
            .from(Id::create("link6"))
            .to(Id::create("link0")) // Non-existent node ID
            .build()
            .unwrap();

        for request in [
            nonexisting_from_link_request,
            nonexisting_to_link_request,
            unreachable_request,
        ]
        .iter()
        {
            let result = router.calc_route((*request).clone());
            // In all cases, should return none
            assert!(result.is_none());
        }
    }

    /// Test that ALT heuristic provides a valid admissible lower bound
    /// (never overestimates the actual distance)
    #[test]
    fn test_alt_heuristic_admissibility() {
        let graph = get_triangle_test_graph();

        let landmark_data = AltLandmarkData::new(&graph).unwrap();
        let alt_heuristic = AltHeuristic::new(landmark_data);

        // Test heuristic estimates for various node pairs
        let test_pairs = vec![("1", "2"), ("2", "3"), ("1", "3"), ("2", "1")];

        // These are the true "distances" between the node pairs based on the triangle test graph
        // and free speed travel disutilities.
        // Note that any travel disutility that is not freespeed-based should return higher or
        // equal travel disutilities, compared to freespeed. For instance, considering
        // maxspeed-based travel disutility, the true distances would be higher, since maxspeed is
        // upper bounded by freespeed. So if the heuristic lower bounds this disutility, then it
        // lower bounds any travel disutility.
        // TODO in theory, one could imagine settings where this doesn't hold, i.e., we have travel
        // disutilities that are lower than pure freespeed-based. This could be if we for instance
        // add negative disutilities for some "comfort" along certain routes. As of now, the ALT
        // would no longer be an admissible heuristic in this case.
        let test_pair_true_distances_freespeed = vec![1.0, 4.0, 2.0, 6.0];

        for (i, (from_str, to_str)) in test_pairs.iter().enumerate() {
            let heuristic_estimate = alt_heuristic.estimate(
                &graph,
                Id::<Node>::create(from_str),
                Id::<Node>::create(to_str),
            );

            // Heuristic should not be NaN
            assert!(
                !heuristic_estimate.is_nan(),
                "Heuristic estimate should not be NaN for {} to {}",
                from_str,
                to_str
            );

            // Heuristic should be non-negative
            assert!(
                heuristic_estimate >= 0.0,
                "Heuristic estimate should be non-negative for {} to {}, got {}",
                from_str,
                to_str,
                heuristic_estimate
            );

            // Heuristic must be lower or equal to the true distance
            assert!(
                heuristic_estimate <= test_pair_true_distances_freespeed[i],
                "Heuristic estimate should always be lower or equal to the true distance"
            );
        }
    }
}
