use rust_road_router::algo::customizable_contraction_hierarchy::{CCH, customize, CustomizedBasic};
use rust_road_router::algo::customizable_contraction_hierarchy::query::Server;
use rust_road_router::datastr::graph::OwnedGraph;
use rust_road_router::datastr::node_order::NodeOrder;

use crate::routing::network_converter::NetworkConverter;

pub struct Router<'router> {
    pub(crate) server: Server<CustomizedBasic<'router, CCH>>,
}

impl<'router> Router<'router> {
    pub(crate) fn new(cch: &'router CCH, graph: &OwnedGraph) -> Router<'router> {
        Router {
            server: Server::new(customize(cch, graph))
        }
    }

    pub(crate) fn customize(&mut self, cch: &'router CCH, graph: &OwnedGraph) {
        self.server = Server::new(customize(cch, graph));
    }

    pub(crate) fn create_cch(converter: &mut NetworkConverter) -> CCH {
        converter.convert_network();
        let owned_graph = Router::create_owned_graph(converter);
        let node_order_vec = converter.node_ordering(false);
        assert_eq!(node_order_vec, vec![2, 3, 1, 0]);
        let node_order = NodeOrder::from_node_order(node_order_vec);
        CCH::fix_order_and_build(&owned_graph, node_order)
    }

    pub(crate) fn create_owned_graph(converter: &NetworkConverter) -> OwnedGraph {
        OwnedGraph::new(converter.routing_kit_network.as_ref().unwrap().first_out().to_owned(),
                        converter.routing_kit_network.as_ref().unwrap().head().to_owned(),
                        converter.routing_kit_network.as_ref().unwrap().travel_time().to_owned())
    }
}