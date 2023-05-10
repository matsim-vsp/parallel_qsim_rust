use crate::simulation::network::routing_kit_network::RoutingKitNetwork;
use crate::simulation::routing::inertial_flow_cutter_adapter::InertialFlowCutterAdapter;
use crate::simulation::routing::router::CustomQueryResult;
use rust_road_router::algo::customizable_contraction_hierarchy::query::{
    PathServerWrapper, Server,
};
use rust_road_router::algo::customizable_contraction_hierarchy::{customize, CustomizedBasic, CCH};
use rust_road_router::algo::{Query, QueryResult, QueryServer};
use rust_road_router::datastr::graph::{NodeId, OwnedGraph, Weight};
use rust_road_router::datastr::node_order::NodeOrder;
use std::env;
use std::path::PathBuf;
use std::ptr::NonNull;

pub struct RoadRouter<'router> {
    pub(crate) server_adapter: Box<ServerAdapter<'router>>,
    pub(crate) current_network: RoutingKitNetwork,
    pub(crate) initial_network: RoutingKitNetwork,
}

impl<'router> RoadRouter<'router> {
    pub(crate) fn new(network: &RoutingKitNetwork, output_dir: PathBuf) -> RoadRouter<'router> {
        RoadRouter {
            server_adapter: ServerAdapter::new(
                RoadRouter::perform_preprocessing(&network, output_dir),
                network,
            ),
            current_network: network.clone(),
            initial_network: network.clone(),
        }
    }

    pub(crate) fn query<'q>(
        &'q mut self,
        from: usize,
        to: usize,
    ) -> QueryResult<PathServerWrapper<'q, CustomizedBasic<'router, CCH>>, Weight> {
        self.server_adapter.query(from, to)
    }

    fn get_end_node(&self, link_id: u64) -> usize {
        let link_id_index = self
            .current_network
            .link_ids
            .iter()
            .position(|&id| id == link_id)
            .unwrap();
        *self.current_network.head.get(link_id_index).unwrap() as usize
    }

    fn get_start_node(&self, link_id: u64) -> usize {
        let link_id_index = self
            .current_network
            .link_ids
            .iter()
            .position(|&id| id == link_id)
            .unwrap();

        let mut result = None;
        for i in 0..self.current_network.first_out.len() {
            if link_id_index >= *self.current_network.first_out.get(i).unwrap() as usize
                && link_id_index < *self.current_network.first_out.get(i + 1).unwrap() as usize
            {
                result = Some(i as usize);
            }
        }

        result.unwrap()
    }

    pub(crate) fn perform_preprocessing(
        network: &RoutingKitNetwork,
        temp_output_folder: PathBuf,
    ) -> CCH {
        let owned_graph = RoadRouter::create_owned_graph(network);

        let inertial_flow_cutter_path = env::var("INERTIAL_FLOW_CUTTER_HOME_DIRECTORY")
            .expect("The environment variable 'INERTIAL_FLOW_CUTTER_HOME_DIRECTORY' is not set.");

        // step 1: compute node ordering
        let node_order_vec = InertialFlowCutterAdapter::new(
            network,
            PathBuf::from(inertial_flow_cutter_path),
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
            routing_kit_network.travel_times.to_owned(),
        )
    }

    pub fn query_links(&mut self, from_link: u64, to_link: u64) -> CustomQueryResult {
        let travel_time;
        let result_edge_path;
        {
            let mut result: QueryResult<
                PathServerWrapper<'_, CustomizedBasic<'router, CCH>>,
                Weight,
            > = self.query(self.get_end_node(from_link), self.get_start_node(to_link));
            travel_time = result.distance();
            result_edge_path = result.node_path();
        }
        let edge_path = result_edge_path
            .map(|node_path| get_edge_path(node_path, &self.current_network))
            .map(|mut path| {
                //add from link at the beginning and to link at the end
                path.insert(0, from_link);
                path.push(to_link);
                path
            });

        CustomQueryResult {
            travel_time,
            path: edge_path,
        }
    }

    pub fn customize(&mut self, network: RoutingKitNetwork) {
        self.current_network = network;

        let cch_ref = NonNull::from(&self.server_adapter.cch);

        let customization = unsafe {
            customize(
                cch_ref.as_ref(),
                &RoadRouter::create_owned_graph(&self.current_network),
            )
        };

        // Using the update method of Server is ok, because a new object of trait type Customized is created
        // which holds a reference the CCH (which is stored in ServerAdapter).
        // This Customized object is swapped
        self.server_adapter
            .as_mut()
            .server
            .as_mut()
            .unwrap()
            .update(customization);
    }

    pub fn get_current_network(&self) -> &RoutingKitNetwork {
        &self.current_network
    }

    pub fn get_initial_travel_time(&self, link_id: u64) -> u32 {
        self.initial_network.get_travel_time_by_link_id(link_id)
    }

    pub fn get_current_travel_time(&self, link_id: u64) -> u32 {
        self.current_network.get_travel_time_by_link_id(link_id)
    }
}

