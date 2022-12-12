use std::env;

use rust_q_sim::routing::network_converter::node_ordering_from_matsim_network;

pub fn main() {
    let args: Vec<String> = env::args().collect();
    assert_eq!(4, args.len(), "You have to provide args: <path to matsim network> <output path in routing kit format> <InertialFlowCutterPath>");
    let matsim_network_path = args.get(1).unwrap();
    let output_path = args.get(2).unwrap();
    let inertial_flow_cutter_path = args.get(3).unwrap();

    node_ordering_from_matsim_network(matsim_network_path, output_path, inertial_flow_cutter_path);
}