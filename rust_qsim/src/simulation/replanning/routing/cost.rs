use crate::simulation::config::Config;
use crate::simulation::id::Id;
use crate::simulation::replanning::routing::travel_time_calculator::GlobalTravelTimeCalculator;
use crate::simulation::scenario::network::Link;
use crate::simulation::scenario::population::InternalPerson;
use crate::simulation::scenario::vehicles::{Garage, InternalVehicle};
use crate::simulation::time::SimTime;
use nohash_hasher::IntMap;
use std::fmt::Debug;
use std::num::FpCategory;
use std::sync::Arc;
use std::time::Duration;

/// Disutility is the unit of the cost values used in routing
pub type Disutility = f64;

/// Travel time function, mapping any network link to a travel time, depending on the departure time
/// and optionally the person and vehicle.
pub trait TravelTime: Debug + Send + Sync {
    /// get travel time of a given link at a given time, optionally for a specific person and vehicle
    fn travel_time(
        &self,
        link: &Link,
        departure_time: SimTime,
        person: Option<&InternalPerson>,
        vehicle: Option<&InternalVehicle>,
    ) -> Duration;
}

/// Travel disutility function, mapping any network link to a travel disutility, depending on the
/// departure time and optionally the person and vehicle.
/// Also provides a method to get a global lower bound on the travel disutility for a given link,
/// over all times, persons and vehicles. This is used to calculate landmark data.
///
/// # Contract
/// - Returned disutilities are used as edge weights in Dijkstra/A* and therefore must be
///   non-negative. NaN is treated as worse than infinity.
/// - `get_link_min_travel_disutility(link)` must be a global lower bound on `travel_disutility`
///   for the given link across all times/persons/vehicles (so ALT remains admissible).
pub trait TravelDisutility: Debug + Send + Sync {
    /// get travel disutility of the given link at the given time, optionally for a specific person
    /// and vehicle
    fn travel_disutility(
        &self,
        link: &Link,
        departure_time: SimTime,
        person: Option<&InternalPerson>,
        vehicle: Option<&InternalVehicle>,
    ) -> Disutility;

    /// Returns the smallest possible travel disutility at the given link, over all times, persons
    /// and vehicles.
    /// This is used when calculating landmark data, to ensure that the ALT heuristic never
    /// overestimates the travel disutility between two nodes.
    fn get_link_min_travel_disutility(&self, link: &Link) -> Disutility;
}

#[derive(Clone, Debug)]
pub struct ScoringBasedTravelTimeAndDisutility {
    marginal_utility_performing_per_subpopulation: IntMap<Id<String>, f64>,
    marginal_utility_traveling_per_mode: IntMap<Id<String>, f64>,
    marginal_utility_distance_per_mode: IntMap<Id<String>, f64>,
    travel_time: GlobalTravelTimeCalculator,
    garage: Arc<Garage>,
}

impl ScoringBasedTravelTimeAndDisutility {
    pub fn new(
        config: &Config,
        garage: Arc<Garage>,
        travel_time: GlobalTravelTimeCalculator,
    ) -> Self {
        let marginal_utility_performing_per_subpopulation = config
            .scoring()
            .agent_params
            .iter()
            .map(|p| (Id::create(&p.subpopulation), p.performing))
            .collect();

        let marginal_utility_traveling_per_mode = config
            .scoring()
            .mode_params
            .iter()
            .map(|p| (Id::create(&p.mode), p.marginal_utility_of_traveling))
            .collect();

        let marginal_utility_distance_per_mode = config
            .scoring()
            .mode_params
            .iter()
            .map(|p| (Id::create(&p.mode), p.marginal_utility_of_distance))
            .collect();

        ScoringBasedTravelTimeAndDisutility {
            marginal_utility_performing_per_subpopulation,
            marginal_utility_traveling_per_mode,
            marginal_utility_distance_per_mode,
            garage,
            travel_time,
        }
    }
}

impl TravelTime for ScoringBasedTravelTimeAndDisutility {
    fn travel_time(
        &self,
        link: &Link,
        departure_time: SimTime,
        person: Option<&InternalPerson>,
        vehicle: Option<&InternalVehicle>,
    ) -> Duration {
        self.travel_time
            .travel_time(link, departure_time, person, vehicle)
    }
}