pub struct ServerAdapter<'adapter> {
    server: Option<Server<CustomizedBasic<'adapter, CCH>>>,
    cch: CCH,
}

impl<'adapter> ServerAdapter<'adapter> {
    pub fn new(cch: CCH, network: &RoutingKitNetwork) -> Box<ServerAdapter<'adapter>> {
        let mut boxed_adapter = Box::new(ServerAdapter { server: None, cch });

        let cch_ref = NonNull::from(&boxed_adapter.cch);
        let metric = &RoadRouter::create_owned_graph(&network);

        // To provide clean architecture, everything related to routing should be placed in this module.
        // In particular, the cch has to live here. Because the Server object of the KIT library gets a reference,
        // it has to life long enough. (Server -> Customized -> CCH) This can only be done with self-referential structs.
        // (https://stackoverflow.com/questions/32300132/why-cant-i-store-a-value-and-a-reference-to-that-value-in-the-same-struct)
        //
        // Therefore, we need unsafe code.
        //
        // It is required that the base address of cch is never changed, thus ServerAdapter must be placed on the heap.
        // Pinning is without effect because ServerAdapter must be mutable since every query mutates the server. :/
        // That's why we have to use raw pointers on the ServerAdapter object which is placed on the heap.
        //
        // IMPORTANT: The ServerAdapter or i.e. CCH is not allowed to be reallocated after instantiation.
        //
        // SAFETY considerations
        // The pointer cch_ref is valid, since the pointee is part of the ServerAdapter object.
        // It is placed on the heap and an instance of CCH. By design of this router module it is not mutated nor reallocated.
        let customization = unsafe { customize(cch_ref.as_ref(), metric) };
        boxed_adapter.as_mut().server = Some(Server::new(customization));

        boxed_adapter
    }

    pub fn query<'q>(
        &'q mut self,
        from: usize,
        to: usize,
    ) -> QueryResult<PathServerWrapper<'q, CustomizedBasic<'adapter, CCH>>, Weight> {
        self.server.as_mut().unwrap().query(Query {
            from: from as NodeId,
            to: to as NodeId,
        })
    }
}

pub(self) fn get_edge_path(path: Vec<NodeId>, network: &RoutingKitNetwork) -> Vec<u64> {
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
                    network,
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
    assert!(
        last_out_index as i64 - first_out_index as i64 >= 0,
        "No outgoing edges!"
    );
    let mut result = None;
    for i in first_out_index..=last_out_index {
        if *network.head.get(i).unwrap() == next_node {
            result = Some(network.link_ids.get(i).unwrap().clone());
            break;
        }
    }
    result.expect("No outgoing edge found!") as u64
}

#[cfg(test)]
mod test {
    use std::fmt::Debug;
    use std::path::PathBuf;
    use std::time::Instant;

    use crate::simulation::routing::network_converter::NetworkConverter;
    use crate::simulation::routing::road_router::{get_edge_path, RoadRouter};
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
    fn test_get_edge_path() {
        let mut network =
            NetworkConverter::convert_xml_network("./assets/routing_tests/triangle-network.xml");
        network.link_ids = vec![0, 1, 2, 3, 4, 5];

        assert_eq!(get_edge_path(vec![1, 2, 3], &network), vec![0, 3]);
        assert_eq!(get_edge_path(vec![1, 3, 2], &network), vec![1, 5]);
        assert_eq!(
            get_edge_path(vec![1, 2, 3, 1, 2, 3], &network),
            vec![0, 3, 4, 0, 3]
        );
    }

    #[test]
    fn test_simple_cch_with_router_and_update() {
        //does only work locally
        let network =
            NetworkConverter::convert_xml_network("./assets/routing_tests/triangle-network.xml");

        let mut router = RoadRouter::new(
            &network,
            PathBuf::from("./test_output/routing/simple_cch_update/"),
        );

        let res12 = router.query(1, 2);
        test_query_result(res12, 1, vec![1, 2]);
        let res32 = router.query(3, 2);
        test_query_result(res32, 3, vec![3, 1, 2]);

        println!("Assign new travel time to edge 1-2: 4");

        let network_new_weights = network.clone_with_new_travel_times(vec![4, 2, 1, 4, 2, 5]);
        router.customize(network_new_weights);
        let new_result = router.query(3, 2);
        test_query_result(new_result, 5, vec![3, 2]);
    }

    #[ignore]
    #[test]
    fn compare_cch_and_dijkstra() {
        let network = NetworkConverter::convert_xml_network("./assets/andorra-network.xml.gz");

        let mut cch_router = RoadRouter::new(
            &network,
            PathBuf::from("./test_output/routing/performance/"),
        );

        let mut dijkstra_router =
            DijkServer::<_, DefaultOps>::new(RoadRouter::create_owned_graph(&network));

        let mut bid_dijkstra_router =
            BidServer::<OwnedGraph, OwnedGraph, BiDirZeroPot, ChooseMinKeyDir>::new(
                RoadRouter::create_owned_graph(&network),
            );

        let owned_graph = RoadRouter::create_owned_graph(&network);
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
