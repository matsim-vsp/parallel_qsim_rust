use log::info;
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{mpsc, Arc};

use crate::io::network::{Attr, Attrs, IOLink, IONetwork, IONode};
use crate::io::population::IOPopulation;
use crate::parallel_simulation::id_mapping::MatsimIdMappings;
use crate::parallel_simulation::messages::Message;
use crate::parallel_simulation::messaging::MessageBroker;
use crate::parallel_simulation::network::network_partition::NetworkPartition;
use crate::parallel_simulation::network::partitioned_network::Network;
use crate::parallel_simulation::partition_info::PartitionInfo;
use crate::parallel_simulation::splittable_population::Population;

#[derive(Debug)]
pub struct Scenario<V: Debug> {
    pub scenarios: Vec<ScenarioPartition<V>>,

    // the properties below are for bookkeeping of ids
    pub id_mappings: MatsimIdMappings,
    link_2_thread: Arc<HashMap<usize, usize>>,
    node_2_thread: Arc<HashMap<usize, usize>>,
}

#[derive(Debug)]
pub struct ScenarioPartition<V: Debug> {
    pub network: NetworkPartition<V>,
    pub population: Population,
    pub msg_broker: MessageBroker,
}

impl<V: Debug> Scenario<V> {
    pub fn from_io(
        io_network: &IONetwork,
        io_population: &IOPopulation,
        num_parts: usize,
        sample_size: f32,
    ) -> Self {
        info!("SplittableScenario: creating Id mappings");
        let id_mappings = MatsimIdMappings::from_io(io_network, io_population);

        info!("SplittableScenario: creating partition information");
        let partition_info =
            PartitionInfo::from_io(io_network, io_population, &id_mappings, num_parts);

        info!("SplittableScenario: creating partitioned network");
        let network = Network::from_io(
            io_network,
            num_parts,
            sample_size,
            |node| partition_info.get_partition(node),
            &id_mappings,
        );

        info!("SplittableScenario: creating partitioned population");
        let mut populations =
            Population::split_from_container(io_population, num_parts, &id_mappings, &network);

        let mut scenario = Scenario {
            scenarios: Vec::new(),
            id_mappings,
            link_2_thread: network.links_2_partition.clone(),
            node_2_thread: network.nodes_2_partition.clone(),
        };

        let mut receivers: Vec<Receiver<Message>> = Vec::new();
        let mut senders: Vec<Sender<Message>> = Vec::new();
        let mut message_brokers = Vec::new();

        info!("SplittableScenario: creating channels for inter thread communication");
        for _ in 0..num_parts {
            let (sender, receiver) = mpsc::channel();
            receivers.push(receiver);
            senders.push(sender);
        }

        for (i, receiver) in receivers.into_iter().enumerate() {
            let network_partition = network.partitions.get(i).unwrap();
            let neighbors = network_partition.neighbors();
            let mut neighbor_senders = HashMap::new();
            let mut remote_senders = HashMap::new();

            for (i_sender, sender) in senders.iter().enumerate() {
                if neighbors.contains(&i_sender) {
                    neighbor_senders.insert(i_sender, sender.clone());
                } else if i_sender != i {
                    remote_senders.insert(i_sender, sender.clone());
                }
            }
            let broker = MessageBroker::new(
                i,
                receiver,
                neighbor_senders,
                remote_senders,
                network.links_2_partition.clone(),
            );
            message_brokers.push(broker);
        }

        info!("Creating scenario partitions.");
        scenario.scenarios = network
            .partitions
            .into_iter()
            // use reverse, because removing from vec at the end avoids shifting
            .enumerate()
            .rev()
            .map(|(i, network_partition)| {
                let population = populations.remove(i);
                let customs = message_brokers.remove(i);
                ScenarioPartition {
                    network: network_partition,
                    population,
                    msg_broker: customs,
                }
            })
            .collect();

        scenario
    }

