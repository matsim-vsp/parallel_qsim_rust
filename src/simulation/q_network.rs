use crate::container::network::{Link, Network};
use crate::simulation::q_vehicle::QVehicle;
use std::collections::{HashMap, VecDeque};

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

    fn add_link(&mut self, link: &'net Link, from_id: usize, to_id: usize) -> usize {
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

    pub fn from_container(network: &Network) -> QNetwork {
        let mut result = QNetwork::new();

        for node in network.nodes() {
            result.add_node(&node.id);
        }

        for link in network.links() {
            let from_id = result.node_id_mapping.get(link.from.as_str()).unwrap();
            let to_id = result.node_id_mapping.get(link.to.as_str()).unwrap();
            result.add_link(link, *from_id, *to_id);
        }

        result
    }
}

#[derive(Debug)]
pub struct QLink {
    id: usize,
    q: VecDeque<QVehicle>,
    length: f32,
    capacity: f32,
    freespeed: f32,
}

impl QLink {
    fn new(id: usize, length: f32, capacity: f32, freespeed: f32) -> QLink {
        QLink {
            id,
            length,
            capacity,
            freespeed,
            q: VecDeque::new(),
        }
    }

    pub fn push_vehicle(&mut self, vehicle: QVehicle) {
        self.q.push_back(vehicle);
    }

    pub fn pop_front(&mut self, now: u32) -> Vec<QVehicle> {
        let mut vehicles: Vec<QVehicle> = Vec::new();
        while let Some(vehicle) = self.q.front() {
            if vehicle.exit_time <= now {
                let vehicle = self.q.pop_front().unwrap();
                vehicles.push(vehicle);
            } else {
                break;
            }
        }

        vehicles
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
            let in_link = links.get_mut(*in_link_index).unwrap();
            let vehicles = in_link.pop_front(now);

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
    use crate::container::network::Network;
    use crate::simulation::q_network::{QLink, QNetwork};
    use crate::simulation::q_vehicle::QVehicle;

    #[test]
    fn create_q_network_from_container_network() {
        let network = Network::from_file("./assets/equil-network.xml");
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
    fn pop_vehicle_from_link() {}
}
