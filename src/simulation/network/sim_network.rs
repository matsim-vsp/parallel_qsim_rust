use std::collections::{HashMap, HashSet};

use crate::simulation::{
    id::Id,
    io::vehicle_definitions::VehicleDefinitions,
    messaging::{
        events::{proto::Event, EventsPublisher},
        messages::proto::Vehicle,
    },
};

use super::{
    global_network::{Link, Network, Node},
    link::{LocalLink, SimLink, SplitInLink, SplitOutLink},
    node::ExitReason,
};

#[derive(Debug)]
pub struct SimNetworkPartition<'n> {
    nodes: Vec<Id<Node>>,
    links: HashMap<Id<Link>, SimLink>,
    global_network: &'n Network<'n>,
}

impl<'n> SimNetworkPartition<'n> {
    pub fn from_network(global_network: &'n Network, partition: usize) -> Self {
        let nodes: HashSet<_> = global_network
            .nodes
            .iter()
            .filter(|node| node.partition == partition)
            .map(|node| node.id.clone())
            .collect();

        let links : HashMap<_, _> = global_network.links.iter()
            .map(|link| {
                (link.id.clone(), Self::create_sim_link(link, partition, &global_network.nodes))
            })
            .collect();

        let link_ids: HashSet<_> = nodes
            .iter()
            .map(|id| global_network.nodes.get(id.internal).unwrap())
            .filter(|node| node.partition == partition)
            .flat_map(|node| node.in_links.iter().chain(node.out_links.iter()))
            .collect(); // collect here to get each link id only once

        let links : HashMap<_, _> = link_ids.iter()
            .map(|link_id| global_network.links.get(link_id.internal).unwrap())
            .map(|link| {
                (
                    link.id.clone(),
                    Self::create_sim_link(link, partition, &global_network.nodes),
                )
            })
            .collect();

        Self::new(Vec::from_iter(nodes), links, global_network)
    }

    fn create_sim_link(link: &Link, partition: usize, all_nodes: &Vec<Node>) -> SimLink {
        let from_part = all_nodes.get(link.from.internal).unwrap().partition;
        let to_part = all_nodes.get(link.to.internal).unwrap().partition;
        let id = &link.id;
        let from_id = &link.from;
        let to_id = &link.to;

        let from_node = all_nodes.get(link.from.internal).unwrap();
        let to_node = all_nodes.get(link.to.internal).unwrap();

        return if from_part == to_part {
            SimLink::LocalLink(LocalLink::from_link(link, 1.0))
        } else {
            if to_part == partition {
                let local_link = LocalLink::from_link(&link, 1.0);
                SimLink::SplitInLink(SplitInLink::new(from_part, local_link))
            } else {
                SimLink::SplitOutLink(SplitOutLink::new(link.id.internal, to_part))
            }
        };
    }

    pub fn new(
        nodes: Vec<Id<Node>>,
        links: HashMap<Id<Link>, SimLink>,
        global_network: &'n Network,
    ) -> Self {
        SimNetworkPartition {
            nodes,
            links,
            global_network,
        }
    }

    pub fn move_nodes(
        &mut self,
        events: &mut EventsPublisher,
        veh_def: Option<&VehicleDefinitions>,
        now: u32,
    ) -> Vec<ExitReason> {
        let mut exited_vehicles = Vec::new();

        for node_id in &self.nodes {
            Self::move_node(
                node_id,
                &self.global_network,
                &mut self.links,
                &mut exited_vehicles,
                events,
                veh_def,
                now,
            );
        }

        exited_vehicles
    }

    fn move_node(
        node_id: &Id<Node>,
        global_network: &Network,
        links: &mut HashMap<Id<Link>, SimLink>,
        exited_vehicles: &mut Vec<ExitReason>,
        events: &mut EventsPublisher,
        veh_def: Option<&VehicleDefinitions>,
        now: u32,
    ) {
        let node = global_network.get_node(node_id);
        for link_id in &node.in_links {
            let vehicles = match links.get_mut(link_id).unwrap() {
                SimLink::LocalLink(l) => l.pop_front(now),
                SimLink::SplitInLink(sl) => sl.local_link_mut().pop_front(now),
                SimLink::SplitOutLink(_) => panic!("No out link expected as in link of a node."),
            };
            for mut vehicle in vehicles {
                if vehicle.is_current_link_last() {
                    vehicle.advance_route_index();
                    exited_vehicles.push(ExitReason::FinishRoute(vehicle));
                } else {
                    if let Some(exit_reason) =
                        Self::move_vehicle(vehicle, veh_def, global_network, links, events, now)
                    {
                        exited_vehicles.push(exit_reason);
                    }
                }
            }
        }
    }

