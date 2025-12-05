use crate::test_simulation::TestExecutorBuilder;
use macros::integration_test;
use rust_qsim::simulation::config::{CommandLineArgs, Config};
use rust_qsim::simulation::id::store_to_file;
use rust_qsim::simulation::network::Network;
use rust_qsim::simulation::population::Population;
use rust_qsim::simulation::vehicles::garage::Garage;
use std::path::{Path, PathBuf};
use std::sync::Arc;

mod test_simulation;

fn create_resources(out_dir: &Path) {
    let input_dir = PathBuf::from("./assets/3-links/");
    let net = Network::from_file_as_is(&input_dir.join("3-links-network.xml"));
    let mut garage = Garage::from_file(&input_dir.join("vehicles.xml"));
    let pop = Population::from_file(&input_dir.join("1-agent-full-leg.xml"), &mut garage);

    store_to_file(&out_dir.join("ids.binpb"));
    net.to_file(&out_dir.join("3-links-network.binpb"));
    pop.to_file(&out_dir.join("1-agent-full-leg.binpb"));
    garage.to_file(&out_dir.join("vehicles.binpb"));
}

#[integration_test(rust_qsim)]
fn execute_3_links_single_part() {
    let test_dir = PathBuf::from("./test_output/simulation/execute_3_links_single_part/");
    create_resources(&test_dir);

    let config_args =
        CommandLineArgs::new_with_path("./tests/resources/3-links/3-links-config-1.yml");

    TestExecutorBuilder::default()
        .config(Arc::new(Config::from(config_args)))
        // .expected_events(None)
        // .additional_subscribers(HashMap::from([(
        //     0,
        //     vec![XmlEventsWriter::register("test_output/test.xml".into())],
        // )]))
        .expected_events(Some("./tests/resources/3-links/expected_events.xml"))
        .build()
        .unwrap()
        .execute();
}

#[integration_test(rust_qsim)]
fn execute_3_links_2_parts() {
    create_resources(&PathBuf::from(
        "./test_output/simulation/execute_3_links_2_parts/",
    ));

    let config_args =
        CommandLineArgs::new_with_path("./tests/resources/3-links/3-links-config-2.yml");

    TestExecutorBuilder::default()
        .config(Arc::new(Config::from(config_args)))
        .expected_events(Some("./tests/resources/3-links/expected_events.xml"))
        .build()
        .unwrap()
        .execute();
}
