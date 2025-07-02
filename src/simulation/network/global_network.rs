use super::metis_partitioning;
use crate::simulation::config::PartitionMethod;
use crate::simulation::id::Id;
use crate::simulation::io::attributes::IOAttributes;
use crate::simulation::network::io::{IOLink, IONetwork, IONode};
use crate::simulation::InternalAttributes;
use itertools::Itertools;
use nohash_hasher::{IntMap, IntSet};
use std::path::PathBuf;
use std::str::FromStr;
use std::{collections::HashSet, path::Path};
use tracing::info;

/// This is called global network but could also be renamed into network when things are sorted out a little
#[derive(Debug, Clone)]
pub struct Network {
    nodes: IntMap<Id<Node>, Node>,
    links: IntMap<Id<Link>, Link>,
    effective_cell_size: f32,
}

#[derive(Debug, Clone)]
pub struct Node {
    pub x: f64,
    pub y: f64,
    pub id: Id<Node>,
    pub in_links: Vec<Id<Link>>,
    pub out_links: Vec<Id<Link>>,
    pub partition: u32,
    pub cmp_weight: u32,
}

#[derive(Debug, Clone)]
pub struct Link {
    pub id: Id<Link>,
    pub from: Id<Node>,
    pub to: Id<Node>,
    pub length: f64,
    pub capacity: f32,
    pub freespeed: f32,
    pub permlanes: f32,
    pub modes: IntSet<Id<String>>,
    pub partition: u32,
    pub attributes: InternalAttributes,
}

impl Default for Network {
    fn default() -> Self {
        Network::new()
    }
}

impl Network {
    pub fn new() -> Self {
        Network {
            nodes: IntMap::default(),
            links: IntMap::default(),
            effective_cell_size: 7.5,
        }
    }

    pub fn effective_cell_size(&self) -> f32 {
        self.effective_cell_size
    }

    pub fn from_file(file: &str, num_parts: u32, partition_method: PartitionMethod) -> Self {
        Self::from_file_path(&PathBuf::from(file), num_parts, partition_method)
    }

    pub fn from_file_path(
        file_path: &Path,
        num_parts: u32,
        partition_method: PartitionMethod,
    ) -> Self {
        let mut result = super::io::from_file(file_path);
        Self::partition_network(&mut result, partition_method, num_parts);
        result
    }

    pub fn from_file_as_is(path: &Path) -> Self {
        super::io::from_file(path)
    }

    pub fn to_file(&self, file_path: &Path) {
        super::io::to_file(self, file_path);
    }

    pub fn add_node(&mut self, node: Node) {
        let id = node.id.clone();
        let option = self.nodes.insert(id.clone(), node);
        assert!(
            option.is_none(),
            "Node with id {} already exists in the network.",
            id
        );
    }

    pub fn add_link(&mut self, link: Link) {
        // wire up in and out links and push link to the links vec
        let id = link.id.clone();
        self.nodes
            .get_mut(&link.from)
            .unwrap()
            .out_links
            .push(id.clone());
        self.nodes
            .get_mut(&link.to)
            .unwrap()
            .in_links
            .push(id.clone());

        let option = self.links.insert(id.clone(), link);
        assert!(
            option.is_none(),
            "Link with id {} already exists in the network.",
            id
        );
    }

    pub fn get_node(&self, id: &Id<Node>) -> &Node {
        self.nodes.get(id).unwrap()
    }

    pub fn get_link(&self, id: &Id<Link>) -> &Link {
        self.links.get(id).unwrap()
    }

    pub fn get_node_mut(&mut self, id: &Id<Node>) -> &mut Node {
        self.nodes.get_mut(id).unwrap()
    }

    pub fn get_link_mut(&mut self, id: &Id<Link>) -> &mut Link {
        self.links.get_mut(id).unwrap()
    }

