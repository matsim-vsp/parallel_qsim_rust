use std::borrow::Borrow;
use std::cell::{Ref, RefCell};
use std::rc::Rc;

use rust_road_router::algo::{customizable_contraction_hierarchy, Query, QueryResult, QueryServer};
use rust_road_router::algo::customizable_contraction_hierarchy::{CCH, customize, Customized, CustomizedBasic};
use rust_road_router::algo::customizable_contraction_hierarchy::query::{PathServerWrapper, Server};
use rust_road_router::datastr::graph::{EdgeId, FirstOutGraph, OwnedGraph, Weight};
use rust_road_router::datastr::node_order::NodeOrder;

use crate::io::network::IONetwork;
use crate::routing::network_converter::{NetworkConverter, node_ordering_from_matsim_network, RoutingKitNetwork};

struct Router<'router> {
    server: Server<CustomizedBasic<'router, CCH>>,
}

impl<'router> Router<'router> {
    fn new(cch: &'router CCH, graph: &OwnedGraph) -> Router<'router> {
        Router {
            server: Server::new(customize(cch, graph))
        }
    }

    fn customize(&mut self, cch: &'router CCH, graph: &OwnedGraph) {
        self.server = Server::new(customize(cch, graph));
    }
}

#[cfg(test)]
mod tests {
    use rust_road_router::{
        algo::{*, dijkstra::{*, query::dijkstra::Server as DijkServer}},
        datastr::graph::*,
    };
    use rust_road_router::algo::customizable_contraction_hierarchy::{CCH, customize};
    use rust_road_router::datastr::node_order::NodeOrder;

    use crate::experiments::routing::rust_road_router::Router;
    use crate::routing::network_converter::NetworkConverter;

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
        OwnedGraph::new(vec![0, 0, 2, 4, 6], vec![2, 3, 2, 3, 1, 2], vec![1, 2, 1, 4, 2, 5])
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
        assert_eq!(result.distance(), Some(5));
        println!("{:#?}", result.node_path());
    }

    #[test]
    fn test_simple_cch() {
        let node_order = NodeOrder::from_node_order(vec![2, 3, 1, 0]);
        let graph = &created_graph_with_isolated_node_0();
        let cch = CCH::fix_order_and_build(graph, node_order);

        let mut server = customizable_contraction_hierarchy::query::Server::new(customize(&cch, graph));
        let mut result = server.query(Query { from: 3, to: 2 });
        assert_eq!(result.distance(), Some(5));
        println!("{:#?}", result.node_path())
    }

    #[ignore]
    #[test]
    fn test_simple_cch_with_router() {
        //does only work locally
        let mut converter = NetworkConverter {
            matsim_network_path: "./assets/routing_tests/triangle-network.xml",
            output_path: "./assets/routing_tests/conversion/",
            inertial_flow_cutter_path: "../InertialFlowCutter",
            routing_kit_network: None,
        };
        converter.convert_network();
        let owned_graph =
            OwnedGraph::new(converter.routing_kit_network.as_ref().unwrap().first_out().to_owned(),
                            converter.routing_kit_network.as_ref().unwrap().head().to_owned(),
                            converter.routing_kit_network.as_ref().unwrap().travel_time().to_owned());

        let node_order_vec = converter.node_ordering(false);
        assert_eq!(node_order_vec, vec![2, 3, 1, 0]);
        let node_order = NodeOrder::from_node_order(node_order_vec);
        let cch = CCH::fix_order_and_build(&owned_graph, node_order);

        let mut server = customizable_contraction_hierarchy::query::Server::new(customize(&cch, &owned_graph));
        let mut result = server.query(Query { from: 3, to: 2 });
        assert_eq!(result.distance().unwrap(), 3);
        let path = result.node_path().unwrap();
        assert_eq!(path, vec![3, 1, 2]);
        println!("Path is {:#?}", path);

        println!("Assign new travel time to edge 1-2: 4");

        let new_owned_graph =
            OwnedGraph::new(converter.routing_kit_network.as_ref().unwrap().first_out().to_owned(),
                            converter.routing_kit_network.as_ref().unwrap().head().to_owned(),
                            vec![4, 2, 1, 4, 2, 5]);

        let mut new_server = customizable_contraction_hierarchy::query::Server::new(customize(&cch, &new_owned_graph));
        let mut new_result = new_server.query(Query { from: 3, to: 2 });
        assert_eq!(new_result.distance().unwrap(), 5);
        let new_path = new_result.node_path().unwrap();
        println!("New path is {:#?}", new_path);
        assert_eq!(new_path, vec![3, 2]);
    }
}