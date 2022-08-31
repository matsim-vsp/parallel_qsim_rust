use crate::io::matsim_id::MatsimId;
use crate::io::network::{IOLink, IONetwork, IONode};
use crate::parallel_simulation::id_mapping::MatsimIdMappings;
use crate::parallel_simulation::network::network_partition::NetworkPartition;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug)]
pub struct Network {
    pub partitions: Vec<NetworkPartition>,
    pub nodes_2_thread: Arc<HashMap<usize, usize>>,
    pub links_2_thread: Arc<HashMap<usize, usize>>,
}

pub struct MutNetwork {
    pub partitions: Vec<NetworkPartition>,
    pub nodes_2_thread: HashMap<usize, usize>,
    pub links_2_thread: HashMap<usize, usize>,
}

impl Network {
    fn from_mut_network(network: MutNetwork) -> Network {
        Network {
            partitions: network.partitions,
            nodes_2_thread: Arc::new(network.nodes_2_thread),
            links_2_thread: Arc::new(network.links_2_thread),
        }
    }

    pub fn from_io<F>(
        io_network: &IONetwork,
        num_part: usize,
        split: F,
        id_mappings: &MatsimIdMappings,
    ) -> Network
    where
        F: Fn(&IONode) -> usize,
    {
        let mut result = MutNetwork::new(num_part);

        for node in io_network.nodes() {
            result.add_node(node, id_mappings, &split);
        }

        for link in io_network.links() {
            result.add_link(link, id_mappings);
        }

        Network::from_mut_network(result)
    }

    pub fn get_thread_for_node(&self, node_id: &usize) -> &usize {
        self.nodes_2_thread.get(node_id).unwrap()
    }

    pub fn get_thread_for_link(&self, link_id: &usize) -> &usize {
        self.links_2_thread.get(link_id).unwrap()
    }
}

impl MutNetwork {
    fn new(num_parts: usize) -> MutNetwork {
        let mut partitions = Vec::with_capacity(num_parts);
        for _ in 0..num_parts {
            partitions.push(NetworkPartition::new());
        }

        MutNetwork {
            partitions,
            nodes_2_thread: HashMap::new(),
            links_2_thread: HashMap::new(),
        }
    }

    fn add_node<F>(&mut self, node: &IONode, id_mappings: &MatsimIdMappings, split: F)
    where
        F: Fn(&IONode) -> usize,
    {
        let thread = split(node);
        let node_id = *id_mappings.nodes.get_internal(node.id()).unwrap();
        let network = self.partitions.get_mut(thread).unwrap();
        network.add_node(node_id);

        self.nodes_2_thread.insert(node_id, thread);
    }

    fn add_link(&mut self, io_link: &IOLink, id_mappings: &MatsimIdMappings) {
        let link_id = *id_mappings.links.get_internal(io_link.id()).unwrap();
        let from_id = *id_mappings
            .nodes
            .get_internal(io_link.from.as_str())
            .unwrap();
        let to_id = *id_mappings.nodes.get_internal(io_link.to.as_str()).unwrap();
        let from_thread = *self.get_thread_for_node(&from_id);
        let to_thread = *self.get_thread_for_node(&to_id);
        let to_network = self.partitions.get_mut(to_thread).unwrap();

        if from_thread == to_thread {
            to_network.add_local_link(io_link, link_id, from_id, to_id);
        } else {
            to_network.add_split_in_link(io_link, link_id, to_id);

            let from_network = self.partitions.get_mut(from_thread).unwrap();
            from_network.add_split_out_link(link_id, from_id, from_thread, to_thread);
        }
        // the link is associated with the network which contains its to-node
        self.links_2_thread.insert(link_id, to_thread);
    }

    fn get_thread_for_node(&self, node_id: &usize) -> &usize {
        self.nodes_2_thread.get(node_id).unwrap()
    }

