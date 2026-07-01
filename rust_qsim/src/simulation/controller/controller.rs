use crate::external_services::AdapterHandle;
use crate::simulation::agents::agent::SimulationAgent;
use crate::simulation::config::{Config, Logging, OverwriteFiles, write_config};
use crate::simulation::controller::{
    ExternalServices, PartitionArgumentsBuilder, create_output_filename,
};
use crate::simulation::events::EventHandlerRegisterFn;
use crate::simulation::framework_events::{
    ControllerEvent, ControllerEventsManager, ControllerListenerRegisterFn,
    MobsimListenerRegisterFn, PartitionListenerRegisterFn,
};
use crate::simulation::messaging::sim_communication::local_communicator::ChannelSimCommunicator;
use crate::simulation::population::agent_source::{
    DynAgentSource, IntoDynAgentSource, PopulationAgentSource,
};
use crate::simulation::scenario::network::Network;
use crate::simulation::scenario::population::Population;
use crate::simulation::scenario::prepare_for_sim::prepare_for_sim;
use crate::simulation::scenario::{MutableScenario, ScenarioPartition};
use crate::simulation::{controller, id, io};
use derive_more::Debug;
use nohash_hasher::IntMap;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
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
    network: Arc<Network>,
    #[debug(skip)]
    agent_source: DynAgentSource,
    controller_events_manager: ControllerEventsManager,
    #[debug(skip)]
    event_handler_per_partition: HashMap<u32, Vec<Box<EventHandlerRegisterFn>>>,
    #[debug(skip)]
    mobsim_event_listener_per_partition: HashMap<u32, Vec<Box<MobsimListenerRegisterFn>>>,
    #[debug(skip)]
    partition_event_listener_per_partition: HashMap<u32, Vec<Box<PartitionListenerRegisterFn>>>,
    external_services: ExternalServices,
    global_barrier: Arc<Barrier>,
    adapter_handles: Vec<AdapterHandle>,
}

pub struct ControllerBuilder {
    scenario: MutableScenario,
    agent_source: DynAgentSource,
    controller_event_register_fn: Vec<Box<ControllerListenerRegisterFn>>,
    event_handler_register_fn: HashMap<u32, Vec<Box<EventHandlerRegisterFn>>>,
    mobsim_event_register_fn: HashMap<u32, Vec<Box<MobsimListenerRegisterFn>>>,
    partition_event_register_fn: HashMap<u32, Vec<Box<PartitionListenerRegisterFn>>>,
    external_services: ExternalServices,
    global_barrier: Option<Arc<Barrier>>,
    adapter_handles: Vec<AdapterHandle>,
}

