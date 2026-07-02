pub mod network;
pub mod population;
pub mod prepare_for_sim;
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

#[derive(Debug, Clone, PartialEq)]
pub struct Coordinate {
    pub x: f64,
    pub y: f64,
    pub z: Option<f64>,
}

impl Coordinate {
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y, z: None }
    }

    pub fn with_z(x: f64, y: f64, z: Option<f64>) -> Self {
        Self { x, y, z }
    }
}

impl Default for Coordinate {
    fn default() -> Self {
        Self::new(0.0, 0.0)
    }
}

/// The mod contains the full mod data.
/// `Network` and `Config` are wrapped by `Arc` since they are shared between threads.
#[derive(Debug)]
pub struct MutableScenario {
    pub network: Arc<Network>,
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
            id::load_from_file(&io::resolve_path(config.context(), path));
        }

        // mandatory content to create a mod
        let network = Arc::new(Self::load_network(&config));
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
/// `Network` and `Config` are wrapped by `Arc` since they are shared between threads.
/// TODO: This could hold a MutableScenario field and the network_partition.
#[derive(Debug)]
pub struct ScenarioPartition {
    pub network: Arc<Network>,
    pub garage: Garage,
    pub population: Population,
    pub network_partition: SimNetworkPartition,
    pub config: Arc<Config>,
}

impl ScenarioPartition {
    pub(crate) fn for_run(
        partition_num: u32,
        network: Arc<Network>,
        garage: Garage,
        config: Arc<Config>,
        population: Population,
    ) -> Self {
        let network_partition = Self::create_network_partition(&config, partition_num, &network);

        info!(
            "Partition #{partition_num} network has: {} nodes and {} links. Population has {} agents",
            network_partition.nodes.len(),
            network_partition.links.len(),
            population.persons.len()
        );
        Self {
            network,
            garage,
            population,
            network_partition,
            config,
        }
    }

    fn create_network_partition(
        config: &Config,
        rank: u32,
        network: &Network,
    ) -> SimNetworkPartition {
        let base_seed = config.computational_setup().random_seed;
        SimNetworkPartition::from_network(network, rank, config.simulation(), base_seed)
    }
}
