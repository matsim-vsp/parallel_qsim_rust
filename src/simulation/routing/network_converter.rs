use crate::simulation::id_mapping::MatsimIdMappings;
use rust_road_router::datastr::graph::{EdgeId, NodeId, Weight};
use std::collections::HashMap;

use crate::simulation::io::network::{IOLink, IONetwork, IONode};
use crate::simulation::io::vehicle_definitions::VehicleDefinitions;
use crate::simulation::network::routing_kit_network::RoutingKitNetwork;

pub struct NetworkConverter {}

impl NetworkConverter {
    pub fn convert_xml_network(matsim_network_path: &str) -> RoutingKitNetwork {
        NetworkConverter::convert_io_network(
            IONetwork::from_file(matsim_network_path),
            None,
            None,
            None,
        )
    }

    pub fn convert_io_network_with_vehicle_definitions(
        matsim_network: IONetwork,
        id_mappings: Option<&MatsimIdMappings>,
        vehicle_definitions: &VehicleDefinitions,
    ) -> HashMap<String, RoutingKitNetwork> {
        vehicle_definitions
            .vehicle_types
            .iter()
            .map(|vt| {
                (
                    vt.id.clone(),
                    Self::convert_io_network(
                        matsim_network.clone(),
                        id_mappings,
                        Some(vt.id.as_str()),
                        vt.maximum_velocity,
                    ),
                )
            })
            .collect::<HashMap<_, _>>()
    }

    pub fn convert_io_network(
        mut matsim_network: IONetwork,
        id_mappings: Option<&MatsimIdMappings>,
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

        Self::check_network_valid(&matsim_network);

        //sort links by from id
        matsim_network
            .links_mut()
            .sort_by_key(|link: &IOLink| link.from.to_lowercase());
        //sort nodes by id
        matsim_network
            .nodes_mut()
            .sort_by_key(|node: &IONode| node.id.to_lowercase());

        let mut links_before = 0;

        for node in matsim_network.nodes() {
            //TODO: make sure, that the coordinate system is correct
            y.push(node.x);
            x.push(node.y);

            first_out.push((links_before) as EdgeId);

            let links: Vec<&IOLink> = matsim_network
                .links()
                .iter()
                .filter(|link: &&IOLink| *link.from == node.id)
                .filter(|&l| match mode.is_some() {
                    true => l.modes().contains(&String::from(mode.unwrap())),
                    false => true,
                })
                .collect();
            links_before += links.len();
            for link in links {
                head.push(NetworkConverter::get_node_index(&matsim_network, &link.to) as NodeId);

                let max_speed = if let Some(max_mode_speed) = max_mode_speed {
                    max_mode_speed.min(link.freespeed)
                } else {
                    link.freespeed
                };
                travel_time.push((link.length / max_speed) as Weight);

                if id_mappings.is_some() {
                    link_ids.push(
                        *id_mappings
                            .unwrap()
                            .links
                            .get_internal(link.id.as_str())
                            .unwrap() as u64,
                    );
                }
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

    //checks whether network consists of unique node ids
    fn check_network_valid(network: &IONetwork) {
        let mut node_ids: Vec<String> = network
            .nodes()
            .iter()
            .map(|n| String::from(&n.id))
            .collect();
        node_ids.dedup();
        assert_eq!(node_ids.len(), network.nodes().len());
    }

    fn get_node_index(network: &IONetwork, id: &str) -> usize {
        network
            .nodes()
            .iter()
            .position(|node| node.id == *id)
            .unwrap()
    }
}

#[cfg(test)]
mod test {
    use crate::simulation::io::network::IONetwork;
    use crate::simulation::io::vehicle_definitions::VehicleDefinitions;
    use crate::simulation::routing::network_converter::NetworkConverter;

    #[test]
    fn test_simple_network() {
        let network =
            NetworkConverter::convert_xml_network("./assets/routing_tests/triangle-network.xml");
        println!("{network:#?}");

        assert_eq!(network.first_out, vec![0, 0, 2, 4, 6]);
        assert_eq!(network.head, vec![2, 3, 2, 3, 1, 2]);
        assert_eq!(network.travel_time, vec![1, 2, 1, 4, 2, 5]);
        assert_eq!(network.link_ids, Vec::<u64>::new());
        // we don't check y and y so far
    }

    #[test]
    fn test_simple_network_with_modes() {
        let vehicle_definitions = VehicleDefinitions::new()
            .add_vehicle_type("car".to_string(), Some(5.))
            .add_vehicle_type("bike".to_string(), Some(2.));

        let mut network = NetworkConverter::convert_io_network_with_vehicle_definitions(
            IONetwork::from_file("./assets/routing_tests/network_different_modes.xml"),
            None,
            &vehicle_definitions,
        );

        println!("{network:#?}");

        let car_network = network.remove("car").unwrap();
        assert_eq!(car_network.first_out, vec![0, 0, 1, 3, 4]);
        assert_eq!(car_network.head, vec![3, 2, 1, 2]);
        assert_eq!(car_network.travel_time, vec![2, 2, 5, 5]);
        assert_eq!(car_network.link_ids, Vec::<u64>::new());

        let bike_network = network.remove("bike").unwrap();
        assert_eq!(bike_network.first_out, vec![0, 0, 1, 3, 4]);
        assert_eq!(bike_network.head, vec![2, 2, 3, 1]);
        assert_eq!(bike_network.travel_time, vec![5, 5, 5, 5]);
        assert_eq!(bike_network.link_ids, Vec::<u64>::new());
    }
}
