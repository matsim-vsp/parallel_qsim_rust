use std::fmt::Display;
use std::fs::{create_dir_all, File, remove_dir_all, remove_file};
use std::io::{BufRead, BufReader, Write};
use std::process::Command;

use rust_road_router::datastr::graph::{EdgeId, NodeId, Weight};

use crate::io::network::{IOLink, IONetwork, IONode};

pub fn node_ordering_from_matsim_network(matsim_network_path: &str, output_path: &str, inertial_flow_cutter_path: &str, save_ordering_to_file: bool) -> Vec<u32> {
    let mut converter = NetworkConverter {
        matsim_network_path,
        output_path,
        inertial_flow_cutter_path,
        routing_kit_network: None,
    };
    converter.node_ordering(save_ordering_to_file)
}

pub struct NetworkConverter<'conv> {
    pub matsim_network_path: &'conv str,
    pub output_path: &'conv str,
    pub inertial_flow_cutter_path: &'conv str,
    pub routing_kit_network: Option<RoutingKitNetwork>,
}

impl NetworkConverter<'_> {
    pub fn convert_network(&mut self) {
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

        self.routing_kit_network = Option::from(RoutingKitNetwork {
            first_out,
            head,
            travel_time,
            latitude,
            longitude,
        });
    }

    fn call_console(&self) -> String {
        self.inertial_flow_cutter_path.to_owned() + &"/build/console"
    }

    fn temp_output_path(&self) -> String {
        self.output_path.to_owned() + &"temp/"
    }

    pub fn node_ordering(&mut self, save_ordering_to_file: bool) -> Vec<u32> {
        match self.routing_kit_network.as_ref() {
            None => self.convert_network(),
            _ => {}
        }

        self.serialize_routing_kit_network();
        let node_ordering = self.call_node_ordering(save_ordering_to_file);
        println!("The following node ordering was calculated: {:#?}", node_ordering);
        node_ordering
    }

    fn call_node_ordering(&self, save_ordering_to_file: bool) -> Vec<u32> {
        let file_names = vec!["head", "travel_time", "first_out", "latitude", "longitude"];
        for f in file_names {
            self.convert_network_into_binary(f);
        }

        let output_file_name = String::from("order");
        self.compute_ordering(&output_file_name);
        self.convert_ordering_into_text(&output_file_name);
        let ordering = self.read_text_ordering(&output_file_name);
        self.clean_temp_directory(&output_file_name, save_ordering_to_file);
        ordering
    }

    fn convert_network_into_binary(&self, file: &str) {
        println!("Converting file {file} into binary.");

        create_dir_all(self.temp_output_path().to_owned() + "binary").expect("Failed to create directory.");

        Command::new(self.call_console())
            .arg("text_to_binary_vector")
            .arg(self.temp_output_path().to_owned() + file)
            .arg(self.temp_output_path().to_owned() + &"binary/" + &file)
            .status()
            .expect("Failed to convert network into binary files.");
    }

    fn compute_ordering(&self, output_file_name: &str) {
        println!("Computing ordering and store in binary file '{output_file_name}'");

        Command::new("python3")
            .arg(self.inertial_flow_cutter_path.to_owned() + "/inertialflowcutter_order.py")
            .arg(self.temp_output_path().to_owned() + "binary/")
            .arg(self.output_path.to_owned() + output_file_name + "_bin")
            .status()
            .expect("Failed to compute ordering");
    }

    fn convert_ordering_into_text(&self, file: &str) {
        println!("Converting ordering into text.");

        Command::new(self.call_console())
            .arg("binary_to_text_vector")
            .arg(self.output_path.to_owned() + file + "_bin")
            .arg(self.output_path.to_owned() + file)
            .status()
            .expect("Failed to convert ordering into text.");
    }

    fn clean_temp_directory(&self, file: &str, save_ordering_to_file: bool) {
        if !save_ordering_to_file {
            remove_dir_all(self.output_path).expect("Could not delete whole output directory.");
        } else {
            remove_file(self.output_path.to_owned() + file + "_bin").expect("Could not delete binary ordering file.");
            remove_dir_all(self.temp_output_path()).expect("Could not remove temporary output directory.");
        }
    }

    fn read_text_ordering(&self, output_file_name: &str) -> Vec<u32> {
        let ordering_file = File::open(self.output_path.to_owned() + output_file_name)
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

    fn serialize_routing_kit_network(&self) {
        create_dir_all(self.temp_output_path()).expect("Failed to create temporary output directory.");

        RoutingKitNetwork::serialize_vector(&self.routing_kit_network.as_ref().unwrap().first_out, self.temp_output_path().to_owned() + "/first_out");
        RoutingKitNetwork::serialize_vector(&self.routing_kit_network.as_ref().unwrap().head, self.temp_output_path().to_owned() + "/head");
        RoutingKitNetwork::serialize_vector(&self.routing_kit_network.as_ref().unwrap().travel_time, self.temp_output_path().to_owned() + "/travel_time");
        RoutingKitNetwork::serialize_vector(&self.routing_kit_network.as_ref().unwrap().latitude, self.temp_output_path().to_owned() + "/latitude");
        RoutingKitNetwork::serialize_vector(&self.routing_kit_network.as_ref().unwrap().longitude, self.temp_output_path().to_owned() + "/longitude");
    }
}


