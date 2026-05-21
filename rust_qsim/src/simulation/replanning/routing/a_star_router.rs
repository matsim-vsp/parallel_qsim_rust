use crate::simulation::id::Id;
use crate::simulation::replanning::routing::a_star_core::{
    AStarCoreResult, AStarRequestBuilder, HeuristicMode, One2OneWithParentsAStarActions,
    a_star_core,
};
use crate::simulation::replanning::routing::alt_landmark_data::AltLandmarkData;
use crate::simulation::replanning::routing::graph::{GraphError, IndexableGraph, NodeIndex};
use crate::simulation::replanning::routing::least_cost_path_calculator::{
    Disutility, LeastCostPath, LeastCostPathCalculator, LeastCostPathRequest, TravelDisutility,
    TravelTime,
};
use crate::simulation::scenario::network::{Link, Node};
use std::time::Duration;
use tracing::{error, warn};

/// A heuristic to be used in A*. Given a from and to-node, estimates the distance, that is,
/// disutility, between them.
/// Is not allowed to overestimate distances. It is expected of implementations to respect this.
pub trait AStarHeuristic: Clone {
    /// Estimate distance, i.e., travel disutility, between from-node and to-node in the given
    /// graph. Never overestimates the disutilility.
    fn estimate(&self, graph: &dyn IndexableGraph, from: Id<Node>, to: Id<Node>) -> Disutility;
}

/// Zero heuristic estimates all distances to be zero. With this, the A* collapses into Dijkstra.
#[derive(Clone)]
pub(crate) struct ZeroHeuristic;

impl AStarHeuristic for ZeroHeuristic {
    fn estimate(&self, _graph: &dyn IndexableGraph, _from: Id<Node>, _to: Id<Node>) -> Disutility {
        0.
    }
}

/// Heuristic that uses landmarks and triangle inequality to estimate distance between two nodes
#[allow(dead_code)]
#[derive(Clone, Debug)]
pub(crate) struct AltHeuristic {
    landmark_data: AltLandmarkData,
}

impl AltHeuristic {
    #[allow(dead_code)]
    pub fn new(landmark_data: AltLandmarkData) -> Self {
        AltHeuristic { landmark_data }
    }
}

