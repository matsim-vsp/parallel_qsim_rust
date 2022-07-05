use crate::container::network::{IONetwork, IONode};
use crate::container::population::IOPopulation;
use crate::parallel_simulation::customs::Customs;
use crate::parallel_simulation::id_mapping::IdMapping;
use crate::parallel_simulation::splittable_network::Network;
use crate::parallel_simulation::splittable_population::Population;
use crate::parallel_simulation::vehicles::VehiclesIdMapping;
use std::sync::{mpsc, Arc};

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
    pub id: usize,
}

impl Scenario {
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

        for _ in 0..size {
            let (sender, receiver) = mpsc::channel();
            let customs = Customs::new(receiver, scenario.link_id_mapping.clone());
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
                    id: i,
                }
            })
            .collect();

        scenario
    }

    pub fn split(node: &IONode) -> usize {
        if node.x < 0. {
            0
        } else {
            1
        }
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

        let scenario = Scenario::from_io(&io_network, &io_population, 2, Scenario::split);

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
