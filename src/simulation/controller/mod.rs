pub mod local_controller;
#[cfg(feature = "mpi")]
pub mod mpi_controller;

use crate::external_services::{AdapterHandle, ExternalServiceType};
use crate::simulation::config::{Config, PartitionMethod, WriteEvents};
use crate::simulation::io::proto_events::ProtoEventsWriter;
use crate::simulation::messaging::events::{EventsPublisher, EventsSubscriber};
use crate::simulation::messaging::sim_communication::message_broker::NetMessageBroker;
use crate::simulation::messaging::sim_communication::SimCommunicator;
use crate::simulation::network::Network;
use crate::simulation::scenario::Scenario;
use crate::simulation::simulation::{Simulation, SimulationBuilder};
use crate::simulation::{id, io, logging};
use derive_builder::Builder;
use nohash_hasher::IntMap;
use std::any::Any;
use std::cell::{RefCell, RefMut};
use std::collections::HashMap;
use std::fmt::Debug;
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::thread::{sleep, JoinHandle};
use std::time::Duration;
use tokio::sync::mpsc::Sender;
use tracing::info;

// This is a wrapper around a Sender that can be used to send requests to an external service.
// The value is of type Arc as this is the adapter running in another thread.
// It needs to be super generic to store any type of Sender.
#[derive(Clone, Debug)]
pub struct RequestSender(Arc<dyn Any + Send + Sync>);

// Implementing this trait provides type safety for the RequestSender. Only Senders can be converted to RequestSender.
// This comes with the requirement that the type T must 'static in particular -- but I think this is ok for now. Paul, jul'25
impl<T> From<Arc<Sender<T>>> for RequestSender
where
    T: Send + 'static,
{
    fn from(value: Arc<Sender<T>>) -> Self {
        RequestSender(value as Arc<dyn Any + Send + Sync>)
    }
}

impl<T> From<Sender<T>> for RequestSender
where
    T: Send + 'static,
{
    fn from(value: Sender<T>) -> Self {
        RequestSender(Arc::new(value) as Arc<dyn Any + Send + Sync>)
    }
}

#[derive(Debug, Default, Clone)]
pub struct ExternalServices(HashMap<ExternalServiceType, RequestSender>);

impl From<HashMap<ExternalServiceType, RequestSender>> for ExternalServices {
    fn from(value: HashMap<ExternalServiceType, RequestSender>) -> Self {
        ExternalServices(value)
    }
}

impl ExternalServices {
    pub fn get_service<T: Any + Send + Sync>(
        &self,
        service_type: ExternalServiceType,
    ) -> Option<&T> {
        self.0
            .get(&service_type)
            .and_then(|s| s.0.downcast_ref::<T>())
    }

    pub fn insert(&mut self, service_type: ExternalServiceType, sender: RequestSender) {
        self.0.insert(service_type, sender);
    }
}

#[derive(Clone, Debug, Builder)]
#[builder(pattern = "owned")]
pub struct ThreadLocalComputationalEnvironment {
    #[builder(default)]
    services: ExternalServices,
    // The value is of type Rc as this is a thread-local events publisher.
    #[builder(default)]
    events_publisher: Rc<RefCell<EventsPublisher>>,
}

impl Default for ThreadLocalComputationalEnvironment {
    fn default() -> Self {
        ThreadLocalComputationalEnvironment {
            services: ExternalServices::default(),
            events_publisher: Rc::new(RefCell::new(EventsPublisher::new())),
        }
    }
}

impl ThreadLocalComputationalEnvironment {
    pub fn get_service<T: Any + Send + Sync>(
        &self,
        service_type: ExternalServiceType,
    ) -> Option<&T> {
        self.services.get_service(service_type)
    }

    pub fn events_publisher_borrow_mut(&mut self) -> RefMut<'_, EventsPublisher> {
        self.events_publisher.borrow_mut()
    }

    pub fn events_publisher(&self) -> Rc<RefCell<EventsPublisher>> {
        self.events_publisher.clone()
    }
}

#[derive(Debug, Builder)]
#[builder(pattern = "owned")]
pub struct PartitionArguments<C: SimCommunicator> {
    communicator: C,
    config: Config,
    #[builder(default)]
    external_services: ExternalServices,
    #[builder(default)]
    events_subscriber: Vec<Box<dyn EventsSubscriber + Send>>,
}

