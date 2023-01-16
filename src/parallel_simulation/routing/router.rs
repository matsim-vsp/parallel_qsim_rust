use crate::parallel_simulation::network::routing_kit_network::RoutingKitNetwork;
use crate::parallel_simulation::routing::inertial_flow_cutter_adapter::InertialFlowCutterAdapter;
use crate::parallel_simulation::routing::network_converter::NetworkConverter;
use geo::Point;
use geo::{Closest, EuclideanDistance};
use rust_road_router::algo::customizable_contraction_hierarchy::query::{
    PathServerWrapper, Server,
};
use rust_road_router::algo::customizable_contraction_hierarchy::{customize, CustomizedBasic, CCH};
use rust_road_router::algo::{Query, QueryResult, QueryServer};
use rust_road_router::datastr::graph::{NodeId, OwnedGraph, Weight};
use rust_road_router::datastr::node_order::NodeOrder;
use std::env;

pub struct Router<'router> {
    pub(crate) server: Server<CustomizedBasic<'router, CCH>>,
    network: RoutingKitNetwork,
}

impl<'router> Router<'router> {
    pub(crate) fn new(cch: &'router CCH, network: &RoutingKitNetwork) -> Router<'router> {
        Router {
            server: Server::new(customize(cch, &Router::create_owned_graph(&network))),
            network: network.clone(),
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

    pub(crate) fn query_coordinates<'q>(
        &'q mut self,
        x_from: f32,
        y_from: f32,
        x_to: f32,
        y_to: f32,
    ) -> CustomQueryResult {
        let network = self.network();
        let mut result: QueryResult<PathServerWrapper<'q, CustomizedBasic<'router, CCH>>, Weight> =
            self.query(
                self.find_nearest_node(x_from, y_from),
                self.find_nearest_node(x_to, y_to),
            );
        let edge_path = result
            .node_path()
            .map(|node_path| get_edge_path(node_path, network));
        CustomQueryResult {
            travel_time: result.distance(),
            path: edge_path,
        }
    }

    pub(crate) fn perform_preprocessing(
        network: &RoutingKitNetwork,
        temp_output_folder: &str,
    ) -> CCH {
        let owned_graph = Router::create_owned_graph(network);

        let inertial_flow_cutter_path = env::var("INERTIAL_FLOW_CUTTER_HOME_DIRECTORY")
            .expect("The environment variable 'INERTIAL_FLOW_CUTTER_HOME_DIRECTORY' is not set.");

        // step 1: compute node ordering
        let node_order_vec = InertialFlowCutterAdapter::new(
            network,
            inertial_flow_cutter_path.as_str(),
            temp_output_folder,
        )
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

    fn find_nearest_node(&self, x: f32, y: f32) -> usize {
        let point = Point::new(x, y);

        let network_points = self
            .network
            .longitude
            .iter()
            .zip(self.network.latitude.iter());

        network_points
            .map(|(long, lat)| point.euclidean_distance(&Point::new(*long, *lat)))
            .enumerate()
            .min_by(|(_, a), (_, b)| a.total_cmp(b))
            .map(|(index, _)| index)
            .unwrap()
    }

    fn network(&self) -> RoutingKitNetwork {
        //TODO ugly?
        self.network.clone()
    }
}

fn get_edge_path(path: Vec<NodeId>, network: RoutingKitNetwork) -> Vec<usize> {
    let mut res = Vec::new();
    let mut last_node: Option<usize> = None;
    for node in path {
        match last_node {
            None => last_node = Some(node as usize),
            Some(n) => {
                let first_out_index = *network.first_out.get(n).unwrap() as usize;
                let last_out_index = (network.first_out.get(n + 1).unwrap() - 1) as usize;
                res.push(find_edge_id_of_outgoing(
                    first_out_index,
                    last_out_index,
                    node,
                    &network,
                ));
                last_node = Some(node as usize)
            }
        }
    }
    res
}

fn find_edge_id_of_outgoing(
    first_out_index: usize,
    last_out_index: usize,
    next_node: NodeId,
    network: &RoutingKitNetwork,
) -> usize {
    assert!(last_out_index - first_out_index >= 0, "No outgoing edges!");
    let mut result = 0;
    for i in first_out_index..=last_out_index {
        if *network.head.get(i).unwrap() == next_node {
            result = network.link_ids.get(i).unwrap().clone();
            break;
        }
        panic!("No outgoing edge found!");
    }
    result
}

pub struct CustomQueryResult {
    pub travel_time: Option<u32>,
    pub path: Option<Vec<usize>>,
}

#[cfg(test)]
mod test {
    //TODO move router tests here
}
