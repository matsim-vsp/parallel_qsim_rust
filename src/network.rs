use std::collections::HashMap;

use petgraph::Graph;
use petgraph::graph::{EdgeIndex, NodeIndex};

/**
The ids are owned strings at the moment. My suspicion is that this needs to be replaced with Rc<String>
Or an id implementation of the same type to allow passing around these ids without always cloning.
 */
struct Node {
    id: String,
    coord: Coord,
}

struct Link {
    id: String,
    freespeed: f32,
    // ommit other parameters for later
    from_id: String,
    to_id: String,
}

struct Coord {
    x: f32,
    y: f32,
}

struct Network {
    graph: Graph<Node, Link>,
    link_ids_2_index: HashMap<String, EdgeIndex>,
    node_ids_2_index: HashMap<String, NodeIndex>,
}

impl Network {
    fn new() -> Network {
        Network {
            graph: Graph::new(),
            link_ids_2_index: HashMap::new(),
            node_ids_2_index: HashMap::new(),
        }
    }

    fn add_node(&mut self, node: Node) {
        let node_id = node.id.clone();
        let node_index = self.graph.add_node(node);
        self.node_ids_2_index.insert(node_id, node_index);
    }

    fn get_node(&self, id: &str) -> &Node {
        let node_index = self.node_ids_2_index.get(id).expect(&format!("No Node for id: {}", id));
        self.graph.node_weight(node_index.clone()).unwrap()
    }

    fn add_link(&mut self, link: Link) {
        let link_id = link.id.clone();
        let from_index = self.node_ids_2_index.get(&link.from_id)
            .expect(&format!("Node with id {} must be inserted first.", link.from_id));
        let to_index = self.node_ids_2_index.get(&link.to_id)
            .expect(&format!("Node with id {} must be inserted first.", link.to_id));
        let link_index = self.graph.add_edge(from_index.clone(), to_index.clone(), link);
        self.link_ids_2_index.insert(link_id, link_index);
    }

    fn get_link(&self, id: &str) -> &Link {
        let link_index = self.link_ids_2_index.get(id).expect(&format!("No Link for id: {}", id));
        self.graph.edge_weight(link_index.clone()).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use crate::network::{Coord, Link, Network, Node};

    #[test]
    fn add_and_get_node() {
        let mut network = Network::new();
        let node = Node { id: String::from("node-id"), coord: Coord { x: 1.0, y: 1.0 } };

        network.add_node(node);
        let result = network.get_node("node-id");

        println!("node with id: {} and x: {} and y {}", result.id, result.coord.x, result.coord.y)
    }

    #[test]
    fn add_and_get_link() {
        let mut network = Network::new();
        let from = Node { id: String::from("from-node"), coord: Coord { x: 1.0, y: 1.0 } };
        let to = Node { id: String::from("to-node"), coord: Coord { x: 1.0, y: 1.0 } };
        let link = Link {
            id: String::from("link-id"),
            freespeed: 0.0,
            from_id: from.id.clone(),
            to_id: to.id.clone(),
        };

        network.add_node(from);
        network.add_node(to);
        network.add_link(link);

        let result = network.get_link("link-id");

        println!("Link with id: {} and freespeed: {} and from_id: {} and to_id: {}", result.id, result.freespeed, result.from_id, result.to_id);
    }

    #[test]
    fn add_multiple_nodes_and_links() {
        let mut network = Network::new();

        for i in 0..99 {
            let node = Node { id: String::from(i.to_string()), coord: Coord { x: i as f32, y: i as f32 } };
            network.add_node(node);
        }

        for i in 1..98 {
            let to_id = i - 1;
            let from_id = i + 1;
            let link = Link { id: format!("link-{}", i), from_id: String::from(from_id.to_string()), to_id: String::from(to_id.to_string()), freespeed: 100.6 };
            network.add_link(link);
        }

        println!("Nodes:");
        for index in network.graph.node_indices() {
            let node = network.graph.node_weight(index).unwrap();
            print!("{}, ", node.id)
        }

        println!("\nLinks: ");
        for index in network.graph.edge_indices() {
            let link = network.graph.edge_weight(index).unwrap();
            print!("{}, ", link.id)
        }
    }
}