impl ControllerBuilder {
    pub fn default_with_scenario(scenario: MutableScenario) -> Self {
        ControllerBuilder {
            scenario,
            agent_source: Arc::new(PopulationAgentSource),
            controller_event_register_fn: Vec::new(),
            event_handler_register_fn: HashMap::new(),
            mobsim_event_register_fn: HashMap::new(),
            partition_event_register_fn: HashMap::new(),
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
        let network = self.scenario.network.clone();

        Ok(Controller {
            scenario: Scenario::Full(self.scenario),
            config,
            network,
            agent_source: self.agent_source,
            controller_events_manager: controller_event_manager,
            event_handler_per_partition: self.event_handler_register_fn,
            mobsim_event_listener_per_partition: self.mobsim_event_register_fn,
            partition_event_listener_per_partition: self.partition_event_register_fn,
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

    pub fn partition_event_register_fn(
        mut self,
        v: HashMap<u32, Vec<Box<PartitionListenerRegisterFn>>>,
    ) -> Self {
        self.partition_event_register_fn = v;
        self
    }

    pub fn agent_source(mut self, source: impl IntoDynAgentSource) -> Self {
        self.agent_source = source.into_dyn_agent_source();
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

        prepare_output_directory(&output_path, self.config.output().overwrite_files)
            .unwrap_or_else(|err| panic!("{err}"));
        fs::create_dir_all(&events_path).expect("Failed to create events output path");

        if Logging::Info == self.config.output().logging {
            let log_path = output_path.join("logs");
            fs::create_dir_all(&log_path).expect("Failed to create logs output path");
        }

        let end_iter = 0u32;
        for iteration in 0..=end_iter {
            self.run_iteration(iteration, end_iter);
        }

        info!("Writing output files:");
        if self.config.output().write_events == crate::simulation::config::WriteEvents::Proto {
            info!("    ... ID store ...");
            Self::write_output_id_store(&output_path);
        }
        info!("    ... Config ...");
        self.write_output_config(output_path.clone());
        info!("    ... Network ...");
        self.write_output_network(output_path.clone());
        info!("    ... Population ...");
        self.write_output_population(output_path.clone());

        self.controller_events_manager
            .process_event(ControllerEvent::shutdown(true));
    }

    fn run_iteration(&mut self, iteration: u32, end_iter: u32) {
        let is_last_iteration = iteration == end_iter;
        info!("=========== Start Iteration {} ===========", iteration);

        self.controller_events_manager
            .process_event(ControllerEvent::iteration_starts(is_last_iteration));

        let population = self.run_mobsim_phase(iteration, is_last_iteration);
        let population = self.run_scoring_phase(iteration, is_last_iteration, population);
        let population = if is_last_iteration {
            population
        } else {
            self.run_replanning_phase(iteration, population)
        };

        self.scenario = Scenario::Full(MutableScenario {
            network: self.network.clone(),
            garage: Default::default(),
            population,
            config: self.config.clone(),
        });

        self.controller_events_manager
            .process_event(ControllerEvent::iteration_ends(is_last_iteration));

        info!("=========== End Iteration {} ===========", iteration);
    }

    fn run_mobsim_phase(&mut self, iteration: u32, is_last_iteration: bool) -> Population {
        info!("Starting mobsim phase for iteration {iteration}");

        self.controller_events_manager
            .process_event(ControllerEvent::before_mobsim(is_last_iteration));

        let handles = self.run_channel();
        let agents = controller::try_join(handles, mem::take(&mut self.adapter_handles));

        self.controller_events_manager
            .process_event(ControllerEvent::after_mobsim(is_last_iteration));

        let persons = agents.into_iter().filter_map(|a| a.into_person()).collect();
        Population::from_persons(persons)
    }

    fn run_scoring_phase(
        &mut self,
        iteration: u32,
        is_last_iteration: bool,
        population: Population,
    ) -> Population {
        info!("Starting scoring phase for iteration {iteration}");

        self.controller_events_manager
            .process_event(ControllerEvent::scoring(is_last_iteration));

        population
    }

    fn run_replanning_phase(&mut self, iteration: u32, population: Population) -> Population {
        info!("Starting replanning phase for iteration {iteration}");

        self.controller_events_manager
            .process_event(ControllerEvent::replanning(false));

        population
    }

    fn run_channel(&mut self) -> IntMap<u32, JoinHandle<Vec<SimulationAgent>>> {
        let mut scenario = {
            let s = mem::replace(&mut self.scenario, Scenario::Partitioned);
            match s {
                Scenario::Partitioned => {
                    panic!(
                        "Wanted to create partitions, but mod is already partitioned. This shouldn't happen."
                    )
                }
                Scenario::Full(s) => s,
            }
        };

        prepare_for_sim(&mut scenario).unwrap_or_else(|err| panic!("{err}"));

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

        let handles: IntMap<u32, _> = comms
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
                    .agent_source(self.agent_source.clone())
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
                    .partition_event_listener(
                        self.partition_event_listener_per_partition
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
        let net_out_path =
            create_output_filename(&output_path, &PathBuf::from("output_network.xml.gz"));

        match &self.scenario {
            Scenario::Partitioned => {
                panic!(
                    "Tried to write network to file, but mod is partitioned. This shouldn't happen."
                )
            }
            Scenario::Full(s) => {
                s.network.to_file(&net_out_path);
            }
        }
    }

    fn write_output_population(&mut self, output_path: impl AsRef<Path>) {
        let pop_out_path =
            create_output_filename(&output_path, &PathBuf::from("output_population.xml.gz"));

        if let Scenario::Full(mut_scen) = &self.scenario {
            mut_scen.population.to_file(&pop_out_path);
        } else {
            panic!("Cannot write output population because it is split among the threads.");
        }
    }

    fn write_output_id_store(output_path: impl AsRef<Path>) {
        id::store_to_file(&output_path.as_ref().join("output_ids.binpb"));
    }
}

fn prepare_output_directory(
    output_path: &Path,
    overwrite_files: OverwriteFiles,
) -> Result<(), String> {
    if output_path.exists() {
        match overwrite_files {
            OverwriteFiles::DeleteDirectoryIfExists => {
                fs::remove_dir_all(output_path).map_err(|err| {
                    format!(
                        "Failed to delete existing output directory {}: {}",
                        output_path.display(),
                        err
                    )
                })?
            }
            OverwriteFiles::FailIfDirectoryExists => {
                return Err(format!(
                    "Output directory already exists: {}",
                    output_path.display()
                ));
            }
            OverwriteFiles::OverwriteExistingFiles => {}
        }
    }

    fs::create_dir_all(output_path).map_err(|err| {
        format!(
            "Failed to create output path {}: {}",
            output_path.display(),
            err
        )
    })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::prepare_output_directory;
    use crate::simulation::config::OverwriteFiles;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn delete_directory_if_exists_recreates_output_dir() {
        let dir = tempdir().unwrap();
        let output_dir = dir.path().join("output");
        fs::create_dir_all(&output_dir).unwrap();
        let stale_file = output_dir.join("stale.txt");
        fs::write(&stale_file, "stale").unwrap();

        prepare_output_directory(&output_dir, OverwriteFiles::DeleteDirectoryIfExists).unwrap();

        assert!(output_dir.exists());
        assert!(!stale_file.exists());
    }

    #[test]
    fn fail_if_directory_exists_returns_error() {
        let dir = tempdir().unwrap();
        let output_dir = dir.path().join("output");
        fs::create_dir_all(&output_dir).unwrap();

        let result = prepare_output_directory(&output_dir, OverwriteFiles::FailIfDirectoryExists);

        assert!(result.is_err());
    }

    #[test]
    fn overwrite_existing_files_keeps_existing_directory_contents() {
        let dir = tempdir().unwrap();
        let output_dir = dir.path().join("output");
        fs::create_dir_all(&output_dir).unwrap();
        let existing_file = output_dir.join("existing.txt");
        fs::write(&existing_file, "keep").unwrap();

        prepare_output_directory(&output_dir, OverwriteFiles::OverwriteExistingFiles).unwrap();

        assert!(output_dir.exists());
        assert!(existing_file.exists());
    }
}
