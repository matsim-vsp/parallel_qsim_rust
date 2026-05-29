use itertools::Itertools;
use nohash_hasher::IntMap;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::info;

use crate::simulation::id::Id;
use crate::simulation::replanning::routing::graph::{
    CsrGraph, ForwardBackwardRoutingGraph, LinkIndex, NodeIndex,
};
use crate::simulation::scenario::network::{Link, Network};

#[allow(dead_code)]

pub fn convert_network_with_modes(
    network: Arc<Network>,
    modes: &Vec<Id<String>>,
) -> IntMap<Id<String>, ForwardBackwardRoutingGraph> {
    modes
        .iter()
        .map(|mode| {
            (
                mode.clone(),
                convert_network_for_mode(network.clone(), Some(mode.clone())),
            )
        })
        .collect()
}

pub(crate) fn convert_network_for_mode(
    network: Arc<Network>,
    mode: Option<Id<String>>,
) -> ForwardBackwardRoutingGraph {
    info!(
        "Converting network to forward backward graph for mode {:?}.",
        mode
    );

    let mut node_index_by_id = IntMap::default();
    let mut node_id_by_index = Vec::new();

    let mut forward_first_out = Vec::new();
    let mut forward_head = Vec::new();
    let mut forward_link_id_by_index = Vec::new();

    let mut backward_first_out = Vec::new();
    let mut backward_head = Vec::new();
    let mut backward_link_id_by_index = Vec::new();

    let nodes = network.get_all_nodes_sorted();
    let node_indices = nodes
        .iter()
        .enumerate()
        .map(|(i, node)| (&node.id, i))
        .collect::<HashMap<_, _>>();

    let mut forward_links_before = 0;
    let mut backward_links_before = 0;

    for (i, node) in nodes.iter().enumerate() {
        forward_first_out.push(forward_links_before as LinkIndex);
        backward_first_out.push(backward_links_before as LinkIndex);

        // keep track of which node id has which index in first_out
        node_index_by_id.insert(node.id.clone(), i as NodeIndex);
        node_id_by_index.push(node.id.clone());

        //calculate adjacent links
        let outgoing_links = get_links(&node.out_links, &network, mode.clone());
        forward_links_before += outgoing_links.len();

        let ingoing_links = get_links(&node.in_links, &network, mode.clone());
        backward_links_before += ingoing_links.len();

        //process outgoing links
        for link in outgoing_links {
            let to_node_index = *node_indices.get(&link.to).unwrap() as NodeIndex;

            forward_head.push(to_node_index);

            forward_link_id_by_index.push(link.id.clone());
        }

        //process ingoing links
        for link in ingoing_links {
            //Watch out: This is in the backward graph
            let to_node_index = *node_indices.get(&link.from).unwrap() as NodeIndex;

            backward_head.push(to_node_index);

            backward_link_id_by_index.push(link.id.clone());
        }
    }
    forward_first_out.push(forward_head.len() as LinkIndex);
    backward_first_out.push(backward_head.len() as LinkIndex);

    let forward_link_index_by_id = forward_link_id_by_index
        .iter()
        .enumerate()
        .map(|(i, id)| (id.clone(), i as LinkIndex))
        .collect::<IntMap<Id<Link>, LinkIndex>>();
    let backward_link_index_by_id = backward_link_id_by_index
        .iter()
        .enumerate()
        .map(|(i, id)| (id.clone(), i as LinkIndex))
        .collect::<IntMap<Id<Link>, LinkIndex>>();

    let forward_graph = CsrGraph::new(forward_first_out, forward_head);
    let backward_graph = CsrGraph::new(backward_first_out, backward_head);

    info!(
        "Finished converting network to forward backward graph for mode {:?}.",
        mode
    );

    ForwardBackwardRoutingGraph::new(
        forward_graph,
        backward_graph,
        Arc::new(network.nodes_with_ids().clone()), // node_id_to_node
        Arc::new(network.links_with_ids().clone()), // link_id_to_link
        node_index_by_id,
        node_id_by_index,
        forward_link_id_by_index,
        backward_link_id_by_index,
        forward_link_index_by_id,
        backward_link_index_by_id,
    )
}