    fn move_vehicle(
        mut vehicle: Vehicle,
        veh_def: Option<&VehicleDefinitions>,
        global_network: &Network,
        links: &mut HashMap<Id<Link>, SimLink>,
        events: &mut EventsPublisher,
        now: u32,
    ) -> Option<ExitReason> {
        events.publish_event(
            now,
            &Event::new_link_leave(vehicle.curr_route_elem as u64, vehicle.id),
        );
        vehicle.advance_route_index();
        let link_id = global_network
            .link_ids
            .get(vehicle.curr_route_elem as usize);
        match links.get_mut(&link_id).unwrap() {
            SimLink::LocalLink(l) => {
                events.publish_event(now, &Event::new_link_enter(l.id() as u64, vehicle.id));
                l.push_vehicle(vehicle, now, veh_def);
                None
            }
            SimLink::SplitOutLink(_) => Some(ExitReason::ReachedBoundary(vehicle)),
            SimLink::SplitInLink(_) => {
                panic!("Not expecting to move a vehicle onto a split in link.")
            }
        }
    }
}

#[cfg(test)]
mod tests {

    use crate::simulation::{
        messaging::{
            events::EventsPublisher,
            messages::proto::{
                leg::Route, Activity, Agent, Leg, NetworkRoute, Plan, Vehicle, VehicleType,
            },
        },
        network::{
            global_network::{Link, Network, Node},
            link::SimLink,
            node::ExitReason,
        },
    };

    use super::SimNetworkPartition;

    #[test]
    fn from_network() {
        let mut network = Network::new();
        let mut sim_nets = create_single_node_sim_network_with_partition(&mut network);
        let net1 = sim_nets.get_mut(0).unwrap();

        // we expect two nodes
        assert_eq!(2, net1.nodes.len());
        // we expect two links one local and one out link
        assert_eq!(2, net1.links.len());
        let local_link = net1.links.get(&net1.global_network.link_ids.get(0)).unwrap();
        assert!(matches!(local_link, SimLink::LocalLink(_)));
        let out_link = net1.links.get(&net1.global_network.link_ids.get(1)).unwrap();
        assert!(matches!(out_link, SimLink::SplitOutLink(_)));

        let net2 = sim_nets.get_mut(1).unwrap();
        println!("{net2:#?}");
        // we expect one node
        assert_eq!(1, net2.nodes.len());
        // we expect one in link
        assert_eq!(1, net2.links.len());
        let in_link = net2.links.get(&net2.global_network.link_ids.get(1)).unwrap();
        assert!(matches!(in_link, SimLink::SplitInLink(_)));
    }

    #[test]
    fn move_nodes_single_node_vehicles_in() {
        let mut network = Network::new();
        let mut sim_network = create_single_node_sim_network(&mut network);
        let mut publisher = EventsPublisher::new();
        let agent = create_agent(1, vec![0]);
        let vehicle = Vehicle::new(1, VehicleType::Network, String::from("car"), agent);
        let in_link_id = sim_network.global_network.link_ids.get(0);
        if let SimLink::LocalLink(link1) = sim_network.links.get_mut(&in_link_id).unwrap() {
            link1.push_vehicle(vehicle, 1, None);
        }

        let exits = sim_network.move_nodes(&mut publisher, None, 11);

        assert_eq!(1, exits.len());
        assert!(matches!(exits.get(0).unwrap(), ExitReason::FinishRoute(_)));
    }

    #[test]
    fn vehicle_in_and_out() {
        let mut network = Network::new();
        let mut sim_network = create_single_node_sim_network(&mut network);
        let mut publisher = EventsPublisher::new();
        let agent = create_agent(1, vec![0, 1]);
        let vehicle = Vehicle::new(1, VehicleType::Network, String::from("car"), agent);
        let in_link_id = sim_network.global_network.link_ids.get(0);
        if let SimLink::LocalLink(link1) = sim_network.links.get_mut(&in_link_id).unwrap() {
            link1.push_vehicle(vehicle, 1, None);
        }

        let exits = sim_network.move_nodes(&mut publisher, None, 11);

        assert_eq!(0, exits.len());
        let out_id = sim_network.global_network.link_ids.get(1);
        if let SimLink::LocalLink(out_link) = sim_network.links.get_mut(&out_id).unwrap() {
            let vehicles = out_link.pop_front(22);
            assert_eq!(1, vehicles.len());
        }
    }

    #[test]
    pub fn vehicle_in_out_boundary() {
        let mut network = Network::new();
        let mut sim_nets = create_single_node_sim_network_with_partition(&mut network);
        let sim_network = sim_nets.get_mut(0).unwrap();
        let mut publisher = EventsPublisher::new();
        let agent = create_agent(1, vec![0, 1]);
        let vehicle = Vehicle::new(1, VehicleType::Network, String::from("car"), agent);
        let in_link_id = sim_network.global_network.link_ids.get(0);
        if let SimLink::LocalLink(link1) = sim_network.links.get_mut(&in_link_id).unwrap() {
            link1.push_vehicle(vehicle, 1, None);
        }

        let exits = sim_network.move_nodes(&mut publisher, None, 11);

        assert_eq!(1, exits.len());
        let exit = exits.first().unwrap();
        assert!(matches!(exit, ExitReason::ReachedBoundary(_)))
    }

