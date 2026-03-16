pub mod network;
pub mod population;
pub mod trip_structure_utils;
pub mod vehicles;

use crate::simulation::config::Config;
use crate::simulation::network::sim_network::SimNetworkPartition;
use crate::simulation::{id, io};
use network::Network;
use population::Population;
use std::sync::Arc;
use tracing::info;
use vehicles::Garage;

/// The mod contains the full mod data.
#[derive(Debug)]
pub struct MutableScenario {
    pub network: Network,
    pub garage: Garage,
    pub population: Population,
    pub config: Arc<Config>,
}

impl MutableScenario {
    pub fn load<C: Into<Arc<Config>>>(config: C) -> Self {
        info!("Start loading mod.");

        let config = config.into();

        if let Some(path) = &config.ids().path {
            info!("Loading IDs from {:?}", path);
            id::load_from_file(&io::resolve_path(config.context(), &path));
        }

        // mandatory content to create a mod
        let network = Self::load_network(&config);
        let mut garage = Self::load_garage(&config);
        let population = Self::load_population(&config, &mut garage);

        MutableScenario {
            network,
            garage,
            population,
            config,
        }
    }

    fn load_network(config: &Config) -> Network {
        if let Some(path) = &config.network().path {
            let net_in_path = io::resolve_path(config.context(), path);
            let num_parts = config.partitioning().num_parts;
            Network::from_file_path(&net_in_path, num_parts, &config.partitioning().method)
        } else {
            Network::default()
        }
    }

    fn load_garage(config: &Config) -> Garage {
        if let Some(path) = &config.vehicles().path {
            let garage_in_path = io::resolve_path(config.context(), path);
            Garage::from_file(&garage_in_path)
        } else {
            Garage::default()
        }
    }

    fn load_population(config: &Config, garage: &mut Garage) -> Population {
        if let Some(path) = &config.population().path {
            let pop_in_path = io::resolve_path(config.context(), path);
            Population::from_file(&pop_in_path, garage)
        } else {
            Population::default()
        }
    }
}

/// The ScenarioPartition contains the mod data for a specific partition.
#[derive(Debug)]
pub struct ScenarioPartition {
    pub(crate) network: Arc<Network>,
    pub(crate) garage: Garage,
    pub(crate) population: Population,
    pub(crate) network_partition: SimNetworkPartition,
    pub(crate) config: Arc<Config>,
}

impl ScenarioPartition {
    pub(crate) fn from(mut scenario: MutableScenario) -> Vec<Self> {
        let network = Arc::new(scenario.network);

        let mut partitions = Vec::new();
        for i in 0..scenario.config.partitioning().num_parts {
            let partition = Self::create_partition(
                i,
                &mut scenario.population,
                network.clone(),
                // this not very nice, since this is a full clone.
                // but for now we are very liberal about when, where and how often agents can access their vehicles.
                // Also, we just have an `unpark` method, no counterpart for adding vehicles. paul, feb '26
                scenario.garage.clone(),
                scenario.config.clone(),
            );
            partitions.push(partition);
        }
        partitions
    }

    fn create_partition(
        partition_num: u32,
        population: &mut Population,
        network: Arc<Network>,
        garage: Garage,
        config: Arc<Config>,
    ) -> Self {
        let network_partition =
            Self::create_network_partition(&config, partition_num, &network, &population);

        let population = population.take_from_filtered_part(&network, partition_num);

        Self {
            network: network.clone(),
            garage,
            population,
            network_partition,
            config: config.clone(),
        }
    }

    fn create_network_partition(
        config: &Config,
        rank: u32,
        network: &Network,
        population: &Population,
    ) -> SimNetworkPartition {
        let base_seed = config.computational_setup().random_seed;
        let partition =
            SimNetworkPartition::from_network(network, rank, config.simulation(), base_seed);
        info!(
            "Partition #{rank} network has: {} nodes and {} links. Population has {} agents",
            partition.nodes.len(),
            partition.links.len(),
            population.persons.len()
        );
        partition
    }
}
