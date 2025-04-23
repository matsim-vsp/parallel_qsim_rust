use crate::simulation::config::{CommandLineArgs, Config};
use crate::simulation::messaging::communication::local_communicator::ChannelSimCommunicator;
use crate::simulation::{controller, logging};
use clap::Parser;
use nohash_hasher::IntMap;
use std::thread;
use std::thread::JoinHandle;
use tracing::info;

pub fn run_channel() {
    let args = CommandLineArgs::parse();
    let config = Config::from_file(&args);

    let _guards = logging::init_logging(&config, &args.config_path, 0);

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
                    .spawn(move || controller::execute_partition(comm, &config_path))
                    .unwrap(),
            )
        })
        .collect();

    controller::try_join(handles);
}
