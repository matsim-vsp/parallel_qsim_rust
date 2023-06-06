use crate::simulation::id::IdImpl;
use std::collections::btree_map::Values;
use std::collections::{BTreeMap, HashSet};
use std::fmt::Debug;

use crate::simulation::io::network::IOLink;
use crate::simulation::network::link::{LocalLink, SimLink, SplitInLink, SplitOutLink};
use crate::simulation::network::node::Node;

#[derive(Debug, Clone)]
pub struct NetworkPartition {
    pub links: BTreeMap<usize, SimLink>,
    pub nodes: BTreeMap<usize, Node>,
}

impl NetworkPartition {
    pub fn new() -> Self {
        Self {
            links: BTreeMap::new(),
            nodes: BTreeMap::new(),
        }
    }

    pub fn len_local_nodes(&self) -> usize {
        self.nodes
            .values()
            .filter(|n| match n {
                Node::LocalNode(_) => true,
                _ => false,
            })
            .count()
    }

    pub fn get_local_nodes(nodes: Values<usize, Node>) -> Vec<&Node> {
        nodes
            .filter(|n| match n {
                Node::LocalNode(_) => true,
                _ => false,
            })
            .collect()
    }

    pub fn add_local_node(&mut self, id: usize, x: f32, y: f32) {
        let node = Node::new_local_node(id, x, y);
        self.nodes.insert(id, node);
    }

    pub fn add_neighbour_node(&mut self, id: usize, x: f32, y: f32) {
        let node = Node::new_neighbour_node(id, x, y);
        self.nodes.insert(id, node);
    }

    pub fn add_local_link(
        &mut self,
        link: &IOLink,
        sample_size: f32,
        id: usize,
        from: usize,
        to: usize,
    ) {
        let new_link = LocalLink::from_io_link(id, link, sample_size, from, to);
        self.links.insert(id, SimLink::LocalLink(new_link));

        // wire up the from and to node
        let from = self.nodes.get_mut(&from).unwrap();
        from.add_out_link(id);
        let to = self.nodes.get_mut(&to).unwrap();
        to.add_in_link(id);
    }

    pub fn add_split_out_link(&mut self, id: usize, from: usize, to_part: usize) {
        let wrapped_id = IdImpl::new_internal(id);
        let new_link = SplitOutLink::new(wrapped_id, to_part);
        self.links.insert(id, SimLink::SplitOutLink(new_link));

        // wire up from node
        let from_node = self.nodes.get_mut(&from).unwrap();
        from_node.add_out_link(id);
    }

    pub fn add_split_in_link(
        &mut self,
        link: &IOLink,
        sample_size: f32,
        id: usize,
        from: usize,
        to: usize,
        from_part: usize,
    ) {
        let local_link = LocalLink::from_io_link(id, link, sample_size, from, to);
        let new_link = SplitInLink::new(from_part, local_link);

        self.links.insert(id, SimLink::SplitInLink(new_link));

        // wire up to node
        let to_node = self.nodes.get_mut(&to).unwrap();
        to_node.add_in_link(id);
    }

    pub fn neighbors(&self) -> HashSet<usize> {
        let distinct_thread_ids: HashSet<usize> = self
            .links
            .values()
            .filter(|link| match link {
                SimLink::LocalLink(_) => false,
                SimLink::SplitInLink(_) => true,
                SimLink::SplitOutLink(_) => true,
            })
            .map(|link| match link {
                SimLink::LocalLink(_) => panic!("Should be filtered."),
                SimLink::SplitInLink(link) => link.neighbor_partition_id(),
                SimLink::SplitOutLink(link) => link.neighbor_partition_id(),
            })
            .collect();

        distinct_thread_ids
    }
}

#[cfg(test)]
mod tests {
    use crate::simulation::io::network::IOLink;
    use crate::simulation::network::network_partition::NetworkPartition;

    /// create a partition with one node which has multiple in and out links
    #[test]
    fn neighbors() {
        let mut network_part: NetworkPartition = NetworkPartition::new();
        let node_id = 1;
        let io_link = IOLink::default();
        network_part.add_local_node(node_id, 0., 0.);

        // add split links. make sure partitions have multiple connections because the method
        // should return each neighbour partition only once.

        // this partition has incoming links from partition 1 and 2
        network_part.add_split_in_link(&io_link, 1., 1, 0, node_id, 1);
        network_part.add_split_in_link(&io_link, 1., 2, 0, node_id, 1);
        network_part.add_split_in_link(&io_link, 1., 3, 0, node_id, 2);

        // this partition has outgoing links to partition 2, 3 and 4
        network_part.add_split_out_link(4, node_id, 2);
        network_part.add_split_out_link(5, node_id, 3);
        network_part.add_split_out_link(6, node_id, 3);
        network_part.add_split_out_link(7, node_id, 4);

        let neighbors = network_part.neighbors();
        assert_eq!(4, neighbors.len());
        assert!(neighbors.contains(&1));
        assert!(neighbors.contains(&2));
        assert!(neighbors.contains(&3));
        assert!(neighbors.contains(&4));
    }
}
