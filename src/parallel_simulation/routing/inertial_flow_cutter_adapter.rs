use crate::parallel_simulation::network::routing_kit_network::RoutingKitNetwork;
use crate::parallel_simulation::routing::network_converter::NetworkConverter;
use std::fmt::Display;
use std::fs::{create_dir_all, remove_dir_all, remove_file, File};
use std::io::{BufRead, BufReader, Write};
use std::process::Command;

pub struct InertialFlowCutterAdapter<'adapter> {
    pub inertial_flow_cutter_path: &'adapter str,
    pub output_path: &'adapter str,
}

impl InertialFlowCutterAdapter<'_> {
    pub fn create_from_network_path<'adapter>(
        matsim_network_path: &'adapter str,
        inertial_flow_cutter_path: &'adapter str,
    ) -> InertialFlowCutterAdapter<'adapter> {
        let network = NetworkConverter::convert_xml_network(matsim_network_path);
        InertialFlowCutterAdapter::new(&network, inertial_flow_cutter_path)
    }

    pub fn new<'adapter, 'network>(
        routing_kit_network: &'network RoutingKitNetwork,
        inertial_flow_cutter_path: &'adapter str,
    ) -> InertialFlowCutterAdapter<'adapter> {
        let result = InertialFlowCutterAdapter {
            inertial_flow_cutter_path,
            output_path: "./output/",
        };
        result.serialize_routing_kit_network(routing_kit_network);
        result
    }

    fn call_console(&self) -> String {
        self.inertial_flow_cutter_path.to_owned() + &"/build/console"
    }

    fn temp_output_path(&self) -> String {
        self.output_path.to_owned() + &"temp/"
    }

    pub fn node_ordering(&mut self, save_ordering_to_file: bool) -> Vec<u32> {
        let node_ordering = self.call_node_ordering(save_ordering_to_file);
        //println!("The following node ordering was calculated: {:#?}", node_ordering);
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

        create_dir_all(self.temp_output_path().to_owned() + "binary")
            .expect("Failed to create directory.");

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
            remove_file(self.output_path.to_owned() + file + "_bin")
                .expect("Could not delete binary ordering file.");
            remove_dir_all(self.temp_output_path())
                .expect("Could not remove temporary output directory.");
        }
    }

    fn read_text_ordering(&self, output_file_name: &str) -> Vec<u32> {
        let ordering_file = File::open(self.output_path.to_owned() + output_file_name)
            .expect("Could not open file with node ordering");
        let buf = BufReader::new(ordering_file);
        let mut v = Vec::new();
        for line in buf.lines() {
            let n = line
                .expect("Could not read line.")
                .parse()
                .expect("Could not parse value.");
            v.push(n);
        }
        v
    }

    pub fn serialize_routing_kit_network(&self, routing_kit_network: &RoutingKitNetwork) {
        create_dir_all(self.temp_output_path())
            .expect("Failed to create temporary output directory.");

        InertialFlowCutterAdapter::serialize_vector(
            &routing_kit_network.first_out,
            self.temp_output_path().to_owned() + "first_out",
        );
        InertialFlowCutterAdapter::serialize_vector(
            &routing_kit_network.head,
            self.temp_output_path().to_owned() + "head",
        );
        InertialFlowCutterAdapter::serialize_vector(
            &routing_kit_network.travel_time,
            self.temp_output_path().to_owned() + "travel_time",
        );
        InertialFlowCutterAdapter::serialize_vector(
            &routing_kit_network.latitude,
            self.temp_output_path().to_owned() + "latitude",
        );
        InertialFlowCutterAdapter::serialize_vector(
            &routing_kit_network.longitude,
            self.temp_output_path().to_owned() + "longitude",
        );
    }

    pub(crate) fn serialize_vector<T: Display>(vector: &Vec<T>, output_file: String) {
        let mut file = File::create(output_file).expect("Unable to create file.");
        for i in vector {
            writeln!(file, "{}", i).expect("Unable to write into file.");
        }
    }
}

#[cfg(test)]
mod test {
    use crate::parallel_simulation::routing::inertial_flow_cutter_adapter::InertialFlowCutterAdapter;
    use crate::parallel_simulation::routing::network_converter::NetworkConverter;

    #[ignore]
    #[test]
    fn test_node_ordering() {
        // This seems to be more like an integration test which needs some steps to be done in advance
        // i.e. installation of InertialFlowCutter library and the required dependencies.
        // If you installed InertialFlowCutter locally this test will work. On github actions it doesn't so far.

        let network =
            NetworkConverter::convert_xml_network("./assets/routing_tests/triangle-network.xml");
        let mut flow_cutter = InertialFlowCutterAdapter::new(&network, "../InertialFlowCutter");

        let ordering = flow_cutter.node_ordering(false);

        assert_eq!(ordering, vec![2, 3, 1, 0])
    }

    #[test]
    fn test_serialization() {
        let network =
            NetworkConverter::convert_xml_network("./assets/routing_tests/triangle-network.xml");
        let adapter = InertialFlowCutterAdapter {
            inertial_flow_cutter_path: "",
            output_path: "./test_output/routing/network_converter/test_serialization/",
        };
        adapter.serialize_routing_kit_network(&network);
        // TODO implement test
    }
}
