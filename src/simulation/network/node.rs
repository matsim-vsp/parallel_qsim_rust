use std::collections::BTreeMap;
use std::fmt::Debug;

use crate::simulation::io::vehicle_definitions::VehicleDefinitions;
use crate::simulation::messaging::events::proto::Event;
use crate::simulation::messaging::events::EventsPublisher;
use crate::simulation::messaging::messages::proto::Vehicle;
use crate::simulation::network::link::Link;
use crate::simulation::network::node::Node::{LocalNode, NeighbourNode};

pub trait NodeVehicle: Debug {
    fn id(&self) -> usize;
    fn advance_route_index(&mut self);
    fn curr_link_id(&self) -> Option<usize>;
    fn is_current_link_last(&self) -> bool;
    fn mode(&self) -> &str;
}

#[derive(Debug, Clone)]
pub enum Node {
    LocalNode(NodeImpl),
    NeighbourNode(NodeImpl),
}

impl Node {
    pub fn new_local_node(id: usize, x: f32, y: f32) -> Self {
        LocalNode(NodeImpl {
            id,
            in_links: Vec::new(),
            out_links: Vec::new(),
            x,
            y,
        })
    }

    pub fn new_neighbour_node(id: usize, x: f32, y: f32) -> Self {
        NeighbourNode(NodeImpl {
            id,
            in_links: Vec::new(),
            out_links: Vec::new(),
            x,
            y,
        })
    }

    pub fn add_in_link(&mut self, id: usize) {
        match self {
            LocalNode(n) => n.in_links.push(id),
            NeighbourNode(_) => {
                panic!("This function is not allowed for NeighbourNode.")
            }
        }
    }

    pub fn add_out_link(&mut self, id: usize) {
        match self {
            LocalNode(n) => {
                n.out_links.push(id);
            }
            NeighbourNode(_) => {
                panic!("This function is not allowed for NeighbourNode.")
            }
        }
    }

    pub fn move_vehicles(
        &self,
        links: &mut BTreeMap<usize, Link>,
        now: u32,
        events: &mut EventsPublisher,
        vehicle_definitions: Option<&VehicleDefinitions>,
    ) -> Vec<ExitReason> {
        match self {
            LocalNode(n) => n.move_vehicles(links, now, events, vehicle_definitions),
            NeighbourNode(_) => {
                panic!("This function is not allowed for NeighbourNode.")
            }
        }
    }

    pub fn x(&self) -> f32 {
        match self {
            LocalNode(n) => n,
            NeighbourNode(n) => n,
        }
        .x
    }

    pub fn y(&self) -> f32 {
        match self {
            LocalNode(n) => n,
            NeighbourNode(n) => n,
        }
        .y
    }
}

#[derive(Debug, Clone)]
pub struct NodeImpl {
    pub id: usize,
    pub in_links: Vec<usize>,
    pub out_links: Vec<usize>,
    pub x: f32,
    pub y: f32,
}

pub enum ExitReason {
    FinishRoute(Vehicle),
    ReachedBoundary(Vehicle),
}

impl NodeImpl {
    pub(crate) fn new(id: usize, x: f32, y: f32) -> NodeImpl {
        NodeImpl {
            id,
            in_links: Vec::new(),
            out_links: Vec::new(),
            x,
            y,
        }
    }

    pub fn add_in_link(&mut self, id: usize) {
        self.in_links.push(id);
    }

    pub fn add_out_link(&mut self, id: usize) {
        self.out_links.push(id);
    }

    pub fn move_vehicles(
        &self,
        links: &mut BTreeMap<usize, Link>,
        now: u32,
        events: &mut EventsPublisher,
        vehicle_definitions: Option<&VehicleDefinitions>,
    ) -> Vec<ExitReason> {
        let mut exited_vehicles = Vec::new();

        for in_link_index in &self.in_links {
            let vehicles: Vec<Vehicle> = match links.get_mut(in_link_index).unwrap() {
                Link::LocalLink(link) => link.pop_front(now),
                Link::SplitInLink(split_link) => split_link.local_link_mut().pop_front(now),
                Link::SplitOutLink(_) => panic!("No split out link expected as in link of a node."),
            };

            for mut vehicle in vehicles {
                if !vehicle.is_current_link_last() {
                    events.publish_event(
                        now,
                        &Event::new_link_leave(*in_link_index as u64, vehicle.id() as u64),
                    );
                }

                vehicle.advance_route_index();
                match vehicle.curr_link_id() {
                    None => exited_vehicles.push(ExitReason::FinishRoute(vehicle)),
                    Some(out_id) => {
                        self.move_vehicle(
                            links,
                            out_id,
                            vehicle,
                            &mut exited_vehicles,
                            now,
                            events,
                            vehicle_definitions,
                        );
                    }
                }
            }
        }
        exited_vehicles
    }

