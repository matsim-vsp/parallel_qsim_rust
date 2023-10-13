use std::ops::Sub;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread::{sleep, JoinHandle};
use std::time::{Duration, Instant};
use std::{fs, thread};

use clap::Parser;
use mpi::traits::{Communicator, CommunicatorCollectives};
use nohash_hasher::IntMap;
use tracing::info;

use crate::simulation::config::{Config, RoutingMode};
use crate::simulation::io::proto_events::ProtoEventsWriter;
use crate::simulation::logging;
use crate::simulation::messaging::events::EventsPublisher;
use crate::simulation::messaging::message_broker::{
    ChannelNetCommunicator, DummyNetCommunicator, MpiNetCommunicator, NetCommunicator,
    NetMessageBroker,
};
use crate::simulation::network::global_network::Network;
use crate::simulation::network::sim_network::SimNetworkPartition;
use crate::simulation::population::population::Population;
use crate::simulation::routing::router::Router;
use crate::simulation::routing::travel_times_collecting_alt_router::TravelTimesCollectingAltRouter;
use crate::simulation::routing::walk_leg_updater::{EuclideanWalkLegUpdater, WalkLegUpdater};
use crate::simulation::simulation::Simulation;
use crate::simulation::vehicles::garage::Garage;

pub fn run_single_partition() {
    let config = Arc::new(Config::parse());
    let _guards = logging::init_logging(config.output_dir.as_ref(), 0.to_string());

    info!("Starting single Partition Simulation");
    let comm = DummyNetCommunicator();
    execute_partition(comm, config);
}

pub fn run_channel() {
    let config = Arc::new(Config::parse());
    let _guards = logging::init_logging(config.output_dir.as_ref(), 0.to_string());

    info!(
        "Starting Multithreaded Simulation with {} partitions.",
        config.num_parts
    );
    let comms = ChannelNetCommunicator::create_n_2_n(config.num_parts);

    let handles: IntMap<u32, JoinHandle<()>> = comms
        .into_iter()
        .map(|comm| {
            let config = config.clone();
            (
                comm.rank(),
                thread::spawn(move || execute_partition(comm, config)),
            )
        })
        .collect();

    try_join(handles);
}

pub fn run_mpi() {
    let universe = mpi::initialize().unwrap();
    let world = universe.world();
    let comm = MpiNetCommunicator {
        mpi_communicator: world,
    };

    let config = Config::parse();
    let _guards = logging::init_logging(config.output_dir.as_ref(), comm.rank().to_string());

    info!(
        "Starting MPI Simulation with {} partitions",
        config.num_parts
    );
    execute_partition(comm, Arc::new(config));

    info!("#{} at barrier.", world.rank());
    universe.world().barrier();
    info!("Process #{} finishing.", world.rank());
}

fn execute_partition<C: NetCommunicator>(comm: C, config: Arc<Config>) {
    let rank = comm.rank();
    let size = config.num_parts;

    info!("Process #{rank} of {size}");

    let output_path = PathBuf::from(&config.output_dir);
    fs::create_dir_all(&output_path).expect("Failed to create output path");

    let network = Network::from_file(
        config.network_file.as_ref(),
        config.num_parts,
        &config.partition_method,
    );
    let mut garage = Garage::from_file(config.vehicles_file.as_ref());

    let forward_backward_graph_by_mode =
        TravelTimesCollectingAltRouter::get_forward_backward_graph_by_mode(
            &network,
            &garage.vehicle_types,
        );

    // write network with new ids to output but only once.
    if rank == 0 {
        network.to_file(&output_path.join("output_network.xml.gz"));
    }

    let population: Population =
        Population::from_file(config.population_file.as_ref(), &network, &mut garage, rank);
    let network_partition = SimNetworkPartition::from_network(&network, rank, config.sample_size);
    info!(
        "Partition #{rank} network has: {} nodes and {} links. Population has {} agents",
        network_partition.nodes.len(),
        network_partition.links.len(),
        population.agents.len()
    );

    let message_broker = NetMessageBroker::new(comm, &network_partition, &network);
    let mut events = EventsPublisher::new();

    let events_file = format!("events.{rank}.pbf");
    let events_path = output_path.join(events_file);
    events.add_subscriber(Box::new(ProtoEventsWriter::new(&events_path)));
    // let travel_time_collector = Box::new(TravelTimeCollector::new());
    //events.add_subscriber(travel_time_collector);
    //events.add_subscriber(Box::new(EventsLogger {}));

    let mut router: Option<Box<dyn Router>> = None;
    let mut walk_leg_finder: Option<Box<dyn WalkLegUpdater>> = None;
    if config.routing_mode == RoutingMode::AdHoc {
        router = Some(Box::new(TravelTimesCollectingAltRouter::new(
            forward_backward_graph_by_mode,
            world.clone(),
            rank,
            network_partition.get_link_ids(),
        )));

        let walking_speed_in_m_per_sec = 1.2;
        walk_leg_finder = Some(Box::new(EuclideanWalkLegUpdater::new(
            walking_speed_in_m_per_sec,
        )))
    }

    let mut simulation = Simulation::new(
        config.clone(),
        network_partition,
        garage,
        population,
        message_broker,
        events,
        router,
        walk_leg_finder,
    );

    let start = Instant::now();
    simulation.run(config.start_time, config.end_time);
    let end = Instant::now();
    let duration = end.sub(start).as_millis() / 1000;
    info!("#{rank} took: {duration}s");

    info!("output dir: {:?}", config.output_dir);
}

/// Have this more complicated join logic, so that threads in the back of the handle vec can also
/// cause the main thread to panic.
fn try_join(mut handles: IntMap<u32, JoinHandle<()>>) {
    while !handles.is_empty() {
        sleep(Duration::from_secs(1)); // test for finished threads once a second
        let mut finished = Vec::new();
        for (i, handle) in handles.iter() {
            if handle.is_finished() {
                finished.push(*i);
            }
        }
        for i in finished {
            let handle = handles.remove(&i).unwrap();
            handle.join().expect("Error in a thread");
        }
    }
}
