use crate::simulation::config::Config;
use crate::simulation::network::sim_network::{SimNetworkPartition, SimNetworkPartitionBuilder};
use crate::simulation::network::Network;
use crate::simulation::population::Population;
use crate::simulation::vehicles::garage::Garage;
use crate::simulation::{id, io};
use std::sync::Arc;
use tracing::info;

/// This enum works as state holder enum for the scenario's population. Either, the scenario is owner
/// of the Population (e.g. at startup and end) or the population is split among the threads.
#[derive(Debug)]
#[allow(dead_code)]
pub enum GlobalPopulation {
    Full(Population),
    Partitioned,
}

/// The scenario contains the full scenario data.
#[derive(Debug)]
pub struct Scenario {
    pub network: Arc<Network>,
    pub garage: Arc<Garage>,
    pub population: GlobalPopulation,
    // this is deliberately an Arc, as it is shared between all partitions and other threads. Otherwise, cloning would be needed.
    pub config: Arc<Config>,
}

impl Scenario {
    pub fn load(config: Arc<Config>) -> Self {
        info!("Start loading scenario.");

        if let Some(ids) = config.ids() {
            info!("Loading IDs from {:?}", ids.path);
            id::load_from_file(&io::resolve_path(config.context(), &ids.path));
        }

        // mandatory content to create a scenario
        let network = Self::load_network(&config);
        let mut garage = Self::load_garage(&config);
        let population = Self::load_population(&config, &mut garage);

        Scenario {
            network: Arc::new(network),
            garage: Arc::new(garage),
            population: GlobalPopulation::Full(population),
            config,
        }
    }

    fn load_network(config: &Config) -> Network {
        let net_in_path = io::resolve_path(config.context(), &config.network().path);
        let num_parts = config.partitioning().num_parts;
        Network::from_file_path(&net_in_path, num_parts, &config.partitioning().method)
    }

    fn load_garage(config: &Config) -> Garage {
        Garage::from_file(&io::resolve_path(config.context(), &config.vehicles().path))
    }

    fn load_population(config: &Config, garage: &mut Garage) -> Population {
        Population::from_file(
            &io::resolve_path(config.context(), &config.population().path),
            garage,
        )
    }
}

/// The ScenarioPartition contains the scenario data for a specific partition.
#[derive(Debug)]
pub struct ScenarioPartition {
    pub(crate) network: Arc<Network>,
    pub(crate) garage: Garage,
    pub(crate) population: Population,
    pub(crate) network_partition: SimNetworkPartition,
    pub(crate) config: Arc<Config>,
}

/// This struct is needed as intermediate step to build a ScenarioPartition.
#[derive(Debug)]
pub struct ScenarioPartitionBuilder {
    network: Arc<Network>,
    garage: Garage,
    population: Population,
    network_partition: SimNetworkPartitionBuilder,
    pub(crate) config: Arc<Config>,
}

impl ScenarioPartitionBuilder {
    pub(crate) fn from(scenario: &mut Scenario) -> Vec<Self> {
        let mut partitions = Vec::new();
        for i in 0..scenario.config.partitioning().num_parts {
            let partition = Self::create_partition(i, scenario);
            partitions.push(partition);
        }
        partitions
    }

    pub fn build(self) -> ScenarioPartition {
        ScenarioPartition {
            network: self.network,
            garage: self.garage,
            population: self.population,
            network_partition: self.network_partition.build(),
            config: self.config,
        }
    }

    fn create_partition(partition_num: u32, scenario: &mut Scenario) -> Self {
        let global_pop = match &mut scenario.population {
            GlobalPopulation::Full(p) => p,
            GlobalPopulation::Partitioned => {
                panic!("Tried to create a partition after the population was already split among the partitions. This is not allowed.")
            }
        };

        let network_partition = Self::create_network_partition(
            &scenario.config,
            partition_num,
            &scenario.network,
            global_pop,
        );

        let population = global_pop.take_from_filtered_part(&scenario.network, partition_num);

        Self {
            network: scenario.network.clone(),
            // this not very nice, but for now we are very liberal about when, where and how often agents can access their vehicles.
            // Also, we just have an `unpark` method, no counterpart for adding vehicles. paul, feb '26
            garage: (*scenario.garage).clone(),
            population,
            network_partition,
            config: scenario.config.clone(),
        }
    }

    fn create_network_partition(
        config: &Config,
        rank: u32,
        network: &Network,
        population: &Population,
    ) -> SimNetworkPartitionBuilder {
        let base_seed = config.computational_setup().random_seed;
        let partition =
            SimNetworkPartitionBuilder::from_network(network, rank, config.simulation(), base_seed);
        info!(
            "Partition #{rank} network has: {} nodes and {} links. Population has {} agents",
            partition.nodes.len(),
            partition.links.len(),
            population.persons.len()
        );
        partition
    }
}
