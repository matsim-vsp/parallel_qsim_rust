use rust_qsim::simulation::config::{CommandLineArgs, Config};
use rust_qsim::simulation::controller::local_controller::LocalControllerBuilder;
use rust_qsim::simulation::scenario::Scenario;
use std::sync::Arc;

fn main() {
    let args =
        CommandLineArgs::new_with_path("rust_qsim/assets/equil-100/run_equil_100.config.yml");
    let config = Config::from(args);

    LocalControllerBuilder::default()
        .scenario(Scenario::load(Arc::new(config)))
        .build()
        .unwrap()
        .run();
}
