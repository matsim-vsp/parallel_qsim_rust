#[cfg(test)]
mod tests {
    use std::fmt::Debug;

    use crate::parallel_simulation::routing::network_converter::NetworkConverter;
    use crate::parallel_simulation::routing::router::Router;
    use rust_road_router::algo::customizable_contraction_hierarchy::{customize, CCH};
    use rust_road_router::datastr::node_order::NodeOrder;
    use rust_road_router::{
        algo::{
            dijkstra::{query::dijkstra::Server as DijkServer, *},
            *,
        },
        datastr::graph::*,
    };

    fn create_graph() -> OwnedGraph {
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

    fn created_graph_with_isolated_node_0() -> OwnedGraph {
        OwnedGraph::new(
            vec![0, 0, 2, 4, 6],
            vec![2, 3, 2, 3, 1, 2],
            vec![1, 2, 1, 4, 2, 5],
        )
    }

    #[test]
    fn test_simple_dijkstra() {
        let mut server = DijkServer::<_, DefaultOps>::new(create_graph());
        let mut result = server.query(Query { from: 2, to: 1 });
        assert_eq!(result.distance(), Some(3));
        println!("{:#?}", result.node_path());
    }

    #[test]
    fn test_simple_dijkstra_with_single_node() {
        let mut server = DijkServer::<_, DefaultOps>::new(created_graph_with_isolated_node_0());
        let mut result = server.query(Query { from: 3, to: 2 });
        assert_eq!(result.distance(), Some(3));
        println!("{:#?}", result.node_path());
    }

    #[test]
    fn test_simple_cch() {
        let node_order = NodeOrder::from_node_order(vec![2, 3, 1, 0]);
        let graph = &created_graph_with_isolated_node_0();
        let cch = CCH::fix_order_and_build(graph, node_order);

        let mut server =
            customizable_contraction_hierarchy::query::Server::new(customize(&cch, graph));
        let mut result = server.query(Query { from: 3, to: 2 });
        assert_eq!(result.distance(), Some(3));
        println!("{:#?}", result.node_path())
    }

    //#[ignore]
    #[test]
    fn test_simple_cch_with_router_and_update() {
        //does only work locally
        let network =
            NetworkConverter::convert_xml_network("./assets/routing_tests/triangle-network.xml");

        let cch = Router::perform_preprocessing(&network, "./output/");
        let mut router = Router::new(&cch, &network);

        let res12 = router.query(1, 2);
        test_query_result(res12, 1, vec![1, 2]);
        let res32 = router.query(3, 2);
        test_query_result(res32, 3, vec![3, 1, 2]);

        println!("Assign new travel time to edge 1-2: 4");

        let new_owned_graph = OwnedGraph::new(
            network.first_out().to_owned(),
            network.head().to_owned(),
            vec![4, 2, 1, 4, 2, 5],
        );
        router.customize(&cch, &new_owned_graph);
        let new_result = router.query(3, 2);
        test_query_result(new_result, 5, vec![3, 2]);
    }

    fn test_query_result<P: PathServer>(
        mut result: QueryResult<P, u32>,
        distance: u32,
        expected_path: Vec<u32>,
    ) where
        <P as PathServer>::NodeInfo: Debug,
        <P as PathServer>::NodeInfo: PartialEq<u32>,
    {
        assert_eq!(result.distance().unwrap(), distance);
        let result_path = result.node_path().unwrap();
        println!("Got path {:#?}", result_path);
        assert_eq!(result_path, expected_path);
    }
}
