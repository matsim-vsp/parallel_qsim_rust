use crate::simulation::id::Id;
use crate::simulation::replanning::routing::graph::{GraphError, LinkIndex, NodeIndex};
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
    /// Returns an error if the link does not exist in the graph
    fn get_end_node(&self, link_id: Id<Link>) -> Result<Id<Node>, GraphError>;
    /// get the start node of a given link, given as link id
    /// Returns an error if the link does not exist in the graph
    fn get_start_node(&self, link_id: Id<Link>) -> Result<Id<Node>, GraphError>;
    // needed to allow cloning of Box<dyn Graph>
    // fn clone_box(&self) -> Box<dyn Graph>;
}
//
// impl Clone for Box<dyn Graph> {
//     fn clone(&self) -> Box<dyn Graph> {
//         self.clone_box()
//     }
// }

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
    fn get_end_node_as_idx(&self, edge: LinkIndex) -> Result<NodeIndex, GraphError>;
    fn get_start_node_as_idx(&self, edge: LinkIndex) -> Result<NodeIndex, GraphError>;
}

pub trait LeastCostPathCalculator {
    // todo QUESTION previously, we had &mut self here, but I don't see why. Do we need it?
    fn calc_route(&self, request: LeastCostPathRequest) -> Option<LeastCostPath>;
}

pub trait TravelTime: Debug {
    fn travel_time(
        &self,
        link: &Link,
        departure_time: Time,
        person: Option<&InternalPerson>,
        vehicle: Option<&InternalVehicle>,
    ) -> Time;
    fn clone_box(&self) -> Box<dyn TravelTime>;
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
#[derive(Builder, Clone)]
pub struct LeastCostPathRequest<'r> {
    pub from: Id<Link>,
    pub to: Id<Link>,
    pub graph: &'r dyn IntNodeGraph, // contains the graph of the network
    #[builder(default)]
    pub departure_time: Time,
    #[builder(default)]
    pub person: Option<&'r InternalPerson>,
    #[builder(default)]
    pub vehicle: Option<&'r InternalVehicle>,
}

// FIXME do we actually want the travel time, or the disutility, or both? Currently, it's the latter but named the former...
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
        // TODO do we want this? or should freespeed be truly freespeed, and we add another implementation of TravelTime that considers the vehicle's max speed?

        // respect the given vehicle type, if provided
        let max_speed = if let Some(v) = vehicle {
            v.max_v.min(link.freespeed)
        } else {
            link.freespeed
        };

        link.length / max_speed
    }
    fn clone_box(&self) -> Box<dyn TravelTime> {
        Box::new(self.clone())
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

    use crate::simulation::replanning::routing::alt_router::AStarRouter;
    use crate::simulation::replanning::routing::alt_router::ZeroHeuristic;
    use crate::simulation::replanning::routing::graph::tests::get_triangle_test_graph;
    use crate::simulation::replanning::routing::least_cost_path_caluclator::Graph;
    use crate::simulation::replanning::routing::least_cost_path_caluclator::{
        FreeSpeedTravelTimeAndDisutility, LeastCostPath, LeastCostPathRequestBuilder,
    };
    use crate::simulation::replanning::routing::least_cost_path_caluclator::{
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
            Box::new(FreeSpeedTravelTimeAndDisutility),
            Box::new(FreeSpeedTravelTimeAndDisutility),
        );
        // triangle graph
        let graph = get_triangle_test_graph();

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
                path: expected_path,
            })
        );
    }

    #[test]
    fn test_free_speed_travel_time_and_disutility() {
        let fpttad = FreeSpeedTravelTimeAndDisutility;

        let graph = get_triangle_test_graph();
        let link = graph.edge(Id::create("4"));

        assert_eq!(fpttad.travel_time(link, 0.0, None, None), 4.0);
        assert_eq!(fpttad.travel_disutility(link, 0.0, None, None), 4.0);

        // also test that the vehicle's max speed is respected

        // vehicle max_v is lower than freespeed, so travel time will be longer
        let vehicle = InternalVehicle::new(0, 0, 1000.0, 0.0);

        assert_eq!(fpttad.travel_time(link, 0.0, None, Some(&vehicle)), 10.0);
        assert_eq!(
            fpttad.travel_disutility(link, 0.0, None, Some(&vehicle)),
            10.0
        );
    }
}