    #[test]
    fn vehicles_in() {

        let mut network = Network::new();
        let mut sim_network = create_single_node_sim_network(&mut network);
        let mut publisher = EventsPublisher::new();
        let agent1 = create_agent(1, vec![0]);
        let vehicle1 = Vehicle::new(1, VehicleType::Network, String::from("car"), agent1);
        let agent2 = create_agent(2, vec![0]);
        let vehicle2 = Vehicle::new(2, VehicleType::Network, String::from("car"), agent2);
        let in_link_id = sim_network.global_network.link_ids.get(0);
        if let SimLink::LocalLink(link1) = sim_network.links.get_mut(&in_link_id).unwrap() {
            link1.push_vehicle(vehicle1, 1, None);
            link1.push_vehicle(vehicle2, 1, None);
        }
        let exited_vehicles = sim_network.move_nodes(&mut publisher, None, 11);

        assert_eq!(1, exited_vehicles.len());
        let entry = exited_vehicles.get(0).unwrap();
        assert!(matches!(entry, ExitReason::FinishRoute(_)));
        if let ExitReason::FinishRoute(vehicle) = entry {
            assert_eq!(1, vehicle.id);
        }

        let exited_vehicles = sim_network.move_nodes(&mut publisher, None, 21);

        assert_eq!(1, exited_vehicles.len());
        let entry = exited_vehicles.get(0).unwrap();
        assert!(matches!(entry, ExitReason::FinishRoute(_)));
        if let ExitReason::FinishRoute(vehicle) = entry {
            assert_eq!(2, vehicle.id);
        }
    }

    #[test]
    fn vehicles_in_and_out() {
        let mut network = Network::new();
        let mut sim_network = create_single_node_sim_network(&mut network);
        let mut publisher = EventsPublisher::new();
        let agent1 = create_agent(1, vec![0, 1]);
        let vehicle1 = Vehicle::new(1, VehicleType::Network, String::from("car"), agent1);
        let agent2 = create_agent(2, vec![0, 1]);
        let vehicle2 = Vehicle::new(2, VehicleType::Network, String::from("car"), agent2);
        let in_link_id = sim_network.global_network.link_ids.get(0);
        if let SimLink::LocalLink(link1) = sim_network.links.get_mut(&in_link_id).unwrap() {
            link1.push_vehicle(vehicle1, 1, None);
            link1.push_vehicle(vehicle2, 1, None);
        }

        let exited_vehicles = sim_network.move_nodes(&mut publisher, None, 11);
        assert_eq!(0, exited_vehicles.len());
        let exited_vehicles = sim_network.move_nodes(&mut publisher, None, 12);
        assert_eq!(0, exited_vehicles.len());

        let out_link = sim_network.links.get_mut(&sim_network.global_network.link_ids.get(1)).unwrap();
        if let SimLink::LocalLink(local_out) = out_link {
            let vehicles = local_out.pop_front(23);
            assert_eq!(1, vehicles.len());
            let vehicles = local_out.pop_front(24);
            assert_eq!(1, vehicles.len());
        }
    }

    fn init_single_node_network(network: &mut Network) {
        let node1 = Node::new(network.node_ids.create_id("node-1"), -100., 0.);
        let node2 = Node::new(network.node_ids.create_id("node-2"), 0., 0.);
        let node3 = Node::new(network.node_ids.create_id("node-3"), 100., 0.);
        let mut link1 = Link::new_with_default(network.link_ids.create_id("link-1"), &node1, &node2);
        link1.capacity = 3600.;
        link1.freespeed = 10.;
        let mut link2 = Link::new_with_default(network.link_ids.create_id("link-2"), &node2, &node3);
        link2.capacity = 3600.;
        link2.freespeed = 10.;

        network.add_node(node1);
        network.add_node(node2);
        network.add_node(node3);
        network.add_link(link1);
        network.add_link(link2);
    }

    fn create_single_node_sim_network_with_partition<'n>(mut network: &'n mut Network) -> Vec<SimNetworkPartition<'n>> {
        init_single_node_network(&mut network);
        let node3 = network.nodes.get_mut(2).unwrap();
        node3.partition = 1;
        let link2 = network.links.get_mut(1).unwrap();
        link2.partition = 1;
        vec![SimNetworkPartition::from_network(network, 0), SimNetworkPartition::from_network(network, 1)]
    }

    fn create_single_node_sim_network<'n>(mut network: &'n mut Network) -> SimNetworkPartition<'n> {
        init_single_node_network(&mut network);
        SimNetworkPartition::from_network(network, 0)
    }

    fn indirection<'n>(net: &'n Network<'n>) -> SimNetworkPartition<'n> {
        SimNetworkPartition {
            nodes: Vec::default(),
            links: std::collections::HashMap::default(),
            global_network: net,
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
