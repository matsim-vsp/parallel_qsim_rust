use std::collections::HashSet;

use nohash_hasher::{IntMap, IntSet};

use crate::simulation::id::IdStore;
use crate::simulation::messaging::messages::proto::StorageCap;
use crate::simulation::{
    id::Id,
    messaging::{
        events::{proto::Event, EventsPublisher},
        messages::proto::Vehicle,
    },
};

use super::{
    global_network::{Link, Network, Node},
    link::{LocalLink, SimLink, SplitInLink, SplitOutLink},
};

#[derive(Debug)]
pub enum ExitReason {
    FinishRoute(Vehicle),
    ReachedBoundary(Vehicle),
}

#[derive(Debug)]
pub struct SimNetworkPartition<'n> {
    pub nodes: Vec<Id<Node>>,
    // use int map as hash map variant with stable order
    pub links: IntMap<Id<Link>, SimLink>,
    pub global_network: &'n Network<'n>,
}

impl<'n> SimNetworkPartition<'n> {
    pub fn from_network(global_network: &'n Network, partition: usize) -> Self {
        let nodes: Vec<_> = global_network
            .nodes
            .iter()
            .filter(|node| node.partition == partition)
            .map(|node| node.id.clone())
            .collect();

        let link_ids: IntSet<_> = nodes
            .iter()
            .map(|id| global_network.nodes.get(id.internal()).unwrap())
            .filter(|node| node.partition == partition)
            .flat_map(|node| node.in_links.iter().chain(node.out_links.iter()))
            .collect(); // collect here to get each link id only once

        let links: IntMap<_, _> = link_ids
            .iter()
            .map(|link_id| global_network.links.get(link_id.internal()).unwrap())
            .map(|link| {
                (
                    link.id.clone(),
                    Self::create_sim_link(
                        link,
                        partition,
                        global_network.effective_cell_size,
                        &global_network.nodes,
                    ),
                )
            })
            .collect();

        Self::new(nodes, links, global_network)
    }

    fn create_sim_link(
        link: &Link,
        partition: usize,
        effective_cell_size: f32,
        all_nodes: &[Node],
    ) -> SimLink {
        let from_part = all_nodes.get(link.from.internal()).unwrap().partition;
        let to_part = all_nodes.get(link.to.internal()).unwrap().partition;

        if from_part == to_part {
            SimLink::Local(LocalLink::from_link(link, 1.0, effective_cell_size))
        } else if to_part == partition {
            let local_link = LocalLink::from_link(link, 1.0, 7.5);
            SimLink::In(SplitInLink::new(from_part, local_link))
        } else {
            SimLink::Out(SplitOutLink::new(link.id.clone(), to_part))
        }
    }

    pub fn new(
        nodes: Vec<Id<Node>>,
        links: IntMap<Id<Link>, SimLink>,
        //links: HashMap<Id<Link>, SimLink>,
        global_network: &'n Network,
    ) -> Self {
        SimNetworkPartition {
            nodes,
            links,
            global_network,
        }
    }

    pub fn neighbors(&self) -> HashSet<usize> {
        let distinct_partitions: HashSet<usize> = self
            .links
            .values()
            .filter(|link| match link {
                SimLink::Local(_) => false,
                SimLink::In(_) => true,
                SimLink::Out(_) => true,
            })
            .map(|link| match link {
                SimLink::Local(_) => panic!("Should be filtered."),
                SimLink::In(link) => link.neighbor_partition_id(),
                SimLink::Out(link) => link.neighbor_partition_id(),
            })
            .collect();
        distinct_partitions
    }

    pub fn send_veh_en_route(&mut self, vehicle: Vehicle, now: u32) {
        let link_id = vehicle.curr_link_id().unwrap_or_else(|| {
            panic!("Vehicle is expected to have a current link id if it is sent onto the network")
        });
        let link_id = self.global_network.link_ids.get(link_id);
        let link = self.links.get_mut(&link_id).unwrap();
        link.push_veh(vehicle, now);
    }

    pub fn move_links(&mut self) -> (Vec<Vehicle>, Vec<StorageCap>) {
        for link in self.links.values_mut() {
            link.update_released_storage_cap();
        }

        (Vec::new(), Vec::new())
    }

    pub fn move_nodes(&mut self, events: &mut EventsPublisher, now: u32) -> Vec<ExitReason> {
        let mut exited_vehicles = Vec::new();

        for node_id in &self.nodes {
            Self::move_node(
                node_id,
                self.global_network,
                &mut self.links,
                &mut exited_vehicles,
                events,
                now,
            );
        }

        exited_vehicles
    }

