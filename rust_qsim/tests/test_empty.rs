use macros::integration_test;
use rust_qsim::simulation::config::Config;
use rust_qsim::simulation::controller::controller::ControllerBuilder;
use rust_qsim::simulation::scenario::Scenario;
use std::sync::Arc;

/// Test that with default structs the simulation runs without errors
#[integration_test(rust_qsim)]
fn test_empty() {
    let config = Config::default();
    let scenario = Scenario::load(Arc::new(config));
    let controller = ControllerBuilder::default_with_scenario(scenario)
        .build()
        .unwrap();
    controller.run();
}
