use crate::io::network::IOLink;
use crate::parallel_simulation::network::link::{Link, LocalLink, SplitInLink, SplitOutLink};
use crate::parallel_simulation::network::node::Node;
use std::collections::{HashMap, HashSet};
use std::fmt::Debug;

#[derive(Debug)]
pub struct NetworkPartition<V: Debug> {
    pub links: HashMap<usize, Link<V>>,
    pub nodes: HashMap<usize, Node>,
}

impl<V: Debug> NetworkPartition<V> {
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

    pub fn add_local_link(
        &mut self,
        link: &IOLink,
        sample_size: f32,
        id: usize,
        from: usize,
        to: usize,
    ) {
        let new_link = LocalLink::from_io_link(id, link, sample_size);
        self.links.insert(id, Link::LocalLink(new_link));

        // wire up the from and to node
        let from = self.nodes.get_mut(&from).unwrap();
        from.add_out_link(id);
        let to = self.nodes.get_mut(&to).unwrap();
        to.add_in_link(id);
    }

    pub fn add_split_out_link(&mut self, id: usize, from: usize, to_part: usize) {
        let new_link = SplitOutLink::new(id, to_part);
        self.links.insert(id, Link::SplitOutLink(new_link));

        // wire up from node
        let from_node = self.nodes.get_mut(&from).unwrap();
        from_node.add_out_link(id);
    }

    pub fn add_split_in_link(
        &mut self,
        link: &IOLink,
        sample_size: f32,
        id: usize,
        to: usize,
        from_part: usize,
    ) {
        let local_link = LocalLink::from_io_link(id, link, sample_size);
        let new_link = SplitInLink::new(from_part, local_link);

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

#[cfg(test)]
mod tests {
    use crate::io::network::IOLink;
    use crate::parallel_simulation::network::network_partition::NetworkPartition;
    use crate::parallel_simulation::vehicles::Vehicle;

    /// create a partition with one node which has multiple in and out links
    #[test]
    fn neighbors() {
        let mut network_part: NetworkPartition<Vehicle> = NetworkPartition::new();
        let node_id = 1;
        let io_link = IOLink::default();
        network_part.add_node(node_id);

        // add split links. make sure partitions have multiple connections because the method
        // should return each neighbour partition only once.

        // this partition has incoming links from partition 1 and 2
        network_part.add_split_in_link(&io_link, 1., 1, node_id, 1);
        network_part.add_split_in_link(&io_link, 1., 2, node_id, 1);
        network_part.add_split_in_link(&io_link, 1., 3, node_id, 2);

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
