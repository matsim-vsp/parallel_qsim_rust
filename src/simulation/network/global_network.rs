use std::collections::HashSet;

use crate::simulation::{
    id::{Id, IdStore},
    io::network::{Attr, IOLink, IONetwork, IONode},
};

/// This is called global network but could also be renamed into network when things are sorted out a little
#[derive(Debug)]
pub struct Network<'a> {
    pub node_ids: IdStore<'a, Node>,
    pub link_ids: IdStore<'a, Link>,
    // we make sure to store each mode only once. This could be optimized further if we'd
    // cache the HashSets which we store in the links. I.e. each combination of modes is only
    // one hash set.
    modes: IdStore<'a, String>,
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
    pub out_links: Vec<Id<Link>>
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

    pub fn add_node(&mut self, io_node: IONode) {
        let id = self.node_ids.create_id(&io_node.id);
        assert_eq!(
            id.internal,
            self.nodes.len(),
            "internal id {} and slot in node vec {} were note the same. Probably, node id {} already exsists.",
            id.internal,
            self.nodes.len(),
            io_node.id
        );

        let attrs = match io_node.attributes {
            Some(attrs) => attrs.attributes,
            None => Vec::new(),
        };
        let node = Node::new(id, io_node.x, io_node.y, attrs);
        self.nodes.push(node);
    }

    pub fn add_link(&mut self, io_link: IOLink) {
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
            .split(",")
            .map(|s| s.trim())
            .map(|mode| self.modes.create_id(mode))
            .collect();
        let from_id = self.node_ids.get_from_ext(&io_link.from);
        let to_id = self.node_ids.get_from_ext(&io_link.to);

        self.nodes.get_mut(from_id.internal).unwrap().out_links.push(id.clone());
        self.nodes.get_mut(to_id.internal).unwrap().in_links.push(id.clone());

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
        self.links.push(link);
    }

    pub fn get_node(&self, id: &Id<Node>) -> &Node {
        self.nodes.get(id.internal).unwrap()
    }

    pub fn get_link(&self, id: &Id<Link>) -> &Link {
        self.links.get(id.internal).unwrap()
    }
}

impl<'a> From<IONetwork> for Network<'a> {
    fn from(io_network: IONetwork) -> Self {
        let mut result = Network::new();

        for node in io_network.nodes.nodes {
            result.add_node(node)
        }

        for link in io_network.links.links {
            result.add_link(link)
        }

        result
    }
}

impl Node {
    fn new(id: Id<Node>, x: f32, y: f32, attrs: Vec<Attr>) -> Self {
        Node { id, x, y, attrs, in_links: Vec::new(), out_links: Vec::new() }
    }
}

impl Link {
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
        }
    }
}

#[cfg(test)]
mod tests {

    use crate::simulation::io::network::{IOLink, IONode};

    use super::Network;

    #[test]
    fn add_node() {
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

        network.add_node(io_node);

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
    #[should_panic]
    fn add_node_reject_duplicate() {
        let external_id = String::from("some-id");
        let x = 1.;
        let y = 2.;
        let io_node = IONode {
            id: external_id.clone(),
            x,
            y,
            attributes: None,
        };
        let io_node_duplicate = IONode {
            id: external_id.clone(),
            x,
            y,
            attributes: None,
        };
        let mut network = Network::new();

        network.add_node(io_node);
        network.add_node(io_node_duplicate);
    }

    #[test]
    fn add_link() {
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
        network.add_node(io_from);
        network.add_node(io_to);
        network.add_link(io_link.clone());

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
}
