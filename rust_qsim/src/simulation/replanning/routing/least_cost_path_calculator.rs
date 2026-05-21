use crate::simulation::id::Id;
use crate::simulation::replanning::routing::graph::IndexableGraph;
use crate::simulation::scenario::network::Link;
use crate::simulation::scenario::population::InternalPerson;
use crate::simulation::scenario::vehicles::InternalVehicle;
use derive_builder::Builder;
use std::fmt::Debug;

// TODO: The comment below is outdated and can go, but:
// should the "Time" used in routing stay f64, or do we use u64 like the SimTime, since it was
// decided that that gives a good enough accuracy and max duration?
// "normal" time representation is u32 for now, but we might want to use f64 for the future
pub type Time = f64; // todo remove, replaced by SimTime
pub type Disutility = f64;

/// Travel time function, mapping any network link to a travel time, depending on the departure time
/// and optionally the person and vehicle.
pub trait TravelTime: Debug {
    fn travel_time(
        &self,
        link: &Link,
        departure_time: Time, // TODO shoulod be SimTime
        person: Option<&InternalPerson>,
        vehicle: Option<&InternalVehicle>,
    ) -> Time;
}

/// Travel disutility function, mapping any network link to a travel disutility, depending on the
/// departure time and optionally the person and vehicle.
pub trait TravelDisutility: Debug {
    fn travel_disutility(
        &self,
        link: &Link,
        departure_time: Time,
        person: Option<&InternalPerson>,
        vehicle: Option<&InternalVehicle>,
    ) -> Disutility;
    // fn get_link_min_travel_disutility(&self, &Link)  returns the smallest possible, used for landmarks
}

/// An implementation of both `TravelTime` and `TravelDisutility`, purely based on freespeed travel
/// times. The travel time is simply the link length divided by the freespeed, ignoring any given
/// vehicle type and its max speed.

/// The travel disutility is equal to the travel time.
#[derive(Clone, Debug)]
pub struct FreeSpeedTravelTimeAndDisutility;

impl TravelTime for FreeSpeedTravelTimeAndDisutility {
    fn travel_time(
        &self,
        link: &Link,
        _departure_time: Time,
        _person: Option<&InternalPerson>,
        _vehicle: Option<&InternalVehicle>,
    ) -> Time {
        // the given vehicle type is ignored => true freespeed
        link.length / link.freespeed // SimTime::from_nanos(1e9 * ...)
    }
}

impl TravelDisutility for FreeSpeedTravelTimeAndDisutility {
    fn travel_disutility(
        &self,
        link: &Link,
        departure_time: Time,
        person: Option<&InternalPerson>,
        vehicle: Option<&InternalVehicle>,
    ) -> Disutility {
        // TODO: Adapt the factor for the Disutility
        self.travel_time(link, departure_time, person, vehicle) * 1.0 // todo .as_secs() * 1.0
        // travel DISutility is simply the travel time here, since higher time corresponds to lower utility
    }
}

/// An implementation of both `TravelTime` and `TravelDisutility`, mostly based on freespeed travel
/// times. However, when a vehicle is given, its max speed is respected, with min(freespeed, v_max)
/// being used to determine the travel time.
/// The travel disutility is equal to the travel time.
#[derive(Clone, Debug)]
pub struct FreeOrMaxSpeedTravelTimeAndDisutility;

impl TravelTime for FreeOrMaxSpeedTravelTimeAndDisutility {
    fn travel_time(
        &self,
        link: &Link,
        _departure_time: Time,
        _person: Option<&InternalPerson>,
        vehicle: Option<&InternalVehicle>,
    ) -> Time {
        // respect the given vehicle type, if provided
        let max_speed = if let Some(v) = vehicle {
            v.max_v.min(link.freespeed)
        } else {
            link.freespeed
        };

        link.length / max_speed
    }
}

impl TravelDisutility for FreeOrMaxSpeedTravelTimeAndDisutility {
    fn travel_disutility(
        &self,
        link: &Link,
        departure_time: Time,
        person: Option<&InternalPerson>,
        vehicle: Option<&InternalVehicle>,
    ) -> Disutility {
        // TODO: Adapt the factor for the Disutility
        self.travel_time(link, departure_time, person, vehicle) * 1.0
        // travel DISutility is simply the travel time here, since higher time corresponds to lower utility
    }
}

