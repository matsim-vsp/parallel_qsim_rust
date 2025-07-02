use std::path::PathBuf;

mod test_simulation;
use rust_q_sim::simulation::config::CommandLineArgs;
use rust_q_sim::simulation::id::store_to_file;
use rust_q_sim::simulation::messaging::sim_communication::local_communicator::DummySimCommunicator;
use rust_q_sim::simulation::network::Network;
use rust_q_sim::simulation::population::Population;
use rust_q_sim::simulation::vehicles::garage::Garage;
use test_simulation::{execute_sim, execute_sim_with_channels, TestSubscriber};

fn create_resources(out_dir: &PathBuf) {
    let input_dir = PathBuf::from("./assets/equil/");
    let net = Network::from_file_as_is(&input_dir.join("equil-network.xml"));
    let mut garage = Garage::from_file(&input_dir.join("equil-vehicles.xml"));
    let pop = Population::from_file(&input_dir.join("equil-1-plan.xml"), &mut garage);

    store_to_file(&out_dir.join("ids.binpb"));
    net.to_file(&out_dir.join("equil-network.binpb"));
    pop.to_file(&out_dir.join("equil-1-plan.binpb"));
    garage.to_file(&out_dir.join("equil-vehicles.binpb"));
}

#[test]
fn execute_equil_single_part() {
    let test_dir = PathBuf::from("./test_output/simulation/equil_single_part/");
    create_resources(&test_dir);

    let config_args = CommandLineArgs {
        config_path: "./tests/resources/equil/equil-config-1.yml".to_string(),
        num_parts: None,
    };

    execute_sim(
        DummySimCommunicator(),
        Box::new(TestSubscriber::new_with_events_from_file(
            "./tests/resources/equil/expected_events.xml",
        )),
        config_args,
    );
}

#[test]
fn execute_equil_2_parts() {
    let test_dir = PathBuf::from("./test_output/simulation/equil_with_channels/");
    create_resources(&test_dir);

    let config_args = CommandLineArgs {
        config_path: "./tests/resources/equil/equil-config-2.yml".to_string(),
        num_parts: None,
    };

    execute_sim_with_channels(config_args, "./tests/resources/equil/expected_events.xml");
}