    fn get_thread_for_link(&self, link_id: &usize) -> &usize {
        self.links_2_thread.get(link_id).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::Network;
    use crate::io::network::{IONetwork, IONode};
    use crate::io::population::IOPopulation;
    use crate::parallel_simulation::id_mapping::MatsimIdMappings;
    use crate::parallel_simulation::network::link::Link;
    use std::collections::HashSet;

    /// This splits the network into 2 parts
    ///                  |
    /// 0----------0-----|-----0----------0
    ///                  |
    #[test]
    fn split_3link_network() {
        let io_network = IONetwork::from_file("./assets/3-links/3-links-network.xml");
        let io_population = IOPopulation::empty();
        let id_mappings = MatsimIdMappings::from_io(&io_network, &io_population);
        let split = |node: &IONode| match node.id.as_str() {
            "node1" => 0,
            "node2" => 0,
            _ => 1,
        };
        let network = Network::from_io(&io_network, 2, split, &id_mappings);
        assert_eq!(2, network.partitions.len());

        let partition1 = network.partitions.get(0).unwrap();
        assert!(partition1
            .nodes
            .contains_key(id_mappings.nodes.get_internal("node1").unwrap()));
        assert!(partition1
            .nodes
            .contains_key(id_mappings.nodes.get_internal("node2").unwrap()));
        assert!(partition1
            .links
            .contains_key(id_mappings.links.get_internal("link1").unwrap()));
        assert!(partition1
            .links
            .contains_key(id_mappings.links.get_internal("link2").unwrap()));
        let link1 = partition1
            .links
            .get(id_mappings.links.get_internal("link1").unwrap())
            .unwrap();
        assert!(matches!(link1, Link::LocalLink(_)));
        let link2 = partition1
            .links
            .get(id_mappings.links.get_internal("link2").unwrap())
            .unwrap();
        assert!(matches!(link2, Link::SplitLink(_)));

        let partition2 = network.partitions.get(1).unwrap();
        assert!(partition2
            .nodes
            .contains_key(id_mappings.nodes.get_internal("node3").unwrap()));
        assert!(partition2
            .nodes
            .contains_key(id_mappings.nodes.get_internal("node4").unwrap()));
        assert!(partition2
            .links
            .contains_key(id_mappings.links.get_internal("link2").unwrap()));
        assert!(partition2
            .links
            .contains_key(id_mappings.links.get_internal("link3").unwrap()));
        let link2 = partition2
            .links
            .get(id_mappings.links.get_internal("link2").unwrap())
            .unwrap();
        assert!(matches!(link2, Link::LocalLink(_)));
        let link3 = partition2
            .links
            .get(id_mappings.links.get_internal("link3").unwrap())
            .unwrap();
        assert!(matches!(link3, Link::LocalLink(_)));
    }

    /// This splits the network into 3 parts, so that we have neighbours and none neighbours
    ///       |                      |
    /// 0-----|-----0----------0-----|-----0
    ///       |                      |
    #[test]
    fn neighbour_node_ids() {
        let io_network = IONetwork::from_file("./assets/3-links/3-links-network.xml");
        let io_population = IOPopulation::empty();
        let id_mappings = MatsimIdMappings::from_io(&io_network, &io_population);
        let split = |node: &IONode| match node.id.as_str() {
            "node1" => 0, // left
            "node4" => 2, // right
            _ => 1,       // center
        };
        let network = Network::from_io(&io_network, 3, split, &id_mappings);

        assert_eq!(3, network.partitions.len());

        let assert_neighbours =
            |expected_thread_ids: HashSet<usize>, actual_thread_ids: HashSet<usize>| {
                assert_eq!(expected_thread_ids.len(), actual_thread_ids.len());
            };

        for (i, partition) in network.partitions.iter().enumerate() {
            match i {
                0 => assert_neighbours(HashSet::from([1]), partition.neighbour_node_ids()),
                1 => assert_neighbours(HashSet::from([0, 2]), partition.neighbour_node_ids()),
                _ => assert_neighbours(HashSet::from([1]), partition.neighbour_node_ids()),
            };
        }
    }
}
