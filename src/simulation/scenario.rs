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

        // mandatory content to create a scenario
        let network = Self::create_network(config, config_path, output_path);
        let mut garage = Self::create_garage(config, config_path);
        let mut population =
            Self::create_population(config, config_path, &network, &mut garage, rank);
        let network_partition = Self::create_network_partition(config, rank, &network, &population);

        // optional stuff
        Self::add_drt(
            config,
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

    fn add_drt(
        config: &Config,
        garage: &mut Garage,
        population: &mut Population,
        network: &Network,
        network_partition: &SimNetworkPartition,
    ) {
        if config.drt().is_none() {
            return;
        }

        Self::add_drt_ids();
        Self::add_drt_driver(config, garage, population, network, network_partition);
    }

    fn add_drt_ids() {
        info!("Creating DRT ids.");

        //activity types
        Id::<String>::create("BeforeVrpSchedule");
        Id::<String>::create("DrtStay");
        Id::<String>::create("DrtBusStop");

        //task types
        Id::<String>::create("DRIVE");
        Id::<String>::create("STOP");
        Id::<String>::create("STAY");
    }

    fn add_drt_driver(
        config: &Config,
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
            let local_drt_vehicles = garage
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
                    let link = v
                        .attributes
                        .get("startLink")
                        .expect("No start link for drt vehicle provided.")
                        .as_string();
                    let link_id = Id::<Link>::get_from_ext(link.as_str()).internal();
                    (link_id, v)
                })
                .filter(|(l, _)| network_partition.get_link_ids().contains(l))
                .collect::<Vec<(u64, &Vehicle)>>();

            //for each drt vehicle, create a driver agent
            for (link, vehicle) in local_drt_vehicles {
                let start = vehicle
                    .attributes
                    .get("serviceBeginTime")
                    .expect("No service begin time for drt vehicle provided.")
                    .as_double() as u32;

                let veh_id = Id::<Vehicle>::get(vehicle.id);
                let person_id = Id::<Person>::create(veh_id.external());
                let from = network_partition.links.get(&link).unwrap().from();
                let x = network.get_node(from).x;
                let y = network.get_node(from).y;

                let mut plan = Plan::new();
                //TODO is Some(start) as end time correct?
                plan.add_act(Activity::new(x, y, 0, link, Some(0), Some(start), None));
                let person_id_internal = person_id.internal();
                population
                    .persons
                    .insert(person_id, Person::new(person_id_internal, plan));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::simulation::config::{CommandLineArgs, Config};
    use crate::simulation::id::Id;
    use crate::simulation::scenario::Scenario;
    use crate::simulation::wire_types::messages::Vehicle;
    use crate::simulation::wire_types::population::Person;
    use std::path::PathBuf;

    #[test]
    fn test() {
        let config_path = "./assets/drt/config.yml";
        let config = Config::from_file(&CommandLineArgs {
            config_path: String::from(config_path),
            num_parts: None,
        });

        let output_path = PathBuf::from(config.output().output_dir);

        let scenario = Scenario::build(&config, &String::from(config_path), 0, &output_path);

        assert_eq!(scenario.network.nodes.len(), 62);
        assert_eq!(scenario.network.links.len(), 170);

        // 10 agents, 1 drt agent
        assert_eq!(scenario.population.persons.len(), 10 + 1);

        // 10 agent vehicles, 1 drt vehicle
        assert_eq!(scenario.garage.vehicles.len(), 10 + 1);

        //there is only one predefined vehicle type (car)
        assert_eq!(scenario.garage.vehicle_types.len(), 1);

        let person_ids = scenario
            .population
            .persons
            .keys()
            .collect::<Vec<&Id<Person>>>();

        let vehicle_ids = scenario
            .garage
            .vehicles
            .keys()
            .collect::<Vec<&Id<Vehicle>>>();

        for n in 0..10u64 {
            assert!(person_ids.contains(&&Id::get_from_ext(format!("passenger{}", n).as_str())));
            assert!(
                vehicle_ids.contains(&&Id::get_from_ext(format!("passenger{}_car", n).as_str()))
            );
        }

        assert!(person_ids.contains(&&Id::get_from_ext("drt")));
        assert!(vehicle_ids.contains(&&Id::get_from_ext("drt")));
    }
}