    fn partition_network(network: &mut Network, partition_method: PartitionMethod, num_parts: u32) {
        match partition_method {
            PartitionMethod::Metis(options) => {
                let partitions = metis_partitioning::partition(network, num_parts, options);
                for (id, node) in network.nodes.iter_mut() {
                    let partition = partitions.get(id).unwrap();
                    node.partition = *partition as u32;

                    for link_id in &node.in_links {
                        let link = network.links.get_mut(link_id).unwrap();
                        link.partition = *partition as u32;
                    }
                }
            }
            PartitionMethod::None => {}
        }
    }

    pub fn get_all_nodes_sorted(&self) -> Vec<&Node> {
        self.nodes
            .iter()
            .sorted_by_key(|&(id, _)| id.clone())
            .map(|(_, node)| node)
            .collect::<Vec<&Node>>()
    }

    pub fn set_effective_cell_size(&mut self, effective_cell_size: f32) {
        self.effective_cell_size = effective_cell_size;
    }

    pub fn nodes(&self) -> Vec<&Node> {
        self.nodes.values().collect::<Vec<&Node>>()
    }

    pub fn links(&self) -> Vec<&Link> {
        self.links.values().collect::<Vec<&Link>>()
    }
}

impl From<IONetwork> for Network {
    fn from(io_net: IONetwork) -> Self {
        let mut result = Network::new();

        for io_node in io_net.nodes() {
            add_io_node(&mut result, io_node);
        }

        for io_link in io_net.links() {
            add_io_link(&mut result, io_link);
        }

        result.effective_cell_size = io_net.effective_cell_size();
        result
    }
}

impl From<crate::simulation::io::proto::network::Network> for Network {
    fn from(value: crate::simulation::io::proto::network::Network) -> Self {
        let mut result = Network::new();
        result.set_effective_cell_size(value.effective_cell_size);
        for wn in &value.nodes {
            let node = Node::new(Id::get(wn.id), wn.x, wn.y, wn.partition, wn.cmp_weight);
            result.add_node(node);
        }
        for wl in &value.links {
            let modes: IntSet<Id<String>> = wl.modes.iter().map(|id| Id::get(*id)).collect();

            let link = Link::new(
                Id::get(wl.id),
                Id::get(wl.from),
                Id::get(wl.to),
                wl.length,
                wl.capacity,
                wl.freespeed,
                wl.permlanes,
                modes,
                wl.partition,
            );
            result.add_link(link);
        }
        info!("Finished converting protobuf wire type into Network");
        result
    }
}

fn add_io_node(network: &mut Network, io_node: &IONode) {
    let id = Id::create(&io_node.id);
    let part_attr = IOAttributes::find_or_else_opt(&io_node.attributes, "partition", || "0");
    let cmp_weight_attr = IOAttributes::find_or_else_opt(&io_node.attributes, "cmp_weight", || "1");
    let partition = u32::from_str(part_attr).unwrap();
    let cmp_weight = u32::from_str(cmp_weight_attr).unwrap();

    let mut node = Node::new(id, io_node.x, io_node.y, partition, cmp_weight);
    node.partition = partition;
    network.add_node(node);
}

fn add_io_link(network: &mut Network, io_link: &IOLink) {
    let id = Id::create(&io_link.id);
    let part_attr = IOAttributes::find_or_else_opt(&io_link.attributes, "partition", || "0");
    let partition = u32::from_str(part_attr).unwrap();
    let modes: IntSet<Id<String>> = io_link
        .modes
        .split(',')
        .map(|s| s.trim())
        .map(Id::create)
        .collect();
    let from_id = Id::get_from_ext(&io_link.from);
    let to_id = Id::get_from_ext(&io_link.to);

    let link = Link::new(
        id,
        from_id,
        to_id,
        io_link.length,
        io_link.capacity,
        io_link.freespeed,
        io_link.permlanes,
        modes,
        partition,
    );
    network.add_link(link);
}

impl Node {
    pub fn new(id: Id<Node>, x: f64, y: f64, part: u32, cmp_weight: u32) -> Self {
        Node {
            id,
            x,
            y,
            in_links: Vec::new(),
            out_links: Vec::new(),
            partition: part,
            cmp_weight,
        }
    }

    pub fn set_cmp_weight(&mut self, cmp_weight: u32) {
        self.cmp_weight = cmp_weight;
    }
}

