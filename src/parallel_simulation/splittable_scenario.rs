use std::sync::mpsc;

use crate::container::network::IONetwork;
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

#[derive(Debug)]
struct PartNode {
    weight: i32,
    out_links: Vec<usize>,
}

#[derive(Debug)]
struct PartLink {
    weight: i32,
    to: usize,
}

impl Scenario {
    pub fn from_io(
        io_network: &IONetwork,
        io_population: &IOPopulation,
        num_parts: usize,
    ) -> Scenario {
        let id_mappings = MatsimIdMappings::from_io(io_network, io_population);
        let partition_info =
            PartitionInfo::from_io(io_network, io_population, &id_mappings, num_parts);

        let network = Network::from_io(
            io_network,
            num_parts,
            |node| partition_info.get_partition(node),
            &id_mappings,
        );
        let mut populations =
            Population::split_from_container(io_population, num_parts, &id_mappings, &network);

        let mut customs_collection = Vec::new();
        let mut senders = Vec::new();

        let mut scenario = Scenario {
            scenarios: Vec::new(),
            id_mappings,
        };

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
}

#[cfg(test)]
mod test {
    use crate::container::network::IONetwork;
    use crate::container::population::IOPopulation;
    use crate::parallel_simulation::splittable_scenario::Scenario;

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
}
