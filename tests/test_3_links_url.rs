use crate::test_simulation::TestExecutorBuilder;
use rust_q_sim::simulation::config::CommandLineArgs;
use rust_q_sim::simulation::id::store_to_file;
use rust_q_sim::simulation::network::Network;
use rust_q_sim::simulation::population::Population;
use rust_q_sim::simulation::vehicles::garage::Garage;
use std::path::PathBuf;

mod test_simulation;

const BASE_URL: &str =
    "https://raw.githubusercontent.com/matsim-vsp/parallel_qsim_rust/refs/heads/43-load-files-via-url/tests/resources/3-links-url";

fn create_resources(out_dir: &PathBuf) {
    let input_dir = PathBuf::from(BASE_URL);

    let net = Network::from_file_as_is(&input_dir.join("3-links-network.xml"));
    let mut garage = Garage::from_file(&input_dir.join("vehicles.xml"));
    let pop = Population::from_file(&input_dir.join("1-agent-full-leg.xml"), &mut garage);

    store_to_file(&out_dir.join("ids.binpb"));
    net.to_file(&out_dir.join("3-links-network.binpb"));
    pop.to_file(&out_dir.join("1-agent-full-leg.binpb"));
    garage.to_file(&out_dir.join("vehicles.binpb"));
}

#[test]
fn execute_3_links_single_part_from_url() {
    let test_dir = PathBuf::from("./test_output/simulation/execute_3_links_single_part_from_url/");
    create_resources(&test_dir);

    let config_url = format!("{}/3-links-config-1.yml", BASE_URL);
    let events_url = format!("{}/expected_events.xml", BASE_URL);

    let config_args = CommandLineArgs::new_with_path(&config_url);

    TestExecutorBuilder::default()
        .config_args(config_args)
        .expected_events(Some(&events_url))
        .build()
        .unwrap()
        .execute();
}
