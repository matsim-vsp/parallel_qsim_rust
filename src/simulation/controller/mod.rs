pub mod local_controller;
#[cfg(feature = "mpi")]
pub mod mpi_controller;

use std::cell::RefCell;
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::thread::{sleep, JoinHandle};
use std::time::Duration;

use crate::simulation::config::{CommandLineArgs, Config, PartitionMethod, WriteEvents};
use crate::simulation::controller::local_controller::ComputationalEnvironment;
use crate::simulation::io::proto_events::ProtoEventsWriter;
use crate::simulation::messaging::events::EventsPublisher;
use crate::simulation::messaging::sim_communication::message_broker::NetMessageBroker;
use crate::simulation::messaging::sim_communication::SimCommunicator;
use crate::simulation::network::Network;
use crate::simulation::scenario::Scenario;
use crate::simulation::simulation::{Simulation, SimulationBuilder};
use crate::simulation::{id, io};
use nohash_hasher::IntMap;
use tokio::sync::watch::{Receiver, Sender};
use tracing::info;

fn execute_partition<C: SimCommunicator>(
    comm: C,
    comp_env: Arc<ComputationalEnvironment>,
    args: &CommandLineArgs,
) {
    let config_path = &args.config_path;
    let config = Config::from_file(args);

    let rank = comm.rank();
    let size = config.partitioning().num_parts;

    let output_path = io::resolve_path(config_path, &config.output().output_dir);
    fs::create_dir_all(&output_path).expect("Failed to create output path");

    if rank == 0 {
        info!("#{rank} preparing to create input for partitions.");
        partition_input(&config, config_path);

        info!(
            "#{rank} loading ids from file: {}",
            config.proto_files().ids
        );
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
        config.computational_setup().global_sync,
    );

    let mut simulation: Simulation<C> =
        SimulationBuilder::new(config, scenario, net_message_broker, events, comp_env).build();

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
    output_path: &Path,
) -> Rc<RefCell<EventsPublisher>> {
    let events = Rc::new(RefCell::new(EventsPublisher::new()));

    if config.output().write_events == WriteEvents::Proto {
        let events_file = format!("events.{rank}.binpb");
        let events_path =
            io::resolve_path(config_path, output_path.join(events_file).to_str().unwrap());
        info!("adding events writer with path: {events_path:?}");
        events
            .borrow_mut()
            .add_subscriber(Box::new(ProtoEventsWriter::new(&events_path)));
    }
    events
}

/// Have this more complicated join logic, so that threads in the back of the handle vec can also
/// cause the main thread to panic.
fn try_join(
    mut handles: IntMap<u32, JoinHandle<()>>,
    adapters: Vec<(JoinHandle<()>, Sender<bool>)>,
) {
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

    // When all simulation threads are finished, we shutdown the adapters.
    for (handle, shutdown_sender) in adapters {
        shutdown_sender.send(true).unwrap();
        let name = handle.thread().name().unwrap().to_string();
        handle
            .join()
            .unwrap_or_else(|_| panic!("Error in adapter thread {:?}", name));
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
