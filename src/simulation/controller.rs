use std::cell::{OnceCell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::thread::{sleep, JoinHandle};
use std::time::Duration;
use std::{fs, thread};

use clap::Parser;
use mpi::traits::{Communicator, CommunicatorCollectives};
use nohash_hasher::IntMap;
use tracing::info;

use crate::simulation::config::{CommandLineArgs, Config, PartitionMethod, WriteEvents};
use crate::simulation::io::proto_events::ProtoEventsWriter;
use crate::simulation::messaging::communication::communicators::{
    ChannelSimCommunicator, MpiSimCommunicator, SimCommunicator,
};
use crate::simulation::messaging::communication::message_broker::NetMessageBroker;
use crate::simulation::messaging::events::EventsPublisher;
use crate::simulation::network::global_network::Network;
use crate::simulation::scenario::Scenario;
use crate::simulation::simulation::Simulation;
use crate::simulation::{id, io, logging};

pub fn run_channel() {
    let args = CommandLineArgs::parse();
    let config = Config::from_file(&args);

    let _guards =
        logging::init_logging(&config, &args.config_path, 0);

    info!(
        "Starting Multithreaded Simulation with {} partitions.",
        config.partitioning().num_parts
    );
    let comms = ChannelSimCommunicator::create_n_2_n(config.partitioning().num_parts);

    let handles: IntMap<u32, JoinHandle<()>> = comms
        .into_iter()
        .map(|comm| {
            let config_path = args.clone();
            (
                comm.rank(),
                thread::Builder::new()
                    .name(comm.rank().to_string())
                    .spawn(move || execute_partition(comm, &config_path))
                    .unwrap(),
            )
        })
        .collect();

    try_join(handles);
}

pub fn run_mpi() {
    let universe = mpi::initialize().unwrap();
    let world = universe.world();
    let size = world.size();
    let rank = world.rank();

    let send_buffer: Vec<OnceCell<Vec<u8>>> = vec![OnceCell::new(); 42]; //TODO set buffer length

    mpi::request::multiple_scope(1, |scope, requests| {
        let comm = MpiSimCommunicator::new(world, scope, requests, &send_buffer);

        let mut args = CommandLineArgs::parse();
        // override the num part argument, with the number of processes mpi has started.
        args.num_parts = Some(size as u32);
        let config = Config::from_file(&args);

        let _guards = logging::init_logging(&config, &args.config_path, comm.rank());

        info!(
            "Starting MPI Simulation with {} partitions",
            config.partitioning().num_parts
        );
        execute_partition(comm, &args);

        info!("#{} at barrier.", rank);
        universe.world().barrier();
        info!("Process #{} finishing.", rank);
    });
}

fn execute_partition<C: SimCommunicator>(comm: C, args: &CommandLineArgs) {
    let config_path = &args.config_path;
    let config = Config::from_file(args);

    let rank = comm.rank();
    let size = config.partitioning().num_parts;

    let output_path = io::resolve_path(config_path, &config.output().output_dir);
    fs::create_dir_all(&output_path).expect("Failed to create output path");

    if rank == 0 {
        info!("#{rank} preparing to create input for partitions.");
        partition_input(&config, config_path);
    }

    info!("Process #{rank} of {size} has started. Waiting for other processes to arrive at initial barrier. ");
    // send emtpy travel times to everybody as a barrier.
    //comm.send_receive_travel_times(0, std::collections::HashMap::new());
    comm.barrier();

    let scenario = Scenario::build(&config, config_path, rank, &output_path);

    let events = create_events(config_path, &config, rank, &output_path);

    let rc_comm = Rc::new(comm);

    let net_message_broker = NetMessageBroker::new(
        Rc::clone(&rc_comm),
        &scenario.network,
        &scenario.network_partition,
    );

    let mut simulation: Simulation<C> =
        Simulation::new(config, scenario, net_message_broker, events);

    // Wait for all processes to arrive at this barrier. This is important to ensure that the
    // instrumentation of the simulation.run() method does not include any time it takes to
    // load the network and population.
    rc_comm.barrier();
    simulation.run();
}

fn create_events(
    config_path: &String,
    config: &Config,
    rank: u32,
    output_path: &PathBuf,
) -> Rc<RefCell<EventsPublisher>> {
    let events = Rc::new(RefCell::new(EventsPublisher::new()));

    if config.output().write_events == WriteEvents::Proto {
        let events_file = format!("events.{rank}.binpb");
        let events_path = io::resolve_path(
            config_path,
            &output_path.join(events_file).to_str().unwrap().to_string(),
        );
        info!("adding events writer with path: {events_path:?}");
        events
            .borrow_mut()
            .add_subscriber(Box::new(ProtoEventsWriter::new(&events_path)));
    }
    events
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

pub fn partition_input(config: &Config, config_path: &String) {
    // If we partition the network it is copied to the output folder.
    // Otherwise, nothing is done, and we can load the network from the input folder directly.
    // In this case, we assume that the #partitions is part of the filename as `network.4.binpb` instead of `network.binpb`.
    id::load_from_file(&io::resolve_path(config_path, &config.proto_files().ids));
    if let PartitionMethod::Metis(_) = config.partitioning().method {
        info!("Config param Partition method was set to metis. Loading input network, running metis conversion and then store it into output folder");
        partition_network(config, config_path);
    }
    // don't do anything. If the network is already partitioned, we'll load it from the input folder.
    /*else {
        info!("Config param Partition method was set to none. Loading network from input, assuming it has partitioning information");
        copy_network_into_output(config, config_path)
    };
    */
}

fn partition_network(config: &Config, config_path: &String) -> Network {
    let net_in_path = io::resolve_path(config_path, &config.proto_files().network);
    let num_parts = config.partitioning().num_parts;
    let network = Network::from_file_path(&net_in_path, num_parts, config.partitioning().method);

    let mut net_out_path = create_output_filename(
        &io::resolve_path(config_path, &config.output().output_dir),
        &net_in_path,
    );
    net_out_path = insert_number_in_proto_filename(&net_out_path, num_parts);
    network.to_file(&net_out_path);
    network
}

pub fn get_numbered_output_filename(output_dir: &Path, input_file: &Path, part: u32) -> PathBuf {
    let out = create_output_filename(output_dir, input_file);
    insert_number_in_proto_filename(&out, part)
}

pub fn create_output_filename(output_dir: &Path, input_file: &Path) -> PathBuf {
    let filename = input_file.file_name().unwrap();
    output_dir.join(filename)
}

pub(crate) fn insert_number_in_proto_filename(path: &Path, part: u32) -> PathBuf {
    let filename = path.file_name().unwrap().to_str().unwrap();
    let mut stripped = filename.strip_suffix(".binpb").unwrap();
    if let Some(s) = stripped.strip_suffix(format!(".{part}").as_str()) {
        stripped = s;
    }
    let new_filename = format!("{stripped}.{part}.binpb");
    path.parent().unwrap().join(new_filename)
}
