use crate::config::Config;
use crate::io::network::IONetwork;
use crate::io::population::IOPopulation;
use crate::mpi::message_broker::MpiMessageBroker;
use crate::mpi::messages::proto::Vehicle;
use crate::mpi::population::Population;
use crate::parallel_simulation::id_mapping::MatsimIdMappings;
use crate::parallel_simulation::network::partitioned_network::Network;
use crate::parallel_simulation::partition_info::PartitionInfo;
use log::info;
use mpi::topology::SystemCommunicator;
use mpi::traits::{Communicator, CommunicatorCollectives};

pub fn run(world: SystemCommunicator, config: Config) {
    let rank = world.rank();
    let size = world.size();

    let io_network = IONetwork::from_file(config.network_file.as_ref());
    let io_population = IOPopulation::from_file(config.population_file.as_ref());
    let id_mappings = MatsimIdMappings::from_io(&io_network, &io_population);
    let partition_info = PartitionInfo::from_io_network(&io_network, &id_mappings, size as usize);
    let network: Network<Vehicle> = Network::from_io(
        &io_network,
        size as usize,
        config.sample_size,
        |node| partition_info.get_partition(node),
        &id_mappings,
    );
    let network_partition = network.partitions.get(rank as usize).unwrap();

    let population = Population::from_io(&io_population, &id_mappings, rank as usize, &network);

    info!(
        "Partition #{rank} network has: {} nodes and {} links. Population has {} agents",
        network_partition.links.len(),
        network_partition.nodes.len(),
        population.agents.len()
    );

    let neighbors = network_partition.neighbors();
    let link_id_mapping = network.links_2_partition;
    let message_broker = MpiMessageBroker::new(world, rank, neighbors, link_id_mapping.clone());
    //Here we should initialize a simulation
    // Simulation::new(config, network, population, message_broker, events);

    info!("Process #{rank} at barrier.");
    world.barrier();
    info!("Process #{rank} finishing.");
}
