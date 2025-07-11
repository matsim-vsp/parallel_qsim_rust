use crate::external_services::ExternalServiceType;
use crate::simulation::config::{CommandLineArgs, Config};
use crate::simulation::controller::PartitionArgumentsBuilder;
use crate::simulation::messaging::events::EventsSubscriber;
use crate::simulation::messaging::sim_communication::local_communicator::ChannelSimCommunicator;
use crate::simulation::{controller, logging};
use clap::Parser;
use nohash_hasher::IntMap;
use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;
use std::thread;
use std::thread::JoinHandle;
use tracing::info;

pub fn run_channel(
    config: Config,
    command_line_args: CommandLineArgs,
    mut events_subscriber_per_partition: HashMap<u32, Vec<Box<dyn EventsSubscriber + Send>>>,
    external_services: HashMap<ExternalServiceType, Arc<dyn Any + Send + Sync>>,
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
                .command_line_args(command_line_args.clone())
                .communicator(comm)
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
    let args = CommandLineArgs::parse();
    let config = Config::from_file(&args);

    let _guards = logging::init_logging(&config, &args.config_path, 0);

    let handles = run_channel(config, args, HashMap::new(), HashMap::new());

    controller::try_join(handles, Default::default())
}
