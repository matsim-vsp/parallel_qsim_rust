use crate::simulation::config::{Config, PartitionMethod};
use crate::simulation::controller::get_numbered_output_filename;
use crate::simulation::network::sim_network::SimNetworkPartition;
use crate::simulation::network::Network;
use crate::simulation::population::Population;
use crate::simulation::vehicles::garage::Garage;
use crate::simulation::{id, io};
use std::path::Path;
use tracing::info;

pub struct Scenario {
    pub network: Network,
    pub garage: Garage,
    pub population: Population,
    pub network_partition: SimNetworkPartition,
}

impl Scenario {
    pub fn build(config: &Config, config_path: &String, rank: u32, output_path: &Path) -> Self {
        id::load_from_file(&io::resolve_path(config_path, &config.proto_files().ids));

        // mandatory content to create a scenario
        let network = Self::create_network(config, config_path, output_path);
        let mut garage = Self::create_garage(config, config_path);
        let population = Self::create_population(config, config_path, &network, &mut garage, rank);
        let network_partition = Self::create_network_partition(config, rank, &network, &population);

        Scenario {
            network,
            garage,
            population,
            network_partition,
        }
    }

    fn create_network(config: &Config, config_path: &String, output_path: &Path) -> Network {
        // if we partition the network is copied to the output folder.
        // otherwise nothing is done and we can load the network from the input folder directly.
        let network_path = if let PartitionMethod::Metis(_) = config.partitioning().method {
            get_numbered_output_filename(
                output_path,
                &io::resolve_path(config_path, &config.proto_files().network),
                config.partitioning().num_parts,
            )
        } else {
            crate::simulation::controller::insert_number_in_proto_filename(
                &io::resolve_path(config_path, &config.proto_files().network),
                config.partitioning().num_parts,
            )
        };
        Network::from_file_as_is(&network_path)
    }

    fn create_garage(config: &Config, config_path: &String) -> Garage {
        Garage::from_file(&io::resolve_path(
            config_path,
            &config.proto_files().vehicles,
        ))
    }

    fn create_population(
        config: &Config,
        config_path: &String,
        network: &Network,
        garage: &mut Garage,
        rank: u32,
    ) -> Population {
        Population::from_file_filtered_part(
            &io::resolve_path(config_path, &config.proto_files().population),
            network,
            garage,
            rank,
        )
    }

    fn create_network_partition(
        config: &Config,
        rank: u32,
        network: &Network,
        population: &Population,
    ) -> SimNetworkPartition {
        let partition = SimNetworkPartition::from_network(network, rank, config.simulation());
        info!(
            "Partition #{rank} network has: {} nodes and {} links. Population has {} agents",
            partition.nodes.len(),
            partition.links.len(),
            population.persons.len()
        );
        partition
    }
}
