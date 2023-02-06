use crate::simulation::network::routing_kit_network::RoutingKitNetwork;
use crate::simulation::routing::inertial_flow_cutter_adapter::InertialFlowCutterAdapter;
use geo::EuclideanDistance;
use geo::Point;
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

    pub(crate) fn customize(&mut self, cch: &'router CCH, network: RoutingKitNetwork) {
        self.network = network;
        self.server
            .update(customize(cch, &Router::create_owned_graph(&self.network)));
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
            routing_kit_network.first_out.to_owned(),
            routing_kit_network.head.to_owned(),
            routing_kit_network.travel_time.to_owned(),
        )
    }

    pub(self) fn find_nearest_node(&self, x: f32, y: f32) -> usize {
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

pub(self) fn get_edge_path(path: Vec<NodeId>, network: RoutingKitNetwork) -> Vec<u64> {
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

pub(self) fn find_edge_id_of_outgoing(
    first_out_index: usize,
    last_out_index: usize,
    next_node: NodeId,
    network: &RoutingKitNetwork,
) -> u64 {
    //TODO this is marked as unnecessary comparison - why?
    assert!(last_out_index - first_out_index >= 0, "No outgoing edges!");
    let mut result = None;
    for i in first_out_index..=last_out_index {
        if *network.head.get(i).unwrap() == next_node {
            result = Some(network.link_ids.get(i).unwrap().clone());
            break;
        }
    }
    result.expect("No outgoing edge found!") as u64
}

pub struct CustomQueryResult {
    pub travel_time: Option<u32>,
    pub path: Option<Vec<u64>>,
}

#[cfg(test)]
mod test {
    use std::fmt::Debug;
    use std::time::Instant;

    use crate::simulation::routing::network_converter::NetworkConverter;
    use crate::simulation::routing::router::{get_edge_path, Router};
    use rand::seq::IteratorRandom;
    use rust_road_router::algo::a_star::BiDirZeroPot;
    use rust_road_router::algo::customizable_contraction_hierarchy::{customize, CCH};
    use rust_road_router::algo::dijkstra::query::bidirectional_dijkstra::Server as BidServer;
    use rust_road_router::algo::dijkstra::DefaultOps;
    use rust_road_router::algo::{
        dijkstra::{query::dijkstra::Server as DijkServer, *},
        *,
    };
    use rust_road_router::algo::{Query, QueryServer};
    use rust_road_router::datastr::graph::{NodeId, OwnedGraph};
    use rust_road_router::datastr::node_order::NodeOrder;

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

    #[test]
    fn test_find_nearest_node() {
        //nodes will be sorted by network converter by there ids
        let network =
            NetworkConverter::convert_xml_network("./assets/routing_tests/triangle-network.xml");
        let cch = Router::perform_preprocessing(&network, "./test_output/routing/nearest_node/");
        let router = Router::new(&cch, &network);

        //(17500,0) is in the middle of 0 and 1
        assert_eq!(0, router.find_nearest_node(-20000., 0.));
        assert_eq!(0, router.find_nearest_node(-17501., 0.));
        assert_eq!(0, router.find_nearest_node(-17500., 0.));
        assert_eq!(0, router.find_nearest_node(-10000000., 0.));

        assert_eq!(1, router.find_nearest_node(-15000., 0.));
        assert_eq!(1, router.find_nearest_node(-17499., 0.));

        //(-1681.5, 5128) is in the middle of 2 and 3
        assert_eq!(2, router.find_nearest_node(-865., 5925.));
        assert_eq!(2, router.find_nearest_node(-1680., 5128.));

        assert_eq!(3, router.find_nearest_node(-2498., 4331.));
        assert_eq!(3, router.find_nearest_node(-1682., 5128.));
    }

    #[test]
    fn test_get_edge_path() {
        let mut network =
            NetworkConverter::convert_xml_network("./assets/routing_tests/triangle-network.xml");
        network.link_ids = vec![0, 1, 2, 3, 4, 5];

        assert_eq!(get_edge_path(vec![1, 2, 3], network.clone()), vec![0, 3]);
        assert_eq!(get_edge_path(vec![1, 3, 2], network.clone()), vec![1, 5]);
        assert_eq!(
            get_edge_path(vec![1, 2, 3, 1, 2, 3], network.clone()),
            vec![0, 3, 4, 0, 3]
        );
    }

    #[test]
    fn test_simple_cch_with_router_and_update() {
        //does only work locally
        let network =
            NetworkConverter::convert_xml_network("./assets/routing_tests/triangle-network.xml");

        let cch =
            Router::perform_preprocessing(&network, "./test_output/routing/simple_cch_update/");
        let mut router = Router::new(&cch, &network);

        let res12 = router.query(1, 2);
        test_query_result(res12, 1, vec![1, 2]);
        let res32 = router.query(3, 2);
        test_query_result(res32, 3, vec![3, 1, 2]);

        println!("Assign new travel time to edge 1-2: 4");

        let network_new_weights =
            RoutingKitNetwork::clone_with_new_travel_times(network, vec![4, 2, 1, 4, 2, 5]);
        router.customize(&cch, network_new_weights);
        let new_result = router.query(3, 2);
        test_query_result(new_result, 5, vec![3, 2]);
    }

    #[ignore]
    #[test]
    fn compare_cch_and_dijkstra() {
        let network = NetworkConverter::convert_xml_network("./assets/andorra-network.xml.gz");

        let cch = Router::perform_preprocessing(&network, "./test_output/routing/performance/");
        let mut cch_router = Router::new(&cch, &network);

        let mut dijkstra_router =
            DijkServer::<_, DefaultOps>::new(Router::create_owned_graph(&network));

        let mut bid_dijkstra_router =
            BidServer::<OwnedGraph, OwnedGraph, BiDirZeroPot, ChooseMinKeyDir>::new(
                Router::create_owned_graph(&network),
            );

        let owned_graph = Router::create_owned_graph(&network);
        let number_of_nodes = owned_graph.first_out().len();
        let from_nodes: Vec<usize> =
            (0..number_of_nodes - 1).choose_multiple(&mut rand::thread_rng(), 1000);
        let to_nodes: Vec<usize> =
            (0..number_of_nodes - 1).choose_multiple(&mut rand::thread_rng(), 1000);

        // ugly code repetition, but the servers do not have a common parent trait :(
        println!("Starting CCH routing.");
        let mut cch_result_distances: Vec<u32> = Vec::new();
        let now = Instant::now();
        for (&from, &to) in from_nodes.iter().zip(to_nodes.iter()) {
            let cch_result = cch_router.query(from, to);
            match cch_result.distance() {
                Some(x) => cch_result_distances.push(x),
                None => {}
            }
        }
        let elapsed = now.elapsed();
        println!("CCH duration: {:?}", elapsed);

        println!("Starting Dijkstra routing.");
        let mut dijkstra_result_distances: Vec<u32> = Vec::new();
        let now = Instant::now();
        for (&from, &to) in from_nodes.iter().zip(to_nodes.iter()) {
            let dijkstra_result = dijkstra_router.query(Query {
                from: from as NodeId,
                to: to as NodeId,
            });
            match dijkstra_result.distance() {
                Some(x) => dijkstra_result_distances.push(x),
                None => {}
            }
        }
        let elapsed = now.elapsed();
        println!("Dijkstra duration: {:?}", elapsed);

        println!("Starting BidDijkstra routing.");
        let mut bid_dijkstra_result_distances: Vec<u32> = Vec::new();
        let now = Instant::now();
        for (&from, &to) in from_nodes.iter().zip(to_nodes.iter()) {
            let bid_dijkstra_result = bid_dijkstra_router.query(Query {
                from: from as NodeId,
                to: to as NodeId,
            });
            match bid_dijkstra_result.distance() {
                Some(x) => bid_dijkstra_result_distances.push(x),
                None => {}
            }
        }
        let elapsed = now.elapsed();
        println!("BidDijkstra duration: {:?}", elapsed);

        let mut counter = 0;
        for (&cch, &dijkstra) in cch_result_distances
            .iter()
            .zip(dijkstra_result_distances.iter())
        {
            assert_eq!(cch, dijkstra, "Distances not equal for index {}.", counter);
            counter += 1;
        }
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
