use crate::simulation::network::routing_kit_network::RoutingKitNetwork;
use log::{debug, info};
use std::fmt::Display;
use std::fs::{create_dir_all, remove_dir_all, remove_file, File};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};

pub struct InertialFlowCutterAdapter {
    pub inertial_flow_cutter_path: PathBuf,
    pub output_folder: PathBuf,
}

impl InertialFlowCutterAdapter {
    pub fn new(
        routing_kit_network: &RoutingKitNetwork,
        inertial_flow_cutter_path: PathBuf,
        output_folder: PathBuf,
    ) -> InertialFlowCutterAdapter {
        let result = InertialFlowCutterAdapter {
            inertial_flow_cutter_path,
            output_folder,
        };
        result.serialize_routing_kit_network(routing_kit_network);
        result
    }

    fn call_console(&self) -> PathBuf {
        self.inertial_flow_cutter_path.join("build").join("console")
    }

    fn temp_output_path(&self) -> PathBuf {
        self.output_folder.join("temp")
    }

    pub fn node_ordering(&mut self, save_ordering_to_file: bool) -> Vec<u32> {
        info!("Compute node ordering.");
        let node_ordering = self.call_node_ordering(save_ordering_to_file);
        //println!("The following node ordering was calculated: {:#?}", node_ordering);
        node_ordering
    }

    // The magic happens here
    fn call_node_ordering(&self, save_ordering_to_file: bool) -> Vec<u32> {
        let file_names = vec!["head", "travel_time", "first_out", "latitude", "longitude"];
        for f in file_names {
            let contains_float = f.eq("latitude") || f.eq("longitude");
            self.convert_file_into_binary(f, contains_float);
        }

        let output_file_name = String::from("order");
        self.compute_ordering(&output_file_name);
        self.convert_ordering_into_text(&output_file_name);
        let ordering = self.read_text_ordering(&output_file_name);
        self.clean_temp_directory(&output_file_name, save_ordering_to_file);
        ordering
    }

    fn convert_file_into_binary(&self, file: &str, contains_float: bool) {
        debug!("Converting file {} into binary.", file);

        create_dir_all(self.temp_output_path().join("binary"))
            .expect("Failed to create directory.");

        let converter_command = match contains_float {
            false => "text_to_binary_vector",
            true => "text_to_binary_float_vector",
        };

        Command::new(self.call_console())
            .arg(converter_command)
            .arg(self.temp_output_path().join(file))
            .arg(self.temp_output_path().join("binary/").join(file))
            .status()
            .expect("Failed to convert network into binary files.");
    }

    fn compute_ordering(&self, output_file_name: &str) {
        debug!(
            "Computing ordering and store in binary file '{}'",
            output_file_name
        );

        Command::new("python3")
            .arg(
                self.inertial_flow_cutter_path
                    .join("inertialflowcutter_order.py"),
            )
            .arg(self.temp_output_path().join("binary/"))
            .arg(
                self.output_folder
                    .join(output_file_name)
                    .to_str()
                    .unwrap()
                    .to_owned()
                    + "_bin",
            )
            .stdout(Stdio::null())
            .status()
            .expect("Failed to compute ordering");
    }

    fn convert_ordering_into_text(&self, file: &str) {
        debug!("Converting ordering into text.");

        Command::new(self.call_console())
            .arg("binary_to_text_vector")
            .arg(self.output_folder.join(file).to_str().unwrap().to_owned() + "_bin")
            .arg(self.output_folder.join(file))
            .status()
            .expect("Failed to convert ordering into text.");
    }

    fn clean_temp_directory(&self, file: &str, save_ordering_to_file: bool) {
        debug!("Cleaning temp output directory.");
        if !save_ordering_to_file {
            remove_dir_all(&self.output_folder).expect("Could not delete whole output directory.");
        } else {
            remove_file(self.output_folder.join(file).join("_bin"))
                .expect("Could not delete binary ordering file.");
            remove_dir_all(self.temp_output_path())
                .expect("Could not remove temporary output directory.");
        }
    }

    fn read_text_ordering(&self, output_file_name: &str) -> Vec<u32> {
        let ordering_file = File::open(self.output_folder.join(output_file_name))
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

        debug!(
            "Serialize Network now with path {:?}.",
            self.temp_output_path()
        );
        InertialFlowCutterAdapter::serialize_vector(
            &routing_kit_network.first_out,
            self.temp_output_path().join("first_out"),
        );
        InertialFlowCutterAdapter::serialize_vector(
            &routing_kit_network.head,
            self.temp_output_path().join("head"),
        );
        InertialFlowCutterAdapter::serialize_vector(
            &routing_kit_network.travel_time,
            self.temp_output_path().join("travel_time"),
        );
        InertialFlowCutterAdapter::serialize_vector(
            &routing_kit_network.latitude,
            self.temp_output_path().join("latitude"),
        );
        InertialFlowCutterAdapter::serialize_vector(
            &routing_kit_network.longitude,
            self.temp_output_path().join("longitude"),
        );
    }

    pub(crate) fn serialize_vector<T: Display>(vector: &Vec<T>, output_file: PathBuf) {
        let mut file = File::create(output_file).expect("Unable to create file.");
        for i in vector {
            writeln!(file, "{}", i).expect("Unable to write into file.");
        }
    }
}

#[cfg(test)]
mod test {
    use crate::simulation::routing::inertial_flow_cutter_adapter::InertialFlowCutterAdapter;
    use crate::simulation::routing::network_converter::NetworkConverter;
    use std::env;
    use std::path::PathBuf;

    #[test]
    fn test_node_ordering() {
        let inertial_flow_cutter_path = env::var("INERTIAL_FLOW_CUTTER_HOME_DIRECTORY")
            .expect("The environment variable 'INERTIAL_FLOW_CUTTER_HOME_DIRECTORY' is not set.");

        let network =
            NetworkConverter::convert_xml_network("./assets/routing_tests/triangle-network.xml");
        let mut flow_cutter = InertialFlowCutterAdapter::new(
            &network,
            PathBuf::from(inertial_flow_cutter_path),
            PathBuf::from("./test_output/routing/node_ordering/"),
        );

        let ordering = flow_cutter.node_ordering(false);

        assert_eq!(ordering, vec![2, 3, 1, 0])
    }

    #[test]
    fn test_serialization() {
        let network =
            NetworkConverter::convert_xml_network("./assets/routing_tests/triangle-network.xml");
        let adapter = InertialFlowCutterAdapter {
            inertial_flow_cutter_path: PathBuf::from(""),
            output_folder: PathBuf::from(
                "./test_output/routing/network_converter/test_serialization/",
            ),
        };
        adapter.serialize_routing_kit_network(&network);
        // TODO implement test
    }
}
