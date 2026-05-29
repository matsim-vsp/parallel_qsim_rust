use crate::simulation::id::Id;
use crate::simulation::scenario::network::Link;
use crate::simulation::scenario::population::InternalPerson;
use crate::simulation::scenario::vehicles::InternalVehicle;
use crate::simulation::time::SimTime;
use derive_builder::Builder;
use std::fmt::Debug;
use std::time::Duration;

pub type Disutility = f64;

/// Travel time function, mapping any network link to a travel time, depending on the departure time
/// and optionally the person and vehicle.
pub trait TravelTime: Debug {
    fn travel_time(
        &self,
        link: &Link,
        departure_time: SimTime,
        person: Option<&InternalPerson>,
        vehicle: Option<&InternalVehicle>,
    ) -> Duration;

    /// helper function, returns a box of self.clone. This allows to implement Clone for
    /// Box<dyn TravelTime>.
    fn box_clone(&self) -> Box<dyn TravelTime>;
}

/// Travel disutility function, mapping any network link to a travel disutility, depending on the
/// departure time and optionally the person and vehicle.
pub trait TravelDisutility: Debug {
    fn travel_disutility(
        &self,
        link: &Link,
        departure_time: SimTime,
        person: Option<&InternalPerson>,
        vehicle: Option<&InternalVehicle>,
    ) -> Disutility;

    /// Returns  the smallest possible travel disutility at the given link, over all times, persons
    /// and vehicles.
    /// This is used when calculating landmark data, to ensure that the ALT heuristic never
    /// overestimates the travel disutility between two nodes.
    fn get_link_min_travel_disutility(&self, link: &Link) -> Disutility;

    /// helper function, returns a box of self.clone. This allows to implement Clone for
    /// Box<dyn TravelDisutility>.
    fn box_clone(&self) -> Box<dyn TravelDisutility>;
}

impl Clone for Box<dyn TravelTime> {
    fn clone(&self) -> Self {
        self.box_clone()
    }
}

impl Clone for Box<dyn TravelDisutility> {
    fn clone(&self) -> Self {
        self.box_clone()
    }
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
        _departure_time: SimTime,
        _person: Option<&InternalPerson>,
        _vehicle: Option<&InternalVehicle>,
    ) -> Duration {
        // the given vehicle type is ignored => true freespeed
        Duration::from_secs_f64(link.length / link.freespeed)
    }

    fn box_clone(&self) -> Box<dyn TravelTime> {
        Box::new(self.clone())
    }
}

impl TravelDisutility for FreeSpeedTravelTimeAndDisutility {
    fn travel_disutility(
        &self,
        link: &Link,
        departure_time: SimTime,
        person: Option<&InternalPerson>,
        vehicle: Option<&InternalVehicle>,
    ) -> Disutility {
        // TODO: Adapt the factor for the Disutility
        self.travel_time(link, departure_time, person, vehicle)
            .as_secs_f64()
            * 1.0
        // travel DISutility is simply the travel time here, since higher time corresponds to lower utility
    }
    // min travel disutility is equal to the travel disutility, since it does not depend on time, person or vehicle
    fn get_link_min_travel_disutility(&self, link: &Link) -> Disutility {
        self.travel_disutility(link, SimTime::from_secs(0), None, None)
    }
    fn box_clone(&self) -> Box<dyn TravelDisutility> {
        Box::new(self.clone())
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
        _departure_time: SimTime,
        _person: Option<&InternalPerson>,
        vehicle: Option<&InternalVehicle>,
    ) -> Duration {
        // respect the given vehicle type, if provided
        let max_speed = if let Some(v) = vehicle {
            v.max_v.min(link.freespeed)
        } else {
            link.freespeed
        };

        Duration::from_secs_f64(link.length / max_speed)
    }
    fn box_clone(&self) -> Box<dyn TravelTime> {
        Box::new(self.clone())
    }
}