/// A request for the calculation of least cost paths. Contain all relevant data for the
/// calculation, that is
/// - from- and to-links, and a reference to the graph representing the network (must be an
///     `IndexableGraph` so that the routing algorithms can be efficient)
/// - the departure time at the from-node and optionally a person and vehicle. These are passed to
///     travel time and disutility functions in routing (the latter being used as cost, and the
///     former used to determine the arrival times at specific nodes)
#[derive(Builder, Clone)]
pub struct LeastCostPathRequest<'r> {
    // From and to are deliberately not nodes but links. This allows considering those links as well during routing.
    pub from: Id<Link>,
    pub to: Id<Link>,
    pub graph: &'r dyn IndexableGraph, // contains the graph of the network
    #[builder(default)]
    pub departure_time: Time,
    #[builder(default)]
    pub person: Option<&'r InternalPerson>,
    #[builder(default)]
    pub vehicle: Option<&'r InternalVehicle>,
}

/// A least cost path, given as a vector of network link ids, together with the travel time needed
/// to take the path and the corresponding travel disutility (it's the latter which is optimal, so
/// it's truly a least-disutility path).
#[derive(PartialEq, Debug)]
pub struct LeastCostPath {
    pub path: Vec<Id<Link>>,
    pub travel_time: Time,
    pub travel_disutility: Disutility,
}

/// Router that calculates a least cost path between given from- and to-links on a given graph.
pub trait LeastCostPathCalculator {
    /// Calculate the least cost path as defined in the request. Requests contain from- and
    /// to-links, a reference to the graph to route on as well as the departure time and an optional
    /// person and vehicle.
    /// If no path is found, either because the to-link is unreachable or because the from- or
    /// to-link do not exist in the graph, None is returned.
    /// Otherwise, the path is returned together with its travel time and disutility.
    // todo QUESTION: previously, we had &mut self here, but I don't see why. Do we need it?
    fn calc_route(&self, request: LeastCostPathRequest) -> Option<LeastCostPath>;
}

#[cfg(test)]
mod tests {
    use crate::simulation::id::Id;

    use crate::simulation::replanning::routing::a_star_router::AStarRouter;
    use crate::simulation::replanning::routing::a_star_router::ZeroHeuristic;
    use crate::simulation::replanning::routing::graph::Graph;
    use crate::simulation::replanning::routing::graph::tests::{
        get_triangle_test_graph, get_triangle_test_network,
    };
    use crate::simulation::replanning::routing::least_cost_path_calculator::{
        FreeOrMaxSpeedTravelTimeAndDisutility, LeastCostPath, LeastCostPathRequestBuilder,
    };
    use crate::simulation::replanning::routing::least_cost_path_calculator::{
        LeastCostPathCalculator, TravelDisutility, TravelTime,
    };
    use crate::simulation::scenario::network::Link;
    use crate::simulation::scenario::vehicles::InternalVehicle;

    /// simple test just to make sure that the interface works. More precise testing is done
    /// in the respective files where implementations of LeastCostPathCaltulator are defined.
    #[test]
    fn test_least_cost_path_interface() {
        // simple A*-Router with zero heuristic => is Dijkstra.
        let router = AStarRouter::new(
            ZeroHeuristic,
            Box::new(FreeOrMaxSpeedTravelTimeAndDisutility),
            Box::new(FreeOrMaxSpeedTravelTimeAndDisutility),
        );
        // triangle graph
        let network = get_triangle_test_network();
        let graph = get_triangle_test_graph(&network);

        let request = LeastCostPathRequestBuilder::default()
            .from(Id::create("1")) // these links are connected via
            .to(Id::create("5")) // link "4", which takes 4 secs
            .graph(&graph)
            .build()
            .unwrap();

        let expected_path: Vec<Id<Link>> = [Id::<Link>::create("4")].into_iter().collect();

        let result = router.calc_route(request);
        assert_eq!(
            result,
            Some(LeastCostPath {
                travel_time: 4.0,
                travel_disutility: 4.0,
                path: expected_path,
            })
        );
    }

    /// Test the FreeOrMaxSpeedTravelTimeAndDisutility implementation of TravelTime and TravelDisutility
    #[test]
    fn test_free_or_max_speed_travel_time_and_disutility() {
        let fomsttad = FreeOrMaxSpeedTravelTimeAndDisutility;

        let network = get_triangle_test_network();
        let graph = get_triangle_test_graph(&network);

        let link = graph.edge(Id::create("4")).unwrap();

        assert_eq!(fomsttad.travel_time(link, 0.0, None, None), 4.0);
        assert_eq!(fomsttad.travel_disutility(link, 0.0, None, None), 4.0);

        // also test that the vehicle's max speed is respected

        // vehicle max_v is lower than freespeed, so travel time will be longer
        let vehicle = InternalVehicle::new(0, 0, 1000.0, 0.0);

        assert_eq!(fomsttad.travel_time(link, 0.0, None, Some(&vehicle)), 10.0);
        assert_eq!(
            fomsttad.travel_disutility(link, 0.0, None, Some(&vehicle)),
            10.0
        );
    }
}
