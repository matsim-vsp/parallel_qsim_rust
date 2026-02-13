use crate::external_services::AdapterHandle;
use crate::simulation::config::write_config;
use crate::simulation::controller::{
    create_output_filename, insert_number_in_proto_filename, ExternalServices,
    PartitionArgumentsBuilder,
};
use crate::simulation::events::OnEventFnBuilder;
use crate::simulation::messaging::sim_communication::local_communicator::ChannelSimCommunicator;
use crate::simulation::scenario::{Scenario, ScenarioPartitionBuilder};
use crate::simulation::{controller, id, io};
use derive_builder::Builder;
use derive_more::Debug;
use nohash_hasher::IntMap;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Barrier};
use std::thread::JoinHandle;
use std::{fs, thread};
use tracing::info;

#[derive(Debug, Builder)]
#[builder(pattern = "owned", build_fn(skip))]
pub struct LocalController {
    scenario: Scenario,
    #[builder(default)]
    #[debug(skip)]
    events_subscriber_per_partition: HashMap<u32, Vec<Box<OnEventFnBuilder>>>,
    #[builder(default)]
    external_services: ExternalServices,
    global_barrier: Arc<Barrier>,
    adapter_handles: Vec<AdapterHandle>,
}

impl LocalControllerBuilder {
    // Implementing a custom build function in order to set the barrier if not set by the user.
    pub fn build(self) -> Result<LocalController, String> {
        let scenario = self.scenario.ok_or("scenario is required")?;

        // create a barrier for the number of partitions, if not provided
        let barrier = self.global_barrier.clone().unwrap_or_else(|| {
            Arc::new(Barrier::new(
                scenario.config.partitioning().num_parts as usize,
            ))
        });

        Ok(LocalController {
            scenario,
            events_subscriber_per_partition: self
                .events_subscriber_per_partition
                .unwrap_or_default(),
            external_services: self.external_services.clone().unwrap_or_default(),
            global_barrier: barrier,
            adapter_handles: self.adapter_handles.unwrap_or_default(),
        })
    }
}

impl LocalController {
    /// Runs the simulation and joins all threads before returning.
    pub fn run(mut self) {
        let output_path = io::resolve_path(
            self.scenario.config.context(),
            &self.scenario.config.output().output_dir,
        );
        fs::create_dir_all(&output_path).expect("Failed to create output path");

        let handles = self.run_channel();
        controller::try_join(handles, std::mem::take(&mut self.adapter_handles));

        info!("=========== End Iteration 0 ===========");

        info!("Writing output files:");
        info!("    ... Config ...");
        self.write_output_config(output_path.clone());
        info!("    ... Network ...");
        self.write_output_network(output_path.clone());

        if self.scenario.config.output().write_events
            == crate::simulation::config::WriteEvents::Proto
        {
            info!("    ... ID store ...");
            Self::write_output_id_store(&output_path);
        }
    }

    fn run_channel(&mut self) -> IntMap<u32, JoinHandle<()>> {
        // Is of type Vec<Option<>> because later we iteratively take the partition builder and construct
        // the actual partitions.
        let mut partitions: Vec<Option<ScenarioPartitionBuilder>> =
            ScenarioPartitionBuilder::from(&mut self.scenario)
                .into_iter()
                .map(Some)
                .collect();

        info!("=========== Start Iteration 0 ===========");

        let num_parts = self.scenario.config.partitioning().num_parts;
        info!(
            "Starting multithreaded Simulation with {} partitions.",
            num_parts
        );
        let comms = ChannelSimCommunicator::create_n_2_n(num_parts);

        let handles: IntMap<u32, JoinHandle<()>> = comms
            .into_iter()
            .map(|comm| {
                let rank = comm.rank();

                // Replaces the Some(partition) with None
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

    fn write_output_config(&mut self, output_path: PathBuf) {
        write_config(self.scenario.config.as_ref(), output_path);
    }

    fn write_output_network(&mut self, output_path: PathBuf) {
        let net_in_path = io::resolve_path(
            self.scenario.config.context(),
            &self.scenario.config.network().path,
        );
        let mut net_out_path = create_output_filename(&output_path, &net_in_path);
        net_out_path = insert_number_in_proto_filename(
            &net_out_path,
            self.scenario.config.partitioning().num_parts,
        );
        self.scenario.network.to_file(&net_out_path);
    }

    fn write_output_id_store(output_path: &PathBuf) {
        id::store_to_file(&output_path.join("output_ids.binpb"));
    }
}
