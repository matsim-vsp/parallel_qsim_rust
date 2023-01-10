#[cfg(test)]
mod test {
    use std::time::Instant;

    use crate::parallel_simulation::routing::network_converter::NetworkConverter;
    use crate::parallel_simulation::routing::router::Router;
    use log::info;
    use rand::seq::{IteratorRandom, SliceRandom};
    use rust_road_router::algo::a_star::BiDirZeroPot;
    use rust_road_router::algo::dijkstra::query::bidirectional_dijkstra::Server as BidServer;
    use rust_road_router::algo::dijkstra::DefaultOps;
    use rust_road_router::algo::{
        dijkstra::{query::dijkstra::Server as DijkServer, *},
        *,
    };
    use rust_road_router::algo::{Query, QueryServer};
    use rust_road_router::datastr::graph::{NodeId, OwnedGraph};

    #[ignore]
    #[test]
    fn compare_cch_and_dijkstra() {
        let network = NetworkConverter::convert_xml_network("./assets/andorra-network.xml.gz");

        let cch = Router::perform_preprocessing(&network, "./output/");
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
}
