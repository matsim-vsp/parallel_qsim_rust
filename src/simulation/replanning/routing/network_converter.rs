use std::collections::HashMap;

use itertools::Itertools;
use nohash_hasher::IntMap;
use tracing::info;

use crate::simulation::id::Id;
use crate::simulation::network::global_network::{Link, Network};
use crate::simulation::replanning::routing::graph::{ForwardBackwardGraph, Graph};
use crate::simulation::vehicles::InternalVehicleType;
use crate::simulation::wire_types::vehicles::VehicleType;

pub struct NetworkConverter {}

impl NetworkConverter {
    pub fn convert_network_with_vehicle_types(
        network: &Network,
        vehicle_types: &IntMap<Id<InternalVehicleType>, InternalVehicleType>,
    ) -> IntMap<Id<InternalVehicleType>, ForwardBackwardGraph> {
        vehicle_types
            .iter()
            .map(|(id, vt)| (id.clone(), Self::convert_network(network, Some(vt))))
            .collect()
    }

    pub(crate) fn convert_network(
        network: &Network,
        vehicle_type: Option<&InternalVehicleType>,
    ) -> ForwardBackwardGraph {
        info!(
            "Converting network to forward backward graph for mode {:?}.",
            vehicle_type
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

        let nodes = network.get_all_nodes_sorted();
        let node_indices = nodes
            .iter()
            .enumerate()
            .map(|(i, node)| (&node.id, i))
            .collect::<HashMap<_, _>>();

        let mut forward_links_before = 0;
        let mut backward_links_before = 0;

        for node in nodes.iter() {
            //set x and y
            y.push(node.x);
            x.push(node.y);

            forward_first_out.push(forward_links_before);
            backward_first_out.push(backward_links_before);

            //calculate adjacent links
            let outgoing_links = Self::get_links(&node.out_links, network, vehicle_type);
            forward_links_before += outgoing_links.len();

            let ingoing_links = Self::get_links(&node.in_links, network, vehicle_type);
            backward_links_before += ingoing_links.len();

            //process outgoing links
            for link in outgoing_links {
                let to_node_index = *node_indices.get(&link.to).unwrap();

                forward_head.push(to_node_index);

                let max_speed = if let Some(vt) = vehicle_type {
                    vt.max_v.min(link.freespeed)
                } else {
                    link.freespeed
                };
                forward_travel_time.push((link.length / max_speed as f64) as u32);

                forward_link_ids.push(link.id.internal());
            }

            //process ingoing links
            for link in ingoing_links {
                //Watch out: This is in the backward graph
                let to_node_index = *node_indices.get(&link.from).unwrap();

                backward_head.push(to_node_index);

                let max_speed = if let Some(vt) = vehicle_type {
                    vt.max_v.min(link.freespeed)
                } else {
                    link.freespeed
                };
                backward_travel_time.push((link.length / max_speed as f64) as u32);

                backward_link_ids.push(link.id.internal());
            }
        }
        forward_first_out.push(forward_head.len());
        backward_first_out.push(backward_head.len());

        let forward_link_id_pos = forward_link_ids
            .iter()
            .enumerate()
            .map(|(i, id)| (*id, i))
            .collect::<HashMap<_, _>>();
        let backward_link_id_pos = backward_link_ids
            .iter()
            .enumerate()
            .map(|(i, id)| (*id, i))
            .collect::<HashMap<_, _>>();

        let forward_graph = Graph {
            first_out: forward_first_out,
            head: forward_head,
            travel_time: forward_travel_time,
            link_ids: forward_link_ids,
            x: x.clone(),
            y: y.clone(),
            link_id_pos: forward_link_id_pos,
        };

        let backward_graph = Graph {
            first_out: backward_first_out,
            head: backward_head,
            travel_time: backward_travel_time,
            link_ids: backward_link_ids,
            x,
            y,
            link_id_pos: backward_link_id_pos,
        };

        info!(
            "Finished converting network to forward backward graph for mode {:?}.",
            vehicle_type
        );

        ForwardBackwardGraph::new(forward_graph, backward_graph)
    }

    fn get_links<'net>(
        link_ids: &[Id<Link>],
        network: &'net Network,
        vehicle_type: Option<&InternalVehicleType>,
    ) -> Vec<&'net Link> {
        link_ids
            .iter()
            .sorted_by_key(|&l| l.internal())
            .map(|l| network.get_link(l))
            .filter(|&l| {
                if let Some(vt) = vehicle_type {
                    l.contains_mode(vt.net_mode.internal())
                } else {
                    true
                }
            })
            .collect::<Vec<&Link>>()
    }
}

#[cfg(test)]
mod test {
    use std::path::PathBuf;

    use crate::simulation::config::{MetisOptions, PartitionMethod};
    use crate::simulation::id::Id;
    use crate::simulation::network::global_network::Network;
    use crate::simulation::replanning::routing::network_converter::NetworkConverter;
    use crate::simulation::vehicles::garage::Garage;
    use crate::simulation::vehicles::InternalVehicleType;
    use crate::test_utils::create_vehicle_type;