    fn move_node(
        node_id: &Id<Node>,
        global_network: &Network,
        links: &mut IntMap<Id<Link>, SimLink>,
        exited_vehicles: &mut Vec<ExitReason>,
        events: &mut EventsPublisher,
        now: u32,
    ) {
        let node = global_network.get_node(node_id);

        for link_id in &node.in_links {
            let in_link = links.get_mut(link_id).unwrap();
            in_link.update_flow_cap(now);

            while Self::should_veh_move_out(link_id, links, &global_network.link_ids, now) {
                // get the mut ref here again, so that the borrow checker lets us borrow the links map
                // for the method above.
                let in_link = links.get_mut(link_id).unwrap();
                let veh = in_link.pop_veh();

                if veh.peek_next_route_element().is_some() {
                    Self::move_vehicle(veh, global_network, links, events, exited_vehicles, now);
                } else {
                    exited_vehicles.push(ExitReason::FinishRoute(veh));
                }
            }
        }
    }

    fn should_veh_move_out(
        in_id: &Id<Link>,
        links: &IntMap<Id<Link>, SimLink>,
        id_store: &IdStore<Link>,
        now: u32,
    ) -> bool {
        let in_link = links.get(in_id).unwrap();
        if let Some(veh_ref) = in_link.offers_veh(now) {
            return if let Some(next_id_int) = veh_ref.peek_next_route_element() {
                // if the vehicle has a next link id, it should move out of the current link, if the
                // next link is free.
                let out_link_id = id_store.get(next_id_int);
                let out_link = links.get(&out_link_id).unwrap();
                out_link.is_available()
            } else {
                // if there is no next link, the vehicle is done with its route and we can take it out
                // of the network
                true
            };
        }
        // if the link doesn't have a vehicle to offer, we don't have to do anything.
        false
    }

