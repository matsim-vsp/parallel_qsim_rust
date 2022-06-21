use crate::container::network::{IOLink, IONetwork, IONode};
use crate::simulation::flow_cap::Flowcap;
use std::collections::{HashMap, VecDeque};
use std::hash::Hash;

use crate::simulation::q_vehicle::QVehicle;

pub struct Network<'a> {
    pub links: HashMap<usize, Link>,
    pub nodes: HashMap<usize, Node>,
    pub link_id_mapping: HashMap<&'a str, usize>,
    pub node_id_mapping: HashMap<&'a str, usize>,
}

impl<'a> Network<'a> {
    fn new() -> Self {
        Self {
            links: HashMap::new(),
            nodes: HashMap::new(),
            link_id_mapping: HashMap::new(),
            node_id_mapping: HashMap::new(),
        }
    }

    pub fn split_from_container(
        container: &IONetwork,
        size: usize,
        splitter: fn(&IONode) -> usize,
    ) -> Vec<Network> {
        // create the result networks which can then be populated
        let mut result = Vec::with_capacity(size);
        let mut global_node_id_mapping: HashMap<&str, usize> = HashMap::new();
        let mut id_2_thread: HashMap<usize, usize> = HashMap::new();

        for i in 0..size {
            result.push(Network::new());
        }

        let mut next_id = 0;
        for node in container.nodes() {
            let thread_id = splitter(node);
            let network = result.get_mut(thread_id).unwrap();

            network.add_node(&node.id, next_id);
            global_node_id_mapping.insert(&node.id, next_id);
            next_id = next_id + 1;
        }

        next_id = 0;
        for link in container.links() {
            let from_id = *global_node_id_mapping.get(link.from.as_str()).unwrap();
            let to_id = *global_node_id_mapping.get(link.to.as_str()).unwrap();

            let from_thread = id_2_thread.get(&from_id).unwrap();
            let to_thread = id_2_thread.get(&to_id).unwrap();

            if from_thread == to_thread {
                let network = result.get_mut(*from_thread).unwrap();
                network.add_link(link, next_id, from_id, to_id)
            } else {
                panic!(
                    "Not yet implemented. Think about what to do, if link crosses thread boundary"
                )
            }

            next_id = next_id + 1;
        }

        result
    }
    fn add_node(&mut self, node_id: &'a str, id: usize) {
        let node = Node::new(id);
        self.nodes.insert(id, node);
        self.node_id_mapping.insert(node_id, id);
    }

    fn add_link(&mut self, link: &'a IOLink, id: usize, from: usize, to: usize) {
        let new_link = Link {
            id,
            q: VecDeque::new(),
            flowcap: Flowcap::new(link.capacity),
            freespeed: link.freespeed,
            length: link.length,
        };
        self.links.insert(id, new_link);
        self.link_id_mapping.insert(&link.id, id);

        // wire up the from and to node
        let from = self.nodes.get_mut(&from).unwrap();
        from.out_links.push(id);
        let to = self.nodes.get_mut(&to).unwrap();
        to.in_links.push(id);
    }
}

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

pub struct Link {
    id: usize,
    q: VecDeque<QVehicle>,
    length: f32,
    freespeed: f32,
    flowcap: Flowcap,
}

pub struct Id {
    thread: usize,
    id: usize,
}

impl Id {
    fn new(thread: usize, id: usize) -> Id {
        Id { thread, id }
    }
}