impl TravelDisutility for FreeOrMaxSpeedTravelTimeAndDisutility {
    fn travel_disutility(
        &self,
        link: &Link,
        departure_time: SimTime,
        person: Option<&InternalPerson>,
        vehicle: Option<&InternalVehicle>,
    ) -> Disutility {
        // TODO: Adapt the factor for the Disutility
        self.travel_time(link, departure_time, person, vehicle)
            .as_secs_f64()
            * 1.0
        // travel DISutility is simply the travel time here, since higher time corresponds to lower utility
    }
    fn get_link_min_travel_disutility(&self, link: &Link) -> Disutility {
        // the min travel disutility is equal to freespeed travel time, which is obtained from
        // the travel disutility function by not passing a vehicle (since that one simply calls
        // the travel time function, which respects the vehicle's max speed if given, and otherwise
        // uses the freespeed)
        self.travel_disutility(link, SimTime::from_secs(0), None, None)
    }
    fn box_clone(&self) -> Box<dyn TravelDisutility> {
        Box::new(self.clone())
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
    #[builder(default)]
    pub departure_time: SimTime,
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
    pub travel_time: Duration,
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
    fn calc_route(&self, request: LeastCostPathRequest) -> Option<LeastCostPath>;
}

#[cfg(test)]
mod tests {
    use crate::simulation::id::Id;
    use std::sync::Arc;
    use std::time::Duration;

    use crate::simulation::replanning::routing::a_star_router::DijkstraRouter;

    use crate::simulation::replanning::routing::graph::Graph;
    use crate::simulation::replanning::routing::graph::tests::{
        get_triangle_test_network, net_to_graph,
    };
    use crate::simulation::replanning::routing::least_cost_path_calculator::{
        Disutility, FreeOrMaxSpeedTravelTimeAndDisutility, LeastCostPath,
        LeastCostPathRequestBuilder,
    };
    use crate::simulation::replanning::routing::least_cost_path_calculator::{
        LeastCostPathCalculator, TravelDisutility, TravelTime,
    };
    use crate::simulation::scenario::network::Link;
    use crate::simulation::scenario::vehicles::InternalVehicle;
    use crate::simulation::time::SimTime;

    /// simple test just to make sure that the interface works. More precise testing is done
    /// in the respective files where implementations of LeastCostPathCaltulator are defined.
    #[test]
    fn test_least_cost_path_interface() {
        // triangle graph
        let network = get_triangle_test_network();

        // DijkstraRouter is an alias for AStarRouter<ZeroHeuristic>
        let router = DijkstraRouter::new(
            Arc::new(network),
            None,
            Box::new(FreeOrMaxSpeedTravelTimeAndDisutility),
            Box::new(FreeOrMaxSpeedTravelTimeAndDisutility),
        );

        let request = LeastCostPathRequestBuilder::default()
            .from(Id::create("1")) // these links are connected via
            .to(Id::create("5")) // link "4", which takes 4 secs
            .build()
            .unwrap();

        let expected_path: Vec<Id<Link>> = [Id::<Link>::create("4")].into_iter().collect();

        let result = router.calc_route(request);
        assert_eq!(
            result,
            Some(LeastCostPath {
                travel_time: Duration::from_secs(4),
                travel_disutility: 4.0 as Disutility,
                path: expected_path,
            })
        );
    }

    /// Test the FreeOrMaxSpeedTravelTimeAndDisutility implementation of TravelTime and TravelDisutility
    #[test]
    fn test_free_or_max_speed_travel_time_and_disutility() {
        let fomsttad = FreeOrMaxSpeedTravelTimeAndDisutility;

        let network = get_triangle_test_network();
        let graph = net_to_graph(&network);

        let link = graph.edge(Id::create("4")).unwrap();

        assert_eq!(
            fomsttad.travel_time(link, SimTime::from_secs(0), None, None),
            Duration::from_secs(4)
        );
        assert_eq!(
            fomsttad.travel_disutility(link, SimTime::from_secs(0), None, None),
            4.0
        );

        // also test that the vehicle's max speed is respected

        // vehicle max_v is lower than freespeed, so travel time will be longer
        let vehicle = InternalVehicle::new(0, 0, 1000.0, 0.0);

        assert_eq!(
            fomsttad.travel_time(link, SimTime::from_secs(0), None, Some(&vehicle)),
            Duration::from_secs(10)
        );
        assert_eq!(
            fomsttad.travel_disutility(link, SimTime::from_secs(0), None, Some(&vehicle)),
            10.0
        );
    }
}
