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

    let converter = NetworkConverter {
        matsim_network_path,
        output_path,
        inertial_flow_cutter_path,
    };

    let network = converter.convert_network();
    network.serialize(output_path);
    let node_ordering = converter.call_node_ordering();
    println!("The following node ordering was calculated: {:#?}", node_ordering);
}

struct NetworkConverter<'conv> {
    matsim_network_path: &'conv str,
    output_path: &'conv str,
    inertial_flow_cutter_path: &'conv str,
}

impl NetworkConverter<'_> {
    fn convert_network(&self) -> RoutingKitNetwork {
        let mut network = IONetwork::from_file(self.matsim_network_path);

        let mut first_out: Vec<EdgeId> = Vec::new();
        let mut head: Vec<NodeId> = Vec::new();
        let mut travel_time: Vec<Weight> = Vec::new();
        let mut latitude: Vec<f32> = Vec::new();
        let mut longitude: Vec<f32> = Vec::new();

        NetworkConverter::check_network_valid(&network);

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
                head.push(NetworkConverter::get_node_index(&network, &link.to) as NodeId);
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

    fn call_console(&self) -> String {
        self.inertial_flow_cutter_path.to_owned() + &"/build/console".to_owned()
    }

    fn call_node_ordering(&self) -> Vec<u32> {
        let file_names = vec!["head", "travel_time", "first_out", "latitude", "longitude"];
        for f in file_names {
            self.convert_network_into_binary(f);
        }

        let output_file_name = String::from("order");
        self.compute_ordering(&output_file_name);
        self.convert_ordering_into_text(&output_file_name);
        self.read_text_ordering(&output_file_name)
    }

    fn convert_network_into_binary(&self, file: &str) {
        println!("Converting file {file} into binary.");

        create_dir_all(self.output_path.to_owned() + &"/binary".to_owned()).expect("Failed to create directory.");

        Command::new(self.call_console())
            .arg("text_to_binary_vector")
            .arg(self.output_path.to_owned() + &"/".to_owned() + &file.to_owned())
            .arg(self.output_path.to_owned() + &"/binary/".to_owned() + &file.to_owned())
            .status()
            .expect("Failed to convert network into binary files.");
    }

    fn compute_ordering(&self, output_file_name: &str) {
        println!("Computing ordering for file {output_file_name}");

        create_dir_all(self.output_path.to_owned() + &"/ordering".to_owned()).expect("Failed to create directory.");

        Command::new("python3")
            .arg(self.inertial_flow_cutter_path.to_owned() + &"/inertialflowcutter_order.py".to_owned())
            .arg(self.output_path.to_owned() + &"/binary/".to_owned())
            .arg(self.output_path.to_owned() + &"/ordering/".to_owned() + &output_file_name.to_owned() + &"_bin".to_owned())
            .status()
            .expect("Failed to compute ordering");
    }

    fn convert_ordering_into_text(&self, file: &str) {
        println!("Converting ordering into text.");

        Command::new(self.call_console())
            .arg("binary_to_text_vector")
            .arg(self.output_path.to_owned() + &"/ordering/".to_owned() + &file.to_owned() + &"_bin".to_owned())
            .arg(self.output_path.to_owned() + &"/ordering/".to_owned() + &file.to_owned())
            .status()
            .expect("Failed to convert ordering into text.");
    }

    fn read_text_ordering(&self, output_file_name: &str) -> Vec<u32> {
        let ordering_file = File::open(self.output_path.to_owned() + &"/ordering/".to_owned() + &output_file_name.to_owned())
            .expect("Could not open file with node ordering");
        let buf = BufReader::new(ordering_file);
        let mut v = Vec::new();
        for line in buf.lines() {
            let n = line.expect("Could not read line.").parse().expect("Could not parse value.");
            v.push(n);
        };
        v
    }

    //checks whether network consists of unique node ids
    fn check_network_valid(network: &IONetwork) {
        let mut node_ids: Vec<String> = network.nodes().iter().map(|n| String::from(&n.id)).collect();
        node_ids.dedup();
        assert_eq!(node_ids.len(), network.nodes().len());
    }

    fn get_node_index(network: &IONetwork, id: &str) -> usize {
        network.nodes().iter().position(|node| node.id == *id).unwrap()
    }
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
        RoutingKitNetwork::serialize_vector(&self.first_out, output_folder.to_owned() + "/first_out");
        RoutingKitNetwork::serialize_vector(&self.head, output_folder.to_owned() + "/head");
        RoutingKitNetwork::serialize_vector(&self.travel_time, output_folder.to_owned() + "/travel_time");
        RoutingKitNetwork::serialize_vector(&self.latitude, output_folder.to_owned() + "/latitude");
        RoutingKitNetwork::serialize_vector(&self.longitude, output_folder.to_owned() + "/longitude");
    }

    fn serialize_vector<T: Display>(vector: &Vec<T>, output_file: String) {
        let mut file = File::create(output_file).expect("Unable to create file.");
        for i in vector {
            writeln!(file, "{}", i).expect("Unable to write into file.");
        }
    }
}

#[cfg(test)]
mod test {
    use crate::NetworkConverter;

    #[test]
    fn test_simple_network() {
        let converter = NetworkConverter {
            matsim_network_path: "./assets/routing_tests/triangle-network.xml",
            output_path: &String::new(),
            inertial_flow_cutter_path: &String::new(),
        };
        let network = converter.convert_network();
        println!("{network:#?}");

        assert_eq!(network.first_out, vec![0, 0, 2, 4, 5]);
        assert_eq!(network.head, vec![2, 3, 2, 3, 1]);
        assert_eq!(network.travel_time, vec![1, 2, 1, 4, 2]);
        // we don't check latitude and longitude so far
    }

    #[test]
    fn test_serialization() {
        let converter = NetworkConverter {
            matsim_network_path: "./assets/routing_tests/triangle-network.xml",
            output_path: &String::new(),
            inertial_flow_cutter_path: &String::new(),
        };
        let network = converter.convert_network();
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