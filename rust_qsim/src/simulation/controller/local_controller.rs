use crate::simulation::config::{write_config, CommandLineArgs, Config};
use crate::simulation::controller::{ExternalServices, PartitionArgumentsBuilder};
use crate::simulation::events::OnEventFnBuilder;
use crate::simulation::logging::init_std_out_logging_thread_local;
use crate::simulation::messaging::sim_communication::local_communicator::ChannelSimCommunicator;
use crate::simulation::scenario::{GlobalScenario, ScenarioPartitionBuilder};
use crate::simulation::{controller, io};
use clap::Parser;
use derive_builder::Builder;
use derive_more::Debug;
use nohash_hasher::IntMap;
use std::collections::HashMap;
use std::sync::{Arc, Barrier};
use std::thread::JoinHandle;
use std::{fs, thread};
use tracing::info;

#[derive(Debug, Builder)]
#[builder(pattern = "owned", build_fn(skip))]
pub struct LocalController {
    global_scenario: GlobalScenario,
    #[builder(default)]
    #[debug(skip)]
    events_subscriber_per_partition: HashMap<u32, Vec<Box<OnEventFnBuilder>>>,
    #[builder(default)]
    external_services: ExternalServices,
    global_barrier: Arc<Barrier>,
}

impl LocalControllerBuilder {
    // Implementing a custom build function in order to set the barrier if not set by the user.
    pub fn build(self) -> Result<LocalController, String> {
        let global_scenario = self.global_scenario.ok_or("global_scenario is required")?;

        // create a barrier for the number of partitions, if not provided
        let barrier = self.global_barrier.clone().unwrap_or_else(|| {
            Arc::new(Barrier::new(
                global_scenario.config.partitioning().num_parts as usize,
            ))
        });

        Ok(LocalController {
            global_scenario,
            events_subscriber_per_partition: self
                .events_subscriber_per_partition
                .unwrap_or_default(),
            external_services: self.external_services.clone().unwrap_or_default(),
            global_barrier: barrier,
        })
    }
}

impl LocalController {
    /// Runs the simulation and joins all threads before returning.
    pub fn run_and_join_handles(self) {
        let handles = self.run();
        controller::try_join(handles, Default::default());
    }

    /// This function starts the simulation threads and returns their JoinHandles. The caller is
    /// responsible for joining the threads! E.g. if not joined, the simulation probably won't run.
    pub fn run(self) -> IntMap<u32, JoinHandle<()>> {
        Self::run_channel(self)
    }

    fn run_channel(mut self) -> IntMap<u32, JoinHandle<()>> {
        let output_path = io::resolve_path(
            self.global_scenario.config.context(),
            &self.global_scenario.config.output().output_dir,
        );
        fs::create_dir_all(&output_path).expect("Failed to create output path");

        write_config(self.global_scenario.config.as_ref(), output_path);

        let num_parts = self.global_scenario.config.partitioning().num_parts;
        let mut partitions: Vec<Option<ScenarioPartitionBuilder>> =
            ScenarioPartitionBuilder::from(self.global_scenario)
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
                    .global_barrier(self.global_barrier.clone())
                    .scenario_partition(partition)
                    .external_services(self.external_services.clone())
                    .events_subscriber(
                        self.events_subscriber_per_partition
                            .remove(&rank)
                            .unwrap_or_default(),
                    )
                    .build()
                    .unwrap();
                (
                    rank,
                    thread::Builder::new()
                        .name(format!("qsim-{}", rank))
                        .spawn(move || controller::execute_partition(args))
                        .unwrap(),
                )
            })
            .collect();

        handles
    }
}
