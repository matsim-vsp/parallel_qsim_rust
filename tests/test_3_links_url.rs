use rust_q_sim::simulation::network::Network;
use rust_q_sim::simulation::population::Population;
use rust_q_sim::simulation::vehicles::garage::Garage;
use std::path::PathBuf;

mod test_simulation;

const BASE_URL: &str =
    "https://raw.githubusercontent.com/matsim-vsp/parallel_qsim_rust/refs/heads/43-load-files-via-url/tests/resources/3-links-url";

#[test]
fn load_files_from_url_have_content() {
    let input_dir = PathBuf::from(BASE_URL);

    // Load network and check if it contains nodes and links
    let net = Network::from_file_as_is(&input_dir.join("3-links-network.xml"));
    assert!(
        !net.nodes().is_empty() && !net.links().is_empty(),
        "Network should contain nodes and links"
    );

    // Load vehicles and check if there is at least one vehicle type
    let mut garage = Garage::from_file(&input_dir.join("vehicles.xml"));
    assert!(
        !garage.vehicle_types.is_empty(),
        "Vehicles file should define at least one vehicle type"
    );

    // Load population and check if there is at least one person
    let pop = Population::from_file(&input_dir.join("1-agent-full-leg.xml"), &mut garage);
    assert!(
        !pop.persons.is_empty(),
        "Population should contain at least one person"
    );

    // Load expected events and check if it's not empty
    let events_url = format!("{}/expected_events.xml", BASE_URL);
    let events = test_simulation::TestSubscriber::expected_events_from_file(&events_url);
    assert!(
        !events.is_empty(),
        "Expected events loaded from URL should not be empty"
    );
}
