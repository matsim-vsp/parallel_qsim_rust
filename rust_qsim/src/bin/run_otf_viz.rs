use rust_qsim::simulation::config::{CommandLineArgs, Config};
use rust_qsim::simulation::controller::controller::ControllerBuilder;
use rust_qsim::simulation::events::visualize::{VisualizeEventMessage, VisualizeEvents};
use rust_qsim::simulation::io;
use rust_qsim::simulation::network::Network;
use rust_qsim::simulation::scenario::Scenario;
use rust_qsim::simulation::vehicles::garage::Garage;
use std::collections::HashMap;
use std::process;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;

fn main() {
    // read the config
    let args = CommandLineArgs::new_with_path("rust_qsim/assets/test/run_equil_100.config.yml");
    let mut config = Config::from(args);

    // read the network, the vehicles from the config
    let network_path = io::resolve_path(config.context(), &config.network().path);
    let vehicles_path = io::resolve_path(config.context(), &config.vehicles().path);
    let network = Network::from_file_path(
        &network_path,
        config.partitioning().num_parts,
        &config.partitioning().method,
    );
    let garage = Garage::from_file(&vehicles_path);

    // create mpsc channel to communicate between the simulation and the viz
    let (event_sender, event_receiver) = mpsc::channel::<VisualizeEventMessage>();

    // is true when the first link enter event arrived (start real time viz after the first event)
    let first_link_enter_seen = Arc::new(AtomicBool::new(false));

    // start the simulation in a seperate thread
    let _sim_thread = thread::spawn(move || {
        // register event handler
        let event_handler_fns = HashMap::from([(
            0,
            vec![VisualizeEvents::register_fn(
                event_sender.clone(),
                first_link_enter_seen.clone(),
            )],
        )]);

        // register mobsim handler
        let mobsim_listener_fns = HashMap::from([(
            0,
            vec![VisualizeEvents::register_mobsim_fn(
                event_sender,
                first_link_enter_seen,
            )],
        )]);

        // start simulation
        ControllerBuilder::default_with_scenario(Scenario::load(Arc::new(config)))
            .event_handler_register_fn(event_handler_fns)
            .mobsim_event_register_fn(mobsim_listener_fns)
            .build()
            .unwrap()
            .run();
    });

    // start bevy viz
    VisualizeEvents::run_window(event_receiver, network, garage);

    // stop when the bevy window is closed
    process::exit(0);
}