    #[test]
    fn test_simple_network() {
        let network = Network::from_file(
            "./assets/routing_tests/triangle-network.xml",
            1,
            PartitionMethod::Metis(MetisOptions::default()),
        );
        let graph = NetworkConverter::convert_network(&network, None);

        assert_eq!(graph.forward_first_out(), &vec![0usize, 0, 2, 4, 6]);
        assert_eq!(graph.forward_head(), &vec![2usize, 3, 2, 3, 1, 2]);
        assert_eq!(graph.forward_travel_time(), &vec![1, 2, 1, 4, 2, 5]);
        assert_eq!(graph.forward_link_ids().len(), 6);

        assert_eq!(graph.backward_graph.first_out, vec![0usize, 0, 1, 4, 6]);
        assert_eq!(graph.backward_graph.head, vec![3usize, 1, 2, 3, 1, 2]);
        assert_eq!(graph.backward_graph.travel_time, vec![2, 1, 1, 5, 2, 4]);
        assert_eq!(graph.backward_graph.link_ids.len(), 6);
        // we don't check y and y so far
    }

    #[test]
    fn test_simple_network_with_modes() {
        let network = Network::from_file(
            "./assets/routing_tests/network_different_modes.xml",
            1,
            PartitionMethod::Metis(MetisOptions::default()),
        );

        let mut garage = Garage::new();

        let car_type_id = Id::<InternalVehicleType>::create("car");
        let car_id = Id::<String>::get_from_ext("car");
        let mut car_veh_type = create_vehicle_type(&car_type_id, car_id);
        car_veh_type.max_v = 5.;
        garage.add_veh_type(car_veh_type);

        let bike_type_id = Id::<InternalVehicleType>::create("bike");
        let bike_id = Id::<String>::get_from_ext("bike");
        let mut bike_veh_type = create_vehicle_type(&bike_type_id, bike_id);
        bike_veh_type.max_v = 2.;
        garage.add_veh_type(bike_veh_type);

        let mut graph_by_mode =
            NetworkConverter::convert_network_with_vehicle_types(&network, &garage.vehicle_types);

        assert_eq!(graph_by_mode.keys().len(), 2);

        let car_network = graph_by_mode.remove(&car_type_id).unwrap();
        assert_eq!(car_network.forward_first_out(), &vec![0, 0, 1, 3, 4]);
        assert_eq!(car_network.forward_head(), &vec![3, 2, 1, 2]);
        assert_eq!(car_network.forward_travel_time(), &vec![2, 2, 5, 5]);
        assert_eq!(car_network.forward_link_ids().len(), 4);

        let bike_network = graph_by_mode.remove(&bike_type_id).unwrap();
        assert_eq!(bike_network.forward_first_out(), &vec![0, 0, 1, 3, 4]);
        assert_eq!(bike_network.forward_head(), &vec![2, 2, 3, 1]);
        assert_eq!(bike_network.forward_travel_time(), &vec![5, 5, 5, 5]);
        assert_eq!(bike_network.forward_link_ids().len(), 4);
    }

    #[test]
    fn test_mode_filter() {
        let network = Network::from_file(
            "./assets/adhoc_routing/no_updates/network.xml",
            1,
            PartitionMethod::Metis(MetisOptions::default()),
        );
        let garage = Garage::from_file(&PathBuf::from(
            "./assets/adhoc_routing/no_updates/vehicles.xml",
        ));

        let vehicle_type2graph =
            NetworkConverter::convert_network_with_vehicle_types(&network, &garage.vehicle_types);

        // No link allows mode "walk". So the graph is expected to be empty.
        let walk_id = &Id::<InternalVehicleType>::get_from_ext("walk");
        assert!(vehicle_type2graph
            .get(walk_id)
            .unwrap()
            .forward_link_ids()
            .is_empty());

        // Test for mode "car"
        let car_id = &Id::<InternalVehicleType>::get_from_ext("car");
        let car_graph = vehicle_type2graph.get(car_id).unwrap();
        let link2_index = car_graph.forward_graph.first_out[2];
        assert_eq!(car_graph.forward_graph.travel_time[link2_index], 100);

        // Test for mode "bike"
        let bike_id = &Id::<InternalVehicleType>::get_from_ext("bike");
        let bike_graph = vehicle_type2graph.get(bike_id).unwrap();
        let link2_index = bike_graph.forward_graph.first_out[2];
        assert_eq!(bike_graph.forward_graph.travel_time[link2_index], 200);
    }

    #[test]
    fn test_different_veh_types_same_net_mode() {
        let network = Network::from_file(
            "./assets/adhoc_routing/no_updates/network.xml",
            1,
            PartitionMethod::Metis(MetisOptions::default()),
        );
        let garage = Garage::from_file(&PathBuf::from(
            "./assets/mode_dependent_routing/vehicle_definitions.xml",
        ));

        let vehicle_type2graph =
            NetworkConverter::convert_network_with_vehicle_types(&network, &garage.vehicle_types);

        assert_eq!(vehicle_type2graph.keys().len(), 2);

        assert_eq!(
            vehicle_type2graph
                .get(&Id::<InternalVehicleType>::get_from_ext("car"))
                .unwrap()
                .forward_graph
                .travel_time,
            vec![10, 10, 50, 100, 10, 10, 50]
        );

        assert_eq!(
            vehicle_type2graph
                .get(&Id::<InternalVehicleType>::get_from_ext("bike"))
                .unwrap()
                .forward_graph
                .travel_time,
            vec![20, 20, 200, 200, 20, 20, 200]
        );
    }
}
