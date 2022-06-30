use crate::container::network::{IONetwork, IONode};
use crate::container::population::IOPopulation;
use crate::parallel_simulation::splittable_network::Network;
use crate::parallel_simulation::splittable_population::Population;
use crate::parallel_simulation::vehicles::VehiclesIdMapping;

pub struct Scenario {
    pub network: Network,
    pub population: Population,
}

impl Scenario {
    pub fn from_io(
        network_container: &IONetwork,
        population_container: &IOPopulation,
    ) -> Vec<Scenario> {
        let vehicle_id_mapping = VehiclesIdMapping::from_population(&population_container);

        let (networks, _, link_id_mapping) =
            Network::split_from_container(network_container, 2, Scenario::split);
        let (populations, _) = Population::split_from_container(
            population_container,
            2,
            &link_id_mapping,
            &vehicle_id_mapping,
        );

        let scenarios: Vec<Scenario> = networks
            .into_iter()
            .zip(populations.into_iter())
            .map(|tuple| Scenario {
                network: tuple.0,
                population: tuple.1,
            })
            .collect();

        scenarios
    }

    fn split(node: &IONode) -> usize {
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

        let scenarios = Scenario::from_io(&io_network, &io_population);

        assert_eq!(2, scenarios.len());
        assert_eq!(
            io_network.nodes().len(),
            scenarios.iter().map(|s| s.network.nodes.len()).sum()
        );
        // can't sum up links because split links are present in both networks.
        assert_eq!(
            io_population.persons.len(),
            scenarios.iter().map(|s| s.population.agents.len()).sum()
        );

        // test the split scenarios for the particular split algorithm we have so far.
        let scenario1 = scenarios.get(0).unwrap();
        assert_eq!(scenario1.network.nodes.len(), 12);
        assert_eq!(scenario1.network.links.len(), 21);
        assert_eq!(scenario1.population.agents.len(), 100);

        let scenario2 = scenarios.get(1).unwrap();
        assert_eq!(scenario2.network.nodes.len(), 3);
        assert_eq!(scenario2.network.links.len(), 12);
        assert_eq!(scenario2.population.agents.len(), 0);
    }
}
