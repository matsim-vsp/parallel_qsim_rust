use crate::config::Config;
use crate::io::network::IONetwork;
use crate::io::non_blocking_io::NonBlocking;
use crate::io::population::IOPopulation;
use crate::parallel_simulation::events::Events;
use crate::parallel_simulation::splittable_scenario::Scenario;
use crate::parallel_simulation::Simulation;
use log::info;
use std::path::Path;
use std::thread;
use std::thread::JoinHandle;

pub fn run(config: Config) {
    let network = IONetwork::from_file(config.network_file.as_ref());
    let population = IOPopulation::from_file(config.population_file.as_ref());

    let scenario = Scenario::from_io(&network, &population, config.num_parts);

    let output_dir_path = Path::new(&config.output_dir);
    let out_network_path = output_dir_path.join("output_network.xml.gz");
    let out_network = scenario.as_network(&network);
    out_network.to_file(&out_network_path);

    let out_events_path = output_dir_path.join("output_events.xml");
    let (events_writer, _guard) = NonBlocking::from_file(&out_events_path.to_str().unwrap());
    let mut events = Events::new(events_writer, config.events_mode.clone());

    let simulations = Simulation::create_simulation_partitions(&config, scenario, &events);

    // create threads and start them
    let join_handles: Vec<JoinHandle<()>> = simulations
        .into_iter()
        .map(|mut simulation| thread::spawn(move || simulation.run()))
        .collect();

    // wait for all threads to finish
    for handle in join_handles {
        handle.join().unwrap();
    }

    // print closing tag in events file.
    events.finish();

    info!("All simulation threads have finished. Waiting for events writer thread and logger thread to finish as well.")
}
