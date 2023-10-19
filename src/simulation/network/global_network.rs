use std::str::FromStr;
use std::{collections::HashSet, path::Path};

use nohash_hasher::IntSet;

use crate::simulation::io::attributes::{Attr, Attrs};
use crate::simulation::vehicles::garage::Garage;
use crate::simulation::{
    id::{Id, IdStore},
    io::network::{IOLink, IONetwork, IONode},
};

use super::metis_partitioning;

/// This is called global network but could also be renamed into network when things are sorted out a little
#[derive(Debug)]
pub struct Network<'a> {
    pub node_ids: IdStore<'a, Node>,
    pub link_ids: IdStore<'a, Link>,
    pub nodes: Vec<Node>,
    pub links: Vec<Link>,
    pub effective_cell_size: f32,
}

#[derive(Debug)]
pub struct Node {
    pub x: f32,
    pub y: f32,
    pub id: Id<Node>,
    pub attrs: Vec<Attr>,
    pub in_links: Vec<Id<Link>>,
    pub out_links: Vec<Id<Link>>,
    pub partition: u32,
}

#[derive(Debug)]
pub struct Link {
    pub id: Id<Link>,
    pub from: Id<Node>,
    pub to: Id<Node>,
    pub length: f32,
    pub capacity: f32,
    pub freespeed: f32,
    pub permlanes: f32,
    pub modes: IntSet<Id<String>>,
    pub attributes: Vec<Attr>,
    pub partition: u32,
}

impl<'a> Default for Network<'a> {
    fn default() -> Self {
        Network::new()
    }
}

impl<'a> Network<'a> {
    pub fn new() -> Self {
        Network {
            node_ids: IdStore::new(),
            link_ids: IdStore::new(),
            nodes: Vec::new(),
            links: Vec::new(),
            effective_cell_size: 7.5,
        }
    }

    pub fn from_file(
        file_path: &str,
        num_parts: u32,
        partition_method: &str,
        garage: &mut Garage,
    ) -> Self {
        let io_network = IONetwork::from_file(file_path);
        let mut result = Network::new();
        Self::init_nodes_and_links(&mut result, io_network, garage);
        Self::partition_network(&mut result, partition_method, num_parts);
        result
    }

    pub fn to_file(&self, file_path: &Path) {
        let mut result = IONetwork::new(None);

        for node in &self.nodes {
            let attributes = Attrs {
                attributes: vec![Attr {
                    name: String::from("partition"),
                    value: node.partition.to_string(),
                    class: String::from("java.lang.Integer"),
                }],
            };
            let io_node = IONode {
                //id: node.id.external().clone(),
                id: node.id.internal().to_string(), // todo replace this with external id, once all output is written using external ids
                x: node.x,
                y: node.y,
                attributes: Some(attributes),
            };
            result.nodes_mut().push(io_node);
        }

        for link in &self.links {
            let modes = link
                .modes
                .iter()
                .map(|m| m.external().to_string())
                .reduce(|modes, mode| format!("{modes},{mode}"))
                .unwrap();
            let attributes = Attrs {
                attributes: vec![Attr {
                    name: String::from("partition"),
                    value: link.partition.to_string(),
                    class: String::from("java.lang.Integer"),
                }],
            };

            let io_link = IOLink {
                //id: link.id.external().clone(),
                id: link.id.internal().to_string(), // todo replace with external id again, once all output translates to external ids
                //from: link.from.external().clone(),
                from: link.from.internal().to_string(),
                //to: link.to.external().clone(),
                to: link.to.internal().to_string(),
                length: link.length,
                capacity: link.capacity,
                freespeed: link.freespeed,
                permlanes: link.permlanes,
                modes,
                attributes: Some(attributes),
            };
            result.links.effective_cell_size = Some(self.effective_cell_size);
            result.links_mut().push(io_link);
        }

        result.to_file(file_path);
    }

    pub fn add_node(&mut self, node: Node) {
        assert_eq!(
            node.id.internal(),
            self.nodes.len(),
            "internal id {} and slot in node vec {} were note the same. Probably, node id {} already exsists.",
            node.id.internal(),
            self.nodes.len(),
            node.id.external()
        );
        self.nodes.push(node);
    }

