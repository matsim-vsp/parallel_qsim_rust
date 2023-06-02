use rust_road_router::datastr::graph::{EdgeId, NodeId, Weight};
use std::collections::HashMap;

use crate::simulation::io::vehicle_definitions::VehicleDefinitions;
use crate::simulation::network::link::Link;
use crate::simulation::network::network::Network;
use crate::simulation::network::routing_kit_network::RoutingKitNetwork;

pub struct NetworkConverter {}

impl NetworkConverter {
    pub fn convert_network_with_vehicle_definitions(
        network: &Network,
        vehicle_definitions: &VehicleDefinitions,
    ) -> HashMap<String, RoutingKitNetwork> {
        vehicle_definitions
            .vehicle_types
            .iter()
            .map(|vt| {
                (
                    vt.id.clone(),
                    Self::convert_network(network, Some(vt.id.as_str()), vt.maximum_velocity),
                )
            })
            .collect::<HashMap<_, _>>()
    }

    pub fn convert_network(
        network: &Network,
        mode: Option<&str>,
        max_mode_speed: Option<f32>,
    ) -> RoutingKitNetwork {
        assert!(
            (mode.is_some() && max_mode_speed.is_some())
                || (!mode.is_some() && !max_mode_speed.is_some()),
            "There must either be both mode and max velocity set or both not."
        );

        let mut first_out = Vec::new();
        let mut head = Vec::new();
        let mut travel_time = Vec::new();
        let mut link_ids = Vec::new();
        let mut x = Vec::new();
        let mut y = Vec::new();

        let links = network.get_all_links_sorted();
        let nodes = network.get_all_nodes_sorted();

        let mut links_before = 0;

        for node in nodes.iter() {
            y.push(node.x());
            x.push(node.y());
            first_out.push((links_before) as EdgeId);

            let outgoing_links = links
                .iter()
                .filter(|&l| l.from_id() == node.id())
                .filter(|&l| {
                    if let Some(mode) = mode {
                        l.contains_mode(&String::from(mode))
                    } else {
                        true
                    }
                })
                .collect::<Vec<&&Link>>();
            links_before += outgoing_links.len();

            for link in outgoing_links {
                let to_node_index = nodes
                    .iter()
                    .position(|&node| node.id() == link.to_id())
                    .unwrap();

                head.push(to_node_index as NodeId);

                let max_speed = if let Some(max_mode_speed) = max_mode_speed {
                    max_mode_speed.min(link.freespeed())
                } else {
                    link.freespeed()
                };
                travel_time.push((link.length() / max_speed) as Weight);

                link_ids.push(link.id() as u64);
            }
        }
        first_out.push(head.len() as EdgeId);

        RoutingKitNetwork {
            first_out,
            head,
            travel_time,
            link_ids,
            x,
            y,
        }
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
            .add_vehicle_type("car".to_string(), Some(5.))
            .add_vehicle_type("bike".to_string(), Some(2.));

        let io_network = IONetwork::from_file("./assets/routing_tests/network_different_modes.xml");
        let io_population = IOPopulation::empty();

        let network = Network::from_io(
            &io_network,
            1,
            1.0,
            |_| 0,
            &MatsimIdMappings::from_io(&io_network, &io_population),
        );

        let mut network = NetworkConverter::convert_network_with_vehicle_definitions(
            &network,
            &vehicle_definitions,
        );

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
