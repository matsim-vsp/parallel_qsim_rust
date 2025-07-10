mod test_simulation;

use std::path::PathBuf;
use rust_q_sim::simulation::config::CommandLineArgs;
use rust_q_sim::simulation::id::store_to_file;
use rust_q_sim::simulation::messaging::sim_communication::local_communicator::DummySimCommunicator;
use rust_q_sim::simulation::network::Network;
use rust_q_sim::simulation::population::Population;
use rust_q_sim::simulation::vehicles::garage::Garage;

use test_simulation::{execute_sim, TestSubscriber};

fn create_resources(out_dir: &PathBuf) {
    let input_dir = PathBuf::from("./assets/3-links/");
    let net = Network::from_file_as_is(&input_dir.join("3-links-network.xml"));
    let mut garage = Garage::from_file(&input_dir.join("vehicles.xml"));
    let pop = Population::from_file(&input_dir.join("1-agent-full-leg.xml"), &mut garage);

    store_to_file(&out_dir.join("ids.binpb"));
    net.to_file(&out_dir.join("3-links-network.binpb"));
    pop.to_file(&out_dir.join("1-agent-full-leg.binpb"));
    garage.to_file(&out_dir.join("vehicles.binpb"));
}

// one agent having a network route, car being not a main mode => simulation should teleport the agent
#[test]
fn teleport_links_main_mode_not_car() {
    let test_dir =
        PathBuf::from("./test_output/simulation/output-teleport-links-main-mode-not-car/");
    create_resources(&test_dir);

    let config_args = CommandLineArgs {
        config_path: "./tests/resources/equil/teleport-links-main-mode-not-car.yml".to_string(),
        num_parts: None,
    };

    execute_sim(
        DummySimCommunicator(),
        Box::new(TestSubscriber::new_with_events_from_file(
            "./tests/resources/equil/expected_events_teleport_links_main_mode_not_car.xml",
        )),
        config_args,
    );
}

// one agent having a generic route, car being not a main mode => simulation should teleport the agent
#[test]
fn teleport_generic_main_mode_not_car() {
    let test_dir =
        PathBuf::from("./test_output/simulation/output-teleport-generic-main-mode-not-car/");
    create_resources(&test_dir);

    let config_args = CommandLineArgs {
        config_path: "./tests/resources/equil/teleport-generic-main-mode-not-car.yml".to_string(),
        num_parts: None,
    };

    execute_sim(
        DummySimCommunicator(),
        Box::new(TestSubscriber::new_with_events_from_file(
            "./tests/resources/equil/expected_events_teleport_generic_main_mode_not_car.xml",
        )),
        config_args,
    );
}

// one agent having a network route, car being a main mode => already implemented
#[test]
fn teleport_links_main_mode_car() {
    let test_dir =
        PathBuf::from("./test_output/simulation/output-teleport-links-main-mode-car/");
    create_resources(&test_dir);

    let config_args = CommandLineArgs {
        config_path: "./tests/resources/equil/teleport-links-main-mode-car.yml".to_string(),
        num_parts: None,
    };

    execute_sim(
        DummySimCommunicator(),
        Box::new(TestSubscriber::new_with_events_from_file(
            "./tests/resources/equil/expected_events_teleport_links_main_mode_car.xml",
        )),
        config_args,
    );
}

// one agent having a generic route, car being a main mode => simulation should crash
#[test]
#[should_panic]
fn teleport_generic_main_mode_car() {
    let test_dir =
        PathBuf::from("./test_output/simulation/output-teleport-generic-main-mode-car/");
    create_resources(&test_dir);

    let config_args = CommandLineArgs {
        config_path: "./tests/resources/equil/teleport-generic-main-mode-car.yml".to_string(),
        num_parts: None,
    };

    execute_sim(
        DummySimCommunicator(),
        Box::new(TestSubscriber::new_with_events_from_file(
            "./tests/resources/equil/expected_events_teleport_generic_main_mode_car.xml",
        )),
        config_args,
    );
}
