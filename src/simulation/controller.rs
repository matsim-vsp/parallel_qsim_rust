use crate::simulation::config::Config;
use crate::simulation::id_mapping::MatsimIdMappings;
use crate::simulation::io::network::IONetwork;
use crate::simulation::io::population::IOPopulation;
use crate::simulation::io::proto_events::ProtoEventsWriter;
use crate::simulation::io::vehicle_definitions::{IOVehicleDefinitions, VehicleDefinitions};
use crate::simulation::messaging::events::EventsPublisher;
use crate::simulation::messaging::message_broker::MpiMessageBroker;
use crate::simulation::messaging::travel_time_collector::TravelTimeCollector;
use crate::simulation::network::sim_network::SimNetworkPartition;
use crate::simulation::simulation::Simulation;
use mpi::topology::SystemCommunicator;
use mpi::traits::{Communicator, CommunicatorCollectives};
use std::ffi::c_int;
use std::fs;
use std::ops::Sub;
use std::path::PathBuf;
use std::time::Instant;
use tracing::info;

pub fn run(world: SystemCommunicator, config: Config) {
    let rank = world.rank();
    let size = world.size();

    info!("Process #{rank} of {size}");

    let output_path = PathBuf::from(&config.output_dir);
    fs::create_dir_all(&output_path).expect("Failed to create output path");

    // TODO remove io_network once all parts are switched to new network impl
    let io_network = IONetwork::from_file(config.network_file.as_ref());
    let io_population = IOPopulation::from_file(config.population_file.as_ref());
    let id_mappings = MatsimIdMappings::from_io(&io_network, &io_population);

    let network = crate::simulation::network::global_network::Network::from_file(
        config.network_file.as_ref(),
        config.num_parts,
    );

    // write network with new ids to output but only once.
    if rank == 0 {
        network.to_file(&output_path.join("output_network.xml.gz"));
    }

    let population = crate::simulation::population::population::Population::from_file(
        config.population_file.as_ref(),
        &network,
        rank as usize,
    );
    let network_partition = SimNetworkPartition::from_network(&network, rank as usize);
    info!(
        "Partition #{rank} network has: {} nodes and {} links. Population has {} agents",
        network_partition.nodes.len(),
        network_partition.links.len(),
        population.agents.len()
    );

    let message_broker = MpiMessageBroker::new(world.clone(), rank, &network_partition);
    let mut events = EventsPublisher::new();

    let events_file = format!("events.{rank}.pbf");
    let events_path = output_path.join(events_file);
    events.add_subscriber(Box::new(ProtoEventsWriter::new(&events_path)));
    let travel_time_collector = Box::new(TravelTimeCollector::new());
    events.add_subscriber(travel_time_collector);
    //events.add_subscriber(Box::new(EventsLogger {}));

    let mut vehicle_definitions: Option<VehicleDefinitions> = None;
    if let Some(vehicle_definitions_file_path) = &config.vehicle_definitions_file {
        let io_vehicle_definitions =
            IOVehicleDefinitions::from_file(vehicle_definitions_file_path.as_ref());
        vehicle_definitions = Some(VehicleDefinitions::from_io(io_vehicle_definitions));
    }

    let mut simulation = Simulation::new(
        &config,
        &id_mappings,
        network_partition,
        population,
        message_broker,
        events,
        vehicle_definitions,
    );

    let start = Instant::now();
    simulation.run(config.start_time, config.end_time);
    let end = Instant::now();
    let duration = end.sub(start).as_millis() / 1000;
    info!("#{rank} took: {duration}s");

    info!("output dir: {:?}", config.output_dir);

    info!("#{rank} at barrier.");
    world.barrier();
    info!("Process #{rank} finishing.");
}

fn get_temp_output_folder(output_dir: &PathBuf, rank: c_int) -> PathBuf {
    output_dir.join("routing").join(format!("{}", rank))
}
