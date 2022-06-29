use crate::container::network::{IONetwork, IONode};
use crate::container::population::{IOPlanElement, IOPopulation, IORoute};
use crate::parallel_simulation::splittable_network::{Network, Node};
use crate::parallel_simulation::splittable_population::Population;
use crate::parallel_simulation::vehicles::VehiclesIdMapping;
use std::sync::Arc;

pub struct Scenario {
    pub network: Network,
    pub population: Population,
}

impl Scenario {
    fn from_io(network_container: &IONetwork, population_container: &IOPopulation) {
        let mut vehicle_id_mapping = VehiclesIdMapping::new();

        let v = population_container.persons.iter()
            .map(|p| p.selected_plan())
            .flat_map(|p| p.elements.iter())
            .for_each(|el| {
                if let IOPlanElement::Leg(leg) = el {
                    if let IORoute::
                }
            });

        let (networks, node_id_mapping, link_id_mapping) =
            Network::split_from_container(network_container, 2, Scenario::split);
        let (populations, agent_id_mapping) = Population::split_from_container(
            population_container,
            2,
            &link_id_mapping,
            &mut vehicle_id_mapping,
        );
    }

    fn split(node: &IONode) -> usize {
        if node.x < 0. {
            0
        } else {
            1
        }
    }
}