    pub fn add_io_node(&mut self, io_node: IONode) {
        let id = self.node_ids.create_id(&io_node.id);
        let part_attr = Attrs::find_or_else_opt(&io_node.attributes, "partition", || "0");
        let partition = u32::from_str(part_attr).unwrap();
        let attrs = match io_node.attributes {
            Some(attrs) => attrs.attributes,
            None => Vec::new(),
        };

        let mut node = Node::new(id, io_node.x, io_node.y);
        node.attrs = attrs;
        node.partition = partition;
        self.add_node(node);
    }

    pub fn add_link(&mut self, link: Link) {
        assert_eq!(
            link.id.internal(),
            self.links.len(),
            "internal id {} and slot in link vec {} were note the same. Probably, this link id {} already exists",
            link.id.internal(),
            self.links.len(),
            link.id.external()
        );

        // wire up in and out links and push link to the links vec
        self.nodes
            .get_mut(link.from.internal())
            .unwrap()
            .out_links
            .push(link.id.clone());
        self.nodes
            .get_mut(link.to.internal())
            .unwrap()
            .in_links
            .push(link.id.clone());
        self.links.push(link);
    }

    pub fn add_io_link(&mut self, io_link: IOLink, garage: &mut Garage) {
        let id = self.link_ids.create_id(&io_link.id);
        assert_eq!(
            id.internal(),
            self.links.len(),
            "internal id {} and slot in link vec {} were note the same. Probably, this link id already exists",
            id.internal(),
            self.links.len()
        );
        let part_attr = Attrs::find_or_else_opt(&io_link.attributes, "partition", || "0");
        let partition = u32::from_str(part_attr).unwrap();
        let attrs = match io_link.attributes {
            Some(attrs) => attrs.attributes,
            None => Vec::new(),
        };
        let modes: IntSet<Id<String>> = io_link
            .modes
            .split(',')
            .map(|s| s.trim())
            .map(|mode| garage.modes.create_id(mode))
            .collect();
        let from_id = self.node_ids.get_from_ext(&io_link.from);
        let to_id = self.node_ids.get_from_ext(&io_link.to);

        let mut link = Link::new(
            id,
            from_id,
            to_id,
            io_link.length,
            io_link.capacity,
            io_link.freespeed,
            io_link.permlanes,
            modes,
            attrs,
        );
        link.partition = partition;
        self.add_link(link);
    }

    pub fn get_node(&self, id: &Id<Node>) -> &Node {
        self.nodes.get(id.internal()).unwrap()
    }

    pub fn get_link(&self, id: &Id<Link>) -> &Link {
        self.links.get(id.internal()).unwrap()
    }

    fn init_nodes_and_links(network: &mut Network, io_network: IONetwork, garage: &mut Garage) {
        for node in io_network.nodes.nodes {
            network.add_io_node(node)
        }

        for link in io_network.links.links {
            network.add_io_link(link, garage)
        }
    }

    fn partition_network(network: &mut Network, partition_method: &str, num_parts: u32) {
        if partition_method.eq("metis") {
            let partitions = metis_partitioning::partition(network, num_parts);
            for node in network.nodes.iter_mut() {
                let partition = partitions[node.id.internal()] as u32;
                node.partition = partition;

                for link_id in &node.in_links {
                    let link = network.links.get_mut(link_id.internal()).unwrap();
                    link.partition = partition;
                }
            }
        } else if partition_method.eq("none") {
            return;
        } else {
            panic!("Unknown partition method: {}", partition_method);
        }
    }
}

impl Node {
    pub fn new(id: Id<Node>, x: f32, y: f32) -> Self {
        Node {
            id,
            x,
            y,
            attrs: Vec::new(),
            in_links: Vec::new(),
            out_links: Vec::new(),
            partition: 0,
        }
    }
}

impl Link {
    #[allow(clippy::too_many_arguments)]
    fn new(
        id: Id<Link>,
        from: Id<Node>,
        to: Id<Node>,
        length: f32,
        capacity: f32,
        freespeed: f32,
        permlanes: f32,
        modes: IntSet<Id<String>>,
        attributes: Vec<Attr>,
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
            attributes,
            partition: 0,
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
            Vec::default(),
        )
    }
}

#[cfg(test)]
mod tests {
    use crate::simulation::io::network::{IOLink, IONode};
    use crate::simulation::vehicles::garage::Garage;

