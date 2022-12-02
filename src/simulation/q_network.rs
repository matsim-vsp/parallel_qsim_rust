use std::collections::{HashMap, VecDeque};

use crate::io::network::{IOLink, IONetwork};
use crate::simulation::flow_cap::Flowcap;
use crate::simulation::q_vehicle::QVehicle;

#[derive(Debug)]
pub struct QNetwork<'net> {
    pub links: Vec<QLink>,
    pub nodes: Vec<QNode>,
    pub link_id_mapping: HashMap<&'net str, usize>,
    pub node_id_mapping: HashMap<&'net str, usize>,
}

impl<'net> QNetwork<'net> {
    fn new() -> QNetwork<'net> {
        QNetwork {
            links: Vec::new(),
            nodes: Vec::new(),
            link_id_mapping: HashMap::new(),
            node_id_mapping: HashMap::new(),
        }
    }

    fn add_node(&mut self, node_id: &'net str) -> usize {
        // create a node with an id. The in and out links will be set once links are inserted
        // into the network
        let next_id = self.nodes.len();
        let q_node = QNode::new(next_id);
        self.nodes.push(q_node);
        self.node_id_mapping.insert(node_id, next_id);
        next_id
    }

    fn add_link(&mut self, link: &'net IOLink, from_id: usize, to_id: usize) -> usize {
        // create a new link and push it onto the link vec
        let next_id = self.links.len();
        let q_link = QLink::new(next_id, link.length, link.capacity, link.freespeed);
        self.links.push(q_link);
        self.link_id_mapping.insert(&link.id, next_id);

        // wire up with the from and to node
        let from = self.nodes.get_mut(from_id).unwrap();
        from.out_links.push(next_id);
        let to = self.nodes.get_mut(to_id).unwrap();
        to.in_links.push(next_id);

        // return the internal id of the link
        next_id
    }

    pub fn from_container(network: &IONetwork) -> QNetwork {
        let mut result = QNetwork::new();

        for node in network.nodes() {
            result.add_node(&node.id);
        }

        for link in network.links() {
            let from_id = *result.node_id_mapping.get(&link.from.as_str()).unwrap();
            let to_id = *result.node_id_mapping.get(&link.to.as_str()).unwrap();
            result.add_link(link, from_id, to_id);
        }

        result
    }
}

#[derive(Debug)]
pub struct QLink {
    id: usize,
    q: VecDeque<QVehicle>,
    length: f32,
    freespeed: f32,
    flowcap: Flowcap,
}

impl QLink {
    fn new(id: usize, length: f32, capacity_h: f32, freespeed: f32) -> QLink {
        QLink {
            id,
            length,
            freespeed,
            q: VecDeque::new(),
            flowcap: Flowcap::new(capacity_h / 3600.0),
        }
    }

    pub fn push_vehicle(&mut self, vehicle: QVehicle) {
        self.q.push_back(vehicle);
    }

    pub fn pop_front(&mut self, now: u32) -> Vec<QVehicle> {
        self.flowcap.update_capacity(now);
        let mut popped_vehicles: Vec<QVehicle> = Vec::new();

        while let Some(vehicle) = self.q.front() {
            if vehicle.exit_time > now || !self.flowcap.has_capacity() {
                break;
            }

            // take the vehicle out of the q, update the flow cap, and put it into the result
            let vehicle = self.q.pop_front().unwrap();
            self.flowcap.consume_capacity(1.0);
            popped_vehicles.push(vehicle);
        }

        popped_vehicles
    }
}

#[derive(Debug)]
pub struct QNode {
    id: usize,
    in_links: Vec<usize>,
    out_links: Vec<usize>,
}

impl QNode {
    fn new(id: usize) -> QNode {
        QNode {
            id,
            in_links: Vec::new(),
            out_links: Vec::new(),
        }
    }

    pub fn move_vehicles(&self, links: &mut Vec<QLink>, now: u32) -> Vec<QVehicle> {
        let mut at_end_of_route = Vec::new();

        for in_link_index in &self.in_links {
            // we obtain a mutable reference of the in_link and fetch all vehicles which are
            // eligible for leaving. We need to have all vehicles at once instead of having a
            // while let loop over in_link.pop_first_vehicle, so that we can release the mutable
            // reference of in_link and therefore links.
            let in_link = links.get_mut(*in_link_index).unwrap();
            let vehicles = in_link.pop_front(now);
            // mut ref of in_link and links ends here. Now we are allowed to have a mutable reference
            // of links again, which we need to mutate the out_link.

            for mut vehicle in vehicles {
                vehicle.advance_route_index();
                match vehicle.current_link_id() {
                    None => at_end_of_route.push(vehicle),
                    Some(out_link_id) => {
                        self.move_vehicle(links, *out_link_id, vehicle, now);
                    }
                }
            }
        }

        at_end_of_route
    }

