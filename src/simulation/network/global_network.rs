use std::{collections::HashSet, path::Path};

use crate::simulation::io::attributes::{Attr, Attrs};
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
    // we make sure to store each mode only once. This could be optimized further if we'd
    // cache the HashSets which we store in the links. I.e. each combination of modes is only
    // one hash set.
    pub modes: IdStore<'a, String>,
    pub nodes: Vec<Node>,
    pub links: Vec<Link>,
}

#[derive(Debug)]
pub struct Node {
    pub x: f32,
    pub y: f32,
    pub id: Id<Node>,
    pub attrs: Vec<Attr>,
    pub in_links: Vec<Id<Link>>,
    pub out_links: Vec<Id<Link>>,
    pub partition: usize,
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
    pub modes: HashSet<Id<String>>,
    pub attributes: Vec<Attr>,
    pub partition: usize,
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
            modes: IdStore::new(),
            nodes: Vec::new(),
            links: Vec::new(),
        }
    }

    pub fn from_file(file_path: &str, num_parts: usize) -> Self {
        let io_network = IONetwork::from_file(file_path);
        let mut result = Network::new();
        Self::init_nodes_and_links(&mut result, io_network);
        Self::partition_network(&mut result, num_parts);
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
                id: node.id.external.clone(),
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
                .map(|m| m.external.clone())
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
                id: link.id.external.clone(),
                from: link.from.external.clone(),
                to: link.to.external.clone(),
                length: link.length,
                capacity: link.capacity,
                freespeed: link.freespeed,
                permlanes: link.permlanes,
                modes,
                attributes: Some(attributes),
            };
            result.links_mut().push(io_link);
        }

        result.to_file(file_path);
    }

    pub fn add_node(&mut self, node: Node) {
        assert_eq!(
            node.id.internal,
            self.nodes.len(),
            "internal id {} and slot in node vec {} were note the same. Probably, node id {} already exsists.",
            node.id.internal,
            self.nodes.len(),
            node.id.external
        );
        self.nodes.push(node);
    }

    pub fn add_io_node(&mut self, io_node: IONode) {
        let id = self.node_ids.create_id(&io_node.id);
        let attrs = match io_node.attributes {
            Some(attrs) => attrs.attributes,
            None => Vec::new(),
        };
        let mut node = Node::new(id, io_node.x, io_node.y);
        node.attrs = attrs;
        self.add_node(node);
    }

    pub fn add_link(&mut self, link: Link) {
        assert_eq!(
            link.id.internal,
            self.links.len(),
            "internal id {} and slot in link vec {} were note the same. Probably, this link id {} already exists",
            link.id.internal,
            self.links.len(),
            link.id.external
        );

        // wire up in and out links and push link to the links vec
        self.nodes
            .get_mut(link.from.internal)
            .unwrap()
            .out_links
            .push(link.id.clone());
        self.nodes
            .get_mut(link.to.internal)
            .unwrap()
            .in_links
            .push(link.id.clone());
        self.links.push(link);
    }

    pub fn add_io_link(&mut self, io_link: IOLink) {
        let id = self.link_ids.create_id(&io_link.id);
        assert_eq!(
            id.internal,
            self.links.len(),
            "internal id {} and slot in link vec {} were note the same. Probably, this link id already exists",
            id.internal,
            self.links.len()
        );

        let attrs = match io_link.attributes {
            Some(attrs) => attrs.attributes,
            None => Vec::new(),
        };
        let modes: HashSet<Id<String>> = io_link
            .modes
            .split(',')
            .map(|s| s.trim())
            .map(|mode| self.modes.create_id(mode))
            .collect();
        let from_id = self.node_ids.get_from_ext(&io_link.from);
        let to_id = self.node_ids.get_from_ext(&io_link.to);

        let link = Link::new(
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
        self.add_link(link);
    }

    pub fn get_node(&self, id: &Id<Node>) -> &Node {
        self.nodes.get(id.internal).unwrap()
    }

    pub fn get_link(&self, id: &Id<Link>) -> &Link {
        self.links.get(id.internal).unwrap()
    }

    fn init_nodes_and_links(network: &mut Network, io_network: IONetwork) {
        for node in io_network.nodes.nodes {
            network.add_io_node(node)
        }

        for link in io_network.links.links {
            network.add_io_link(link)
        }
    }

    fn partition_network(network: &mut Network, num_parts: usize) {
        let partitions = metis_partitioning::partition(network, num_parts);
        println!("{partitions:?}");
        for node in network.nodes.iter_mut() {
            let partition = partitions[node.id.internal] as usize;
            node.partition = partition;

            for link_id in &node.in_links {
                let link = network.links.get_mut(link_id.internal).unwrap();
                link.partition = partition;
            }
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
        modes: HashSet<Id<String>>,
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
        assert_eq!(0, id.internal);
        assert_eq!(external_id, id.external);

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

        let mut network = Network::new();
        network.add_io_node(io_from);
        network.add_io_node(io_to);
        network.add_io_link(io_link.clone());

        let from = network.get_node(&network.node_ids.get_from_ext(&ext_from_id));
        let to = network.get_node(&network.node_ids.get_from_ext(&ext_to_id));
        let link = network.get_link(&network.link_ids.get_from_ext(&ext_link_id));

        assert_eq!(from.id, link.from);
        assert_eq!(to.id, link.to);
        assert_eq!(ext_link_id, link.id.external);
        assert_eq!(io_link.length, link.length);
        assert_eq!(io_link.capacity, link.capacity);
        assert_eq!(io_link.freespeed, link.freespeed);
        assert_eq!(io_link.permlanes, link.permlanes);

        assert!(link.modes.contains(&network.modes.get_from_ext("car")));
        assert!(link.modes.contains(&network.modes.get_from_ext("ride")));
        assert!(link.modes.contains(&network.modes.get_from_ext("bike")));
    }

    #[test]
    fn from_file() {
        let network = Network::from_file("./assets/equil/equil-network.xml", 2);

        // check partitioning
        let expected_partitions = [0, 0, 0, 1, 1, 1, 1, 1, 1, 0, 0, 1, 0, 0, 0];
        for node in &network.nodes {
            let expected_partition = expected_partitions[node.id.internal];
            assert_eq!(expected_partition, node.partition);
        }
        for link in &network.links {
            let expected_partition = expected_partitions[link.to.internal];
            assert_eq!(expected_partition, link.partition);
        }

        // probe in and out links
        for node in &network.nodes {
            match &node.id.internal {
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
