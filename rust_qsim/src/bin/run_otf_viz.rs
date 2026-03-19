use rust_qsim::simulation::config::{CommandLineArgs, Config};
use rust_qsim::simulation::controller::controller::ControllerBuilder;
use rust_qsim::simulation::events::visualize::{OTFVizEventMessages, VisualizeEvents};
use rust_qsim::simulation::scenario::MutableScenario;
use std::collections::HashMap;
use std::process;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;

fn main() {
    // read the config
    let args = CommandLineArgs::new_with_path("rust_qsim/assets/test/run_viz_test_config.yml");
    let config = Arc::new(Config::from_args(args));

    // load scenario (network + garage + population)
    let scenario = MutableScenario::load(config);

    // clone network and garage for the viz thread
    let network = scenario.network.clone();
    let garage = scenario.garage.clone();

    // create mpsc channel to communicate between the simulation and the viz
    let (event_sender, event_receiver) = mpsc::channel::<OTFVizEventMessages>();

    // is true when the first link enter event arrived (start real time viz after the first event)
    let first_link_enter_seen = Arc::new(AtomicBool::new(false));

    // start the simulation in a seperate thread
    let _sim_thread = thread::spawn(move || {
        // register event handler
        let event_handler = HashMap::from([(
            0,
            vec![VisualizeEvents::register_fn(
                event_sender.clone(),
                first_link_enter_seen.clone(),
            )],
        )]);

        // register mobsim handler
        let mobsim_listener = HashMap::from([(
            0,
            vec![VisualizeEvents::register_mobsim_fn(
                event_sender,
                first_link_enter_seen,
            )],
        )]);

        // start simulation
        ControllerBuilder::default_with_scenario(scenario)
            .event_handler_register_fn(event_handler)
            .mobsim_event_register_fn(mobsim_listener)
            .build()
            .unwrap()
            .run();
    });

    // start bevy viz
    VisualizeEvents::run_window(event_receiver, network, garage);

    // stop when the bevy window is closed
    process::exit(0);
}