impl AStarHeuristic for AltHeuristic {
    /// Estimate the distance between the from- and to-node in the given graph by using landmarks.
    fn estimate(&self, graph: &dyn IndexableGraph, from: Id<Node>, to: Id<Node>) -> Disutility {
        /* The ALT algorithm uses two lower bounds for each Landmark:
         * given: source node S, target node T, landmark L
         * then, due to the triangle inequality:
         *  1) ST + TL >= SL --> ST >= SL - TL (forward estimate)
         *  2) LS + ST >= LT --> ST >= LT - LS (backward estimate)
         * The algorithm is interested in the largest possible value of (SL-TL) and (LT-LS),
         * as this gives the closest approximation for the minimal travel disutility required to
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

        let result: Disutility = if h < 0.0 { 0.0 } else { h };

        result
    }
}

/// A* router, an implementation of the LeastCostPathCalculator trait.
/// Owns a heuristic, a travel time calculator and a travel disutility calculator, which are used
/// in the A* search.
/// The heuristic is used to estimate the remaining travel time to the destination, and should be
/// admissible (i.e., never overestimate the actual remaining travel time).
/// The travel time is used to track the arrival time at the nodes along the path, while the travel
/// disutility is used as cost, i.e., this is what the A* search minimizes.
pub struct AStarRouter<H: AStarHeuristic> {
    heuristic: H,
    travel_time: Box<dyn TravelTime>,
    travel_disutility: Box<dyn TravelDisutility>,
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
    /// Given a to-node and a vector of parents, extracts the path to the to-node by recursing
    /// through the list of parents.
    /// Stops when a node with no parent is found, which should be the from-node. But this is not
    /// verified here, since this function doesn't know the from-node.
    /// However, the below function extract_link_path, which calls this function, does verify that
    /// the found path is correct.
    fn extract_node_path(to: NodeIndex, parents: Vec<Option<NodeIndex>>) -> Vec<NodeIndex> {
        let mut node_path = Vec::new();
        let mut current = to;

        node_path.push(to);
        while let Some(father) = parents[current] {
            // while a parent node exists
            node_path.push(father); // add it to the path
            current = father; // and continue with that node, i.e., look for its parent next
        }

        node_path.reverse();
        node_path
    }

    /// Given a to-link, a vector parents and the graph, extracts the path of links to the to-link.
    /// Uses the above extract_node_path to get the path of nodes, and then looks up the
    /// corresponding links in the graph.
    /// Calls the below `verify_path` to check correctness of the found path. Because of this, a
    /// from-link must also be given.
    fn extract_link_path(
        to_link: Id<Link>,
        from_link: Id<Link>,
        parents: Vec<Option<NodeIndex>>,
        graph: &dyn IndexableGraph,
    ) -> Result<Option<Vec<Id<Link>>>, GraphError> {
        // convert given "to" link id to node id, by looking for the start node of the link
        let to_node_id = graph.get_start_node(to_link.clone())?;
        let to_node_idx = graph.get_node_idx_from_id(to_node_id);

        // get node path
        let node_path = Self::extract_node_path(to_node_idx, parents);

        let mut link_path = Vec::new();

        // look for link connecting node i and node i+1
        for i in 0..node_path.len() - 1 {
            let start_node = node_path[i];
            let end_node = node_path[i + 1];

            // go through outgoing edges of the start node, and find the one that has the end node
            // as head
            for j in graph.outgoing_edges_as_idx(start_node) {
                if graph.get_end_node_as_idx(j)? == end_node {
                    // a link connecting the start node and the end node was found, now get the
                    // actual Id<Link> of the link
                    link_path.push(graph.get_link_id_from_idx(j)?);
                    break;
                }
            }
        }

        // verify the found path: if incorrect, return None instead of a path
        if !Self::verify_path(&link_path, graph, from_link, to_link)? {
            return Ok(None);
        }
        Ok(Some(link_path))
    }

    /// Given a path, graph from- and to-link, verifies that the path starts at the end node of the
    /// from-link and ends at the start node of the to-link.
    fn verify_path(
        path: &Vec<Id<Link>>,
        graph: &dyn IndexableGraph,
        from_link: Id<Link>,
        to_link: Id<Link>,
    ) -> Result<bool, GraphError> {
        let end_node_of_from_link = graph.get_end_node(from_link)?;
        let start_node_of_to_link = graph.get_start_node(to_link)?;

        let last_index = match path.len() {
            0 => return Ok(end_node_of_from_link == start_node_of_to_link),
            path_length => path_length - 1,
        };

        let first_node_of_path = graph.get_start_node(path[0].clone())?;
        let last_node_of_path = graph.get_end_node(path[last_index].clone())?;

        // verify if path starts at end node of from-link and ends at start node of to-link
        Ok(first_node_of_path == end_node_of_from_link
            && last_node_of_path == start_node_of_to_link)
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

        // convert to-node id to node index
        let to_node_idx = request.graph.get_node_idx_from_id(to_node_id);

        // initialize parents vector and the disutility from the from-node to the to-node.
        // owned by this function but filled by a_star_core
        let mut parents = vec![None; request.graph.num_nodes()];
        let optimal_disutility: Disutility;
        let associated_travel_time: Duration;

        // create request for a_star_core
        let a_star_request = match AStarRequestBuilder::default()
            // copies graph, from, to, departure time, person, vehicle values from the lcp request
            .from_least_cost_path_request(&request)
        {
            Ok(builder) => {
                // if succesful, continue building
                builder
                    // set heuristic to the heuristic of the router
                    .heuristic_mode(HeuristicMode::with_heuristic(&self.heuristic))
                    .travel_time(self.travel_time.as_ref())
                    .travel_disutility(self.travel_disutility.as_ref())
                    // set AStarActions to the use case One to One with parent tracking
                    .options(One2OneWithParentsAStarActions::new(to_node_idx, parents))
                    .build()
                    .unwrap()
            }
            Err(err) => {
                // else, likely the given from- or to-links do not exist
                warn!(
                    "Error building A* request from least cost path request: {}, \
                    cannot calculate path",
                    err
                );
                return None;
            }
        };

        // call a_star_core with the request, and extract the distance to the goal and the
        // parents vector from the result
        (optimal_disutility, associated_travel_time, parents) = match a_star_core(a_star_request) {
            // Standard case: A* returned a valid result.
            Ok(AStarCoreResult::SingleDistWithParents(distance, time, parents)) => {
                // if the returned distance to the target is infinity or NaN, it is unreachable, so
                // we return None
                if distance == f64::INFINITY || distance.is_nan() {
                    warn!(
                        "To link {} is unreachable from from link {}, cannot calculate path",
                        request.to, request.from
                    );
                    return None;
                }
                // else, we take the found shortest "distance" as the optimal disutility
                (distance, time, parents)
            }
            // Unsuccesful case: Some error occurred in A*, e.g., a given link or node was not
            // found, so we cannot calculate a path. Return None
            Err(e) => {
                warn!("Error during A*: {} cannot calculate path.", e);
                return None;
            }
            // Unrecoverable error: A* returned the wrong result type. This should not happen,
            // since we use the A* use case OneToOneWithParents, which always builds results
            // of type SingleDistWithParents.
            _ => panic!(
                "A* with One2OneWithParentsAStarActions should return \
                SingleDistWithParents result"
            ),
        };

        let link_path =
            match Self::extract_link_path(request.to, request.from, parents, request.graph) {
                Ok(Some(link_path)) => link_path, // all good, path was found
                Ok(None) => {
                    // verification negative: incorrect path was found
                    error!(
                        "Path search unsuccesful: A path was found, but it does not connect \
                    the given from- and to-links. Something went wrong in Dijkstra or path \
                    extraction."
                    );
                    return None;
                }
                // from- or to-link not found in the graph. Note: this case should never occur,
                // since an invalid from- or to-link would have been detected already during A*
                Err(e) => {
                    error!(
                        "Path search unsuccessful: A path was found, but when verifying its\
                    correctness, an error occured: {}",
                        e
                    );
                    return None;
                }
            };

        return Some(LeastCostPath {
            path: link_path,
            travel_time: associated_travel_time,
            travel_disutility: optimal_disutility,
        });
    }
}

#[cfg(test)]
mod tests {
    use crate::simulation::replanning::routing::least_cost_path_calculator::TravelDisutility;
    use crate::simulation::replanning::routing::least_cost_path_calculator::TravelTime;
    use crate::simulation::replanning::routing::least_cost_path_calculator::{
        Disutility, FreeSpeedTravelTimeAndDisutility,
    };
    use crate::simulation::scenario::population::InternalPerson;

    use crate::simulation::replanning::routing::least_cost_path_calculator::{
        FreeOrMaxSpeedTravelTimeAndDisutility, LeastCostPathCalculator,
    };

    use crate::simulation::config::{MetisOptions, PartitionMethod};
    use crate::simulation::id::Id;
    use crate::simulation::replanning::routing::a_star_router::{
        AStarHeuristic, AStarRouter, AltHeuristic, ZeroHeuristic,
    };
    use crate::simulation::replanning::routing::alt_landmark_data::AltLandmarkData;
    use crate::simulation::replanning::routing::graph::IndexableGraph;
    use crate::simulation::replanning::routing::graph::tests::{
        get_triangle_test_graph, get_triangle_test_network,
    };
    use crate::simulation::replanning::routing::least_cost_path_calculator::{
        LeastCostPath, LeastCostPathRequestBuilder,
    };
    use crate::simulation::replanning::routing::network_converter::NetworkConverter;
    use crate::simulation::scenario::network::{Link, Network, Node};
    use crate::simulation::scenario::vehicles::InternalVehicleType;
    use crate::simulation::scenario::vehicles::{Garage, InternalVehicle};
    use crate::simulation::time::SimTime;
    use std::time::Duration;

    use macros::integration_test;
    use std::collections::HashMap;
    use std::path::PathBuf;

    /// Runs an A* least cost path run based on the given input and compares to expected output.
    fn calc_route_and_check<H: AStarHeuristic>(
        router: &AStarRouter<H>,
        graph: &dyn IndexableGraph,
        from: &str,
        to: &str,
        vehicle: Option<&InternalVehicle>,
        expected_travel_time: Option<Duration>,
        expexted_travel_disutility: Option<Disutility>,
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
        let expected_result = match (
            expected_travel_time,
            expexted_travel_disutility,
            expected_path,
        ) {
            (Some(tt), Some(td), Some(expected_path)) => Some(LeastCostPath {
                travel_time: tt,
                travel_disutility: td,
                path: expected_path
                    .iter()
                    .map(|link_id_str| Id::create(link_id_str))
                    .collect(),
            }),
            (None, None, None) => None,
            _ => panic!(
                "Expected travel time, expected travel disutility and expected path \
            should either all be None or both be Some"
            ),
        };

        assert_eq!(result, expected_result)
    }

    /// Time-dependent travel disutility for testing.
    /// Disutility = (free or max speed) travel_time * (1 + 10 * departure time)
    /// Very fast increase in disutility with time, to ensure that we see a difference also for
    /// very short routes, such as in the triangle test graph.
    #[derive(Clone, Debug)]
    struct TimeDependentDisutility;

    impl TravelDisutility for TimeDependentDisutility {
        fn travel_disutility(
            &self,
            link: &Link,
            departure_time: SimTime,
            _person: Option<&InternalPerson>,
            vehicle: Option<&InternalVehicle>,
        ) -> Disutility {
            // Get base travel time using free or max speed
            let free_speed_calc = FreeOrMaxSpeedTravelTimeAndDisutility;
            let travel_time = free_speed_calc.travel_time(link, departure_time, None, vehicle);

            // Apply time-dependent factor: increases with time (minimal congestion at time 0)
            let time_dep_factor = 1 + 10 * departure_time.as_secs();

            (travel_time * time_dep_factor as u32).as_secs_f64()
        }
        fn get_link_min_travel_disutility(&self, link: &Link) -> Disutility {
            // Get base travel time using free or max speed
            let free_speed_calc = FreeOrMaxSpeedTravelTimeAndDisutility;
            // min travel disutility is at time 0, and coincides with the free or max speed travel
            // time, since the time dependent factor is 1 at time 0
            free_speed_calc.get_link_min_travel_disutility(link)
        }
    }

    /// simple test of Dijkstra (A* with zero heuristic) and free speed travel disutility
    #[test]
    // #[ignore] //ignored because we use a global ID store now and the internal IDs are not predictable anymore // TODO I don't understand this message really. Seems to work for me?
    fn test_simple_dijkstra_routing() {
        let network = get_triangle_test_network();
        let graph = get_triangle_test_graph(&network);

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
            None,                         // vehicle
            Some(Duration::from_secs(6)), // tt
            Some(6.0 as Disutility),      // td
            Some(vec!["4", "5"]),
        ); // previously, the node path was returned, which is [2, 3, 1]
        calc_route_and_check(
            &router,
            &graph,
            "2",
            "3",
            None,                         // vehicle
            Some(Duration::from_secs(3)), // tt
            Some(3.0),                    // td
            Some(vec!["5", "1"]),
        ); // previously, the node path was returned, which is [3, 1, 2]
        calc_route_and_check(
            &router,
            &graph,
            "1",
            "5",
            None,                         // vehicle
            Some(Duration::from_secs(4)), // tt
            Some(4.0 as Disutility),      // td
            Some(vec!["4"]),
        ); // previously, the node path was returned, which is [2, 3]
    }

    /// Test routing with ALT heuristic, with two different vehicle types (car and bike) that have
    /// different travel times on the same links, and thus different optimal paths.
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
                        Box::new(FreeOrMaxSpeedTravelTimeAndDisutility),
                        Box::new(FreeOrMaxSpeedTravelTimeAndDisutility),
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
            Some(Duration::from_secs(240)),        // Some(280.0),
            Some(240.0 as Disutility),             // Some(280.0),
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
            Some(Duration::from_secs(100)), // tt
            Some(100.0 as Disutility),      // td
            Some(vec!["link5", "link6"]),   // Some(vec![0, 1, 6, 4, 5]),
        )
    }

    /// Test that ALT heuristic and zero heuristic find the same optimal path
    #[test]
    fn test_alt_vs_zero_heuristic_same_result() {
        let network = get_triangle_test_network();
        let graph = get_triangle_test_graph(&network);

        // Router with zero heuristic (pure Dijkstra)
        let zero_router = AStarRouter::new(
            ZeroHeuristic,
            Box::new(FreeOrMaxSpeedTravelTimeAndDisutility),
            Box::new(FreeOrMaxSpeedTravelTimeAndDisutility),
        );

        // Router with ALT heuristic
        let landmark_data = AltLandmarkData::new(&graph).unwrap();
        let alt_router = AStarRouter::new(
            AltHeuristic::new(landmark_data),
            Box::new(FreeOrMaxSpeedTravelTimeAndDisutility),
            Box::new(FreeOrMaxSpeedTravelTimeAndDisutility),
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
        let network = get_triangle_test_network();
        let graph = get_triangle_test_graph(&network);

        let router = AStarRouter::new(
            ZeroHeuristic,
            Box::new(FreeOrMaxSpeedTravelTimeAndDisutility),
            Box::new(FreeOrMaxSpeedTravelTimeAndDisutility),
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
        assert_eq!(path.travel_time, Duration::from_secs(0));
        assert_eq!(path.travel_disutility, 0.0);
        assert!(path.path.is_empty());
    }

    /// Test time-dependent routing: when travel disutility varies with time, the returned travel
    /// disutility differs from the time-independent case, even if the travel times are the same.
    #[test]
    fn test_time_dependent_routing() {
        let network = get_triangle_test_network();
        let graph = get_triangle_test_graph(&network);

        // Create a router with time-independent disutility
        // disutility = freespeed travel_time
        let router_time_indep = AStarRouter::new(
            ZeroHeuristic,
            Box::new(FreeOrMaxSpeedTravelTimeAndDisutility),
            Box::new(FreeOrMaxSpeedTravelTimeAndDisutility),
        );

        // Create a router with time-dependent disutility
        // disutility = freespeed travel_time * (1 + 10 * departure_time)
        // Note that at departure_time=0, the disutility coincides with the time-independent router
        // from above.
        // Therefore, if both routers start at the same time, if they return different disutilities,
        // this implies that time-dependent routing is working (or is at least doing something)
        let router_time_dep = AStarRouter::new(
            ZeroHeuristic,
            Box::new(FreeOrMaxSpeedTravelTimeAndDisutility),
            Box::new(TimeDependentDisutility),
        );

        // Route at time 0.0
        let request = LeastCostPathRequestBuilder::default()
            .graph(&graph)
            .from(Id::create("1"))
            .to(Id::create("2"))
            .departure_time(SimTime::from_secs(0))
            .build()
            .unwrap();

        let result_time_indep = router_time_indep.calc_route(request.clone()).unwrap();
        let result_time_dep = router_time_dep.calc_route(request).unwrap();

        let tt_time_indep = result_time_indep.travel_time;
        let tt_time_dep = result_time_dep.travel_time;

        let td_time_indep = result_time_indep.travel_disutility;
        let td_time_dep = result_time_dep.travel_disutility;

        let td_ratio = td_time_indep / td_time_dep;

        // travel times should be the same, since only the disutility is time dependent in our case
        assert_eq!(tt_time_indep, tt_time_dep);
        // travel disutilities should not be the same
        assert!(
            td_ratio < 1.0,
            "Ratio of time independent disutility to time dependent disutility should be less \
            than 1.0, since the time dependent disutility increases with time, but got {}",
            td_ratio
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
            Box::new(FreeOrMaxSpeedTravelTimeAndDisutility),
            Box::new(FreeOrMaxSpeedTravelTimeAndDisutility),
        );

        // Verify the behaviour when the from-link or to-link doesn't exist, and when they exist but
        // are not connected

        let nonexisting_from_link_request = LeastCostPathRequestBuilder::default()
            .graph(&graph)
            .from(Id::create("link100"))
            .to(Id::create("link4")) // Non-existent link ID
            .build()
            .unwrap();
        let nonexisting_to_link_request = LeastCostPathRequestBuilder::default()
            .graph(&graph)
            .from(Id::create("link0"))
            .to(Id::create("link999")) // Non-existent link ID
            .build()
            .unwrap();
        let unreachable_request = LeastCostPathRequestBuilder::default()
            .graph(&graph)
            .from(Id::create("link6"))
            .to(Id::create("link0")) // Link is not reachable
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
        let network = get_triangle_test_network();
        let graph = get_triangle_test_graph(&network);

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
