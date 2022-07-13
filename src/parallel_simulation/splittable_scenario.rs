use std::path::Path;
use std::sync::mpsc;

use crate::container::network::{Attr, Attrs, IONetwork};
use crate::container::population::IOPopulation;
use crate::parallel_simulation::customs::Customs;
use crate::parallel_simulation::id_mapping::MatsimIdMappings;
use crate::parallel_simulation::partition_info::PartitionInfo;
use crate::parallel_simulation::splittable_network::{Network, NetworkPartition};
use crate::parallel_simulation::splittable_population::Population;

#[derive(Debug)]
pub struct Scenario {
    pub scenarios: Vec<ScenarioPartition>,

    // the properties below are for bookkeeping of ids
    id_mappings: MatsimIdMappings,
}

#[derive(Debug)]
pub struct ScenarioPartition {
    pub network: NetworkPartition,
    pub population: Population,
    pub customs: Customs,
}

impl Scenario {
    pub fn from_io(
        io_network: &mut IONetwork,
        io_population: &IOPopulation,
        num_parts: usize,
        output_folder: &Path,
    ) -> Scenario {
        println!("Splittable Scenario creating Id mappings");
        let id_mappings = MatsimIdMappings::from_io(io_network, io_population);

        println!("Splittable Scenario creating partition information");
        let partition_info =
            PartitionInfo::from_io(io_network, io_population, &id_mappings, num_parts);

        println!("Splittable Scenario adding partition information to io network.");
        Scenario::add_thread_attr(io_network, &partition_info);

        println!("Splittable Scenario creating partitioned network");
        let network = Network::from_io(
            io_network,
            num_parts,
            |node| partition_info.get_partition(node),
            &id_mappings,
        );

        println!("Splittable Scenario creating partitioned population");
        let mut populations =
            Population::split_from_container(io_population, num_parts, &id_mappings, &network);

        let mut customs_collection = Vec::new();
        let mut senders = Vec::new();

        let mut scenario = Scenario {
            scenarios: Vec::new(),
            id_mappings,
        };

        println!("Splittable Scenario creating channels for inter thread communication");
        for i in 0..num_parts {
            let (sender, receiver) = mpsc::channel();
            let customs = Customs::new(i, receiver, network.links_2_thread.clone());
            customs_collection.push(customs);
            senders.push(sender);
        }

        for (i_custom, customs) in customs_collection.iter_mut().enumerate() {
            for (i_sender, sender) in senders.iter().enumerate() {
                if i_custom != i_sender {
                    customs.add_sender(i_sender, sender.clone());
                }
            }
        }

        println!("Splittable Scenario creating scenario partitions.");
        scenario.scenarios = network
            .partitions
            .into_iter()
            // use reverse, because removing from vec at the end avoids shifting
            .enumerate()
            .rev()
            .map(|(i, network_partition)| {
                let population = populations.remove(i);
                let customs = customs_collection.remove(i);
                ScenarioPartition {
                    network: network_partition,
                    population,
                    customs,
                }
            })
            .collect();

        scenario
    }

    fn add_thread_attr(io_network: &mut IONetwork, partition_info: &PartitionInfo) {
        for node in io_network.nodes_mut() {
            let partition = partition_info.get_partition(node);
            let attrs = node.attributes.get_or_insert(Attrs {
                attributes: Vec::new(),
            });
            attrs.attributes.push(Attr {
                name: String::from("thread"),
                value: partition.to_string(),
                class: String::from("java.lang.String"),
            })
        }
    }
}

#[cfg(test)]
mod test {
    use crate::container::network::IONetwork;
    use crate::container::population::IOPopulation;
    use crate::parallel_simulation::splittable_scenario::Scenario;
    use std::path::Path;

    /*  #[test]
     fn create_scenarios() {
         let io_network = IONetwork::from_file("./assets/equil-network.xml");
         let io_population = IOPopulation::from_file("./assets/equil_output_plans.xml.gz");

         let scenario = Scenario::from_io(&io_network, &io_population, 2, |node| {
             if node.x < 0. {
                 0
             } else {
                 1
             }
         });

         assert_eq!(2, scenario.scenarios.len());
         assert_eq!(
             io_network.nodes().len(),
             scenario
                 .scenarios
                 .iter()
                 .map(|s| s.network.nodes.len())
                 .sum()
         );
         // can't sum up links because split links are present in both networks.
         assert_eq!(
             io_population.persons.len(),
             scenario
                 .scenarios
                 .iter()
                 .map(|s| s.population.agents.len())
                 .sum()
         );

         // test the split scenarios for the particular split algorithm we have so far.
         let scenario1 = scenario.scenarios.get(0).unwrap();
         assert_eq!(scenario1.network.nodes.len(), 3);
         assert_eq!(scenario1.network.links.len(), 12);
         assert_eq!(scenario1.population.agents.len(), 0);

         let scenario2 = scenario.scenarios.get(1).unwrap();
         assert_eq!(scenario2.network.nodes.len(), 12);
         assert_eq!(scenario2.network.links.len(), 21);
         assert_eq!(scenario2.population.agents.len(), 100);
     }

     #[test]
     fn partition_equil_scenario() {
         let io_network = IONetwork::from_file("./assets/equil-network.xml");
         let io_population = IOPopulation::from_file("./assets/equil_output_plans.xml.gz");

         let scenario = Scenario::partition_containers(&io_network, &io_population, 2);

         println!("{scenario:#?}")
     }

    */

    #[test]
    fn create_3_links_scenario() {
        let mut io_network = IONetwork::from_file("./assets/3-links/3-links-network.xml");
        let io_population = IOPopulation::from_file("./assets/3-links/1-agent.xml");
        let num_parts = 2;
        let output_folder = Path::new(
            "./test_output/parallel_simulation/splittable_scenario/create_3_links_scenario/",
        );
        let scenario = Scenario::from_io(&mut io_network, &io_population, num_parts, output_folder);

        let network_file = output_folder.join("output_network.xml.gz");
        io_network.to_file(&network_file);

        println!("Done");
    }

    #[test]
    fn create_equil_scenario() {
        let mut io_network = IONetwork::from_file("./assets/equil-network.xml");
        let io_population = IOPopulation::from_file("./assets/equil_output_plans.xml.gz");
        let num_parts = 2;
        let output_folder = Path::new(
            "./test_output/parallel_simulation/splittable_scenario/create_equil_scenario",
        );
        let scenario = Scenario::from_io(&mut io_network, &io_population, num_parts, output_folder);

        let network_file = output_folder.join("output_network.xml.gz");
        io_network.to_file(&network_file);

        println!("Done");
    }

    #[test]
    #[ignore]
    fn create_berlin_scenario() {
        let mut io_network =
            IONetwork::from_file("/home/janek/test-files/berlin-v5.5.3-1pct.output_network.xml.gz");
        let io_population = IOPopulation::from_file(
            "/home/janek/test-files/berlin-v5.5.3-1pct.selected_output_plans.xml.gz",
        );
        let num_parts = 4;
        let output_folder = Path::new(
            "./test_output/parallel_simulation/splittable_scenario/create_berlin_scenario/",
        );

        let scenario = Scenario::from_io(&mut io_network, &io_population, num_parts, output_folder);

        println!("Create Berlin Scenario Test: Finished creating scenario. Writing network.");
        let network_file = output_folder.join("output_network.xml.gz");
        io_network.to_file(&network_file);

        println!("Done");
    }
}
