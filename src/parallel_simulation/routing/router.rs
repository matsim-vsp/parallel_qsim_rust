use crate::parallel_simulation::network::routing_kit_network::RoutingKitNetwork;
use crate::parallel_simulation::routing::inertial_flow_cutter_adapter::InertialFlowCutterAdapter;
use crate::parallel_simulation::routing::network_converter::NetworkConverter;
use rust_road_router::algo::customizable_contraction_hierarchy::query::{
    PathServerWrapper, Server,
};
use rust_road_router::algo::customizable_contraction_hierarchy::{customize, CustomizedBasic, CCH};
use rust_road_router::algo::{Query, QueryResult, QueryServer};
use rust_road_router::datastr::graph::{NodeId, OwnedGraph, Weight};
use rust_road_router::datastr::node_order::NodeOrder;

pub struct Router<'router> {
    pub(crate) server: Server<CustomizedBasic<'router, CCH>>,
}

impl<'router> Router<'router> {
    pub(crate) fn new(cch: &'router CCH, network: &RoutingKitNetwork) -> Router<'router> {
        Router {
            server: Server::new(customize(cch, &Router::create_owned_graph(&network))),
        }
    }

    pub(crate) fn customize(&mut self, cch: &'router CCH, graph: &OwnedGraph) {
        self.server = Server::new(customize(cch, graph));
    }

    pub(crate) fn query<'q>(
        &'q mut self,
        from: usize,
        to: usize,
    ) -> QueryResult<PathServerWrapper<'q, CustomizedBasic<'router, CCH>>, Weight> {
        self.server.query(Query {
            from: from as NodeId,
            to: to as NodeId,
        })
    }

    pub(crate) fn perform_preprocessing(
        network: &RoutingKitNetwork,
        inertial_flow_cutter_path: &str,
        temp_output_folder: &str,
    ) -> CCH {
        let owned_graph = Router::create_owned_graph(network);

        // step 1: compute node ordering
        let node_order_vec =
            InertialFlowCutterAdapter::new(network, inertial_flow_cutter_path, temp_output_folder)
                .node_ordering(false);
        let node_order = NodeOrder::from_node_order(node_order_vec);

        // step 2: compute customization
        CCH::fix_order_and_build(&owned_graph, node_order)
    }

    // creates a copy of RoutingKitNetwork
    pub(crate) fn create_owned_graph(routing_kit_network: &RoutingKitNetwork) -> OwnedGraph {
        OwnedGraph::new(
            routing_kit_network.first_out().to_owned(),
            routing_kit_network.head().to_owned(),
            routing_kit_network.travel_time().to_owned(),
        )
    }
}
