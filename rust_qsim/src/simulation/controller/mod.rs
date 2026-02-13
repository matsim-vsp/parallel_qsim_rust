pub mod local_controller;

use crate::external_services::{AdapterHandle, ExternalServiceType, RequestToAdapter};
use crate::simulation::config::{Config, WriteEvents};
use crate::simulation::events::{EventsManager, OnEventFnBuilder};
use crate::simulation::io::proto::proto_events::ProtoEventsWriter;
use crate::simulation::io::proto::xml_events::XmlEventsWriter;
use crate::simulation::messaging::sim_communication::message_broker::NetMessageBroker;
use crate::simulation::messaging::sim_communication::SimCommunicator;
use crate::simulation::scenario::ScenarioPartitionBuilder;
use crate::simulation::simulation::{Simulation, SimulationBuilder};
use crate::simulation::{io, logging};
use derive_builder::Builder;
use derive_more::Debug;
use nohash_hasher::IntMap;
use std::any::Any;
use std::cell::{RefCell, RefMut};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::{Arc, Barrier};
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
    T: RequestToAdapter + 'static,
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

/// Holds a map of external services that can be used in the simulation.
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

/// This struct holds objects that are local to a thread running a simulation partition.
/// They function as a connector between the simulation partition and the "outside" computational context.
#[derive(Clone, Debug, Builder)]
#[builder(pattern = "owned")]
pub struct ThreadLocalComputationalEnvironment {
    #[builder(default)]
    services: ExternalServices,
    // The value is of type Rc as this is a thread-local events publisher.
    #[builder(default)]
    events_publisher: Rc<RefCell<EventsManager>>,
}

impl Default for ThreadLocalComputationalEnvironment {
    fn default() -> Self {
        ThreadLocalComputationalEnvironment {
            services: ExternalServices::default(),
            events_publisher: Rc::new(RefCell::new(EventsManager::new())),
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

    pub fn events_publisher_borrow_mut(&mut self) -> RefMut<'_, EventsManager> {
        self.events_publisher.borrow_mut()
    }

    pub fn events_publisher(&self) -> Rc<RefCell<EventsManager>> {
        self.events_publisher.clone()
    }
}

#[derive(Debug, Builder)]
#[builder(pattern = "owned")]
pub struct PartitionArguments<C: SimCommunicator> {
    communicator: C,
    // Holding the builder instead of the built struct, the final struct holds ThreadRng, which can only be built on the corresponding thread.
    scenario_partition: ScenarioPartitionBuilder,
    #[builder(default)]
    external_services: ExternalServices,
    #[builder(default)]
    #[debug(skip)]
    events_subscriber: Vec<Box<OnEventFnBuilder>>,
    global_barrier: Arc<Barrier>,
}

fn execute_partition<C: SimCommunicator>(partition_arguments: PartitionArguments<C>) {
    let config = &partition_arguments.scenario_partition.config;
    let _guards = logging::init_logging(config, partition_arguments.communicator.rank());

    let comm = partition_arguments.communicator;
    let external_services = partition_arguments.external_services;
    let subscribers = partition_arguments.events_subscriber;

    let rank = comm.rank();
    let size = config.partitioning().num_parts;

    let scenario = partition_arguments.scenario_partition.build();

    let events = create_events(&scenario.config, rank, subscribers);

    let net_message_broker = NetMessageBroker::new(
        Rc::new(comm),
        &scenario.network,
        &scenario.network_partition,
        scenario.config.computational_setup().global_sync,
    );

    let comp_env = ThreadLocalComputationalEnvironmentBuilder::default()
        .services(external_services)
        .events_publisher(events.clone())
        .build()
        .unwrap();

    let mut simulation: Simulation<C> =
        SimulationBuilder::new(scenario, net_message_broker, comp_env).build();

    // Wait for all processes to arrive at this barrier. This is important to ensure that the
    // instrumentation of the simulation.run() method does not include any time it takes to
    // load the network and population.
    info!("Process #{rank} of {size} has arrived at initial barrier. Waiting for other processes and potential external services to reach global barrier.");
    partition_arguments.global_barrier.wait();
    simulation.run();

    // Drop guards here to make sure that the logging is flushed before we exit.
    // This is important for integration tests when the same test thread executes multiple simulations
    // one after another and consequently initializes logging for each test case.
    drop(_guards);
}

fn create_events(
    config: &Config,
    rank: u32,
    additional_subscribers: Vec<Box<OnEventFnBuilder>>,
) -> Rc<RefCell<EventsManager>> {
    let output_path = io::resolve_path(config.context(), &config.output().output_dir);

    let mut events = EventsManager::new();

    match config.output().write_events {
        WriteEvents::None => {}
        WriteEvents::Proto => {
            let events_file = format!("events.{rank}.binpb");
            let events_path = io::resolve_path(config.context(), &output_path.join(events_file));
            info!("adding events writer with path: {events_path:?}");
            ProtoEventsWriter::register(events_path)(&mut events)
        }
        WriteEvents::XmlGz => {
            let events_file = format!("events.{rank}.xml.gz");
            let events_path = io::resolve_path(config.context(), &output_path.join(events_file));
            info!("adding events writer with path: {events_path:?}");
            XmlEventsWriter::register(events_path)(&mut events)
        }
    }

    for subscriber in additional_subscribers {
        subscriber(&mut events);
    }

    Rc::new(RefCell::new(events))
}

/// Joins all simulation threads and then shuts down all adapter threads.
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
            let name = handle
                .thread()
                .name()
                .unwrap_or("unnamed_thread")
                .to_string();
            handle
                .join()
                .unwrap_or_else(|_| panic!("Error in adapter thread {:?}", name));
        }
    }

    // When all simulation threads are finished, we shutdown the adapters.
    for a in adapters {
        a.shutdown_sender.send(true).unwrap();
        let name = a
            .handle
            .thread()
            .name()
            .unwrap_or("unnamed_thread")
            .to_string();
        a.handle
            .join()
            .unwrap_or_else(|_| panic!("Error in adapter thread {:?}", name));
    }
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

    let (stripped, ext) = if filename.ends_with(".xml.gz") {
        (filename.strip_suffix(".xml.gz").unwrap(), "xml.gz")
    } else if filename.ends_with(".xml") {
        (filename.strip_suffix(".xml").unwrap(), "xml")
    } else if filename.ends_with(".binpb") {
        (filename.strip_suffix(".binpb").unwrap(), "binpb")
    } else {
        panic!("Unknown file extension")
    };

    let stripped = stripped
        .strip_suffix(format!(".{part}").as_str())
        .unwrap_or_else(|| stripped);

    let new_filename = format!("{stripped}.{part}.{ext}");
    path.parent().unwrap().join(new_filename)
}
