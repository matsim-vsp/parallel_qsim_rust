use std::env;
use std::fmt::Display;
use std::fs::{create_dir_all, File};
use std::io::{BufRead, BufReader, Write};
use std::process::Command;

use rust_road_router::datastr::graph::{EdgeId, NodeId, Weight};

use rust_q_sim::io::network::{IOLink, IONetwork, IONode};

pub fn main() {
    let args: Vec<String> = env::args().collect();
    assert_eq!(4, args.len(), "You have to provide args: <path to matsim network> <output path in routing kit format> <InertialFlowCutterPath>");
    let matsim_network_path = args.get(1).unwrap();
    let output_path = args.get(2).unwrap();
    let inertial_flow_cutter_path = args.get(3).unwrap();

    let network = convert_network(matsim_network_path);
    network.serialize(output_path);
    let node_ordering = call_node_ordering(inertial_flow_cutter_path, output_path);
    println!("The following node ordering was calculated: {:#?}", node_ordering);
}

fn call_node_ordering(inertial_flow_cutter_path: &String, output_path: &String) -> Vec<u32> {
    let file_names = vec!["head", "travel_time", "first_out", "latitude", "longitude"];
    for f in file_names {
        convert_network_into_binary(&inertial_flow_cutter_path, output_path, f);
    }

    let output_file_name = String::from("order");
    compute_ordering(&inertial_flow_cutter_path, output_path, &output_file_name);
    convert_ordering_into_text(&inertial_flow_cutter_path, output_path, &output_file_name);
    read_text_ordering(output_path, &output_file_name)
}

fn read_text_ordering(output_path: &String, output_file_name: &String) -> Vec<u32> {
    let ordering_file = File::open(output_path.to_owned() + &"/ordering/".to_owned() + &output_file_name.to_owned())
        .expect("Could not open file with node ordering");
    let buf = BufReader::new(ordering_file);
    let mut v = Vec::new();
    for line in buf.lines() {
        let n = line.expect("Could not read line.").parse().expect("Could not parse value.");
        v.push(n);
    };
    v
}

fn convert_ordering_into_text(inertial_flow_path: &str, data_path: &str, file: &str) {
    println!("Converting ordering into text.");

    Command::new(inertial_flow_path.to_owned() + &"/build/console".to_owned())
        .arg("binary_to_text_vector")
        .arg(data_path.to_owned() + &"/ordering/".to_owned() + &file.to_owned() + &"_bin".to_owned())
        .arg(data_path.to_owned() + &"/ordering/".to_owned() + &file.to_owned())
        .status()
        .expect("Failed to convert ordering into text.");
}

fn compute_ordering(inertial_flow_path: &str, data_path: &str, output_file_name: &str) {
    println!("Computing ordering for file {output_file_name}");

    create_dir_all(data_path.to_owned() + &"/ordering".to_owned()).expect("Failed to create directory.");

    Command::new("python3")
        .arg(inertial_flow_path.to_owned() + &"/inertialflowcutter_order.py".to_owned())
        .arg(data_path.to_owned() + &"/binary/".to_owned())
        .arg(data_path.to_owned() + &"/ordering/".to_owned() + &output_file_name.to_owned() + &"_bin".to_owned())
        .status()
        .expect("Failed to compute ordering");
}

fn convert_network_into_binary(inertial_flow_path: &str, data_path: &str, file: &str) {
    println!("Converting file {file} into binary.");

    create_dir_all(data_path.to_owned() + &"/binary".to_owned()).expect("Failed to create directory.");

    Command::new(inertial_flow_path.to_owned() + &"/build/console".to_owned())
        .arg("text_to_binary_vector")
        .arg(data_path.to_owned() + &"/".to_owned() + &file.to_owned())
        .arg(data_path.to_owned() + &"/binary/".to_owned() + &file.to_owned())
        .status()
        .expect("Failed to convert network into binary files.");
}

fn convert_network(path: &str) -> RoutingKitNetwork {
    let mut network = IONetwork::from_file(path);

    let mut first_out: Vec<EdgeId> = Vec::new();
    let mut head: Vec<NodeId> = Vec::new();
    let mut travel_time: Vec<Weight> = Vec::new();
    let mut latitude: Vec<f32> = Vec::new();
    let mut longitude: Vec<f32> = Vec::new();

    check_network_valid(&network);

    //sort links by from id
    network.links_mut().sort_by_key(|link: &IOLink| link.from.to_lowercase());
    //sort nodes by id
    network.nodes_mut().sort_by_key(|node: &IONode| node.id.to_lowercase());

    let mut links_before = 0;

    for node in network.nodes() {
        //TODO: make sure, that the coordinate system is correct
        longitude.push(node.x);
        latitude.push(node.y);

        first_out.push((links_before) as EdgeId);

        let links: Vec<&IOLink> = network.links().iter().filter(|link: &&IOLink| *link.from == node.id).collect();
        links_before += links.len();
        for link in links {
            head.push(get_node_index(&network, &link.to) as NodeId);
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
    let mut node_ids: Vec<String> = network.nodes().iter().map(|n| String::from(&n.id)).collect();
    node_ids.dedup();
    assert_eq!(node_ids.len(), network.nodes().len());
}

fn get_node_index(network: &IONetwork, id: &String) -> usize {
    network.nodes().iter().position(|node| node.id == *id).unwrap()
}

#[derive(Debug)]
struct RoutingKitNetwork {
    //CSR graph representation
    first_out: Vec<EdgeId>,
    head: Vec<NodeId>,
    travel_time: Vec<Weight>,
    latitude: Vec<f32>,
    longitude: Vec<f32>,
}

impl RoutingKitNetwork {
    fn serialize(&self, output_folder: &str) {
        serialize_vector(&self.first_out, output_folder.to_owned() + "/first_out");
        serialize_vector(&self.head, output_folder.to_owned() + "/head");
        serialize_vector(&self.travel_time, output_folder.to_owned() + "/travel_time");
        serialize_vector_float(&self.latitude, output_folder.to_owned() + "/latitude");
        serialize_vector_float(&self.longitude, output_folder.to_owned() + "/longitude");
    }
}

fn serialize_vector<T: Display>(vector: &Vec<T>, output_file: String) {
    let mut file = File::create(output_file).expect("Unable to create file.");
    for i in vector {
        writeln!(file, "{}", i).expect("Unable to write into file.");
    }
}

fn serialize_vector_float(vector: &Vec<f32>, output_file: String) {
    let mut file = File::create(output_file).expect("Unable to create file.");
    for i in vector {
        writeln!(file, "{}", i).expect("Unable to write into file.");
    }
}

#[cfg(test)]
mod test {
    use crate::convert_network;

    #[test]
    fn test_simple_network() {
        let network = convert_network("./assets/routing_tests/triangle-network.xml");
        println!("{network:#?}");

        assert_eq!(network.first_out, vec![0, 0, 2, 4, 5]);
        assert_eq!(network.head, vec![2, 3, 2, 3, 1]);
        assert_eq!(network.travel_time, vec![1, 2, 1, 4, 2]);
        // we don't check latitude and longitude so far
    }

    #[test]
    fn test_serialization() {
        let network = convert_network("./assets/routing_tests/triangle-network.xml");
        network.serialize(&String::from("./assets/routing_tests/serialization"));
        // TODO implement test
    }

    #[test]
    fn test_node_ordering() {
        // This seems to be more like an integration test which needs some steps to be done in advance
        // i.e. installation of InertialFlowCutter library and the required dependencies.
        // I think it's ok so far to check that program flow manually.
    }
}