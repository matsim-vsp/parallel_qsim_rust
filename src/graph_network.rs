struct Node {
    id: String,
    value: i32,
}

struct Link {
    id: String,
    freespeed: f32,
}


#[cfg(test)]
mod tests {
    use petgraph::{Direction, Graph};

    use crate::graph_network::{Link, Node};

    #[test]
    fn hello_graph() {
        let mut graph = Graph::<Node, Link>::new();

        let from = graph.add_node(Node { id: "from".to_string(), value: 10 });
        let to = graph.add_node(Node { id: "to".to_string(), value: 20 });
        graph.add_edge(from, to, Link { id: "link".to_string(), freespeed: 36.6 });

        for i in graph.node_indices() {
            let node_data = graph.node_weight(i).unwrap();
            println!("[id: {}, value: {}]", node_data.id, node_data.value)
        }

        for i in graph.edge_indices() {
            let edge_data = graph.edge_weight(i).unwrap();
            println!("Link: [id: {}, freespeed: {}]", edge_data.id, edge_data.freespeed);
        }

        for edge_ref in graph.edges_directed(from, Direction::Outgoing) {
            let id = &edge_ref.weight().id;
            println!("{} is an outgoing link of from node", id)
        }
    }
}