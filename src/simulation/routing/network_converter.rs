use std::collections::HashMap;

use crate::simulation::id::Id;
use crate::simulation::network::global_network::{Link, Network};
use crate::simulation::routing::graph::{ForwardBackwardGraph, Graph};
use crate::simulation::vehicles::vehicle_type::VehicleType;

pub struct NetworkConverter {}

impl NetworkConverter {
    pub fn convert_network_with_vehicle_types(
        network: &Network,
        vehicle_types: &HashMap<Id<VehicleType>, VehicleType>,
    ) -> HashMap<u64, ForwardBackwardGraph> {
        vehicle_types
            .iter()
            .map(|(_, t)| {
                (
                    t.net_mode.internal as u64,
                    Self::convert_network(network, Some(t.net_mode.internal as u64), Some(t.max_v)),
                )
            })
            .collect::<HashMap<_, _>>()
    }

    pub fn convert_network(
        network: &Network,
        mode: Option<u64>,
        max_mode_speed: Option<f32>,
    ) -> ForwardBackwardGraph {
        assert!(
            (mode.is_some() && max_mode_speed.is_some())
                || (!mode.is_some() && !max_mode_speed.is_some()),
            "There must either be both mode and max velocity set or both not."
        );

        let mut forward_first_out = Vec::new();
        let mut forward_head = Vec::new();
        let mut forward_travel_time = Vec::new();
        let mut forward_link_ids = Vec::new();

        let mut backward_first_out = Vec::new();
        let mut backward_head = Vec::new();
        let mut backward_travel_time = Vec::new();
        let mut backward_link_ids = Vec::new();

        let mut x = Vec::new();
        let mut y = Vec::new();

        let links = network.get_all_links_sorted();
        let nodes = network.get_all_nodes_sorted();

        let mut forward_links_before = 0;
        let mut backward_links_before = 0;

        for node in nodes.iter() {
            //set x and y
            y.push(node.x);
            x.push(node.y);

            forward_first_out.push(forward_links_before);
            backward_first_out.push(backward_links_before);

            //calculate adjacent links
            let outgoing_links = links
                .iter()
                .filter(|&l| l.from == node.id)
                .filter(|&l| {
                    if let Some(mode) = mode {
                        l.contains_mode(mode)
                    } else {
                        true
                    }
                })
                .collect::<Vec<&&Link>>();
            forward_links_before += outgoing_links.len();

            let ingoing_links = links
                .iter()
                .filter(|&l| l.to == node.id)
                .filter(|&l| {
                    if let Some(mode) = mode {
                        l.contains_mode(mode)
                    } else {
                        true
                    }
                })
                .collect::<Vec<&&Link>>();
            backward_links_before += ingoing_links.len();

            //process outgoing links
            for link in outgoing_links {
                let to_node_index = nodes.iter().position(|&node| node.id == link.to).unwrap();

                forward_head.push(to_node_index);

                let max_speed = if let Some(max_mode_speed) = max_mode_speed {
                    max_mode_speed.min(link.freespeed)
                } else {
                    link.freespeed
                };
                forward_travel_time.push((link.length / max_speed));

                forward_link_ids.push(link.id.internal);
            }

            //process ingoing links
            for link in ingoing_links {
                //Watch out: This is in the backward graph
                let to_node_index = nodes.iter().position(|&node| node.id == link.from).unwrap();

                backward_head.push(to_node_index);

                let max_speed = if let Some(max_mode_speed) = max_mode_speed {
                    max_mode_speed.min(link.freespeed)
                } else {
                    link.freespeed
                };
                backward_travel_time.push((link.length / max_speed));

                backward_link_ids.push(link.id.internal);
            }
        }
        forward_first_out.push(forward_head.len());
        backward_first_out.push(backward_head.len());

        let forward_graph = Graph {
            first_out: forward_first_out,
            head: forward_head,
            travel_time: forward_travel_time,
            link_ids: forward_link_ids,
            x: x.clone(),
            y: y.clone(),
        };

        let backward_graph = Graph {
            first_out: backward_first_out,
            head: backward_head,
            travel_time: backward_travel_time,
            link_ids: backward_link_ids,
            x,
            y,
        };

        ForwardBackwardGraph::new(forward_graph, backward_graph)
    }
}

#[cfg(test)]
mod test {
    use crate::simulation::id_mapping::MatsimIdMappings;
    use crate::simulation::io::network::IONetwork;
    use crate::simulation::io::population::IOPopulation;
    use crate::simulation::io::vehicle_definitions::VehicleDefinitions;
    use crate::simulation::network::network::Network;
    use crate::simulation::routing::network_converter::NetworkConverter;

    #[test]
    fn test_simple_network() {
        let io_network = IONetwork::from_file("./assets/routing_tests/triangle-network.xml");
        let io_population = IOPopulation::empty();

        let network = Network::from_io(
            &io_network,
            1,
            1.0,
            |_| 0,
            &MatsimIdMappings::from_io(&io_network, &io_population),
        );

        let network = NetworkConverter::convert_network(&network, None, None);

        println!("{network:#?}");

        assert_eq!(network.first_out, vec![0, 0, 2, 4, 6]);
        assert_eq!(network.head, vec![2, 3, 2, 3, 1, 2]);
        assert_eq!(network.travel_time, vec![1, 2, 1, 4, 2, 5]);
        assert_eq!(network.link_ids.len(), 6);
        // we don't check y and y so far
    }

    #[test]
    fn test_simple_network_with_modes() {
        let vehicle_definitions = VehicleDefinitions::new()
            .add_vehicle_type("car".to_string(), Some(5.), "car".to_string())
            .add_vehicle_type("bike".to_string(), Some(2.), "bike".to_string());

        let io_network = IONetwork::from_file("./assets/routing_tests/network_different_modes.xml");
        let io_population = IOPopulation::empty();

        let network = Network::from_io(
            &io_network,
            1,
            1.0,
            |_| 0,
            &MatsimIdMappings::from_io(&io_network, &io_population),
        );

        let mut network =
            NetworkConverter::convert_network_with_vehicle_types(&network, &vehicle_definitions);

        println!("{network:#?}");

        let car_network = network.remove("car").unwrap();
        assert_eq!(car_network.first_out, vec![0, 0, 1, 3, 4]);
        assert_eq!(car_network.head, vec![3, 2, 1, 2]);
        assert_eq!(car_network.travel_time, vec![2, 2, 5, 5]);
        assert_eq!(car_network.link_ids.len(), 4);

        let bike_network = network.remove("bike").unwrap();
        assert_eq!(bike_network.first_out, vec![0, 0, 1, 3, 4]);
        assert_eq!(bike_network.head, vec![2, 2, 3, 1]);
        assert_eq!(bike_network.travel_time, vec![5, 5, 5, 5]);
        assert_eq!(bike_network.link_ids.len(), 4);
    }
}
