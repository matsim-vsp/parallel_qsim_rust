use std::path::PathBuf;

use rust_q_sim::simulation::config::{CommandLineArgs, Config};
use rust_q_sim::simulation::id::store_to_file;
use rust_q_sim::simulation::logging;
use rust_q_sim::simulation::logging::init_std_out_logging;
use rust_q_sim::simulation::messaging::communication::communicators::DummySimCommunicator;
use rust_q_sim::simulation::network::global_network::Network;
use rust_q_sim::simulation::population::population::Population;
use rust_q_sim::simulation::vehicles::garage::Garage;

use crate::test_simulation::{execute_sim, execute_sim_with_channels, TestSubscriber};

mod test_simulation;

fn create_resources(in_dir: &PathBuf, out_dir: &PathBuf) {
    let net = Network::from_file_as_is(&in_dir.join("network.xml"));
    let mut garage = Garage::from_file(&in_dir.join("vehicles.xml"));
    let pop = Population::from_file(&in_dir.join("agents.xml"), &mut garage);

    store_to_file(&out_dir.join("ids.binpb"));
    net.to_file(&out_dir.join("network.binpb"));
    pop.to_file(&out_dir.join("agents.binpb"));
    garage.to_file(&out_dir.join("vehicles.binpb"));
}

#[test]
fn execute_adhoc_routing_one_part_no_updates() {
    create_resources(
        &PathBuf::from("./assets/adhoc_routing/no_updates/"),
        &PathBuf::from("./test_output/simulation/adhoc_routing/no_updates/one_part/"),
    );
    let config_args = CommandLineArgs {
        config_path: "./tests/resources/adhoc_routing/no_updates/config-1.yml".to_string(),
        num_parts: None,
    };

    execute_sim(
        DummySimCommunicator(),
        Box::new(TestSubscriber::new_with_events_from_file(
            "./tests/resources/adhoc_routing/no_updates/expected_events.xml",
        )),
        config_args,
    );
}

#[test]
#[serial_test::serial]
fn execute_adhoc_routing_two_parts_no_updates() {
    create_resources(
        &PathBuf::from("./assets/adhoc_routing/no_updates/"),
        &PathBuf::from("./test_output/simulation/adhoc_routing/no_updates/two_parts/"),
    );

    let config_args = CommandLineArgs {
        config_path: "./tests/resources/adhoc_routing/no_updates/config-2.yml".to_string(),
        num_parts: None,
    };

    execute_sim_with_channels(
        config_args,
        "./tests/resources/adhoc_routing/no_updates/expected_events.xml",
    );
}

#[test]
fn execute_adhoc_routing_one_part_with_updates() {
    create_resources(
        &PathBuf::from("./assets/adhoc_routing/with_updates/"),
        &PathBuf::from("./test_output/simulation/adhoc_routing/with_updates/one_part/"),
    );
    let args = CommandLineArgs {
        config_path: "./tests/resources/adhoc_routing/with_updates/config-1.yml".to_string(),
        num_parts: None,
    };

    execute_sim(
        DummySimCommunicator(),
        Box::new(TestSubscriber::new_with_events_from_file(
            "./tests/resources/adhoc_routing/with_updates/expected_events.xml",
        )),
        args,
    );
}

#[test]
#[serial_test::serial]
fn execute_adhoc_routing_two_parts_with_updates() {
    init_std_out_logging();

    create_resources(
        &PathBuf::from("./assets/adhoc_routing/with_updates/"),
        &PathBuf::from("./test_output/simulation/adhoc_routing/with_updates/two_parts/"),
    );

    let args = CommandLineArgs {
        config_path: "./tests/resources/adhoc_routing/with_updates/config-2.yml".to_string(),
        num_parts: None,
    };

    execute_sim_with_channels(
        args,
        "./tests/resources/adhoc_routing/with_updates/expected_events.xml",
    );
}