    fn move_vehicle(
        &self,
        links: &mut Vec<QLink>,
        out_link_id: usize,
        mut vehicle: QVehicle,
        now: u32,
    ) {
        let out_link = links.get_mut(out_link_id).unwrap();
        let exit_time = now + (out_link.length / out_link.freespeed) as u32;

        println!(
            "Time: {now}. Moving vehicle #{} to #{out_link_id}. Exit time is: {exit_time}",
            vehicle.id
        );

        vehicle.exit_time = exit_time;
        out_link.push_vehicle(vehicle);
    }
}

#[cfg(test)]
mod tests {
    use crate::io::network::IONetwork;
    use crate::simulation::q_network::QLink;
    use crate::simulation::q_network::QNetwork;
    use crate::simulation::q_vehicle::QVehicle;

    #[test]
    fn q_network_from_container() {
        let network = IONetwork::from_file("./assets/equil/equil-network.xml");
        let q_network = QNetwork::from_container(&network);

        println!("{q_network:#?}");
        // check the overall structure
        assert_eq!(network.nodes().len(), q_network.nodes.len());
        assert_eq!(network.links().len(), q_network.links.len());

        // check node "2", which should have index 1 now. It should have 1 in_link and 9 out_links
        let internal_id_for_node2 = q_network.node_id_mapping.get("2").unwrap();
        let node2 = q_network.nodes.get(*internal_id_for_node2).unwrap();
        assert_eq!(1, node2.id);
        assert_eq!(1, node2.in_links.len());
        assert_eq!(9, node2.out_links.len());

        // in link should be id:0
        assert_eq!(0, *node2.in_links.get(0).unwrap());

        // out links should be from 1 to 9
        let mut index: usize = 1;
        for id in &node2.out_links {
            assert_eq!(index, *id);
            index = index + 1;
        }
    }

    #[test]
    fn link_pop_front_exit_time_constraint() {
        let id1 = 1;
        let id2 = 2;
        let mut vehicle1 = QVehicle::new(id1, 1, Vec::new());
        let mut vehicle2 = QVehicle::new(id2, 2, Vec::new());
        vehicle1.exit_time = 1;
        vehicle2.exit_time = 5;
        let mut link = QLink::new(1, 10.0, 3600.0, 1.0);
        link.push_vehicle(vehicle1);
        link.push_vehicle(vehicle2);

        // this should not do anything because the exit time of the vehicle is not yet reached
        let popped_vehicles = link.pop_front(0);
        assert_eq!(0, popped_vehicles.len());

        // now is equal to the vehicle1's exit time and it should be able to leave
        let popped_vehicles = link.pop_front(1);
        assert_eq!(1, popped_vehicles.len());
        let popped1 = popped_vehicles.first().unwrap();
        assert_eq!(id1, popped1.id);

        // now is greater than vehicle2's exit time. it should leave now as wel
        let popped_vehicles = link.pop_front(10);
        assert_eq!(1, popped_vehicles.len());
        let popped2 = popped_vehicles.first().unwrap();
        assert_eq!(id2, popped2.id);
    }

    #[test]
    fn link_pop_front_capacity_constraint() {
        let id1 = 1;
        let id2 = 2;
        let vehicle1 = QVehicle::new(id1, 1, Vec::new());
        let vehicle2 = QVehicle::new(id2, 2, Vec::new());
        let mut link = QLink::new(1, 10.0, 900., 1.0);
        link.push_vehicle(vehicle1);
        link.push_vehicle(vehicle2);

        // according to their exit times both vehicles could leave the link immediately, but only
        // one can leave every 4 timesteps because of the link's capacity
        let popped_vehicles = link.pop_front(0);
        assert_eq!(1, popped_vehicles.len());
        let popped1 = popped_vehicles.first().unwrap();
        assert_eq!(id1, popped1.id);

        let popped_vehicles = link.pop_front(1);
        assert_eq!(0, popped_vehicles.len());

        let popped_vehicles = link.pop_front(4);
        assert_eq!(1, popped_vehicles.len());
        let popped2 = popped_vehicles.first().unwrap();
        assert_eq!(id2, popped2.id);
    }
}
