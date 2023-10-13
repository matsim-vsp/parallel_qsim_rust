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
                    Self::convert_network(network, Some(&t.net_mode), Some(t.max_v)),
                )
            })
            .collect::<HashMap<_, _>>()
    }

    pub fn convert_network(
        network: &Network,
        mode: Option<&Id<String>>,
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
                        l.contains_mode(mode.internal as u64)
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
                        l.contains_mode(mode.internal as u64)
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
    use crate::simulation::io::network::IONetwork;
    use crate::simulation::io::population::IOPopulation;
    use crate::simulation::network::global_network::Network;
    use crate::simulation::routing::network_converter::NetworkConverter;
    use crate::simulation::vehicles::garage::Garage;
    use crate::simulation::vehicles::vehicle_type::VehicleType;

    #[test]
    fn test_simple_network() {
        let io_network = IONetwork::from_file("./assets/routing_tests/triangle-network.xml");
        let io_population = IOPopulation::empty();

        let mut garage = Garage::new();

        let network = Network::from_file(
            "./assets/routing_tests/triangle-network.xml",
            1,
            &mut garage,
        );

        let graph = NetworkConverter::convert_network(&network, None, None);

        println!("{graph:#?}");

        assert_eq!(graph.forward_first_out(), &vec![0usize, 0, 2, 4, 6]);
        assert_eq!(graph.forward_head(), &vec![2usize, 3, 2, 3, 1, 2]);
        assert_eq!(graph.forward_travel_time(), &vec![1., 2., 1., 4., 2., 5.]);
        assert_eq!(graph.forward_link_ids().len(), 6);

        assert_eq!(graph.backward_graph.first_out, vec![0usize, 0, 1, 4, 6]);
        assert_eq!(graph.backward_graph.head, vec![3usize, 1, 2, 3, 1, 2]);
        assert_eq!(
            graph.backward_graph.travel_time,
            vec![2., 1., 1., 5., 2., 4.]
        );
        assert_eq!(graph.backward_graph.link_ids.len(), 6);
        // we don't check y and y so far
    }

    #[test]
    fn test_simple_network_with_modes() {
        let mut garage = Garage::new();

        let car_type_id = garage.vehicle_type_ids.create_id("car");
        let car_mode = garage.modes.create_id("car");
        let mut car_veh_type = VehicleType::new(car_type_id, car_mode);
        car_veh_type.max_v = 5.;
        garage.add_veh_type(car_veh_type);

        let bike_type_id = garage.vehicle_type_ids.create_id("bike");
        let bike_mode = garage.modes.create_id("bike");
        let mut bike_veh_type = VehicleType::new(bike_type_id, bike_mode);
        bike_veh_type.max_v = 2.;
        garage.add_veh_type(bike_veh_type);

        let network = Network::from_file(
            "./assets/routing_tests/network_different_modes.xml",
            1,
            &mut garage,
        );

        let mut graph_by_mode =
            NetworkConverter::convert_network_with_vehicle_types(&network, &garage.vehicle_types);

        println!("{graph_by_mode:#?}");

        let car_network = graph_by_mode.remove(&0).unwrap();
        assert_eq!(car_network.forward_first_out(), &vec![0, 0, 1, 3, 4]);
        assert_eq!(car_network.forward_head(), &vec![3, 2, 1, 2]);
        assert_eq!(car_network.forward_travel_time(), &vec![2., 2., 5., 5.]);
        assert_eq!(car_network.forward_link_ids().len(), 4);

        let bike_network = graph_by_mode.remove(&1).unwrap();
        assert_eq!(bike_network.forward_first_out(), &vec![0, 0, 1, 3, 4]);
        assert_eq!(bike_network.forward_head(), &vec![2, 2, 3, 1]);
        assert_eq!(bike_network.forward_travel_time(), &vec![5., 5., 5., 5.]);
        assert_eq!(bike_network.forward_link_ids().len(), 4);
    }
}