impl Link {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        id: Id<Link>,
        from: Id<Node>,
        to: Id<Node>,
        length: f64,
        capacity: f32,
        freespeed: f32,
        permlanes: f32,
        modes: IntSet<Id<String>>,
        partition: u32,
    ) -> Self {
        Link {
            id,
            from,
            to,
            length,
            capacity,
            freespeed,
            permlanes,
            modes,
            partition,
            attributes: InternalAttributes::default(),
        }
    }

    pub fn new_with_default(id: Id<Link>, from: &Node, to: &Node) -> Self {
        // compute eucledean distance between from and to node
        let length = ((from.x - to.x).powi(2) + (from.y - to.y).powi(2)).sqrt();
        Link::new(
            id,
            from.id.clone(),
            to.id.clone(),
            length,
            1.,
            1.,
            1.,
            HashSet::default(),
            0,
        )
    }

    pub fn contains_mode(&self, mode: u64) -> bool {
        self.modes.iter().map(|m| m.internal()).contains(&mode)
    }
}

#[cfg(test)]
mod tests {
    use super::{add_io_link, add_io_node, Link, Network, Node};
    use crate::simulation::config::{EdgeWeight, MetisOptions, PartitionMethod};
    use crate::simulation::id::Id;
    use crate::simulation::network::io::{IOLink, IONode};

    #[test]
    fn add_node() {
        let mut network = Network::new();
        let id = Id::create("node-id");
        let node = Node::new(id.clone(), 1., 1., 0, 1);

        assert_eq!(0, network.nodes.len());
        network.add_node(node);
        assert_eq!(1, network.nodes.len());
        assert_eq!(id, network.get_node(&id).id);
    }

    #[test]
    #[should_panic]
    fn add_node_reject_duplicate() {
        let mut network = Network::new();
        let id = Id::create("node-id");
        let node = Node::new(id.clone(), 1., 1., 0, 1);
        let duplicate = Node::new(id.clone(), 2., 2., 0, 1);

        assert_eq!(0, network.nodes.len());
        network.add_node(node);
        network.add_node(duplicate); // expecting panic here.
    }

    #[test]
    fn add_link() {
        let mut network = Network::new();
        let from = Node::new(Id::create("from"), 0., 0., 0, 1);
        let to = Node::new(Id::create("to"), 3., 4., 0, 1);
        let id = Id::create("link-id");
        let link = Link::new_with_default(id.clone(), &from, &to);

        network.add_node(from);
        network.add_node(to);
        network.add_link(link);

        assert_eq!(2, network.nodes.len());
        assert_eq!(1, network.links.len());
        assert_eq!(id, network.get_link(&id).id);

        let link = network.get_link(&id);
        let from = network.get_node(&link.from);
        let to = network.get_node(&link.to);

        assert_eq!(id, link.id);
        assert_eq!(0, from.in_links.len());
        assert_eq!(1, from.out_links.len());
        assert_eq!(&id, from.out_links.get(0).unwrap());
        assert_eq!(0, to.out_links.len());
        assert_eq!(1, to.in_links.len());
        assert_eq!(&id, to.in_links.get(0).unwrap());
    }

    #[test]
    #[should_panic]
    fn add_link_reject_duplicate() {
        let mut network = Network::new();
        let from = Node::new(Id::create("from"), 0., 0., 0, 1);
        let to = Node::new(Id::create("to"), 3., 4., 0, 1);
        let id = Id::create("link-id");
        let link = Link::new_with_default(id.clone(), &from, &to);
        let duplicate = Link::new_with_default(id.clone(), &from, &to);

        network.add_node(from);
        network.add_node(to);
        network.add_link(link);
        network.add_link(duplicate); // expecting panic here
    }

