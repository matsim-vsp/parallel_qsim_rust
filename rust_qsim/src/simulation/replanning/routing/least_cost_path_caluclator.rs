use crate::simulation::id::Id;
use crate::simulation::replanning::routing::graph::{LinkIndex, NodeIndex};
use crate::simulation::scenario::network::{Link, Node};
use crate::simulation::scenario::population::InternalPerson;
use crate::simulation::scenario::vehicles::InternalVehicle;
use derive_builder::Builder;
use std::fmt::Debug;

// "normal" time representation is u32 for now, but we might want to use f64 for the future
pub type Time = f64;
pub type Utility = f64;

#[deprecated]
pub struct CustomQueryResult {
    pub travel_time: Option<u32>,
    pub path: Option<Vec<u64>>,
}

/// A (directed) graph whose nodes are Ids of network nodes, and edges are Ids of network links.
/// The graph is used for routing, and the nodes and links can be accessed by their id.
pub trait Graph: Debug {
    /// get network node from node id
    fn node(&self, id: Id<Node>) -> &Node;
    /// get network link from link id
    fn edge(&self, id: Id<Link>) -> &Link;
    /// get slice of the outgoing edges, as link ids, of a given node, given as node id
    fn outgoing_edges(&self, node: Id<Node>) -> &[Id<Link>];
    /// get slice of the incoming edges, as link ids, of a given node, given as node id
    fn incoming_edges(&self, node: Id<Node>) -> &[Id<Link>];
    fn num_nodes(&self) -> usize;
    /// get the end node (head) of a given link, given as link id
    fn get_end_node(&self, link_id: Id<Link>) -> Id<Node>;
    /// get the start node of a given link, given as link id
    fn get_start_node(&self, link_id: Id<Link>) -> Id<Node>;
    // needed to allow cloning of Box<dyn Graph>
    fn clone_box(&self) -> Box<dyn Graph>;
}

impl Clone for Box<dyn Graph> {
    fn clone(&self) -> Box<dyn Graph> {
        self.clone_box()
    }
}

/// A (directed) graph where nodes and links can be accessed by both their id and their index.
/// Mirrors many trait methods from its supertrait "Graph", but using indices instead of ids.
/// This is used to keep the routing algorithms efficient, while still being able to use the ids in
/// the rest of the code.
pub trait IntNodeGraph: Graph {
    fn get_node_idx_from_id(&self, id: Id<Node>) -> NodeIndex;
    fn get_link_idx_from_id(&self, id: Id<Link>) -> LinkIndex;
    fn get_node_id_from_idx(&self, idx: NodeIndex) -> Id<Node>;
    fn get_link_id_from_idx(&self, idx: LinkIndex) -> Id<Link>;
    fn get_node_from_idx(&self, idx: NodeIndex) -> &Node;
    fn get_link_from_idx(&self, idx: LinkIndex) -> &Link;
    fn outgoing_edges_as_idx(&self, node: NodeIndex) -> Vec<LinkIndex>;
    fn incoming_edges_as_idx(&self, node: NodeIndex) -> Vec<LinkIndex>;
    fn get_end_node_as_idx(&self, edge: LinkIndex) -> NodeIndex;
}

pub trait LeastCostPathCalculator {
    // todo QUESTION previously, we had &mut self here, but I don't see why. Do we need it?
    fn calc_route(&self, request: LeastCostPathRequest) -> Option<LeastCostPath>;
}

pub trait TravelTime: Clone + Debug {
    fn travel_time(
        &self,
        link: &Link,
        departure_time: Time,
        person: Option<&InternalPerson>,
        vehicle: Option<&InternalVehicle>,
    ) -> Time;
}

pub trait TravelDisutility: Debug {
    fn travel_disutility(
        &self,
        link: &Link,
        departure_time: Time,
        person: Option<&InternalPerson>,
        vehicle: Option<&InternalVehicle>,
    ) -> Utility;
    fn clone_box(&self) -> Box<dyn TravelDisutility>;
}
// From and to are deliberately not nodes but links. This allows considering those links as well during routing.
#[derive(Builder)]
pub struct LeastCostPathRequest<'r> {
    pub from: Id<Link>,
    pub to: Id<Link>,
    pub graph: &'r Box<dyn IntNodeGraph>, // contains the graph of the network
    pub departure_time: Time,
    pub person: Option<&'r InternalPerson>,
    pub vehicle: Option<&'r InternalVehicle>,
}

#[derive(PartialEq, Debug)]
pub struct LeastCostPath {
    pub path: Vec<Id<Link>>,
    pub travel_time: f64,
}

#[derive(Clone, Debug)]
pub struct FreeSpeedTravelTimeAndDisutility;

impl TravelTime for FreeSpeedTravelTimeAndDisutility {
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
    fn clone_box(&self) -> Box<dyn TravelDisutility> {
        Box::new(self.clone())
    }
}

#[cfg(test)]
mod tests {
    use crate::simulation::id::Id;
    use crate::simulation::replanning::routing::alt_landmark_data::AltLandmarkData;
    use crate::simulation::replanning::routing::alt_router::ZeroHeuristic;
    use crate::simulation::replanning::routing::alt_router::{AStarRouter, AltHeuristic};
    use crate::simulation::replanning::routing::graph::tests::get_triangle_test_graph;
    use crate::simulation::replanning::routing::least_cost_path_caluclator::{
        FreeSpeedTravelTimeAndDisutility, IntNodeGraph, LeastCostPathRequestBuilder,
    };
    use crate::simulation::replanning::routing::least_cost_path_caluclator::{
        LeastCostPathCalculator, LeastCostPathRequest,
    };

    #[test]
    fn test() {
        let router = AStarRouter::new(ZeroHeuristic, Box::new(FreeSpeedTravelTimeAndDisutility));
        let graph_boxed: Box<dyn IntNodeGraph> = Box::new(get_triangle_test_graph());
        let request = LeastCostPathRequestBuilder::default()
            .from(Id::create("from"))
            .to(Id::create("to"))
            .graph(&graph_boxed)
            .build()
            .unwrap();

        let option = router.calc_route(request);
        matches!(option, None);
    }

    // todo need test for AltHeuristic as well. So far, that would require a manual construction
    // of that request as well (even though it uses a builder, but that hardly makes it less large)
    // also, the two requests are clearly related, so I need to think about how to handle that.

    // TODO maybe this test is supposed to live in AltRouter? that's where the tests have been
    // so far.
    #[test]
    fn test_alt_heuristic() {
        let graph_boxed: Box<dyn IntNodeGraph> = Box::new(get_triangle_test_graph());

        let landmark_data = AltLandmarkData::new(&graph_boxed);

        let heuristic = AltHeuristic::new(landmark_data);

        let lcp_request = LeastCostPathRequestBuilder::default()
            .from(Id::create("from"))
            .to(Id::create("to"))
            .graph(&graph_boxed)
            .build()
            .unwrap();

        let router = AStarRouter::new(heuristic, Box::new(FreeSpeedTravelTimeAndDisutility));
        // FIXME complete or remove test
        // assert_eq!(router.calc_route(lcp_request),);
    }
}
