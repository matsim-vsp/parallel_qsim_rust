use crate::test_simulation::{execute_sim, execute_sim_with_channels, TestSubscriber};
use rust_q_sim::simulation::config::CommandLineArgs;
use rust_q_sim::simulation::id::store_to_file;
use rust_q_sim::simulation::messaging::events::EventsSubscriber;
use rust_q_sim::simulation::network::Network;
use rust_q_sim::simulation::population::Population;
use rust_q_sim::simulation::vehicles::garage::Garage;
use std::path::PathBuf;

mod test_simulation;

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

#[test]
fn execute_3_links_single_part() {
    let test_dir = PathBuf::from("./test_output/simulation/execute_3_links_single_part/");
    create_resources(&test_dir);

    let config_args = CommandLineArgs {
        config_path: "./tests/resources/3-links/3-links-config-1.yml".to_string(),
        num_parts: None,
    };

    execute_sim(
        vec![Box::new(TestSubscriber::new_with_events_from_file(
            "./tests/resources/3-links/expected_events.xml",
        ))],
        config_args,
    );
}

#[test]
fn execute_3_links_2_parts() {
    create_resources(&PathBuf::from(
        "./test_output/simulation/execute_3_links_2_parts/",
    ));
    let config_args = CommandLineArgs {
        config_path: "./tests/resources/3-links/3-links-config-2.yml".to_string(),
        num_parts: None,
    };

    execute_sim_with_channels(config_args, "./tests/resources/3-links/expected_events.xml");
}