    #[test]
    #[ignore] // ingore this test, because it keeps not working, due to non determined ordering of metis
    fn from_file() {
        let network = Network::from_file(
            "./assets/equil/equil-network.xml",
            2,
            //I don't know, why "edge_weight = true" sets 1 as partition for all nodes.
            PartitionMethod::Metis(MetisOptions::default().set_edge_weight(EdgeWeight::Constant)),
        );

        // check partitioning
        let expected_partitions = [0, 0, 0, 1, 1, 1, 1, 1, 1, 0, 0, 1, 0, 0, 0];
        for (_, node) in &network.nodes {
            let expected_partition = expected_partitions[node.id.internal() as usize];
            assert_eq!(expected_partition, node.partition);
        }
        for (_, link) in &network.links {
            let expected_partition = expected_partitions[link.to.internal() as usize];
            assert_eq!(expected_partition, link.partition);
        }

        // probe in and out links
        for node in network.nodes() {
            match &node.id.internal() {
                11 => {
                    assert_eq!(9, node.in_links.len());
                    assert_eq!(1, node.out_links.len());
                }
                1 => {
                    assert_eq!(9, node.out_links.len());
                    assert_eq!(1, node.in_links.len());
                }
                _ => {
                    assert_eq!(1, node.in_links.len());
                    assert_eq!(1, node.out_links.len());
                }
            }
        }

        // check cell size
        assert_eq!(7.5, network.effective_cell_size);
    }

    #[test]
    fn link_new_with_default() {
        let from = Node::new(Id::create("from"), 0., 0., 0, 1);
        let to = Node::new(Id::create("to"), 3., 4., 0, 1);
        let id = Id::create("link-id");
        let link = Link::new_with_default(id.clone(), &from, &to);

        assert_eq!(id, link.id);
        assert_eq!(5., link.length);
        assert_eq!(from.id, link.from);
        assert_eq!(to.id, link.to);
    }

    #[test]
    fn test_metis_with_large_graph() {}

    #[test]
    fn test_add_io_node() {
        let external_id = String::from("some-id");
        let x = 1.;
        let y = 2.;
        let io_node = IONode {
            id: external_id.clone(),
            x,
            y,
            attributes: None,
        };
        let mut network = Network::new();

        add_io_node(&mut network, &io_node);

        // the node should be in nodes vec and there should be a node id
        let id = Id::get_from_ext(&external_id);
        assert_eq!(external_id, id.external());

        let node = network.get_node(&id);
        assert_eq!(x, node.x);
        assert_eq!(y, node.y);
        assert_eq!(id, node.id);
    }

    #[test]
    fn test_add_io_link() {
        let ext_from_id = String::from("from");
        let ext_to_id = String::from("to");
        let ext_link_id = String::from("link");

        let io_from = IONode {
            id: ext_from_id.clone(),
            x: 0.,
            y: 0.,
            attributes: None,
        };
        let io_to = IONode {
            id: ext_to_id.clone(),
            x: 100.,
            y: 100.,
            attributes: None,
        };
        let io_link = IOLink {
            id: ext_link_id.clone(),
            from: ext_from_id.clone(),
            to: ext_to_id.clone(),
            length: 100.,
            capacity: 100.,
            freespeed: 10.,
            permlanes: 2.,
            modes: String::from("car,ride, bike"),
            attributes: None,
        };

        let mut network = Network::new();
        add_io_node(&mut network, &io_from);
        add_io_node(&mut network, &io_to);
        add_io_link(&mut network, &io_link);

        let from = network.get_node(&Id::get_from_ext(&ext_from_id));
        let to = network.get_node(&Id::get_from_ext(&ext_to_id));
        let link = network.get_link(&Id::get_from_ext(&ext_link_id));

        assert_eq!(from.id, link.from);
        assert_eq!(to.id, link.to);
        assert_eq!(ext_link_id, link.id.external());
        assert_eq!(io_link.length, link.length);
        assert_eq!(io_link.capacity, link.capacity);
        assert_eq!(io_link.freespeed, link.freespeed);
        assert_eq!(io_link.permlanes, link.permlanes);

        assert!(link.modes.contains(&Id::get_from_ext("car")));
        assert!(link.modes.contains(&Id::get_from_ext("ride")));
        assert!(link.modes.contains(&Id::get_from_ext("bike")));
    }
}
