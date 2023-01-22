use crate::event_test_utils::run_simulation_and_compare_events;
use rust_q_sim::config::{Config, RoutingMode};

mod event_test_utils;

#[test]
fn three_link_network() {
    let config = Config::builder()
        .network_file(String::from("./assets/3-links/3-links-network.xml"))
        .population_file(String::from("./assets/3-links/1-agent.xml"))
        .output_dir(String::from(
            "./test_output/controller_it/three_link_network",
        ))
        .num_parts(1)
        .build();
    run_simulation_and_compare_events(config, "tests/resources/three_link")
}

#[test]
fn three_link_network_adhoc_routing() {
    let config = Config::builder()
        .network_file(String::from("./assets/3-links/3-links-network.xml"))
        .population_file(String::from("./assets/3-links/1-agent-no-leg.xml"))
        .output_dir(String::from(
            "./test_output/controller_it/three_link_network",
        ))
        .num_parts(1)
        .set_routing_mode(RoutingMode::AdHoc)
        .build();
    run_simulation_and_compare_events(config, "tests/resources/three_link")
}
