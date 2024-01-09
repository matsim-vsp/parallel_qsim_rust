use itertools::Itertools;
use std::collections::hash_map::Entry;
use std::collections::HashMap;

use nohash_hasher::IntMap;
use tracing::info;

use crate::simulation::id::Id;
use crate::simulation::network::global_network::{Link, Network};
use crate::simulation::replanning::routing::graph::{ForwardBackwardGraph, Graph};
use crate::simulation::wire_types::vehicles::VehicleType;

pub struct NetworkConverter {}

impl NetworkConverter {
    pub fn convert_network_with_vehicle_types(
        network: &Network,
        vehicle_types: &IntMap<Id<VehicleType>, VehicleType>,
    ) -> HashMap<u64, ForwardBackwardGraph> {
        let mode_max_v_vec = vehicle_types
            .iter()
            .map(|(_, t)| (t.net_mode, t.max_v))
            .collect();

        Self::mode_max_v_to_map(mode_max_v_vec)
            .iter()
            .map(|(mode, max_v)| {
                (
                    *mode,
                    Self::convert_network(network, Some(&Id::get(*mode)), Some(*max_v)),
                )
            })
            .collect()
    }

    pub(crate) fn convert_network(
        network: &Network,
        mode: Option<&Id<String>>,
        max_mode_speed: Option<f32>,
    ) -> ForwardBackwardGraph {
        info!(
            "Converting network to forward backward graph for mode {:?}.",
            mode
        );

        assert!(
            (mode.is_some() && max_mode_speed.is_some())
                || (mode.is_none() && max_mode_speed.is_none()),
            "Either both mode and maximum speed must be set, or neither must be set."
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
            let outgoing_links = Self::get_links(&node.out_links, &network, &mode);
            forward_links_before += outgoing_links.len();

            let ingoing_links = Self::get_links(&node.in_links, &network, &mode);
            backward_links_before += ingoing_links.len();

            //process outgoing links
            for link in outgoing_links {
                let to_node_index = *node_indices.get(&link.to).unwrap();

                forward_head.push(to_node_index);

                let max_speed = if let Some(max_mode_speed) = max_mode_speed {
                    max_mode_speed.min(link.freespeed)
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

                let max_speed = if let Some(max_mode_speed) = max_mode_speed {
                    max_mode_speed.min(link.freespeed)
                } else {
                    link.freespeed
                };
                backward_travel_time.push((link.length / max_speed as f64) as u32);

                backward_link_ids.push(link.id.internal());
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

        info!(
            "Finished converting network to forward backward graph for mode {:?}.",
            mode
        );

        ForwardBackwardGraph::new(forward_graph, backward_graph)
    }

    fn mode_max_v_to_map(input: Vec<(u64, f32)>) -> HashMap<u64, f32> {
        let mut result = HashMap::new();
        for (key, value) in &input {
            match result.entry(*key) {
                Entry::Occupied(_) => {
                    panic!("Duplicated key found in the input vector: {:?}", input);
                }
                Entry::Vacant(e) => e.insert(*value),
            };
        }
        result
    }

    fn get_links<'net>(
        link_ids: &Vec<Id<Link>>,
        network: &'net Network,
        mode: &Option<&Id<String>>,
    ) -> Vec<&'net Link> {
        link_ids
            .iter()
            .sorted_by_key(|&l| l.internal())
            .map(|l| network.get_link(l))
            .filter(|&l| {
                if let Some(mode) = mode {
                    l.contains_mode(mode.internal())
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
    use crate::simulation::wire_types::vehicles::VehicleType;
    use crate::test_utils::create_vehicle_type;

    #[test]
    fn test_simple_network() {
        let network = Network::from_file(
            "./assets/routing_tests/triangle-network.xml",
            1,
            PartitionMethod::Metis(MetisOptions::default()),
        );
        let graph = NetworkConverter::convert_network(&network, None, None);

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

        let car_type_id = Id::<VehicleType>::create("car");
        let car_id = Id::<String>::get_from_ext("car");
        let car_id_internal = car_id.internal();
        let mut car_veh_type = create_vehicle_type(car_type_id, car_id);
        car_veh_type.max_v = 5.;
        garage.add_veh_type(car_veh_type);

        let bike_type_id = Id::<VehicleType>::create("bike");
        let bike_id = Id::<String>::get_from_ext("bike");
        let bike_id_internal = bike_id.internal();
        let mut bike_veh_type = create_vehicle_type(bike_type_id, bike_id);
        bike_veh_type.max_v = 2.;
        garage.add_veh_type(bike_veh_type);

        let mut graph_by_mode =
            NetworkConverter::convert_network_with_vehicle_types(&network, &garage.vehicle_types);

        assert_eq!(graph_by_mode.keys().len(), 2);

        let car_network = graph_by_mode.remove(&car_id_internal).unwrap();
        assert_eq!(car_network.forward_first_out(), &vec![0, 0, 1, 3, 4]);
        assert_eq!(car_network.forward_head(), &vec![3, 2, 1, 2]);
        assert_eq!(car_network.forward_travel_time(), &vec![2, 2, 5, 5]);
        assert_eq!(car_network.forward_link_ids().len(), 4);

        let bike_network = graph_by_mode.remove(&bike_id_internal).unwrap();
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
        let garage = Garage::from_file(&PathBuf::from("./assets/adhoc_routing/vehicles.xml"));

        let vehicle_type2graph =
            NetworkConverter::convert_network_with_vehicle_types(&network, &garage.vehicle_types);

        // No link as mode "walk". So we expect the resulting walk network as empty.
        let walk_id = &Id::<String>::get_from_ext("walk").internal();
        let node_count = vehicle_type2graph.get(walk_id).unwrap().number_of_nodes();
        let link_count = vehicle_type2graph.get(walk_id).unwrap().number_of_links();

        assert_eq!(node_count, 7);
        assert_eq!(link_count, 0);

        // Test for mode "car"
        let car_id = &Id::<String>::get_from_ext("car").internal();
        let car_graph = vehicle_type2graph.get(car_id).unwrap();
        let link2_index = car_graph.forward_graph.first_out[2];
        assert_eq!(car_graph.forward_graph.travel_time[link2_index], 100);

        // Test for mode "bike"
        let bike_id = &Id::<String>::get_from_ext("bike").internal();
        let bike_graph = vehicle_type2graph.get(bike_id).unwrap();
        let link2_index = bike_graph.forward_graph.first_out[2];
        assert_eq!(bike_graph.forward_graph.travel_time[link2_index], 200);
    }
}
