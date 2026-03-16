use crate::external_services::AdapterHandle;
use crate::simulation::config::{write_config, Config, Logging};
use crate::simulation::controller::{
    create_output_filename, insert_number_in_proto_filename, ExternalServices,
    PartitionArgumentsBuilder,
};
use crate::simulation::events::EventHandlerRegisterFn;
use crate::simulation::framework_events::{
    ControllerEvent, ControllerEventsManager, ControllerListenerRegisterFn,
    MobsimListenerRegisterFn,
};
use crate::simulation::messaging::sim_communication::local_communicator::ChannelSimCommunicator;
use crate::simulation::scenario::{MutableScenario, ScenarioPartition};
use crate::simulation::{controller, id, io};
use derive_more::Debug;
use nohash_hasher::IntMap;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Barrier};
use std::thread::JoinHandle;
use std::{fs, mem, thread};
use tracing::info;

#[derive(Debug)]
pub enum Scenario {
    Partitioned,
    Full(MutableScenario),
}

#[derive(Debug)]
pub struct Controller {
    scenario: Scenario,
    config: Arc<Config>,
    controller_events_manager: ControllerEventsManager,
    #[debug(skip)]
    event_handler_per_partition: HashMap<u32, Vec<Box<EventHandlerRegisterFn>>>,
    #[debug(skip)]
    mobsim_event_listener_per_partition: HashMap<u32, Vec<Box<MobsimListenerRegisterFn>>>,
    external_services: ExternalServices,
    global_barrier: Arc<Barrier>,
    adapter_handles: Vec<AdapterHandle>,
}

pub struct ControllerBuilder {
    scenario: MutableScenario,
    controller_event_register_fn: Vec<Box<ControllerListenerRegisterFn>>,
    event_handler_register_fn: HashMap<u32, Vec<Box<EventHandlerRegisterFn>>>,
    mobsim_event_register_fn: HashMap<u32, Vec<Box<MobsimListenerRegisterFn>>>,
    external_services: ExternalServices,
    global_barrier: Option<Arc<Barrier>>,
    adapter_handles: Vec<AdapterHandle>,
}

impl ControllerBuilder {
    pub fn default_with_scenario(scenario: MutableScenario) -> Self {
        ControllerBuilder {
            scenario,
            controller_event_register_fn: Vec::new(),
            event_handler_register_fn: HashMap::new(),
            mobsim_event_register_fn: HashMap::new(),
            external_services: ExternalServices::default(),
            global_barrier: None,
            adapter_handles: Vec::new(),
        }
    }

    // Implementing a custom build function in order to set the barrier if not set by the user.
    pub fn build(mut self) -> Result<Controller, String> {
        // create a barrier for the number of partitions, if not provided
        let barrier = self.global_barrier.take().unwrap_or_else(|| {
            Arc::new(Barrier::new(
                self.scenario.config.partitioning().num_parts as usize,
            ))
        });

        let mut controller_event_manager = ControllerEventsManager::default();
        for register_fn in self.controller_event_register_fn {
            register_fn(&mut controller_event_manager);
        }

        let config = self.scenario.config.clone();

        Ok(Controller {
            scenario: Scenario::Full(self.scenario),
            config,
            controller_events_manager: controller_event_manager,
            event_handler_per_partition: self.event_handler_register_fn,
            mobsim_event_listener_per_partition: self.mobsim_event_register_fn,
            external_services: self.external_services,
            global_barrier: barrier,
            adapter_handles: self.adapter_handles,
        })
    }

    pub fn controller_event_register_fn(
        mut self,
        v: Vec<Box<ControllerListenerRegisterFn>>,
    ) -> Self {
        self.controller_event_register_fn = v;
        self
    }

    pub fn event_handler_register_fn(
        mut self,
        v: HashMap<u32, Vec<Box<EventHandlerRegisterFn>>>,
    ) -> Self {
        self.event_handler_register_fn = v;
        self
    }

    pub fn mobsim_event_register_fn(
        mut self,
        v: HashMap<u32, Vec<Box<MobsimListenerRegisterFn>>>,
    ) -> Self {
        self.mobsim_event_register_fn = v;
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

impl Controller {
    /// Runs the simulation and joins all threads before returning.
    pub fn run(mut self) {
        self.controller_events_manager
            .process_event(ControllerEvent::startup(true));

        let output_path = io::resolve_path(self.config.context(), &self.config.output().output_dir);

        let events_path = output_path.join("events");

        fs::create_dir_all(&output_path).expect("Failed to create output path");
        fs::create_dir_all(&events_path).expect("Failed to create events output path");

        if Logging::Info == self.config.output().logging {
            let log_path = output_path.join("logs");
            fs::create_dir_all(&log_path).expect("Failed to create logs output path");
        }

        // TODO This needs to be shifted to the end once we have iterations. Therefore, the threads need the simulation agents. paul, mar '26
        info!("Writing output files:");
        info!("    ... Config ...");
        self.write_output_config(output_path.clone());
        info!("    ... Network ...");
        self.write_output_network(output_path.clone());

        info!("=========== Start Iteration 0 ===========");

        self.controller_events_manager
            .process_event(ControllerEvent::before_mobsim(true));

        let handles = self.run_channel();
        controller::try_join(handles, mem::take(&mut self.adapter_handles));

        self.controller_events_manager
            .process_event(ControllerEvent::after_mobsim(true));

        self.controller_events_manager
            .process_event(ControllerEvent::iteration_ends(true));

        info!("=========== End Iteration 0 ===========");

        if self.config.output().write_events == crate::simulation::config::WriteEvents::Proto {
            info!("    ... ID store ...");
            Self::write_output_id_store(&output_path);
        }

        self.controller_events_manager
            .process_event(ControllerEvent::shutdown(true));
    }

    fn run_channel(&mut self) -> IntMap<u32, JoinHandle<()>> {
        let scenario = {
            let s = mem::replace(&mut self.scenario, Scenario::Partitioned);
            match s {
                Scenario::Partitioned => {
                    panic!("Wanted to create partitions, but scenario is already partitioned. This shouldn't happen.")
                }
                Scenario::Full(s) => s,
            }
        };

        // Is of type Vec<Option<>> because later we iteratively take the partition builder and construct
        // the actual partitions.
        let mut partitions: Vec<Option<ScenarioPartition>> = ScenarioPartition::from(scenario)
            .into_iter()
            .map(Some)
            .collect();

        let num_parts = self.config.partitioning().num_parts;
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
        write_config(self.config.as_ref(), output_path);
    }

    fn write_output_network(&mut self, output_path: PathBuf) {
        let net_in_path = if let Some(path) = &self.config.network().path {
            io::resolve_path(self.config.context(), path)
        } else {
            io::resolve_path(self.config.context(), &PathBuf::from("network.xml.gz"))
        };
        let mut net_out_path = create_output_filename(&output_path, &net_in_path);
        net_out_path =
            insert_number_in_proto_filename(&net_out_path, self.config.partitioning().num_parts);

        match &self.scenario {
            Scenario::Partitioned => {
                panic!("Tried to write network to file, but scenario is partitioned. This shouldn't happen.")
            }
            Scenario::Full(s) => {
                s.network.to_file(&net_out_path);
            }
        }
    }

    fn write_output_id_store(output_path: &PathBuf) {
        id::store_to_file(&output_path.join("output_ids.binpb"));
    }
}
