use std::collections::HashSet;

use crate::simulation::{
    id::{Id, IdStore},
    io::network::{Attr, IOLink, IONode},
};

/// This is called global network but could also be renamed into network when things are sorted out a little
pub struct Network<'a> {
    node_ids: IdStore<'a, Node>,
    link_ids: IdStore<'a, Link>,
    // we make sure to store each mode only once. This could be optimized further if we'd
    // cache the HashSets which we store in the links. I.e. each combination of modes is only
    // one hash set. 
    modes: IdStore<'a, String>,
    nodes: Vec<Node>,
    links: Vec<Link>,
}

pub struct Node {
    x: f32,
    y: f32,
    id: Id<Node>,
    attrs: Vec<Attr>,
}

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
            "internal id {} and slot in node vec {} were note the same",
            id.internal,
            self.nodes.len()
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
            "internal id {} and slot in link vec {} were note the same",
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
        let from = self.node_ids.get_from_ext(&io_link.from);
        let to = self.node_ids.get_from_ext(&io_link.to);

        let link = Link::new(
            id,
            from,
            to,
            io_link.length,
            io_link.capacity,
            io_link.freespeed,
            io_link.permlanes,
            modes,
            attrs,
        );
        self.links.push(link);
    }
}

impl Node {
    fn new(id: Id<Node>, x: f32, y: f32, attrs: Vec<Attr>) -> Self {
        Node { id, x, y, attrs }
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
