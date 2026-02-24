use rust_qsim::simulation::config::{CommandLineArgs, Config};
use rust_qsim::simulation::controller::controller::ControllerBuilder;
use rust_qsim::simulation::events::controller_event_printer::{
    BeforeMobsimMessage, ControllerEventPrinter,
};
use rust_qsim::simulation::scenario::Scenario;
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;

fn main() {
    let args =
        CommandLineArgs::new_with_path("rust_qsim/assets/equil-100/run_equil_100.config.yml");
    let config = Config::from(args);
    let (event_tx, _) = mpsc::channel::<BeforeMobsimMessage>();

    let sim_thread = thread::spawn(move || {
        ControllerBuilder::default_with_scenario(Scenario::load(Arc::new(config)))
            .controller_event_register_fn(vec![ControllerEventPrinter::register_fn(event_tx)])
            .build()
            .unwrap()
            .run();
    });

    sim_thread.join().expect("Simulation thread failed");
}