pub fn execute_partition<C: SimCommunicator>(partition_arguments: PartitionArguments<C>) {
    let _guards = logging::init_logging(
        &partition_arguments.config,
        partition_arguments.communicator.rank(),
    );

    let comm = partition_arguments.communicator;
    let external_services = partition_arguments.external_services;
    let subscribers = partition_arguments.events_subscriber;

    let config = partition_arguments.config;

    let rank = comm.rank();
    let size = config.partitioning().num_parts;

    let output_path = io::resolve_path(config.context(), &config.output().output_dir);
    fs::create_dir_all(&output_path).expect("Failed to create output path");

    if rank == 0 {
        info!("#{rank} preparing to create input for partitions.");
        partition_input(&config);

        info!(
            "#{rank} loading ids from file: {}",
            config.proto_files().ids.display()
        );

        id::load_from_file(&io::resolve_path(
            config.context(),
            &config.proto_files().ids,
        ));
    }

    info!("Process #{rank} of {size} has started. Waiting for other processes to arrive at initial barrier. ");
    comm.barrier();

    let scenario = Scenario::build(&config, rank, &output_path);

    let events = create_events(&config, rank, &output_path, subscribers);

    let rc_comm = Rc::new(comm);

    let net_message_broker = NetMessageBroker::new(
        Rc::clone(&rc_comm),
        &scenario.network,
        &scenario.network_partition,
        config.computational_setup().global_sync,
    );

    let comp_env = ThreadLocalComputationalEnvironmentBuilder::default()
        .services(external_services)
        .events_publisher(events.clone())
        .build()
        .unwrap();

    let mut simulation: Simulation<C> =
        SimulationBuilder::new(config, scenario, net_message_broker, comp_env).build();

    // Wait for all processes to arrive at this barrier. This is important to ensure that the
    // instrumentation of the simulation.run() method does not include any time it takes to
    // load the network and population.
    rc_comm.barrier();
    simulation.run();

    // Drop guards here to make sure that the logging is flushed before we exit.
    // This is important for integration tests when the same test thread executes multiple simulations
    // one after another and consequently initializes logging for each test case.
    drop(_guards);
}

fn create_events(
    config: &Config,
    rank: u32,
    output_path: &Path,
    additional_subscribers: Vec<Box<dyn EventsSubscriber + Send>>,
) -> Rc<RefCell<EventsPublisher>> {
    let events = Rc::new(RefCell::new(EventsPublisher::new()));

    if config.output().write_events == WriteEvents::Proto {
        let events_file = format!("events.{rank}.binpb");
        let events_path = io::resolve_path(config.context(), &output_path.join(events_file));
        info!("adding events writer with path: {events_path:?}");
        events
            .borrow_mut()
            .add_subscriber(Box::new(ProtoEventsWriter::new(&events_path)));
    }

    for subscriber in additional_subscribers {
        events.borrow_mut().add_subscriber(subscriber);
    }

    events
}

/// Have this more complicated join logic, so that threads in the back of the handle vec can also
/// cause the main thread to panic.
pub fn try_join(mut handles: IntMap<u32, JoinHandle<()>>, adapters: Vec<AdapterHandle>) {
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
    for a in adapters {
        a.shutdown_sender.send(true).unwrap();
        let name = a.handle.thread().name().unwrap().to_string();
        a.handle
            .join()
            .unwrap_or_else(|_| panic!("Error in adapter thread {:?}", name));
    }
}

pub fn partition_input(config: &Config) {
    // If we partition the network it is copied to the output folder.
    // Otherwise, nothing is done, and we can load the network from the input folder directly.
    // In this case, we assume that the #partitions is part of the filename as `network.4.binpb` instead of `network.binpb`.
    id::load_from_file(&io::resolve_path(
        config.context(),
        &config.proto_files().ids,
    ));
    if let PartitionMethod::Metis(_) = config.partitioning().method {
        info!("Config param Partition method was set to metis. Loading input network, running metis conversion and then store it into output folder");
        partition_network(config);
    }
    // don't do anything. If the network is already partitioned, we'll load it from the input folder.
    /*else {
        info!("Config param Partition method was set to none. Loading network from input, assuming it has partitioning information");
        copy_network_into_output(config, config_path)
    };
    */
}

fn partition_network(config: &Config) -> Network {
    let net_in_path = io::resolve_path(config.context(), &config.proto_files().network);
    let num_parts = config.partitioning().num_parts;
    let network = Network::from_file_path(&net_in_path, num_parts, config.partitioning().method);

    let mut net_out_path = create_output_filename(
        &io::resolve_path(config.context(), &config.output().output_dir),
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
