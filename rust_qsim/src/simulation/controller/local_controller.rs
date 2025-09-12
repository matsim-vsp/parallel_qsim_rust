use crate::simulation::config::{CommandLineArgs, Config};
use crate::simulation::controller;
use crate::simulation::controller::{ExternalServices, PartitionArgumentsBuilder};
use crate::simulation::logging::init_std_out_logging_thread_local;
use crate::simulation::messaging::events::EventsSubscriber;
use crate::simulation::messaging::sim_communication::local_communicator::ChannelSimCommunicator;
use crate::simulation::scenario::{GlobalScenario, ScenarioPartitionBuilder};
use clap::Parser;
use derive_builder::Builder;
use nohash_hasher::IntMap;
use std::collections::HashMap;
use std::thread;
use std::thread::JoinHandle;
use tracing::info;

#[derive(Debug, Builder)]
#[builder(pattern = "owned")]
pub struct LocalController {
    global_scenario: GlobalScenario,
    #[builder(default)]
    events_subscriber_per_partition: HashMap<u32, Vec<Box<dyn EventsSubscriber + Send>>>,
    #[builder(default)]
    external_services: ExternalServices,
}

impl LocalController {
    pub fn run(self) -> IntMap<u32, JoinHandle<()>> {
        let handles = Self::run_channel(
            self.global_scenario,
            self.events_subscriber_per_partition,
            self.external_services,
        );
        handles
    }

    fn run_channel(
        global_scenario: GlobalScenario,
        mut events_subscriber_per_partition: HashMap<u32, Vec<Box<dyn EventsSubscriber + Send>>>,
        external_services: ExternalServices,
    ) -> IntMap<u32, JoinHandle<()>> {
        let num_parts = global_scenario.config.partitioning().num_parts;
        let mut partitions: Vec<Option<ScenarioPartitionBuilder>> =
            ScenarioPartitionBuilder::from(global_scenario)
                .into_iter()
                .map(Some)
                .collect();

        info!(
            "Starting multithreaded Simulation with {} partitions.",
            num_parts
        );
        let comms = ChannelSimCommunicator::create_n_2_n(num_parts);

        let handles: IntMap<u32, JoinHandle<()>> = comms
            .into_iter()
            .map(|comm| {
                let rank = comm.rank();

                let partition = partitions[rank as usize]
                    .take()
                    .expect("No empty partition");

                let args = PartitionArgumentsBuilder::default()
                    .communicator(comm)
                    .scenario_partition(partition)
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
}

pub fn run_channel_from_args() {
    let _guard = init_std_out_logging_thread_local();

    let args = CommandLineArgs::parse();
    info!("Started with args: {:?}", args);

    // Load and adapt config
    let config = Config::from(args);

    // Load and adapt scenario
    let scenario = GlobalScenario::build(config);

    // Create and run simulation
    let controller = LocalControllerBuilder::default()
        .global_scenario(scenario)
        .events_subscriber_per_partition(HashMap::default())
        .external_services(ExternalServices::default())
        .build()
        .unwrap();

    let handles = controller.run();
    controller::try_join(handles, Default::default())
}