impl TravelDisutility for ScoringBasedTravelTimeAndDisutility {
    fn travel_disutility(
        &self,
        link: &Link,
        departure_time: SimTime,
        person: Option<&InternalPerson>,
        vehicle: Option<&InternalVehicle>,
    ) -> Disutility {
        let max_performing_factor = self
            .marginal_utility_performing_per_subpopulation
            .values()
            .map(|x| *x)
            .reduce(f64::max)
            .unwrap();

        let min_traveling_factor = self
            .marginal_utility_traveling_per_mode
            .values()
            .map(|x| *x)
            .reduce(f64::min)
            .unwrap();

        let min_distance_factor = self
            .marginal_utility_distance_per_mode
            .values()
            .map(|x| *x)
            .reduce(f64::min)
            .unwrap();

        let performing_factor = if let Some(person) = person {
            self.marginal_utility_performing_per_subpopulation
                .get(&person.subpopulation())
                .unwrap_or(&max_performing_factor)
        } else {
            &max_performing_factor
        };

        let mut traveling_factor = &min_traveling_factor;
        let mut distance = &min_distance_factor;

        if let Some(vehicle) = vehicle {
            let mode = &self
                .garage
                .vehicle_types
                .get(&vehicle.vehicle_type)
                .as_ref()
                .unwrap()
                .net_mode;
            traveling_factor = self
                .marginal_utility_traveling_per_mode
                .get(mode)
                .unwrap_or(&min_traveling_factor);
            distance = self
                .marginal_utility_distance_per_mode
                .get(mode)
                .unwrap_or(&min_distance_factor);
        };

        // traveling is normally negative, so convert it into positive disutility by negating it
        let travel_cost_factor = (-traveling_factor + performing_factor) / 3600.;

        let distance_term = if distance == &0. {
            0.
        } else {
            // distance is normally negative, so convert it into positive disutility by negating it
            -distance * link.length
        };

        self.travel_time(link, departure_time, person, vehicle)
            .as_secs_f64()
            * travel_cost_factor
            + distance_term
    }

    fn get_link_min_travel_disutility(&self, link: &Link) -> Disutility {
        self.travel_disutility(link, SimTime::from_secs(0), None, None)
    }
}

/// An implementation of both `TravelTime` and `TravelDisutility`, purely based on freespeed travel
/// times. The travel time is simply the link length divided by the freespeed, ignoring any given
/// vehicle type and its max speed.
///
/// The travel disutility is equal to the travel time.
#[derive(Clone, Debug)]
pub struct FreeSpeedTravelTimeAndDisutility {}

impl TravelTime for FreeSpeedTravelTimeAndDisutility {
    fn travel_time(
        &self,
        link: &Link,
        _departure_time: SimTime,
        _person: Option<&InternalPerson>,
        _vehicle: Option<&InternalVehicle>,
    ) -> Duration {
        // the given vehicle type is ignored => true freespeed
        travel_time(link.length, link.freespeed)
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
        self.travel_time(link, departure_time, person, vehicle)
            .as_secs_f64()
            * 1.0
    }

    // min travel disutility is equal to the travel disutility, since it does not depend on time, person or vehicle
    fn get_link_min_travel_disutility(&self, link: &Link) -> Disutility {
        self.travel_disutility(link, SimTime::from_secs(0), None, None)
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

        travel_time(link.length, max_speed)
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
        self.travel_time(link, departure_time, person, vehicle)
            .as_secs_f64()
            * 1.0
    }
    fn get_link_min_travel_disutility(&self, link: &Link) -> Disutility {
        self.travel_disutility(link, SimTime::from_secs(0), None, None)
    }
}

fn travel_time(length: f64, speed: f64) -> Duration {
    if length < 10e-10 {
        return Duration::ZERO;
    }
    let duration = length / speed;
    match duration.classify() {
        FpCategory::Nan | FpCategory::Infinite => Duration::MAX,
        FpCategory::Normal | FpCategory::Subnormal | FpCategory::Zero => {
            Duration::from_secs_f64(duration)
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::simulation::id::Id;
    use crate::simulation::replanning::routing::cost::FreeOrMaxSpeedTravelTimeAndDisutility;
    use crate::simulation::replanning::routing::cost::TravelDisutility;
    use crate::simulation::replanning::routing::cost::TravelTime;
    use crate::simulation::replanning::routing::graph::Graph;
    use crate::simulation::replanning::routing::graph::tests::{
        get_triangle_test_network, net_to_graph,
    };
    use crate::simulation::scenario::vehicles::InternalVehicle;
    use crate::simulation::time::SimTime;
    use macros::deterministic_id_test;
    use std::time::Duration;

    /// Test the FreeOrMaxSpeedTravelTimeAndDisutility implementation of TravelTime and TravelDisutility
    #[deterministic_id_test]
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
