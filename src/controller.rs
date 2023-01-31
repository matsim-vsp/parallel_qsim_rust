use crate::config::{Config, RoutingMode};
use crate::io::network::IONetwork;
use crate::io::population::IOPopulation;
use crate::parallel_simulation::routing::network_converter::NetworkConverter;
use crate::parallel_simulation::splittable_scenario::Scenario;
use crate::parallel_simulation::Simulation;
use log::info;
use std::fs::remove_dir_all;
use std::ops::Sub;
use std::path::Path;
use std::thread;
use std::thread::JoinHandle;
use std::time::Instant;

pub fn run(config: Config) {
    let network = IONetwork::from_file(config.network_file.as_ref());
    let population = IOPopulation::from_file(config.population_file.as_ref());

    let scenario = Scenario::from_io(&network, &population, config.num_parts, config.sample_size);

    let output_dir_path = Path::new(&config.output_dir);
    let out_network_path = output_dir_path.join("output_network.xml.gz");
    let out_network = scenario.as_network(&network);
    out_network.to_file(&out_network_path);

    let routing_kit_network =
        NetworkConverter::convert_io_network(network, Some(&scenario.id_mappings));

    let simulations =
        Simulation::create_simulation_partitions(&config, scenario, routing_kit_network);
    // do very basic timing
    let start = Instant::now();
    // create threads and start them
    let join_handles: Vec<JoinHandle<()>> = simulations
        .into_iter()
        .map(|mut simulation| thread::spawn(move || simulation.run()))
        .collect();

    // wait for all threads to finish
    for handle in join_handles {
        handle.join().unwrap();
    }

    let end = Instant::now();
    let duration = end.sub(start);
    info!("Simulation ran for: {}s", duration.as_millis() / 1000);

    if config.routing_mode == Some(RoutingMode::AdHoc) {
        remove_dir_all(config.output_dir + "/routing")
            .expect("Couldn't remove temporary routing folder.");
    }

    info!("All simulation threads have finished. Waiting for events writer thread and logger thread to finish as well.")
}
