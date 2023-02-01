use crate::event_test_utils::{compare_events, run_mpi_simulation_and_convert_events};
use serial_test::serial;

mod event_test_utils;

#[test]
#[serial]
fn test_three_link_without_routing() {
    test_three_link("use-plans", "assets/3-links/1-agent.xml", "static")
}

#[test]
#[serial]
fn test_three_link_with_routing_no_legs_in_plans() {
    test_three_link("ad-hoc", "assets/3-links/1-agent-no-leg.xml", "adhoc_legs")
}

#[test]
#[serial]
fn test_three_link_with_routing_legs_in_plans() {
    test_three_link("ad-hoc", "assets/3-links/1-agent.xml", "adhoc_no_legs")
}

fn test_three_link(routing_mode: &str, plans_file: &str, output_dir: &str) {
    let output_dir = format!("test_output/mpi_test/three_link/{}/", output_dir);
    run_mpi_simulation_and_convert_events(
        1,
        "assets/3-links/3-links-network.xml",
        plans_file,
        output_dir.as_str(),
        routing_mode,
    );
    compare_events(output_dir.as_str(), "tests/resources/three_link")
}
