use crate::external_services::routing::{InternalRoutingRequest, RoutingServiceAdapter};
use crate::external_services::{execute_adapter, ExternalServiceType};
use crate::simulation::config::{CommandLineArgs, Config};
use crate::simulation::messaging::events::EventsPublisher;
use crate::simulation::messaging::sim_communication::local_communicator::ChannelSimCommunicator;
use crate::simulation::{controller, logging};
use clap::Parser;
use derive_builder::Builder;
use nohash_hasher::IntMap;
use std::any::Any;
use std::cell::{RefCell, RefMut};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use std::thread;
use std::thread::JoinHandle;
use tracing::info;

#[derive(Clone, Debug, Builder)]
pub struct ComputationalEnvironment {
    // This is an Arc, since it is shared across threads.
    #[builder(default)]
    services: HashMap<ExternalServiceType, Arc<dyn Any + Send + Sync>>,
    #[builder(default)]
    events_publisher: Rc<RefCell<EventsPublisher>>,
}

impl Default for ComputationalEnvironment {
    fn default() -> Self {
        ComputationalEnvironment {
            services: HashMap::new(),
            events_publisher: Rc::new(RefCell::new(EventsPublisher::new())),
        }
    }
}

impl ComputationalEnvironment {
    pub fn get_service<T: Any + Send + Sync>(
        &self,
        service_type: &ExternalServiceType,
    ) -> Option<&T> {
        self.services
            .get(service_type)
            .and_then(|s| s.downcast_ref::<T>())
    }

    pub fn events_publisher_borrow_mut(&mut self) -> RefMut<'_, EventsPublisher> {
        self.events_publisher.borrow_mut()
    }

    pub fn events_publisher(&self) -> Rc<RefCell<EventsPublisher>> {
        self.events_publisher.clone()
    }
}

pub fn run_channel() {
    let args = CommandLineArgs::parse();
    let config = Config::from_file(&args);

    let _guards = logging::init_logging(&config, &args.config_path, 0);

    info!(
        "Starting multithreaded Simulation with {} partitions.",
        config.partitioning().num_parts
    );
    let comms = ChannelSimCommunicator::create_n_2_n(config.partitioning().num_parts);

    // let (tx, rx) = mpsc::channel(10000);
    // let (shutdown_send, shutdown_recv) = tokio::sync::watch::channel(false);
    //
    // let adapter = RoutingServiceAdapter::new("");
    //
    // let routing_thread = thread::Builder::new()
    //     .name("routing_adapter".to_string())
    //     .spawn(move || execute_adapter(rx, adapter, shutdown_recv));
    //
    // let mut map = HashMap::new();
    // map.insert(
    //     ExternalServiceType::Routing("pt".to_string()),
    //     Box::new(tx) as Box<dyn Any + Send + Sync>,
    // );

    let handles: IntMap<u32, JoinHandle<()>> = comms
        .into_iter()
        .map(|comm| {
            // let comp_env = computational_env.clone();
            let config_path = args.clone();
            (
                comm.rank(),
                thread::Builder::new()
                    .name(comm.rank().to_string())
                    .spawn(move || {
                        controller::execute_partition(comm, Default::default(), &config_path)
                    })
                    .unwrap(),
            )
        })
        .collect();

    // controller::try_join(handles, vec![(routing_thread.unwrap(), shutdown_send)]);
    controller::try_join(handles, vec![]);
}