    pub fn as_network(&self, original_io_network: &IONetwork) -> IONetwork {
        let mut result = IONetwork::new(None);

        for node in original_io_network.nodes() {
            let internal_id = self
                .id_mappings
                .nodes
                .get_internal(node.id.as_ref())
                .unwrap();
            let partition = self.node_2_thread.get(internal_id).unwrap();
            let attributes = Scenario::<V>::create_partition_attr(*partition);
            let new_node = IONode {
                id: internal_id.to_string(),
                x: node.x,
                y: node.y,
                attributes,
            };
            result.nodes_mut().push(new_node);
        }

        for link in original_io_network.links() {
            let internal_id = self
                .id_mappings
                .links
                .get_internal(link.id.as_ref())
                .unwrap();
            let internal_from = *self
                .id_mappings
                .nodes
                .get_internal(link.from.as_ref())
                .unwrap();
            let internal_to = *self
                .id_mappings
                .nodes
                .get_internal(link.to.as_ref())
                .unwrap();
            let partition = self.link_2_thread.get(internal_id).unwrap();
            let attributes = Scenario::<V>::create_partition_attr(*partition);
            let new_link = IOLink {
                id: internal_id.to_string(),
                attributes,
                from: internal_from.to_string(),
                to: internal_to.to_string(),
                freespeed: link.freespeed,
                capacity: link.capacity,
                length: link.length,
                permlanes: link.permlanes,
            };
            result.links_mut().push(new_link);
        }

        result
    }

    fn create_partition_attr(partition: usize) -> Option<Attrs> {
        let attrs = Attrs {
            attributes: vec![Attr {
                name: String::from("partition"),
                value: partition.to_string(),
                class: String::from("java.lang.Integer"),
            }],
        };
        Some(attrs)
    }
}

#[cfg(test)]
mod test {
    use crate::io::network::IONetwork;
    use crate::io::population::IOPopulation;
    use crate::parallel_simulation::splittable_scenario::Scenario;
    use crate::parallel_simulation::vehicles::Vehicle;
    use std::path::Path;

    #[test]
    fn create_3_links_scenario() {
        let mut io_network = IONetwork::from_file("./assets/3-links/3-links-network.xml");
        let io_population = IOPopulation::from_file("./assets/3-links/1-agent.xml");
        let num_parts = 2;
        let output_folder = Path::new(
            "./test_output/parallel_simulation/splittable_scenario/create_3_links_scenario/",
        );
        let scenario: Scenario<Vehicle> =
            Scenario::from_io(&mut io_network, &io_population, num_parts, 1.);

        let out_network = scenario.as_network(&io_network);
        let network_file = output_folder.join("output_network.xml.gz");
        out_network.to_file(&network_file);

        println!("Done");
    }

    #[test]
    fn create_equil_scenario() {
        let mut io_network = IONetwork::from_file("./assets/equil/equil-network.xml");
        let io_population = IOPopulation::from_file("./assets/equil/equil-plans.xml.gz");
        let num_parts = 2;
        let output_folder = Path::new(
            "./test_output/parallel_simulation/splittable_scenario/create_equil_scenario",
        );
        let scenario: Scenario<Vehicle> =
            Scenario::from_io(&mut io_network, &io_population, num_parts, 1.);

        let out_network = scenario.as_network(&io_network);
        let network_file = output_folder.join("output_network.xml.gz");
        out_network.to_file(&network_file);

        println!("Done");
    }

    #[test]
    #[ignore]
    fn create_berlin_scenario() {
        let mut io_network = IONetwork::from_file(
            "/home/janek/test-files/berlin-v5.5.3-1pct.no-pt-output_network.xml.gz",
        );
        let io_population = IOPopulation::from_file(
            "/home/janek/test-files/berlin-v5.5.3.selected-no-pt_output_plans.xml.gz",
        );
        let num_parts = 12;
        let output_folder = Path::new(
            "./test_output/parallel_simulation/splittable_scenario/create_berlin_scenario/",
        );

        let scenario: Scenario<Vehicle> =
            Scenario::from_io(&mut io_network, &io_population, num_parts, 1.);

        println!("Create Berlin Scenario Test: Finished creating scenario. Writing network.");
        let out_network = scenario.as_network(&io_network);
        let network_file = output_folder.join("output_12_network.xml.gz");
        out_network.to_file(&network_file);

        println!("Done");
    }
}
