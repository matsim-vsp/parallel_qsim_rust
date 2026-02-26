use rust_qsim::simulation::config::{CommandLineArgs, Config};
use rust_qsim::simulation::controller::controller::ControllerBuilder;
use rust_qsim::simulation::events::visualize::{VisualizeEventMessage, VisualizeEvents};
use rust_qsim::simulation::io;
use rust_qsim::simulation::network::Network;
use rust_qsim::simulation::scenario::Scenario;
use rust_qsim::simulation::vehicles::garage::Garage;
use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;

fn main() {
    let args =
        CommandLineArgs::new_with_path("rust_qsim/assets/equil-100/run_equil_100.config.yml");
    // let args = CommandLineArgs::new_with_path("rust_qsim/assets/kehlheim/kehlheim_config.yml");
    let config = Config::from(args);

    let network_path = io::resolve_path(config.context(), &config.network().path);
    let vehicles_path = io::resolve_path(config.context(), &config.vehicles().path);
    let network = Network::from_file_path(
        &network_path,
        config.partitioning().num_parts,
        &config.partitioning().method,
    );
    let garage = Garage::from_file(&vehicles_path);
    let (event_tx, event_rx) = mpsc::channel::<VisualizeEventMessage>();
    let first_link_enter_seen = Arc::new(AtomicBool::new(false));

    let sim_thread = thread::spawn(move || {
        let event_handler_register_fns = HashMap::from([(
            0,
            vec![VisualizeEvents::register_fn(
                event_tx.clone(),
                first_link_enter_seen.clone(),
            )],
        )]);
        let mobsim_listener_register_fns = HashMap::from([(
            0,
            vec![VisualizeEvents::register_mobsim_fn(
                event_tx,
                first_link_enter_seen,
            )],
        )]);

        // thread::sleep(Duration::from_secs(10));

        ControllerBuilder::default_with_scenario(Scenario::load(Arc::new(config)))
            .event_handler_register_fn(event_handler_register_fns)
            .mobsim_event_register_fn(mobsim_listener_register_fns)
            .build()
            .unwrap()
            .run();
    });

    VisualizeEvents::run_window(event_rx, network, garage);

    sim_thread.join().expect("Simulation thread failed");
}
