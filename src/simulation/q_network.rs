use crate::container::network::Node;
use crate::container::network::{Link, Network};
use crate::simulation::q_vehicle::QVehicle;
use std::collections::HashMap;
use std::hash::Hash;

#[derive(Debug)]
pub struct QNetwork {
    links: Vec<QLink>,
    nodes: Vec<QNode>,
}

impl QNetwork {
    fn new() -> QNetwork {
        QNetwork {
            links: Vec::new(),
            nodes: Vec::new(),
        }
    }

    fn add_node(&mut self) -> usize {
        // create a node with an id. The in and out links will be set once links are inserted
        // into the network
        let next_id = self.nodes.len();
        let q_node = QNode::new(next_id);
        self.nodes.push(q_node);
        next_id
    }

    fn add_link(&mut self, link: &Link, from_id: usize, to_id: usize) -> usize {
        // create a new link and push it onto the link vec
        let next_id = self.links.len();
        let q_link = QLink::new(next_id, link.length, link.capacity, link.freespeed);
        self.links.push(q_link);

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

        let node_id_map: HashMap<&String, usize> = network
            .nodes()
            .iter()
            .map(|node| {
                let internal_id = result.add_node();
                (&node.id, internal_id)
            })
            .collect();

        for link in network.links() {
            let from_id = node_id_map.get(&link.from).unwrap();
            let to_id = node_id_map.get(&link.to).unwrap();
            result.add_link(link, *from_id, *to_id);
        }

        result
    }
}

#[derive(Debug)]
struct QLink {
    id: usize,
    q: Vec<QVehicle>,
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
            q: Vec::new(),
        }
    }
}

#[derive(Debug)]
struct QNode {
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
}

#[cfg(test)]
mod tests {
    use crate::container::network::Network;
    use crate::simulation::q_network::QNetwork;

    #[test]
    fn create_q_network_from_container_network() {
        let network = Network::from_file("./assets/network.xml");
        let q_network = QNetwork::from_container(&network);

        // check the overall structure
        assert_eq!(network.nodes().len(), q_network.nodes.len());
        assert_eq!(network.links().len(), q_network.links.len());

        // check node "2", which should have index 1 now. It should have 1 in_link and 9 out_links
        let node2 = q_network.nodes.get(1).unwrap();
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
}
