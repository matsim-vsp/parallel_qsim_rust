use macros::integration_test;
use rust_qsim::simulation::config::{Config, OverwriteFiles};
use rust_qsim::simulation::controller::controller::ControllerBuilder;
use rust_qsim::simulation::scenario::MutableScenario;

/// Test that with default structs the simulation runs without errors
#[integration_test(rust_qsim)]
fn test_empty() {
    let mut config = Config::default();
    config.output_mut().overwrite_files = OverwriteFiles::DeleteDirectoryIfExists;
    config.output_mut().output_dir = "./test_output/simulation/empty".parse().unwrap();
    let scenario = MutableScenario::load(config);
    let controller = ControllerBuilder::default_with_scenario(scenario)
        .build()
        .unwrap();
    controller.run();
}
