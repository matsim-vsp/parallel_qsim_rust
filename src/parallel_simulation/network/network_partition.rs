use crate::io::network::IOLink;
use crate::parallel_simulation::network::link::{Link, LocalLink, SplitInLink, SplitOutLink};
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

    pub fn add_split_out_link(&mut self, id: usize, from: usize, to_thread: usize) {
        let new_link = SplitOutLink::new(id, to_thread);
        self.links.insert(id, Link::SplitOutLink(new_link));

        // wire up from node
        let from_node = self.nodes.get_mut(&from).unwrap();
        from_node.add_out_link(id);
    }

    pub fn add_split_in_link(&mut self, link: &IOLink, id: usize, to: usize, from_thread: usize) {
        let local_link = LocalLink::from_io_link(id, link);
        let new_link = SplitInLink::new(from_thread, local_link);

        self.links.insert(id, Link::SplitInLink(new_link));

        // wire up to node
        let to_node = self.nodes.get_mut(&to).unwrap();
        to_node.add_in_link(id);
    }

    pub fn neighbors(&self) -> HashSet<usize> {
        let distinct_thread_ids: HashSet<usize> = self
            .links
            .values()
            .filter(|link| match link {
                Link::LocalLink(_) => false,
                Link::SplitInLink(_) => true,
                Link::SplitOutLink(_) => true,
            })
            .map(|link| match link {
                Link::LocalLink(_) => panic!("Should be filtered."),
                Link::SplitInLink(link) => link.neighbor_partition_id(),
                Link::SplitOutLink(link) => link.neighbor_partition_id(),
            })
            .collect();

        distinct_thread_ids
    }
}
