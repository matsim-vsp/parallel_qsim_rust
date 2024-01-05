use crate::test_simulation::{execute_sim, execute_sim_with_channels, TestSubscriber};
use rust_q_sim::simulation::config::CommandLineArgs;
use rust_q_sim::simulation::messaging::communication::communicators::DummySimCommunicator;

mod test_simulation;

#[test]
fn execute_adhoc_routing_one_part_no_updates() {
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
fn execute_adhoc_routing_two_parts_no_updates() {
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
    let config_args = CommandLineArgs {
        config_path: "./tests/resources/adhoc_routing/with_updates/config-1.yml".to_string(),
        num_parts: None,
    };

    execute_sim(
        DummySimCommunicator(),
        Box::new(TestSubscriber::new_with_events_from_file(
            "./tests/resources/adhoc_routing/with_updates/expected_events.xml",
        )),
        config_args,
    );
}

#[test]
fn execute_adhoc_routing_two_parts_with_updates() {
    let config_args = CommandLineArgs {
        config_path: "./tests/resources/adhoc_routing/with_updates/config-2.yml".to_string(),
        num_parts: None,
    };

    execute_sim_with_channels(
        config_args,
        "./tests/resources/adhoc_routing/with_updates/expected_events.xml",
    );
}
