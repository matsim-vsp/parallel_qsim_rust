#[cfg(test)]
mod tests {
    use fast_paths::InputGraph;

    #[test]
    fn test_simple_graph() {
        let mut graph = InputGraph::new();
        graph.add_edge(0, 1, 1);
        graph.add_edge(0, 2, 2);
        graph.add_edge(1, 1, 1);
        graph.add_edge(1, 2, 4);
        graph.add_edge(2, 0, 2);

        graph.freeze();
        let fast_graph = fast_paths::prepare(&graph);

        //NOT CCH. Ordering of nodes takes number of inserted shortcuts into account.
        let node_ordering = fast_graph.get_node_ordering();

        let shortest_path = fast_paths::calc_path(&fast_graph, 2, 1);
        match shortest_path {
            Some(p) => {
                let weight = p.get_weight();
                assert_eq!(weight, 3);
                let nodes = p.get_nodes();
                println!("Weight was {}, Path was {:#?}", weight, nodes);
            }
            None => {}
        }
    }
}