fn get_links<'net>(
    link_ids: &'net [Id<Link>],
    network: &'net Network,
    mode: Option<Id<String>>,
) -> Vec<&'net Link> {
    link_ids
        .iter()
        .sorted_by_key(|&l| l.internal())
        .map(|l| network.get_link(l))
        .filter(|&l| {
            if let Some(m) = mode.clone() {
                // if a mode was given, only include links that allow this mode
                l.contains_mode(m)
            } else {
                // if no mode was given, include all links
                true
            }
        })
        .collect::<Vec<&Link>>()
}

#[cfg(test)]
mod test {
    use crate::simulation::config::{MetisOptions, PartitionMethod};
    use crate::simulation::id::Id;
    use crate::simulation::replanning::routing::graph::tests::{
        get_triangle_test_network, net_to_graph,
    };
    use crate::simulation::replanning::routing::network_converter;
    use crate::simulation::scenario::network::Network;
    use crate::simulation::scenario::vehicles::Garage;
    use crate::simulation::scenario::vehicles::InternalVehicleType;
    use std::path::PathBuf;
    use std::sync::Arc;

    #[test]
    fn test_simple_network() {
        let network = Network::from_file(
            "./assets/routing_tests/triangle-network.xml",
            1,
            &PartitionMethod::Metis(MetisOptions::default()),
        );
        let graph = network_converter::convert_network_for_mode(Arc::new(network), None);

        assert_eq!(graph.forward_first_out(), &vec![0usize, 0, 2, 4, 6]);
        assert_eq!(graph.forward_head(), &vec![2usize, 3, 2, 3, 1, 2]);
        assert_eq!(graph.forward_link_ids().len(), 6);

        assert_eq!(graph.backward_first_out(), &vec![0usize, 0, 1, 4, 6]);
        assert_eq!(graph.backward_head(), &vec![3usize, 1, 2, 3, 1, 2]);
        assert_eq!(graph.backward_link_ids().len(), 6);
        // we don't check y and y so far
    }

    /// test whether the network converter correctly creates separate graphs for different modes,
    /// and only includes links that allow the respective mode in each graph
    #[test]
    fn test_simple_network_with_modes() {
        let network = Network::from_file(
            "./assets/routing_tests/network_different_modes.xml",
            1,
            &PartitionMethod::Metis(MetisOptions::default()),
        );

        let car_mode_id = Id::<String>::get_from_ext("car");
        let bike_mode_id = Id::<String>::get_from_ext("bike");

        // create graphs based on the given network based on the given nodes
        let mut graph_by_mode = network_converter::convert_network_with_modes(
            Arc::new(network),
            &vec![car_mode_id.clone(), bike_mode_id.clone()],
        );

        assert_eq!(graph_by_mode.keys().len(), 2);

        let car_graph = graph_by_mode.remove(&car_mode_id.clone()).unwrap();
        assert_eq!(car_graph.forward_first_out(), &vec![0, 0, 1, 3, 4]);
        assert_eq!(car_graph.forward_head(), &vec![3, 2, 1, 2]);
        assert_eq!(car_graph.forward_link_ids().len(), 4);

        let bike_graph = graph_by_mode.remove(&bike_mode_id.clone()).unwrap();
        assert_eq!(bike_graph.forward_first_out(), &vec![0, 0, 1, 3, 4]);
        assert_eq!(bike_graph.forward_head(), &vec![2, 2, 3, 1]);
        assert_eq!(bike_graph.forward_link_ids().len(), 4);
    }

