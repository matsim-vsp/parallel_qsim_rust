use crate::simulation::config::{CommandLineArgs, Config};
use crate::simulation::controller;
use crate::simulation::controller::{ExternalServices, PartitionArgumentsBuilder};
use crate::simulation::logging::init_std_out_logging;
use crate::simulation::messaging::events::EventsSubscriber;
use crate::simulation::messaging::sim_communication::local_communicator::ChannelSimCommunicator;
use clap::Parser;
use nohash_hasher::IntMap;
use std::collections::HashMap;
use std::thread;
use std::thread::JoinHandle;
use tracing::info;

pub fn run_channel(
    config: Config,
    mut events_subscriber_per_partition: HashMap<u32, Vec<Box<dyn EventsSubscriber + Send>>>,
    external_services: ExternalServices,
) -> IntMap<u32, JoinHandle<()>> {
    info!(
        "Starting multithreaded Simulation with {} partitions.",
        config.partitioning().num_parts
    );
    let comms = ChannelSimCommunicator::create_n_2_n(config.partitioning().num_parts);

    let handles: IntMap<u32, JoinHandle<()>> = comms
        .into_iter()
        .map(|comm| {
            let rank = comm.rank();
            let args = PartitionArgumentsBuilder::default()
                .communicator(comm)
                .config(config.clone())
                .external_services(external_services.clone())
                .events_subscriber(
                    events_subscriber_per_partition
                        .remove(&rank)
                        .unwrap_or_default(),
                )
                .build()
                .unwrap();
            (
                rank,
                thread::Builder::new()
                    .name(rank.to_string())
                    .spawn(move || controller::execute_partition(args))
                    .unwrap(),
            )
        })
        .collect();

    handles
}

pub fn run_channel_from_args() {
    let _guard = init_std_out_logging();

    let args = CommandLineArgs::parse();
    info!("Started with args: {:?}", args);

    let config = Config::from(args);

    let handles = run_channel(config, HashMap::new(), ExternalServices::default());

    controller::try_join(handles, Default::default())
}
