use crate::simulation::config::{Config, PartitionMethod};
use crate::simulation::controller::get_numbered_output_filename;
use crate::simulation::id::Id;
use crate::simulation::network::global_network::{Link, Network};
use crate::simulation::network::sim_network::SimNetworkPartition;
use crate::simulation::population::population::Population;
use crate::simulation::vehicles::garage::Garage;
use crate::simulation::wire_types::messages::Vehicle;
use crate::simulation::wire_types::population::{Activity, Person, Plan};
use crate::simulation::{id, io};
use std::path::PathBuf;
use tracing::info;

pub struct Scenario {
    pub network: Network,
    pub garage: Garage,
    pub population: Population,
    pub network_partition: SimNetworkPartition,
}

impl Scenario {
    pub fn build(config: &Config, config_path: &String, rank: u32, output_path: &PathBuf) -> Self {
        id::load_from_file(&io::resolve_path(config_path, &config.proto_files().ids));

        let network = Self::create_network(config, config_path, output_path);
        let mut garage = Self::create_garage(config, config_path);
        let mut population =
            Self::create_population(config, config_path, &network, &mut garage, rank);
        let network_partition = Self::create_network_partition(config, rank, &network, &population);
        Self::create_drt_driver(
            config,
            rank,
            &mut garage,
            &mut population,
            &network,
            &network_partition,
        );

        Scenario {
            network,
            garage,
            population,
            network_partition,
        }
    }

    fn create_network(config: &Config, config_path: &String, output_path: &PathBuf) -> Network {
        // if we partition the network is copied to the output folder.
        // otherwise nothing is done and we can load the network from the input folder directly.
        let network_path = if let PartitionMethod::Metis(_) = config.partitioning().method {
            get_numbered_output_filename(
                &output_path,
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
            &network,
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
        let partition = SimNetworkPartition::from_network(&network, rank, config.simulation());
        info!(
            "Partition #{rank} network has: {} nodes and {} links. Population has {} agents",
            partition.nodes.len(),
            partition.links.len(),
            population.persons.len()
        );
        partition
    }

    fn create_drt_driver(
        config: &Config,
        rank: u32,
        garage: &mut Garage,
        population: &mut Population,
        network: &Network,
        network_partition: &SimNetworkPartition,
    ) {
        if let Some(drt) = &config.drt() {
            info!("Creating DRT drivers");

            let drt_modes = drt
                .services
                .iter()
                .map(|s| s.mode.clone())
                .collect::<Vec<String>>();

            //fetch all drt vehicles starting on this partition
            let drt_vehicles = garage
                .vehicles
                .values()
                .filter(|&v| {
                    if let Some(value) = v.attributes.get("dvrpMode") {
                        drt_modes.contains(&value.as_string())
                    } else {
                        false
                    }
                })
                .map(|v| {
                    let link = v.attributes.get("startLink").unwrap().as_string();
                    let link_id = Id::<Link>::get_from_ext(link.as_str()).internal();
                    (link_id, v)
                })
                .filter(|(l, _)| network_partition.get_link_ids().contains(l))
                .collect::<Vec<(u64, &Vehicle)>>();

            //for each drt vehicle, create a driver agent
            for (link, vehicle) in drt_vehicles {
                let start = vehicle.attributes.get("serviceBeginTime").unwrap().as_int() as u32;

                let veh_id = Id::<Vehicle>::get(vehicle.id);
                let person_id = Id::<Person>::create(veh_id.external()).internal();
                let from = network_partition.links.get(&link).unwrap().from();
                let x = network.get_node(from).x;
                let y = network.get_node(from).y;

                let mut plan = Plan::new();
                //TODO is Some(start) as end time correct?
                plan.add_act(Activity::new(x, y, 0, link, Some(0), Some(start), None));
                Person::new(person_id, plan);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test() {}
}
