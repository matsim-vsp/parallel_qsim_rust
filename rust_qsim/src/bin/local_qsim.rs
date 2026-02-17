use clap::Parser;
use rust_qsim::simulation::config::{CommandLineArgs, Config};
use rust_qsim::simulation::controller::local_controller::LocalControllerBuilder;
use rust_qsim::simulation::logging::init_std_out_logging_thread_local;
use rust_qsim::simulation::scenario::Scenario;
use std::sync::Arc;
use tracing::info;

fn main() {
    let _guard = init_std_out_logging_thread_local();

    let args = CommandLineArgs::parse();
    info!("Started with args: {:?}", args);

    // Load and adapt config
    let config = Arc::new(Config::from(args));

    // Load and adapt scenario
    let scenario = Scenario::load(config);

    // Create and run simulation
    let controller = LocalControllerBuilder::default()
        .scenario(scenario)
        .build()
        .unwrap();

    controller.run()
}
