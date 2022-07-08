use std::collections::HashMap;
use std::sync::{mpsc, Arc};

use metis::Graph;

use crate::container::network::{IONetwork, IONode};
use crate::container::population::IOPopulation;
use crate::parallel_simulation::customs::Customs;
use crate::parallel_simulation::id_mapping::IdMapping;
use crate::parallel_simulation::splittable_network::Network;
use crate::parallel_simulation::splittable_population::Population;
use crate::parallel_simulation::vehicles::VehiclesIdMapping;

pub struct Scenario {
    pub scenarios: Vec<ScenarioSlice>,

    // the properties below are for bookkeeping of ids
    link_id_mapping: Arc<IdMapping>,
    node_id_mapping: Arc<IdMapping>,
    agent_id_mapping: Arc<IdMapping>,
    vehicle_id_mapping: Arc<VehiclesIdMapping>,
}

pub struct ScenarioSlice {
    pub network: Network,
    pub population: Population,
    pub customs: Customs,
}

struct MatsimIdMapping<'a> {
    matsim_2_internal: HashMap<&'a str, usize>,
    internal_2_matsim: HashMap<usize, String>,
}

impl<'a> MatsimIdMapping<'a> {
    fn new() -> MatsimIdMapping<'a> {
        MatsimIdMapping {
            matsim_2_internal: HashMap::new(),
            internal_2_matsim: HashMap::new(),
        }
    }
}

struct PartNode {
    weight: i32,
    out_links: Vec<usize>,
}

struct PartLink {
    weight: i32,
    to: usize,
}

impl Scenario {
    fn map_node_ids<'a>(network_container: &IONetwork) -> MatsimIdMapping<'a> {
        let mut mapping = MatsimIdMapping::new();

        for (i, node) in network_container.nodes().iter().enumerate() {
            mapping.internal_2_matsim.insert(i, node.id.clone());
            if let Some(matsim_id) = mapping.internal_2_matsim.get(&i) {
                mapping.matsim_2_internal.insert(matsim_id.as_str(), i);
            }
        }

        mapping
    }

    fn map_link_ids<'a>(network_container: &IONetwork) -> MatsimIdMapping<'a> {
        let mut mapping = MatsimIdMapping::new();

        for (i, link) in network_container.links().iter().enumerate() {
            mapping.internal_2_matsim.insert(i, link.id.clone());
            if let Some(matsim_id) = mapping.internal_2_matsim.get(&i) {
                mapping.matsim_2_internal.insert(matsim_id.as_str(), i);
            }
        }

        mapping
    }

    fn map_person_ids<'a>(population_container: &IOPopulation) -> MatsimIdMapping<'a> {
        let mut mapping = MatsimIdMapping::new();

        for (i, person) in population_container.persons.iter().enumerate() {
            mapping.internal_2_matsim.insert(i, person.id.clone());
            if let Some(matsim_id) = mapping.internal_2_matsim.get(&i) {
                mapping.matsim_2_internal.insert(matsim_id.as_str(), i);
            }
        }

        mapping
    }

    fn partition_containers(
        network_container: &IONetwork,
        population_container: &IOPopulation,
        num_parts: i32,
    ) -> Vec<i32> {
        let node_id_mapping = Self::map_node_ids(network_container);
        let link_id_mapping = Self::map_link_ids(network_container);
        let agent_id_mapping = Self::map_person_ids(population_container);

        let mut nodes: Vec<_> = network_container
            .nodes()
            .iter()
            .map(|node| PartNode {
                weight: 0,
                out_links: Vec::new(),
            })
            .collect();

        let links: Vec<_> = network_container
            .links()
            .iter()
            .map(|link| {
                let link_id = link_id_mapping
                    .matsim_2_internal
                    .get(link.id.as_str())
                    .unwrap();
                let to_node_id = node_id_mapping
                    .matsim_2_internal
                    .get(link.to.as_str())
                    .unwrap();

                // put link into out links list of from node
                let from_node_id = node_id_mapping
                    .matsim_2_internal
                    .get(link.from.as_str())
                    .unwrap();
                let from_node = nodes.get_mut(*from_node_id).unwrap();
                from_node.out_links.push(*link_id);

                PartLink {
                    to: *to_node_id,
                    weight: 0,
                }
            })
            .collect();

        let result = Self::partition(nodes, links, num_parts);
        result
    }

    fn partition(nodes: Vec<PartNode>, links: Vec<PartLink>, num_parts: i32) -> Vec<i32> {
        let mut xadj: Vec<i32> = Vec::from([0]);
        let mut adjncy: Vec<i32> = Vec::new();
        let mut adjwgt: Vec<i32> = Vec::new();
        let mut vwgt: Vec<i32> = Vec::new();
        let mut result = vec![0x00; nodes.len()];

        for node in nodes {
            let num_out_links = node.out_links.len() as i32;
            let next_adjacency_index = xadj.last().unwrap() + num_out_links;
            xadj.push(next_adjacency_index as i32);
            vwgt.push(node.weight);

            for id in node.out_links {
                let link = links.get(id).unwrap();
                adjncy.push(link.to as i32);
                adjwgt.push(link.weight);
            }
        }

        Graph::new(1, num_parts, &mut xadj, &mut adjncy)
            .set_adjwgt(&mut adjwgt)
            .set_vwgt(&mut vwgt)
            .part_kway(&mut result)
            .unwrap();

        result
    }

    pub fn from_io(
        network_container: &IONetwork,
        population_container: &IOPopulation,
        size: usize,
        split: fn(&IONode) -> usize,
    ) -> Scenario {
        let vehicle_id_mapping = VehiclesIdMapping::from_population(&population_container);

        let (networks, node_id_mapping, link_id_mapping) =
            Network::split_from_container(network_container, size, split);
        let (mut populations, agent_id_mapping) = Population::split_from_container(
            &population_container,
            size,
            &link_id_mapping,
            &vehicle_id_mapping,
        );

        let mut customs_collection = Vec::new();
        let mut senders = Vec::new();

        let mut scenario = Scenario {
            scenarios: Vec::new(),
            vehicle_id_mapping: Arc::new(vehicle_id_mapping),
            agent_id_mapping: Arc::new(agent_id_mapping),
            node_id_mapping: Arc::new(node_id_mapping),
            link_id_mapping: Arc::new(link_id_mapping),
        };

        for i in 0..size {
            let (sender, receiver) = mpsc::channel();
            let customs = Customs::new(i, receiver, scenario.link_id_mapping.clone());
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

        scenario.scenarios = networks
            .into_iter()
            // use reverse, because removing from vec at the end avoids shifting
            .enumerate()
            .rev()
            .map(|(i, network)| {
                let population = populations.remove(i);
                let customs = customs_collection.remove(i);
                ScenarioSlice {
                    network,
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

    #[test]
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
}
