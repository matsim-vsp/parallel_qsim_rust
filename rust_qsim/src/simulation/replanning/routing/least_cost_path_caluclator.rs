use crate::generated::population::Person;
use crate::simulation::id::Id;
use crate::simulation::replanning::routing::dijsktra::{Dijkstra, Distance};
use crate::simulation::replanning::routing::graph;
use crate::simulation::replanning::routing::graph::{ForwardBackwardGraph, LinkIndex, NodeIndex};
use crate::simulation::scenario::network::{Link, Network, Node};
use crate::simulation::scenario::population::InternalPerson;
use crate::simulation::scenario::vehicles::InternalVehicle;
use keyed_priority_queue::{Entry, KeyedPriorityQueue};

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
    fn num_nodes(&self) -> usize;
    fn get_end_node(&self, link_id: Id<Link>) -> Id<Node>;
    fn get_start_node(&self, link_id: Id<Link>) -> Id<Node>;
}

pub trait IntNodeGraph: Graph {
    fn get_node_idx_from_id(&self, id: Id<Node>) -> NodeIndex;
    fn get_link_idx_from_id(&self, id: Id<Link>) -> LinkIndex;
    fn get_node_id_from_idx(&self, idx: NodeIndex) -> Id<Node>;
    fn get_link_id_from_idx(&self, idx: LinkIndex) -> Id<Link>;
    fn get_node_from_idx(&self, idx: NodeIndex) -> &Node;
    fn get_link_from_idx(&self, idx: LinkIndex) -> &Link;
    fn outgoing_edges_as_idx(&self, node: NodeIndex) -> Vec<LinkIndex>;
    fn get_end_node_as_idx(&self, edge: LinkIndex) -> NodeIndex;
}

pub trait LeastCostPathCalculator {
    fn calc_route<G: IntNodeGraph>(
        &mut self,
        request: LeastCostPathRequest<G>,
    ) -> Option<LeastCostPath>;
}

pub trait TravelTime {
    fn travel_time(
        &self,
        link: &Link,
        departure_time: Time,
        person: Option<&InternalPerson>,
        vehicle: Option<&InternalVehicle>,
    ) -> Time;
}

pub trait TravelDisutility {
    fn travel_disutility(
        &self,
        link: &Link,
        departure_time: Time,
        person: Option<&InternalPerson>,
        vehicle: Option<&InternalVehicle>,
    ) -> Utility;
}

// From and to are deliberately not nodes but links. This allows considering those links as well during routing.
pub struct LeastCostPathRequest<'r, G: IntNodeGraph + ?Sized> {
    pub from: Id<Link>,
    pub to: Id<Link>,
    pub graph: &'r G, // contains the graph of the network
    pub departure_time: Time,
    pub person: Option<&'r InternalPerson>,
    pub vehicle: Option<&'r InternalVehicle>,
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
        _person: Option<&InternalPerson>,
        _vehicle: Option<&InternalVehicle>,
    ) -> Time {
        link.length / link.freespeed
    }
}

// TODO always use travel disutility
impl TravelDisutility for FreeSpeedTravelTimeAndDisutility {
    fn travel_disutility(
        &self,
        link: &Link,
        departure_time: Time,
        person: Option<&InternalPerson>,
        vehicle: Option<&InternalVehicle>,
    ) -> Utility {
        // TODO: Adapt the factor for the Disutility
        self.travel_time(link, departure_time, person, vehicle) * 1.0
        // travel DISutility is simply the travel time here, since higher time corresponds to lower utility
    }
}

#[cfg(test)]
mod tests {
    use crate::simulation::id::Id;
    use crate::simulation::replanning::routing::alt_router::AStarRouter;
    use crate::simulation::replanning::routing::alt_router::ZeroHeuristic;
    use crate::simulation::replanning::routing::least_cost_path_caluclator::FreeSpeedTravelTimeAndDisutility;
    use crate::simulation::replanning::routing::least_cost_path_caluclator::{
        LeastCostPathCalculator, LeastCostPathRequest,
    };

    #[test]
    fn test() {
        let mut router =
            AStarRouter::new(ZeroHeuristic, Box::new(FreeSpeedTravelTimeAndDisutility));
        let request = LeastCostPathRequest {
            from: Id::create("from"),
            to: Id::create("to"),
            departure_time: 0.0,
            person: None,
            vehicle: None,
        };
        let option = router.calc_route(request);
        matches!(option, None);
    }
}
