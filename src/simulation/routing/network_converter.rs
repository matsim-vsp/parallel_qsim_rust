use crate::simulation::id::Id;
use rust_road_router::datastr::graph::{EdgeId, NodeId, Weight};
use std::collections::HashMap;

use crate::simulation::io::vehicle_definitions::VehicleDefinitions;
use crate::simulation::network::global_network::{Link, Network};
use crate::simulation::network::routing_kit_network::RoutingKitNetwork;

pub struct NetworkConverter {}

impl NetworkConverter {
    pub fn convert_network_with_vehicle_definitions(
        network: &Network,
        vehicle_definitions: &VehicleDefinitions,
    ) -> HashMap<Id<String>, RoutingKitNetwork> {
        vehicle_definitions
            .vehicle_types
            .iter()
            .map(|vt| {
                let mode_id = network.modes.get_from_ext(&vt.id);

                (
                    mode_id.clone(),
                    Self::convert_network(network, Some(mode_id), vt.maximum_velocity),
                )
            })
            .collect()
    }

    pub fn convert_network(
        network: &Network,
        mode: Option<Id<String>>,
        max_mode_speed: Option<f32>,
    ) -> RoutingKitNetwork {
        assert!(
            (mode.is_some() && max_mode_speed.is_some())
                || (!mode.is_some() && !max_mode_speed.is_some()),
            "There must either be both mode and max velocity set or both not."
        );

        let mut first_out = Vec::new();
        let mut head = Vec::new();
        let mut travel_times = Vec::new();
        let mut link_ids = Vec::new();
        let mut x = Vec::new();
        let mut y = Vec::new();

        let mut links_before = 0;

        // nodes are stored in consecutive order by internal node id. Therefore we can
        // simply iterate over the nodes of the global network
        for node in &network.nodes {
            y.push(node.y);
            x.push(node.x);
            first_out.push(links_before as EdgeId);

            let out_links: Vec<&Link> = node
                .out_links
                .iter()
                .map(|id| network.get_link(&id))
                .filter(|link| Self::filter_mode(link, &mode))
                .collect();
            links_before += out_links.len();

            for link in out_links {
                head.push(link.to.internal as NodeId);
                let max_speed = Self::max_speed(link, &max_mode_speed);
                let travel_time = link.length / max_speed;
                travel_times.push(travel_time as Weight);
                link_ids.push(link.id.internal as u64);
            }
        }
        first_out.push(head.len() as EdgeId);

        RoutingKitNetwork {
            first_out,
            head,
            travel_time: travel_times,
            link_ids,
            x,
            y,
        }
    }

    fn filter_mode(link: &Link, mode: &Option<Id<String>>) -> bool {
        if let Some(mode) = mode {
            link.modes.contains(mode)
        } else {
            true
        }
    }

    fn max_speed(link: &Link, mode_speed: &Option<f32>) -> f32 {
        let used_mode_speed = if let Some(speed) = mode_speed {
            *speed
        } else {
            f32::INFINITY
        };
        link.freespeed.max(used_mode_speed)
    }
}

#[cfg(test)]
mod test {
    use crate::simulation::io::network::IONetwork;
    use crate::simulation::io::population::IOPopulation;
    use crate::simulation::io::vehicle_definitions::VehicleDefinitions;
    use crate::simulation::routing::network_converter::NetworkConverter;

    #[test]
    fn test_simple_network() {
        let io_network = IONetwork::from_file("./assets/routing_tests/triangle-network.xml");
        let io_population = IOPopulation::empty();

        let network = crate::simulation::network::global_network::Network::from_file(
            "./assets/routing_tests/triangle-network.xml",
            1,
        );

        let routing_network = NetworkConverter::convert_network(&network, None, None);

        println!("{routing_network:#?}");

        assert_eq!(routing_network.first_out, vec![0, 0, 2, 4, 6]);
        assert_eq!(routing_network.head, vec![2, 3, 2, 3, 1, 2]);
        assert_eq!(routing_network.travel_time, vec![1, 2, 1, 4, 2, 5]);
        assert_eq!(routing_network.link_ids.len(), 6);
        // we don't check y and y so far
    }

    #[test]
    fn test_simple_network_with_modes() {
        let vehicle_definitions = VehicleDefinitions::new()
            .add_vehicle_type("car".to_string(), Some(5.), "car".to_string())
            .add_vehicle_type("bike".to_string(), Some(2.), "bike".to_string());

        let network = crate::simulation::network::global_network::Network::from_file(
            "./assets/routing_tests/triangle-network.xml",
            1,
        );

        let mut routing_network = NetworkConverter::convert_network_with_vehicle_definitions(
            &network,
            &vehicle_definitions,
        );

        println!("{routing_network:#?}");

        let car_network = routing_network
            .remove(&network.modes.get_from_ext("car"))
            .unwrap();
        assert_eq!(car_network.first_out, vec![0, 0, 1, 3, 4]);
        assert_eq!(car_network.head, vec![3, 2, 1, 2]);
        assert_eq!(car_network.travel_time, vec![2, 2, 5, 5]);
        assert_eq!(car_network.link_ids.len(), 4);

        let bike_network = routing_network
            .remove(&network.modes.get_from_ext("bike"))
            .unwrap();
        assert_eq!(bike_network.first_out, vec![0, 0, 1, 3, 4]);
        assert_eq!(bike_network.head, vec![2, 2, 3, 1]);
        assert_eq!(bike_network.travel_time, vec![5, 5, 5, 5]);
        assert_eq!(bike_network.link_ids.len(), 4);
    }
}
