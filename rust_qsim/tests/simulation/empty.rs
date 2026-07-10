use macros::deterministic_id_test;
use rust_qsim::simulation::config::{Config, OverwriteFiles};
use rust_qsim::simulation::controller::controller::ControllerBuilder;
use rust_qsim::simulation::scenario::Scenario;

/// Test that with default structs the simulation runs without errors
#[deterministic_id_test(rust_qsim)]
fn empty_simulation_runs() {
    let mut config = Config::default();
    config.output_mut().overwrite_files = OverwriteFiles::DeleteDirectoryIfExists;
    config.output_mut().output_dir = "./test_output/simulation/empty".parse().unwrap();
    let scenario = Scenario::load(config);
    let controller = ControllerBuilder::default_with_scenario(scenario)
        .build()
        .unwrap();
    controller.run();
}
