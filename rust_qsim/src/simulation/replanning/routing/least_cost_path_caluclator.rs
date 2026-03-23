use crate::generated::population::Person;
use crate::simulation::id::Id;
use crate::simulation::scenario::network::{Link, Node};
use crate::simulation::scenario::vehicles::InternalVehicle;

// "normal" time representation is u32 for now, but we might want to use f64 for the future
pub type Time = f64;
pub type Utility = f64;

#[deprecated]
pub struct CustomQueryResult {
    pub travel_time: Option<u32>,
    pub path: Option<Vec<u64>>,
}

pub trait Graph {
    fn node(&self, id: Id<Node>) -> &Node;
    fn edge(&self, id: Id<Link>) -> &Link;
    fn outgoing_edges(&self, node: Id<Node>) -> &[Id<Link>];
    fn incoming_edges(&self, node: Id<Node>) -> &[Id<Link>];
}

pub trait LeastCostPathCalculator {
    fn calc_route(&mut self, request: LeastCostPathRequest) -> Option<LeastCostPath>;
}

pub trait TravelTime {
    fn travel_time(
        &self,
        link: &Link,
        departure_time: Time,
        person: &Person,
        vehicle: &InternalVehicle,
    ) -> Time;
}

pub trait TravelDisutility {
    fn travel_disutility(
        &self,
        link: &Link,
        departure_time: Time,
        person: &Person,
        vehicle: &InternalVehicle,
    ) -> Time;
}

// From and to are deliberately not nodes but links. This allows considering those links as well during routing.
pub struct LeastCostPathRequest<'r> {
    pub from: Id<Link>,
    pub to: Id<Link>,
    pub departure_time: Time,
    pub person: &'r Person,
    pub vehicle: &'r InternalVehicle,
}

pub struct LeastCostPath {
    pub path: Vec<Id<Link>>,
    pub travel_time: f64,
}

pub struct FreeSpeedTravelTimeAndDisutility;

impl TravelTime for FreeSpeedTravelTimeAndDisutility {
    fn travel_time(
        &self,
        link: &Link,
        _departure_time: Time,
        _person: &Person,
        _vehicle: &InternalVehicle,
    ) -> Time {
        link.length / link.freespeed
    }
}

impl TravelDisutility for FreeSpeedTravelTimeAndDisutility {
    fn travel_disutility(
        &self,
        link: &Link,
        departure_time: Time,
        person: &Person,
        vehicle: &InternalVehicle,
    ) -> Utility {
        // TODO: Adapt the factor for the Disutility
        self.travel_time(link, departure_time, person, vehicle) * -1.0
    }
}
