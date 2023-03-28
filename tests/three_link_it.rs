use crate::event_test_utils::{compare_events, run_mpi_simulation_and_convert_events};
use serial_test::serial;

mod event_test_utils;

#[test]
#[serial]
/// Network: 3 links in a line.
/// Agents: 1 agent travelling line.
/// Route: Given.
fn test_three_link_without_routing() {
    test_three_link(
        "use-plans",
        "assets/3-links/1-agent.xml",
        None,
        "static",
        "tests/resources/three_link",
    )
}

#[test]
#[serial]
/// Network: 3 links in a line.
/// Agents: 1 agent travelling line.
/// Route: Will be calculated during qsim. There are no legs in plans file.  
fn test_three_link_with_routing_no_legs_in_plans() {
    test_three_link(
        "ad-hoc",
        "assets/3-links/1-agent-no-leg.xml",
        None,
        "adhoc_legs",
        "tests/resources/three_link",
    )
}

#[test]
#[serial]
/// Network: 3 links in a line.
/// Agents: 1 agent travelling line.
/// Route: Will be calculated during qsim. There are legs in plans file which will be discarded.
fn test_three_link_with_routing_legs_in_plans() {
    test_three_link(
        "ad-hoc",
        "assets/3-links/1-agent.xml",
        None,
        "adhoc_no_legs",
        "tests/resources/three_link",
    )
}

#[test]
#[serial]
/// Network: 3 links in a line.
/// Agents: 1 agent travelling line.
/// Route: Given.
/// Vehicle definitions: Given. Everything should be like simple test case without routing.
fn test_three_link_one_agent_with_vehicle_definitions() {
    test_three_link(
        "use-plans",
        "assets/3-links/1-agent.xml",
        Some("assets/3-links/vehicle_definitions.xml"),
        "static",
        "tests/resources/three_link",
    )
}

#[test]
#[serial]
/// Network: 3 links in a line.
/// Agents: 3 agents travelling line. Order: car -> bike -> car. There is enough time between agents, thus no jam.
/// Route: Given.
/// Vehicle definitions: Given. Bike travel time is longer than car.
fn test_three_link_multiple_agents_with_vehicle_definitions() {
    test_three_link(
        "use-plans",
        "assets/3-links/3-agent.xml",
        Some("assets/3-links/vehicle_definitions.xml"),
        "static/multiple_agents/no_jam",
        "tests/resources/three_link/multiple_agents/no_jam",
    )
}

#[test]
#[serial]
/// Network: 3 links in a line.
/// Agents: 3 agents travelling line. car -> car -> bike. There is jam due to the bike.
/// Route: Given.
/// Vehicle definitions: Given. Bike travel time is longer than car. Car
fn test_three_link_multiple_agents_with_jam_by_vehicle_definitions() {
    test_three_link(
        "use-plans",
        "assets/3-links/3-agent_jam.xml",
        Some("assets/3-links/vehicle_definitions.xml"),
        "static/multiple_agents/with_jam",
        "tests/resources/three_link/multiple_agents/with_jam",
    )
}

fn test_three_link(
    routing_mode: &str,
    plans_file: &str,
    vehicle_definitions_file: Option<&str>,
    output_dir: &str,
    expected_events_dir: &str,
) {
    let output_dir = format!("test_output/mpi_test/three_link/{}/", output_dir);
    run_mpi_simulation_and_convert_events(
        1,
        "assets/3-links/3-links-network.xml",
        plans_file,
        output_dir.as_str(),
        routing_mode,
        vehicle_definitions_file,
    );
    compare_events(output_dir.as_str(), expected_events_dir)
}
