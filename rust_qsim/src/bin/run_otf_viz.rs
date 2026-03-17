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
    // Load the config
    let args =
        CommandLineArgs::new_with_path("rust_qsim/assets/equil-100/run_equil_100.config.yml");
    let mut config = Config::from(args);

    // Use one partition
    config.partitioning_mut().num_parts = 1;

    // Load network and vehicles
    let network_path = io::resolve_path(config.context(), &config.network().path);
    let vehicles_path = io::resolve_path(config.context(), &config.vehicles().path);
    let network = Network::from_file_path(
        &network_path,
        config.partitioning().num_parts,
        &config.partitioning().method,
    );
    let garage = Garage::from_file(&vehicles_path);

    // Create the channel to communicate between the simulation and die visualization (sending MATSim Events)
    let (event_tx, event_rx) = mpsc::channel::<VisualizeEventMessage>();

    // Shared flag used to start realtime pacing only after first LinkEnter.
    let first_link_enter_seen = Arc::new(AtomicBool::new(false));
    // Shared pause flag written by UI and read by simulation event listener.
    let pause_requested = Arc::new(AtomicBool::new(false));
    let pause_requested_for_sim = pause_requested.clone();

    // Start simulation in a second thread.
    let sim_thread = thread::spawn(move || {
        // Register regular event handlers
        let event_handler_register_fns = HashMap::from([(
            0,
            vec![VisualizeEvents::register_fn(
                event_tx.clone(),
                first_link_enter_seen.clone(),
            )],
        )]);

        // Register mobsim step handler
        let mobsim_listener_register_fns = HashMap::from([(
            0,
            vec![VisualizeEvents::register_mobsim_fn(
                event_tx,
                first_link_enter_seen,
                pause_requested_for_sim,
            )],
        )]);

        // Build and run the simulation controller.
        ControllerBuilder::default_with_scenario(Scenario::load(Arc::new(config)))
            .event_handler_register_fn(event_handler_register_fns)
            .mobsim_event_register_fn(mobsim_listener_register_fns)
            .build()
            .unwrap()
            .run();
    });

    // Start the UI (main thread)
    VisualizeEvents::run_window(event_rx, network, garage, pause_requested);

    // Exit the process after the UI window is closed
    process::exit(0);
}
