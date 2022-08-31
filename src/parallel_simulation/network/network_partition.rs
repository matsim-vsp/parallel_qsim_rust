use crate::io::network::IOLink;
use crate::parallel_simulation::network::link::{Link, LocalLink, SplitLink};
use crate::parallel_simulation::network::node::Node;
use std::collections::{HashMap, HashSet};

#[derive(Debug)]
pub struct NetworkPartition {
    pub links: HashMap<usize, Link>,
    pub nodes: HashMap<usize, Node>,
}

impl NetworkPartition {
    pub fn new() -> Self {
        Self {
            links: HashMap::new(),
            nodes: HashMap::new(),
        }
    }

    pub fn add_node(&mut self, id: usize) {
        let node = Node::new(id);
        self.nodes.insert(id, node);
    }

    pub fn add_local_link(&mut self, link: &IOLink, id: usize, from: usize, to: usize) {
        let new_link = LocalLink::from_io_link(id, link);
        self.links.insert(id, Link::LocalLink(new_link));

        // wire up the from and to node
        let from = self.nodes.get_mut(&from).unwrap();
        from.add_out_link(id);
        let to = self.nodes.get_mut(&to).unwrap();
        to.add_in_link(id);
    }

    pub fn add_split_out_link(
        &mut self,
        id: usize,
        from: usize,
        from_thread: usize,
        to_thread: usize,
    ) {
        let new_link = SplitLink::new(id, from_thread, to_thread);
        self.links.insert(id, Link::SplitLink(new_link));

        // wire up from node
        let from_node = self.nodes.get_mut(&from).unwrap();
        from_node.add_out_link(id);
    }

    pub fn add_split_in_link(&mut self, link: &IOLink, id: usize, to: usize) {
        let new_link = LocalLink::from_io_link(id, link);
        self.links.insert(id, Link::LocalLink(new_link));

        // wire up to node
        let to_node = self.nodes.get_mut(&to).unwrap();
        to_node.add_in_link(id);
    }

    pub fn neighbour_node_ids(&self) -> HashSet<usize> {
        let ids: HashSet<usize> = self
            .links
            .iter()
            .filter_map(|entry| match entry.1 {
                Link::LocalLink(_) => None,
                Link::SplitLink(link) => Some(link.to_thread_id()),
            })
            .collect();
        return ids;
    }
}
