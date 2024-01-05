use crate::test_simulation::{execute_sim, execute_sim_with_channels, TestSubscriber};
use rust_q_sim::simulation::config::CommandLineArgs;
use rust_q_sim::simulation::messaging::communication::communicators::DummySimCommunicator;

mod test_simulation;

#[test]
fn execute_3_links_single_part() {
    let config_args = CommandLineArgs {
        config_path: "./tests/resources/3-links/3-links-config-1.yml".to_string(),
        num_parts: None,
    };

    execute_sim(
        DummySimCommunicator(),
        Box::new(TestSubscriber::new_with_events_from_file(
            "./tests/resources/3-links/expected_events.xml",
        )),
        config_args,
    );
}

#[test]
fn execute_3_links_2_parts() {
    let config_args = CommandLineArgs {
        config_path: "./tests/resources/3-links/3-links-config-2.yml".to_string(),
        num_parts: None,
    };

    execute_sim_with_channels(config_args, "./tests/resources/3-links/expected_events.xml");
}