    fn move_vehicle(
        &self,
        links: &mut BTreeMap<usize, Link>,
        out_link_id: usize,
        vehicle: Vehicle,
        exited_vehicles: &mut Vec<ExitReason>,
        now: u32,
        events: &mut EventsPublisher,
        vehicle_definitions: Option<&VehicleDefinitions>,
    ) {
        match links.get_mut(&out_link_id).unwrap() {
            Link::LocalLink(local_link) => {
                events.publish_event(
                    now,
                    &Event::new_link_enter(local_link.id() as u64, vehicle.id() as u64),
                );
                local_link.push_vehicle(vehicle, now, vehicle_definitions);
            }
            Link::SplitOutLink(_) => exited_vehicles.push(ExitReason::ReachedBoundary(vehicle)),
            Link::SplitInLink(_) => {
                panic!("Not expecting to move a vehicle onto a split in link.")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use crate::simulation::messaging::events::EventsPublisher;
    use crate::simulation::messaging::messages::proto::leg::Route;
    use crate::simulation::messaging::messages::proto::{
        Activity, Agent, Leg, NetworkRoute, Plan, Vehicle, VehicleType,
    };
    use crate::simulation::network::link::{Link, LocalLink, SplitOutLink};
    use crate::simulation::network::node::{ExitReason, NodeImpl};

    #[test]
    fn init() {
        let node = NodeImpl::new(1, 0., 0.);

        assert_eq!(1, node.id);
        assert!(node.in_links.is_empty());
        assert!(node.out_links.is_empty());
    }

    #[test]
    fn vehicle_in() {
        let mut node = NodeImpl::new(1, 0., 0.);
        let mut local_in_link = LocalLink::new(1, 20., 40., 20., vec![], 1., 0, 0);
        let agent = create_agent(1, vec![1]);
        let vehicle = Vehicle::new(1, VehicleType::Network, String::from("car"), agent);
        //let vehicle = Vehicle::new(1, 1, vec![1], String::from("car"));
        local_in_link.push_vehicle(vehicle, 1, None);
        node.add_in_link(local_in_link.id());
        let in_link = Link::LocalLink(local_in_link);
        let mut links: BTreeMap<usize, Link> = BTreeMap::from([(1, in_link)]);
        let mut events = EventsPublisher::new();

        let exited_vehicles = node.move_vehicles(&mut links, 1, &mut events, None);

        assert_eq!(1, exited_vehicles.len());
        let entry = exited_vehicles.get(0).unwrap();
        assert!(matches!(entry, ExitReason::FinishRoute(_)));
    }

    #[test]
    fn vehicle_in_and_out() {
        let mut node = NodeImpl::new(1, 0., 0.);
        let mut local_in_link = LocalLink::new(1, 20., 40., 20., vec![], 1., 0, 0);
        let local_out_link = LocalLink::new(2, 20., 40., 20., vec![], 1., 0, 0);
        let agent = create_agent(1, vec![1, 2]);
        let vehicle = Vehicle::new(1, VehicleType::Network, String::from("car"), agent);
        local_in_link.push_vehicle(vehicle, 1, None);
        node.add_in_link(local_in_link.id());
        node.add_out_link(local_out_link.id());
        let in_link = Link::LocalLink(local_in_link);
        let out_link = Link::LocalLink(local_out_link);
        let mut links: BTreeMap<usize, Link> = BTreeMap::from([(1, in_link), (2, out_link)]);
        let mut events = EventsPublisher::new();

        let exited_vehicles = node.move_vehicles(&mut links, 1, &mut events, None);

        assert_eq!(0, exited_vehicles.len());
        let out_link_ref = links.get_mut(&2).unwrap();
        if let Link::LocalLink(local_out) = out_link_ref {
            let vehicles = local_out.pop_front(1);
            assert_eq!(1, vehicles.len());
        }
    }

    #[test]
    pub fn vehicle_in_out_boundary() {
        let mut node = NodeImpl::new(1, 0., 0.);
        let mut local_in_link = LocalLink::new(1, 20., 40., 20., vec![], 1., 0, 0);
        let split_out_link = SplitOutLink::new(2, 2);
        let agent = create_agent(1, vec![1, 2]);
        let vehicle = Vehicle::new(1, VehicleType::Network, String::from("car"), agent);
        local_in_link.push_vehicle(vehicle, 1, None);
        node.add_in_link(local_in_link.id());
        node.add_out_link(split_out_link.id());
        let in_link = Link::LocalLink(local_in_link);
        let out_link = Link::SplitOutLink(split_out_link);
        let mut links: BTreeMap<usize, Link> = BTreeMap::from([(1, in_link), (2, out_link)]);
        let mut events = EventsPublisher::new();

        let exited_vehicles = node.move_vehicles(&mut links, 1, &mut events, None);

        assert_eq!(1, exited_vehicles.len());
        assert!(matches!(
            exited_vehicles.get(0).unwrap(),
            ExitReason::ReachedBoundary(_)
        ));
    }

    #[test]
    fn vehicles_in() {
        let mut node = NodeImpl::new(1, 0., 0.);
        let mut local_in_link = LocalLink::new(1, 3600., 40., 20., vec![], 1., 0, 0);

        let agent_1 = create_agent(1, vec![1]);
        let vehicle_1 = Vehicle::new(1, VehicleType::Network, String::from("car"), agent_1);
        let agent_2 = create_agent(2, vec![1]);
        let vehicle_2 = Vehicle::new(2, VehicleType::Network, String::from("car"), agent_2);

        local_in_link.push_vehicle(vehicle_1, 1, None);
        local_in_link.push_vehicle(vehicle_2, 1, None);
        node.add_in_link(local_in_link.id());
        let in_link = Link::LocalLink(local_in_link);
        let mut links: BTreeMap<usize, Link> = BTreeMap::from([(1, in_link)]);
        let mut events = EventsPublisher::new();

        let exited_vehicles = node.move_vehicles(&mut links, 1, &mut events, None);

        assert_eq!(1, exited_vehicles.len());
        let entry = exited_vehicles.get(0).unwrap();
        assert!(matches!(entry, ExitReason::FinishRoute(_)));
        if let ExitReason::FinishRoute(vehicle) = entry {
            assert_eq!(1, vehicle.id);
        }

        let exited_vehicles = node.move_vehicles(&mut links, 2, &mut events, None);

        assert_eq!(1, exited_vehicles.len());
        let entry = exited_vehicles.get(0).unwrap();
        assert!(matches!(entry, ExitReason::FinishRoute(_)));
        if let ExitReason::FinishRoute(vehicle) = entry {
            assert_eq!(2, vehicle.id);
        }
    }

    #[test]
    fn vehicles_in_and_out() {
        let mut node = NodeImpl::new(1, 0., 0.);
        let mut local_in_link = LocalLink::new(1, 10000., 40., 20., vec![], 1., 0, 0);
        let local_out_link = LocalLink::new(2, 10000., 40., 20., vec![], 1., 0, 0);

        let agent_1 = create_agent(1, vec![1, 2]);
        let vehicle_1 = Vehicle::new(1, VehicleType::Network, String::from("car"), agent_1);
        let agent_2 = create_agent(2, vec![1, 2]);
        let vehicle_2 = Vehicle::new(2, VehicleType::Network, String::from("car"), agent_2);

        local_in_link.push_vehicle(vehicle_1, 1, None);
        local_in_link.push_vehicle(vehicle_2, 1, None);
        node.add_in_link(local_in_link.id());
        node.add_out_link(local_out_link.id());
        let in_link = Link::LocalLink(local_in_link);
        let out_link = Link::LocalLink(local_out_link);
        let mut links: BTreeMap<usize, Link> = BTreeMap::from([(1, in_link), (2, out_link)]);
        let mut events = EventsPublisher::new();

        let exited_vehicles = node.move_vehicles(&mut links, 1, &mut events, None);
        assert_eq!(0, exited_vehicles.len());

        let out_link_ref = links.get_mut(&2).unwrap();
        if let Link::LocalLink(local_out) = out_link_ref {
            let vehicles = local_out.pop_front(1);
            assert_eq!(2, vehicles.len());
        }
    }

    fn create_agent(id: u64, route: Vec<u64>) -> Agent {
        let route = Route::NetworkRoute(NetworkRoute::new(id, route));
        let leg = Leg::new(route, "car", None, None);
        let act = Activity::new(0., 0., String::from("some-type"), 1, None, None, None);
        let mut plan = Plan::new();
        plan.add_act(act);
        plan.add_leg(leg);
        let mut agent = Agent::new(id, plan);
        agent.advance_plan();

        agent
    }
}
