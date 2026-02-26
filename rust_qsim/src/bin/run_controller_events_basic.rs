use rust_qsim::simulation::config::{CommandLineArgs, Config};
use rust_qsim::simulation::controller::controller::ControllerBuilder;
use rust_qsim::simulation::events::controller_event_printer::{
    BeforeSimStepMessage, ControllerEventPrinter,
};
use rust_qsim::simulation::scenario::Scenario;
use std::collections::HashMap;
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;

fn main() {
    let args =
        CommandLineArgs::new_with_path("rust_qsim/assets/equil-100/run_equil_100.config.yml");
    let config = Config::from(args);
    let (event_tx, _event_rx) = mpsc::channel::<BeforeSimStepMessage>();
    let mobsim_listener_register_fns = HashMap::from([(
        0,
        vec![ControllerEventPrinter::register_fn(event_tx)],
    )]);

    let sim_thread = thread::spawn(move || {
        ControllerBuilder::default_with_scenario(Scenario::load(Arc::new(config)))
            .mobsim_event_register_fn(mobsim_listener_register_fns)
            .build()
            .unwrap()
            .run();
    });

    sim_thread.join().expect("Simulation thread failed");
}
