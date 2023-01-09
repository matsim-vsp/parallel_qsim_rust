use std::fmt::Display;
use std::fs::{create_dir_all, remove_dir_all, remove_file, File};
use std::io::{BufRead, BufReader, Write};
use std::process::Command;

use rust_road_router::datastr::graph::{EdgeId, NodeId, Weight};

use crate::io::network::{IOLink, IONetwork, IONode};
use crate::parallel_simulation::network::routing_kit_network::RoutingKitNetwork;

pub struct NetworkConverter {}

impl NetworkConverter {
    pub fn convert_xml_network(matsim_network_path: &str) -> RoutingKitNetwork {
        NetworkConverter::convert_io_network(IONetwork::from_file(matsim_network_path))
    }

    pub fn convert_io_network(mut matsim_network: IONetwork) -> RoutingKitNetwork {
        let mut first_out: Vec<EdgeId> = Vec::new();
        let mut head: Vec<NodeId> = Vec::new();
        let mut travel_time: Vec<Weight> = Vec::new();
        let mut latitude: Vec<f32> = Vec::new();
        let mut longitude: Vec<f32> = Vec::new();

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
            longitude.push(node.x.floor());
            latitude.push(node.y.floor());

            first_out.push((links_before) as EdgeId);

            let links: Vec<&IOLink> = matsim_network
                .links()
                .iter()
                .filter(|link: &&IOLink| *link.from == node.id)
                .collect();
            links_before += links.len();
            for link in links {
                head.push(NetworkConverter::get_node_index(&matsim_network, &link.to) as NodeId);
                travel_time.push((link.length / link.freespeed) as Weight);
            }
        }
        first_out.push(head.len() as EdgeId);

        RoutingKitNetwork {
            first_out,
            head,
            travel_time,
            latitude,
            longitude,
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
    use crate::parallel_simulation::routing::network_converter::NetworkConverter;

    #[test]
    fn test_simple_network() {
        let network =
            NetworkConverter::convert_xml_network("./assets/routing_tests/triangle-network.xml");
        println!("{network:#?}");

        assert_eq!(network.first_out, vec![0, 0, 2, 4, 6]);
        assert_eq!(network.head, vec![2, 3, 2, 3, 1, 2]);
        assert_eq!(network.travel_time, vec![1, 2, 1, 4, 2, 5]);
        // we don't check latitude and longitude so far
    }
}
