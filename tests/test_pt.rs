use crate::test_simulation::TestExecutorBuilder;
use rust_q_sim::simulation::config::CommandLineArgs;
use rust_q_sim::simulation::id::store_to_file;
use rust_q_sim::simulation::network::Network;
use rust_q_sim::simulation::population::Population;
use rust_q_sim::simulation::pt::TransitSchedule;
use rust_q_sim::simulation::vehicles::garage::Garage;
use std::path::PathBuf;

mod test_simulation;

fn create_resources(out_dir: &PathBuf) {
    let input_dir = PathBuf::from("./assets/pt_tutorial/");
    let net = Network::from_file_as_is(&input_dir.join("multimodalnetwork.xml"));
    let mut garage = Garage::from_file(&input_dir.join("vehicles.xml"));
    let pop = Population::from_file(&input_dir.join("plans_1.xml.gz"), &mut garage);
    TransitSchedule::from_file(&input_dir.join("transitschedule.xml"));

    store_to_file(&out_dir.join("ids.binpb"));
    net.to_file(&out_dir.join("network.binpb"));
    pop.to_file(&out_dir.join("plans_1.binpb"));
    garage.to_file(&out_dir.join("vehicles.binpb"));
}

#[test]
fn test_pt_tutorial() {
    let test_dir = PathBuf::from("./test_output/simulation/pt_tutorial/");
    create_resources(&test_dir);

    let config_args =
        CommandLineArgs::new_with_path("./tests/resources/pt_tutorial/pt_tutorial_config.yml");

    TestExecutorBuilder::default()
        .config_args(config_args)
        .expected_events("./tests/resources/pt_tutorial/expected_events.xml")
        .build()
        .unwrap()
        .execute();
}
