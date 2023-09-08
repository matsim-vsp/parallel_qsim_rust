use serial_test::serial;

use crate::event_test_utils::{compare_events, run_mpi_simulation_and_convert_events};

mod event_test_utils;

#[test]
#[serial]
#[ignore]
/// Network: 3 links in a line.
/// Agents: 1 agent travelling line.
/// Route: Given. Plan also includes walking legs and car interactions.
fn test_three_link_default() {
    test_three_link(
        "assets/3-links/1-agent.xml",
        None,
        "static",
        "tests/resources/three_link",
    )
}

#[test]
#[serial]
#[ignore]
/// Network: 3 links in a line.
/// Agents: 1 agent travelling line.
/// Route: Will be calculated during qsim. There are legs in plans file which will be discarded.
fn test_three_link_full_legs_in_plans() {
    test_three_link(
        "assets/3-links/1-agent-full-leg.xml",
        None,
        "static/full_legs",
        "tests/resources/three_link/full_legs",
    )
}

#[test]
#[serial]
#[ignore]
/// Network: 3 links in a line.
/// Agents: 1 agent travelling line.
/// Route: Will be computed during qsim. There are main legs in plans file.
fn test_three_link_with_routing_legs_in_plans() {
    test_three_link(
        "assets/3-links/1-agent.xml",
        None,
        "adhoc/with_legs",
        "tests/resources/three_link/full_legs",
    )
}

#[test]
#[serial]
#[ignore]
/// Network: 3 links in a line.
/// Agents: 1 agent travelling line.
/// Route: Given.
/// Vehicle definitions: Given. Everything should be like simple test case without routing.
fn test_three_link_one_agent_with_vehicle_definitions() {
    test_three_link(
        "assets/3-links/1-agent.xml",
        Some("assets/3-links/vehicle_definitions.xml"),
        "static/vehicle_definitions",
        "tests/resources/three_link",
    )
}

#[test]
#[serial]
#[ignore]
/// Network: 3 links in a line.
/// Agents: 3 agents travelling line. Order: car -> bike -> car. There is enough time between agents, thus no jam.
/// Route: Given.
/// Vehicle definitions: Given. Bike travel time is longer than car.
fn test_three_link_multiple_agents_with_vehicle_definitions() {
    test_three_link(
        "assets/3-links/3-agent.xml",
        Some("assets/3-links/vehicle_definitions.xml"),
        "static/multiple_agents/no_jam",
        "tests/resources/three_link/multiple_agents/no_jam",
    )
}

#[test]
#[serial]
#[ignore]
/// Network: 3 links in a line.
/// Agents: 3 agents travelling line. car -> car -> bike. There is jam due to the bike.
/// Route: Given.
/// Vehicle definitions: Given. Bike travel time is longer than car. Car
fn test_three_link_multiple_agents_with_jam_by_vehicle_definitions() {
    test_three_link(
        "assets/3-links/3-agent_jam.xml",
        Some("assets/3-links/vehicle_definitions.xml"),
        "static/multiple_agents/with_jam",
        "tests/resources/three_link/multiple_agents/with_jam",
    )
}

fn test_three_link(
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
        vehicle_definitions_file,
    );
    compare_events(output_dir.as_str(), expected_events_dir)
}