    /// Test that all links exist in both forward and backward directions
    #[test]
    fn test_all_links_in_both_directions() {
        let network = get_triangle_test_network();
        let graph = net_to_graph(&network);

        // Every link should exist in forward_link_ids
        // and also in backward_link_ids (possibly at different position)
        let forward_links = graph.forward_link_ids();
        let backward_links = graph.backward_link_ids();

        assert_eq!(
            forward_links.len(),
            backward_links.len(),
            "Should have same number of links in both directions"
        );

        for forward_link in forward_links {
            let found_in_backward = backward_links.iter().any(|bl| bl == forward_link);
            assert!(
                found_in_backward,
                "Every forward link should exist in backward links"
            );
        }
    }

    #[test]
    #[ignore] // ignore after architecture change. Need to consider whether we need this module at all.
    fn test_mode_filter() {
        let network = Network::from_file(
            "./assets/adhoc_routing/no_updates/network.xml",
            1,
            &PartitionMethod::Metis(MetisOptions::default()),
        );
        let garage = Garage::from_file(&PathBuf::from(
            "./assets/adhoc_routing/no_updates/vehicles.xml",
        ));

        let vehicle_type2graph = network_converter::convert_network_with_modes(
            Arc::new(network),
            &garage
                .vehicle_types
                .iter()
                .map(|(_vt_id, vt)| vt.net_mode.clone())
                .collect(),
        );

        // No link allows mode "walk". So the graph is expected to be empty.
        let walk_id = &Id::<InternalVehicleType>::get_from_ext("walk");
        assert!(
            vehicle_type2graph
                .get(&garage.vehicle_types[walk_id].net_mode)
                .unwrap()
                .forward_link_ids()
                .is_empty()
        );

        // Note: the part below was commented out because we no longer store travel time in the
        // graph.

        // // Test for mode "car"
        // let car_id = &Id::<InternalVehicleType>::get_from_ext("car");
        // let car_graph = vehicle_type2graph.get(car_id).unwrap();
        // let link2_index = car_graph.forward_graph.first_out[2];
        // // assert_eq!(car_graph.forward_graph.travel_time[link2_index], 100);
        //
        // // Test for mode "bike"
        // let bike_id = &Id::<InternalVehicleType>::get_from_ext("bike");
        // let bike_graph = vehicle_type2graph.get(bike_id).unwrap();
        // let link2_index = bike_graph.forward_graph.first_out[2];
        // assert_eq!(bike_graph.forward_graph.travel_time[link2_index], 200);
    }

    #[test]
    #[ignore] // ignore after architecture change. Need to consider whether we need this module at all.
    fn test_different_veh_types_same_net_mode() {
        let network = Network::from_file(
            "./assets/adhoc_routing/no_updates/network.xml",
            1,
            &PartitionMethod::Metis(MetisOptions::default()),
        );
        let garage = Garage::from_file(&PathBuf::from(
            "./assets/mode_dependent_routing/vehicle_definitions.xml",
        ));

        let vehicle_type2graph = network_converter::convert_network_with_modes(
            Arc::new(network),
            &garage
                .vehicle_types
                .iter()
                .map(|(_vt_id, vt)| vt.net_mode.clone())
                .collect(),
        );

        assert_eq!(vehicle_type2graph.keys().len(), 2);

        // Note: the part below was commented out since we no longer store travel times in the
        // graph

        // assert_eq!(
        //     vehicle_type2graph
        //         .get(&Id::<InternalVehicleType>::get_from_ext("car"))
        //         .unwrap()
        //         .forward_graph
        //         .travel_time,
        //     vec![10, 10, 50, 100, 10, 10, 50]
        // );
        //
        // assert_eq!(
        //     vehicle_type2graph
        //         .get(&Id::<InternalVehicleType>::get_from_ext("bike"))
        //         .unwrap()
        //         .forward_graph
        //         .travel_time,
        //     vec![20, 20, 200, 200, 20, 20, 200]
        // );
    }
}
