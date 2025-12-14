use crate::simulation::config::Config;
use crate::simulation::controller::{create_output_filename, insert_number_in_proto_filename};
use crate::simulation::network::sim_network::{SimNetworkPartition, SimNetworkPartitionBuilder};
use crate::simulation::network::Network;
use crate::simulation::population::Population;
use crate::simulation::vehicles::garage::Garage;
use crate::simulation::{id, io};
use std::sync::Arc;
use tracing::info;

/// The GlobalScenario contains the full scenario data.
#[derive(Debug)]
pub struct GlobalScenario {
    pub network: Network,
    pub garage: Garage,
    pub population: Population,
    // this is deliberately an Arc, as it is shared between all partitions and other threads. Otherwise, cloning would be needed.
    pub config: Arc<Config>,
}

impl GlobalScenario {
    pub fn load(config: Arc<Config>) -> Self {
        id::load_from_file(&io::resolve_path(config.context(), &config.ids().path));

        // mandatory content to create a scenario
        let network = Self::create_network(&config);
        let mut garage = Self::create_garage(&config);
        let population = Self::create_population(&config, &mut garage);

        GlobalScenario {
            network,
            garage,
            population,
            config,
        }
    }

    fn create_network(config: &Config) -> Network {
        let net_in_path = io::resolve_path(config.context(), &config.network().path);
        let num_parts = config.partitioning().num_parts;
        let network =
            Network::from_file_path(&net_in_path, num_parts, config.partitioning().method);

        let mut net_out_path = create_output_filename(
            &io::resolve_path(config.context(), &config.output().output_dir),
            &net_in_path,
        );
        net_out_path = insert_number_in_proto_filename(&net_out_path, num_parts);
        network.to_file(&net_out_path);
        network
    }

    fn create_garage(config: &Config) -> Garage {
        Garage::from_file(&io::resolve_path(config.context(), &config.vehicles().path))
    }

    fn create_population(config: &Config, garage: &mut Garage) -> Population {
        Population::from_file(
            &io::resolve_path(config.context(), &config.population().path),
            garage,
        )
    }
}

/// The ScenarioPartition contains the scenario data for a specific partition.
#[derive(Debug)]
pub struct ScenarioPartition {
    pub(crate) network: Network,
    pub(crate) garage: Garage,
    pub(crate) population: Population,
    pub(crate) network_partition: SimNetworkPartition,
    pub(crate) config: Arc<Config>,
}

/// This struct is needed as intermediate step to build a ScenarioPartition.
#[derive(Debug)]
pub struct ScenarioPartitionBuilder {
    network: Network,
    garage: Garage,
    population: Population,
    network_partition: SimNetworkPartitionBuilder,
    pub(crate) config: Arc<Config>,
}

impl ScenarioPartitionBuilder {
    pub(crate) fn from(mut value: GlobalScenario) -> Vec<Self> {
        let mut partitions = Vec::new();
        for i in 0..value.config.partitioning().num_parts {
            let partition = Self::create_partition(i, &mut value);
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

    fn create_partition(partition_num: u32, global_scenario: &mut GlobalScenario) -> Self {
        let network_partition = Self::create_network_partition(
            &global_scenario.config,
            partition_num,
            &global_scenario.network,
            &global_scenario.population,
        );

        let population = global_scenario
            .population
            .take_from_filtered_part(&global_scenario.network, partition_num);

        Self {
            network: global_scenario.network.clone(),
            garage: global_scenario.garage.clone(),
            population,
            network_partition,
            config: global_scenario.config.clone(),
        }
    }

    fn create_network_partition(
        config: &Config,
        rank: u32,
        network: &Network,
        population: &Population,
    ) -> SimNetworkPartitionBuilder {
        let partition =
            SimNetworkPartitionBuilder::from_network(network, rank, config.simulation());
        info!(
            "Partition #{rank} network has: {} nodes and {} links. Population has {} agents",
            partition.nodes.len(),
            partition.links.len(),
            population.persons.len()
        );
        partition
    }
}