    fn move_vehicle(
        mut vehicle: Vehicle,
        global_network: &Network,
        links: &mut IntMap<Id<Link>, SimLink>,
        events: &mut EventsPublisher,
        exited_vehicles: &mut Vec<ExitReason>,
        now: u32,
    ) {
        events.publish_event(
            now,
            &Event::new_link_leave(vehicle.curr_route_elem as u64, vehicle.id),
        );
        vehicle.advance_route_index();
        let link_id = global_network.link_ids.get(vehicle.curr_link_id().unwrap());
        match links.get_mut(&link_id).unwrap() {
            SimLink::Local(l) => {
                events.publish_event(
                    now,
                    &Event::new_link_enter(l.id.internal() as u64, vehicle.id),
                );
                l.push_vehicle(vehicle, now);
            }
            SimLink::Out(_) => exited_vehicles.push(ExitReason::ReachedBoundary(vehicle)),
            SimLink::In(_) => {
                panic!("Not expecting to move a vehicle onto a split in link.")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::simulation::messaging::messages::proto::Route;
    use crate::simulation::vehicles::garage::Garage;
    use crate::simulation::{
        messaging::{
            events::EventsPublisher,
            messages::proto::{Activity, Agent, Leg, Plan, Vehicle},
        },
        network::{
            global_network::{Link, Network, Node},
            link::SimLink,
        },
    };

    use super::ExitReason;
    use super::SimNetworkPartition;

    #[test]
    fn from_network() {
        let mut network = Network::new();
        let mut sim_nets = create_three_node_sim_network_with_partition(&mut network);
        let net1 = sim_nets.get_mut(0).unwrap();

        // we expect two nodes
        assert_eq!(2, net1.nodes.len());
        // we expect two links one local and one out link
        assert_eq!(2, net1.links.len());
        let local_link = net1
            .links
            .get(&net1.global_network.link_ids.get(0))
            .unwrap();
        assert!(matches!(local_link, SimLink::Local(_)));
        let out_link = net1
            .links
            .get(&net1.global_network.link_ids.get(1))
            .unwrap();
        assert!(matches!(out_link, SimLink::Out(_)));

        let net2 = sim_nets.get_mut(1).unwrap();
        // we expect one node
        assert_eq!(1, net2.nodes.len());
        // we expect one in link
        assert_eq!(1, net2.links.len());
        let in_link = net2
            .links
            .get(&net2.global_network.link_ids.get(1))
            .unwrap();
        assert!(matches!(in_link, SimLink::In(_)));
    }

    #[test]
    fn move_nodes_free_flow_exit_end() {
        let mut publisher = EventsPublisher::new();
        let mut garage = Garage::new();
        let global_net = Network::from_file("./assets/3-links/3-links-network.xml", 1, &mut garage);
        let mut network = SimNetworkPartition::from_network(&global_net, 0);
        let agent = create_agent(1, vec![0, 1, 2]);
        let vehicle = Vehicle::new(1, 0, 10., 1., Some(agent));
        network.send_veh_en_route(vehicle, 0);

        for i in 0..120 {
            let result = network.move_nodes(&mut publisher, i);

            if i == 120 {
                assert!(!result.is_empty());
                if let ExitReason::FinishRoute(veh) = result.first().unwrap() {
                    assert!(veh.curr_link_id().is_none());
                } else {
                    panic!("This should have exit reason finish route.")
                }
            }
        }
    }

    #[test]
    fn move_nodes_free_flow_exit_boundary() {
        let mut publisher = EventsPublisher::new();
        let mut garage = Garage::new();
        let global_net = Network::from_file("./assets/3-links/3-links-network.xml", 2, &mut garage);
        let mut network = SimNetworkPartition::from_network(&global_net, 1);
        let agent = create_agent(1, vec![0, 1, 2]);
        let vehicle = Vehicle::new(1, 0, 10., 100., Some(agent));
        network.send_veh_en_route(vehicle, 0);

        for i in 0..120 {
            let result = network.move_nodes(&mut publisher, i);

            if !result.is_empty() {
                assert_eq!(10, i);
                if let ExitReason::ReachedBoundary(veh) = result.first().unwrap() {
                    assert!(veh.curr_link_id().is_some());
                    assert_eq!(1, veh.curr_link_id().unwrap());
                } else {
                    panic!("This should have exit reason reached boundary route.")
                }
            }
        }
    }

    #[test]
    fn move_nodes_flow_cap_constraint() {
        let mut publisher = EventsPublisher::new();
        let mut garage = Garage::new();
        let global_net = Network::from_file("./assets/3-links/3-links-network.xml", 1, &mut garage);
        let mut network = SimNetworkPartition::from_network(&global_net, 0);

        // place 100 vehicles on first link
        for i in 0..100 {
            let agent = create_agent(i, vec![0]);
            let vehicle = Vehicle::new(i, 0, 10., 1., Some(agent));
            network.send_veh_en_route(vehicle, 0);
        }

        // all vehicle only have to traverse link1. Link1 can release one vehicle/s, first one at t=10
        // this way we should have 10 vehicles released at t=20
        let mut counter = 0;
        for i in 0..110 {
            let result = network.move_nodes(&mut publisher, i);
            if i < 10 {
                assert!(result.is_empty());
            } else {
                assert_eq!(1, result.len());
                counter += 1;
            }
        }
        assert_eq!(100, counter);
    }

    #[test]
    fn move_nodes_storage_cap_constraint() {
        let mut publisher = EventsPublisher::new();
        let mut garage = Garage::new();
        let mut global_net =
            Network::from_file("./assets/3-links/3-links-network.xml", 1, &mut garage);
        global_net.effective_cell_size = 10.;

        let id_1 = global_net.link_ids.get_from_ext("link1");
        let id_2 = global_net.link_ids.get_from_ext("link2");
        let mut network = SimNetworkPartition::from_network(&global_net, 0);

        //place 10 vehicles on link2 so that it is jammed
        // vehicles are very slow, so that the first vehicle should leave link2 at t=1000
        for i in 0..10 {
            let agent = create_agent(i, vec![id_2.internal() as u64, 2]);
            let vehicle = Vehicle::new(i, 0, 1., 10., Some(agent));
            network.send_veh_en_route(vehicle, 0);
        }

        // place 1 vehicle onto link1 which has to wait until link2 has free storage cap
        // as the first vehicle leaves link2 at t=1000 this vehicle can leave link1 and enter link2 at
        // the next timestep at t=1001
        let agent = create_agent(11, vec![id_1.internal() as u64, 1, 2]);
        let vehicle = Vehicle::new(11, 0, 10., 1., Some(agent));
        network.send_veh_en_route(vehicle, 0);

        for now in 0..1010 {
            network.move_nodes(&mut publisher, now);
            network.move_links();

            let link1 = network.links.get(&id_1).unwrap();
            if (10..1001).contains(&now) {
                assert!(link1.offers_veh(now).is_some());
            } else {
                assert!(link1.offers_veh(now).is_none());
            }
        }
    }

    #[test]
    fn neighbors() {
        let mut net = Network::new();
        let mut node = Node::new(net.node_ids.create_id("node-1"), -0., 0.);
        node.partition = 0;

        let mut node_1_1 = Node::new(net.node_ids.create_id("node-1-1"), -0., 0.);
        node_1_1.partition = 1;
        let mut node_1_2 = Node::new(net.node_ids.create_id("node-1-2"), -0., 0.);
        node_1_2.partition = 1;

        let mut node_2_1 = Node::new(net.node_ids.create_id("node-2-1"), -0., 0.);
        node_2_1.partition = 2;
        let mut node_3_1 = Node::new(net.node_ids.create_id("node-3-1"), -0., 0.);
        node_3_1.partition = 3;
        let mut node_4_1 = Node::new(net.node_ids.create_id("not-a-neighbor"), 0., 0.);
        node_4_1.partition = 4;

        // create in links from partitions 1 and 2. 2 incoming links from partition 1, one incoming from
        // partition 2
        let in_link_1_1 =
            Link::new_with_default(net.link_ids.create_id("in-link-1-1"), &node_1_1, &node);
        let in_link_1_2 =
            Link::new_with_default(net.link_ids.create_id("in-link-1-2"), &node_1_2, &node);
        let in_link_2_1 =
            Link::new_with_default(net.link_ids.create_id("in-link-2-1"), &node_2_1, &node);

        // create out links to partitions 1 and 3
        let out_link_1_1 =
            Link::new_with_default(net.link_ids.create_id("out-link-1-1"), &node, &node_1_1);
        let out_link_1_2 =
            Link::new_with_default(net.link_ids.create_id("out-link-1-2"), &node, &node_1_2);
        let out_link_3_1 =
            Link::new_with_default(net.link_ids.create_id("out-link-3-1"), &node, &node_3_1);

        net.add_node(node);
        net.add_node(node_1_1);
        net.add_node(node_1_2);
        net.add_node(node_2_1);
        net.add_node(node_3_1);
        net.add_node(node_4_1);
        net.add_link(in_link_1_1);
        net.add_link(in_link_1_2);
        net.add_link(in_link_2_1);
        net.add_link(out_link_1_1);
        net.add_link(out_link_1_2);
        net.add_link(out_link_3_1);

        let sim_net = SimNetworkPartition::from_network(&net, 0);

        let neighbors = sim_net.neighbors();
        assert_eq!(3, neighbors.len());
        assert!(neighbors.contains(&1));
        assert!(neighbors.contains(&2));
        assert!(neighbors.contains(&3));
        assert!(!neighbors.contains(&4));
    }

    fn init_three_node_network(network: &mut Network) {
        let node1 = Node::new(network.node_ids.create_id("node-1"), -100., 0.);
        let node2 = Node::new(network.node_ids.create_id("node-2"), 0., 0.);
        let node3 = Node::new(network.node_ids.create_id("node-3"), 100., 0.);
        let mut link1 =
            Link::new_with_default(network.link_ids.create_id("link-1"), &node1, &node2);
        link1.capacity = 3600.;
        link1.freespeed = 10.;
        let mut link2 =
            Link::new_with_default(network.link_ids.create_id("link-2"), &node2, &node3);
        link2.capacity = 3600.;
        link2.freespeed = 10.;

        network.add_node(node1);
        network.add_node(node2);
        network.add_node(node3);
        network.add_link(link1);
        network.add_link(link2);
    }

    fn create_three_node_sim_network_with_partition<'n>(
        network: &'n mut Network,
    ) -> Vec<SimNetworkPartition<'n>> {
        init_three_node_network(network);
        let node3 = network.nodes.get_mut(2).unwrap();
        node3.partition = 1;
        let link2 = network.links.get_mut(1).unwrap();
        link2.partition = 1;
        vec![
            SimNetworkPartition::from_network(network, 0),
            SimNetworkPartition::from_network(network, 1),
        ]
    }

    fn create_agent(id: u64, route: Vec<u64>) -> Agent {
        let route = Route {
            veh_id: id,
            distance: 0.0,
            route,
        };
        let leg = Leg::new(route, 0, 0, None);
        let act = Activity::new(0., 0., 0, 1, None, None, None);
        let mut plan = Plan::new();
        plan.add_act(act);
        plan.add_leg(leg);
        let mut agent = Agent::new(id, plan);
        agent.advance_plan();

        agent
    }
}
