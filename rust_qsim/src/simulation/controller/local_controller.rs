use crate::external_services::AdapterHandle;
use crate::simulation::config::write_config;
use crate::simulation::controller::{
    create_output_filename, insert_number_in_proto_filename, ExternalServices,
    PartitionArgumentsBuilder,
};
use crate::simulation::events::EventHandlerRegistrator;
use crate::simulation::framework_events::{
    ControllerEvent, ControllerEventBus, ControllerListenerRegistrator, GeneralControllerEvent,
    MobsimListenerRegistrator,
};
use crate::simulation::messaging::sim_communication::local_communicator::ChannelSimCommunicator;
use crate::simulation::scenario::{Scenario, ScenarioPartitionBuilder};
use crate::simulation::{controller, id, io};
use derive_more::Debug;
use nohash_hasher::IntMap;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Barrier};
use std::thread::JoinHandle;
use std::{fs, thread};
use tracing::info;

#[derive(Debug)]
pub struct LocalController {
    scenario: Scenario,
    controller_event_bus: ControllerEventBus,
    #[debug(skip)]
    event_handler_per_partition: HashMap<u32, Vec<Box<EventHandlerRegistrator>>>,
    #[debug(skip)]
    mobsim_event_listener_per_partition: HashMap<u32, Vec<Box<MobsimListenerRegistrator>>>,
    external_services: ExternalServices,
    global_barrier: Arc<Barrier>,
    adapter_handles: Vec<AdapterHandle>,
}

pub struct LocalControllerBuilder {
    scenario: Scenario,
    controller_event_registrators: Vec<Box<ControllerListenerRegistrator>>,
    event_handler_registrators: HashMap<u32, Vec<Box<EventHandlerRegistrator>>>,
    mobsim_event_registrators: HashMap<u32, Vec<Box<MobsimListenerRegistrator>>>,
    external_services: ExternalServices,
    global_barrier: Option<Arc<Barrier>>,
    adapter_handles: Vec<AdapterHandle>,
}

impl LocalControllerBuilder {
    pub fn default_with_scenario(scenario: Scenario) -> Self {
        LocalControllerBuilder {
            scenario,
            controller_event_registrators: Vec::new(),
            event_handler_registrators: HashMap::new(),
            mobsim_event_registrators: HashMap::new(),
            external_services: ExternalServices::default(),
            global_barrier: None,
            adapter_handles: Vec::new(),
        }
    }

    // Implementing a custom build function in order to set the barrier if not set by the user.
    pub fn build(mut self) -> Result<LocalController, String> {
        // create a barrier for the number of partitions, if not provided
        let barrier = self.global_barrier.take().unwrap_or_else(|| {
            Arc::new(Barrier::new(
                self.scenario.config.partitioning().num_parts as usize,
            ))
        });

        let mut controller_event_bus = ControllerEventBus::default();
        for registrator in self.controller_event_registrators {
            registrator(&mut controller_event_bus);
        }

        Ok(LocalController {
            scenario: self.scenario,
            controller_event_bus,
            event_handler_per_partition: self.event_handler_registrators,
            mobsim_event_listener_per_partition: self.mobsim_event_registrators,
            external_services: self.external_services,
            global_barrier: barrier,
            adapter_handles: self.adapter_handles,
        })
    }

    pub fn controller_event_registrators(
        mut self,
        v: Vec<Box<ControllerListenerRegistrator>>,
    ) -> Self {
        self.controller_event_registrators = v;
        self
    }

    pub fn event_handler_registrators(
        mut self,
        v: HashMap<u32, Vec<Box<EventHandlerRegistrator>>>,
    ) -> Self {
        self.event_handler_registrators = v;
        self
    }

    pub fn mobsim_event_registrators(
        mut self,
        v: HashMap<u32, Vec<Box<MobsimListenerRegistrator>>>,
    ) -> Self {
        self.mobsim_event_registrators = v;
        self
    }

    pub fn external_services(mut self, e: ExternalServices) -> Self {
        self.external_services = e;
        self
    }

    pub fn global_barrier(mut self, b: Arc<Barrier>) -> Self {
        self.global_barrier = Some(b);
        self
    }

    pub fn adapter_handles(mut self, v: Vec<AdapterHandle>) -> Self {
        self.adapter_handles = v;
        self
    }
}

impl LocalController {
    /// Runs the simulation and joins all threads before returning.
    pub fn run(mut self) {
        let _ =
            self.controller_event_bus
                .process(ControllerEvent::Startup(GeneralControllerEvent {
                    iteration: 0,
                    last_iteration: 0,
                }));

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
                    .event_handler(
                        self.event_handler_per_partition
                            .remove(&rank)
                            .unwrap_or_default(),
                    )
                    .mobsim_event_listener(
                        self.mobsim_event_listener_per_partition
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