    use super::{Link, Network, Node};

    #[test]
    fn add_node() {
        let mut network = Network::new();
        let id = network.node_ids.create_id("node-id");
        let node = Node::new(id.clone(), 1., 1.);

        assert_eq!(0, network.nodes.len());
        network.add_node(node);
        assert_eq!(1, network.nodes.len());
        assert_eq!(id, network.get_node(&id).id);
    }

    #[test]
    #[should_panic]
    fn add_node_reject_duplicate() {
        let mut network = Network::new();
        let id = network.node_ids.create_id("node-id");
        let node = Node::new(id.clone(), 1., 1.);
        let duplicate = Node::new(id.clone(), 2., 2.);

        assert_eq!(0, network.nodes.len());
        network.add_node(node);
        network.add_node(duplicate); // expecting panic here.
    }

    #[test]
    fn add_link() {
        let mut network = Network::new();
        let from = Node::new(network.node_ids.create_id("from"), 0., 0.);
        let to = Node::new(network.node_ids.create_id("to"), 3., 4.);
        let id = network.link_ids.create_id("link-id");
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
        let from = Node::new(network.node_ids.create_id("from"), 0., 0.);
        let to = Node::new(network.node_ids.create_id("to"), 3., 4.);
        let id = network.link_ids.create_id("link-id");
        let link = Link::new_with_default(id.clone(), &from, &to);
        let duplicate = Link::new_with_default(id.clone(), &from, &to);

        network.add_node(from);
        network.add_node(to);
        network.add_link(link);
        network.add_link(duplicate); // expecting panic here
    }

    #[test]
    fn add_io_node() {
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

        network.add_io_node(io_node);

        // the node should be in nodes vec and there should be a node id
        let id = network.node_ids.get_from_ext(&external_id);
        assert_eq!(0, id.internal());
        assert_eq!(external_id, id.external());

        let node = network.get_node(&id);
        assert_eq!(x, node.x);
        assert_eq!(y, node.y);
        assert_eq!(id, node.id);
    }

    #[test]
    fn add_io_link() {
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

        let mut garage = Garage::new();
        let mut network = Network::new();
        network.add_io_node(io_from);
        network.add_io_node(io_to);
        network.add_io_link(io_link.clone(), &mut garage);

        let from = network.get_node(&network.node_ids.get_from_ext(&ext_from_id));
        let to = network.get_node(&network.node_ids.get_from_ext(&ext_to_id));
        let link = network.get_link(&network.link_ids.get_from_ext(&ext_link_id));

        assert_eq!(from.id, link.from);
        assert_eq!(to.id, link.to);
        assert_eq!(ext_link_id, link.id.external());
        assert_eq!(io_link.length, link.length);
        assert_eq!(io_link.capacity, link.capacity);
        assert_eq!(io_link.freespeed, link.freespeed);
        assert_eq!(io_link.permlanes, link.permlanes);

        assert!(link.modes.contains(&garage.modes.get_from_ext("car")));
        assert!(link.modes.contains(&garage.modes.get_from_ext("ride")));
        assert!(link.modes.contains(&garage.modes.get_from_ext("bike")));
    }

    #[test]
    fn from_file() {
        let mut garage = Garage::default();
        let network =
            Network::from_file("./assets/equil/equil-network.xml", 2, "metis", &mut garage);

        // check partitioning
        let expected_partitions = [0, 0, 0, 1, 1, 1, 1, 1, 1, 0, 0, 1, 0, 0, 0];
        for node in &network.nodes {
            let expected_partition = expected_partitions[node.id.internal()];
            assert_eq!(expected_partition, node.partition);
        }
        for link in &network.links {
            let expected_partition = expected_partitions[link.to.internal()];
            assert_eq!(expected_partition, link.partition);
        }

        // probe in and out links
        for node in &network.nodes {
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
        let mut network = Network::new();
        let from = Node::new(network.node_ids.create_id("from"), 0., 0.);
        let to = Node::new(network.node_ids.create_id("to"), 3., 4.);
        let id = network.link_ids.create_id("link-id");
        let link = Link::new_with_default(id.clone(), &from, &to);

        assert_eq!(id, link.id);
        assert_eq!(5., link.length);
        assert_eq!(from.id, link.from);
        assert_eq!(to.id, link.to);
    }
}
