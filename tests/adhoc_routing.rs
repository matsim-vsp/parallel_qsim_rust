use crate::event_test_utils::run_simulation_and_compare_events;
use rust_q_sim::config::{Config, RoutingMode};

mod event_test_utils;

#[test]
fn adhoc_routing() {
    let config = Config::builder()
        .network_file(String::from("./assets/adhoc_routing/network.xml"))
        .population_file(String::from("./assets/adhoc_routing/agents_no_leg.xml"))
        .output_dir(String::from("./test_output/controller_it/adhoc_routing"))
        .num_parts(1)
        .set_routing_mode(RoutingMode::AdHoc)
        .build();
    run_simulation_and_compare_events(config, "tests/resources/adhoc_routing")
}