#[derive(Debug)]
pub struct RoutingKitNetwork {
    //CSR graph representation
    first_out: Vec<EdgeId>,
    head: Vec<NodeId>,
    travel_time: Vec<Weight>,
    latitude: Vec<f32>,
    longitude: Vec<f32>,
}

impl RoutingKitNetwork {
    fn serialize_vector<T: Display>(vector: &Vec<T>, output_file: String) {
        let mut file = File::create(output_file).expect("Unable to create file.");
        for i in vector {
            writeln!(file, "{}", i).expect("Unable to write into file.");
        }
    }
    pub fn first_out(&self) -> &Vec<EdgeId> {
        &self.first_out
    }
    pub fn head(&self) -> &Vec<NodeId> {
        &self.head
    }
    pub fn travel_time(&self) -> &Vec<Weight> {
        &self.travel_time
    }
}

#[cfg(test)]
mod test {
    use crate::routing::network_converter::{NetworkConverter, node_ordering_from_matsim_network};

    #[test]
    fn test_simple_network() {
        let mut converter = NetworkConverter {
            matsim_network_path: "./assets/routing_tests/triangle-network.xml",
            output_path: &String::new(),
            inertial_flow_cutter_path: &String::new(),
            routing_kit_network: None,
        };
        converter.convert_network();
        let network = converter.routing_kit_network.as_ref().unwrap();
        println!("{network:#?}");

        assert_eq!(network.first_out, vec![0, 0, 2, 4, 6]);
        assert_eq!(network.head, vec![2, 3, 2, 3, 1, 2]);
        assert_eq!(network.travel_time, vec![1, 2, 1, 4, 2, 5]);
        // we don't check latitude and longitude so far
    }

    #[test]
    fn test_serialization() {
        let mut converter = NetworkConverter {
            matsim_network_path: "./assets/routing_tests/triangle-network.xml",
            output_path: "./assets/routing_tests/serialization/",
            inertial_flow_cutter_path: &String::new(),
            routing_kit_network: None,
        };
        converter.convert_network();
        converter.serialize_routing_kit_network();
        // TODO implement test
    }

    #[ignore]
    #[test]
    fn test_node_ordering() {
        // This seems to be more like an integration test which needs some steps to be done in advance
        // i.e. installation of InertialFlowCutter library and the required dependencies.
        // If you installed InertialFlowCutter locally this test will work. On github actions it doesn't so far.
        let ordering = node_ordering_from_matsim_network("./assets/routing_tests/triangle-network.xml", "./assets/routing_tests/conversion/", "../InertialFlowCutter", false);
        assert_eq!(ordering, vec![2, 3, 1, 0])
    }
}