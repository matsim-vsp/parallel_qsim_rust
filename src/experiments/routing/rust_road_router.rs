#[cfg(test)]
mod tests {
    use rust_road_router::{
        algo::{*, dijkstra::{*, query::dijkstra::Server as DijkServer}},
        datastr::graph::*,
    };
    use rust_road_router::algo::customizable_contraction_hierarchy::{CCH, customize};
    use rust_road_router::datastr::node_order::NodeOrder;

    fn get_graph() -> OwnedGraph {
        /*
        CSR graph representation
        first_out[n]: index in head, where to_nodes of edges from node n begin
        weight[n]: weight of edge
        Matrix: 0 1 2
              0 . 1 2
              1 . 1 4
              2 2 . .
        */
        OwnedGraph::new(vec![0, 2, 4, 5], vec![1, 2, 1, 2, 0], vec![1, 2, 1, 4, 2])
    }

    fn get_graph_single_node0() -> OwnedGraph {
        OwnedGraph::new(vec![0, 0, 2, 4, 5], vec![2, 3, 2, 3, 1], vec![1, 2, 1, 4, 2])
    }

    #[test]
    fn test_simple_dijkstra() {
        let mut server = DijkServer::<_, DefaultOps>::new(get_graph());
        let mut result = server.query(Query { from: 2, to: 1 });
        assert_eq!(result.distance(), Some(3));
        println!("{:#?}", result.node_path());
    }

    #[test]
    fn test_simple_dijkstra_with_single_node() {
        let mut server = DijkServer::<_, DefaultOps>::new(get_graph_single_node0());
        let mut result = server.query(Query { from: 3, to: 2 });
        assert_eq!(result.distance(), Some(3));
        println!("{:#?}", result.node_path());
    }

    #[test]
    fn test_simple_cch() {
        let node_order = NodeOrder::from_node_order(vec![1, 0, 2]);
        let cch = CCH::fix_order_and_build(&get_graph(), node_order);

        let mut server = customizable_contraction_hierarchy::query::Server::new(customize(&cch, &get_graph()));
        let mut result = server.query(Query { from: 2, to: 1 });
        assert_eq!(result.distance(), Some(3));
        println!("{:#?}", result.node_path())
    }
}