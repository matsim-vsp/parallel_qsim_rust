use rust_qsim::simulation::config::{CommandLineArgs, Config};
use rust_qsim::simulation::controller::local_controller::LocalControllerBuilder;
use rust_qsim::simulation::events::print_events::PrintEvents;
use rust_qsim::simulation::scenario::Scenario;
use std::collections::HashMap;
use std::sync::Arc;

fn main() {
    let args =
        CommandLineArgs::new_with_path("rust_qsim/assets/equil-100/run_equil_100.config.yml");
    let config = Config::from(args);

    let mut subscribers = HashMap::new();
    subscribers.insert(0, vec![PrintEvents::register()]);

    LocalControllerBuilder::default()
        .scenario(Scenario::load(Arc::new(config)))
        .events_subscriber_per_partition(subscribers)
        .build()
        .unwrap()
        .run();
}
