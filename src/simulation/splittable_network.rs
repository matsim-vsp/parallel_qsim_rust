use crate::container::network::{IOLink, IONetwork, IONode};
use crate::simulation::flow_cap::Flowcap;
use std::collections::{HashMap, VecDeque};

use crate::simulation::q_vehicle::QVehicle;

pub struct IdMappings {
    node_2_thread: HashMap<usize, usize>,
    link_2_thread: HashMap<usize, usize>,
}

#[derive(Debug)]
pub struct Network {
    pub links: HashMap<usize, Link>,
    pub nodes: HashMap<usize, Node>,
}

impl Network {
    fn new() -> Self {
        Self {
            links: HashMap::new(),
            nodes: HashMap::new(),
        }
    }

    pub fn split_from_container(
        container: &IONetwork,
        size: usize,
        splitter: fn(&IONode) -> usize,
    ) -> (Vec<Network>, IdMappings) {
        // create the result networks which can then be populated
        let mut result = Vec::with_capacity(size);

        let mut node_id_mapping: HashMap<&str, usize> = HashMap::new();
        let mut link_id_mapping: HashMap<&str, usize> = HashMap::new();

        let mut id_mapping = IdMappings {
            link_2_thread: HashMap::new(),
            node_2_thread: HashMap::new(),
        };

        for _i in 0..size {
            result.push(Network::new());
        }

        let mut next_id = 0;
        for node in container.nodes() {
            let thread_id = splitter(node);
            let network = result.get_mut(thread_id).unwrap();

            network.add_node(next_id);
            node_id_mapping.insert(&node.id, next_id);
            id_mapping.node_2_thread.insert(next_id, thread_id);
            next_id = next_id + 1;
        }

        next_id = 0;
        for link in container.links() {
            let from_id = *node_id_mapping.get(link.from.as_str()).unwrap();
            let to_id = *node_id_mapping.get(link.to.as_str()).unwrap();

            let from_thread = *id_mapping.node_2_thread.get(&from_id).unwrap();
            let to_thread = *id_mapping.node_2_thread.get(&to_id).unwrap();

            if from_thread == to_thread {
                let network = result.get_mut(from_thread).unwrap();
                network.add_local_link(link, next_id, from_id, to_id);
            } else {
                let from_network = result.get_mut(from_thread).unwrap();
                from_network.add_split_out_link(next_id, from_id, from_thread, to_thread);

                let to_network = result.get_mut(to_thread).unwrap();
                to_network.add_split_in_link(link, next_id, to_id);
            }
            link_id_mapping.insert(&link.id, next_id);
            // the link is associated with the network which contains its to-node
            id_mapping.link_2_thread.insert(next_id, to_thread);
            next_id = next_id + 1;
        }

        (result, id_mapping)
    }

    fn add_node(&mut self, id: usize) {
        let node = Node::new(id);
        self.nodes.insert(id, node);
    }

    fn add_local_link(&mut self, link: &IOLink, id: usize, from: usize, to: usize) {
        let new_link = LocalLink {
            id,
            q: VecDeque::new(),
            flowcap: Flowcap::new(link.capacity),
            freespeed: link.freespeed,
            length: link.length,
        };
        self.links.insert(id, Link::LocalLink(new_link));

        // wire up the from and to node
        let from = self.nodes.get_mut(&from).unwrap();
        from.out_links.push(id);
        let to = self.nodes.get_mut(&to).unwrap();
        to.in_links.push(id);
    }

    fn add_split_out_link(&mut self, id: usize, from: usize, from_thread: usize, to_thread: usize) {
        let new_link = SplitLink {
            id,
            from_thread_id: from_thread,
            to_thread_id: to_thread,
        };
        self.links.insert(id, Link::SplitLink(new_link));

        // wire up from node
        let from_node = self.nodes.get_mut(&from).unwrap();
        from_node.out_links.push(id);
    }

    fn add_split_in_link(&mut self, link: &IOLink, id: usize, to: usize) {
        let new_link = LocalLink {
            id,
            q: VecDeque::new(),
            flowcap: Flowcap::new(link.capacity),
            freespeed: link.freespeed,
            length: link.length,
        };
        self.links.insert(id, Link::LocalLink(new_link));

        // wire up to node
        let to_node = self.nodes.get_mut(&to).unwrap();
        to_node.in_links.push(id);
    }
}

#[derive(Debug)]
pub struct Node {
    id: usize,
    in_links: Vec<usize>,
    out_links: Vec<usize>,
}

impl Node {
    fn new(id: usize) -> Node {
        Node {
            id,
            in_links: Vec::new(),
            out_links: Vec::new(),
        }
    }
}

#[derive(Debug)]
pub enum Link {
    LocalLink(LocalLink),
    SplitLink(SplitLink),
}

#[derive(Debug)]
pub struct LocalLink {
    id: usize,
    q: VecDeque<QVehicle>,
    length: f32,
    freespeed: f32,
    flowcap: Flowcap,
}

#[derive(Debug)]
pub struct SplitLink {
    id: usize,
    from_thread_id: usize,
    to_thread_id: usize,
}

#[cfg(test)]
mod tests {
    use crate::container::network::{IONetwork, IONode};
    use crate::simulation::splittable_network::Network;

    #[test]
    fn from_container() {
        let io_network = IONetwork::from_file("./assets/equil-network.xml");
        let (split_networks, id_mappings) = Network::split_from_container(&io_network, 2, split);

        assert_eq!(split_networks.len(), 2);

        let first = split_networks.get(0).unwrap();
        assert_eq!(first.nodes.len(), 8);
        assert_eq!(first.links.len(), 17);

        let second = split_networks.get(1).unwrap();
        assert_eq!(second.nodes.len(), 7);
        assert_eq!(second.links.len(), 16);

        assert_eq!(id_mappings.link_2_thread.len(), 23);
        assert_eq!(id_mappings.node_2_thread.len(), 15);
    }

    fn split(node: &IONode) -> usize {
        let node_group_1 = vec!["1", "2", "3", "4", "5", "6", "7", "15"];
        if node_group_1.contains(&node.id.as_str()) {
            0
        } else {
            1
        }
    }
}
