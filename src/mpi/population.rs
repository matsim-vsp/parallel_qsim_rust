use crate::config::RoutingMode;
use crate::io::population::{IOPerson, IOPlanElement, IOPopulation};
use crate::mpi::messages::proto::Agent;
use crate::parallel_simulation::id_mapping::{MatsimIdMapping, MatsimIdMappings};
use crate::parallel_simulation::network::partitioned_network::Network;
use std::collections::HashMap;
use std::fmt::Debug;

pub struct Population {
    pub agents: HashMap<usize, Agent>,
}

impl Population {
    fn new() -> Population {
        Population {
            agents: HashMap::new(),
        }
    }

    pub fn from_io<V: Debug>(
        io_population: &IOPopulation,
        id_mappings: &MatsimIdMappings,
        partition: usize,
        network: &Network<V>,
        routing_mode: RoutingMode,
    ) -> Population {
        let mut result = Population::new();

        for io_person in &io_population.persons {
            let link_id = Self::link_id_first_act(io_person, &id_mappings.links);
            let agent_partition = *network.partition_for_link(&link_id);

            // take only agents which start on our partition
            if agent_partition == partition {
                let agent = Agent::from_io(io_person, id_mappings, routing_mode);
                result.agents.insert(agent.id(), agent);
            }
        }

        result
    }

    fn link_id_first_act(io_person: &IOPerson, link_id_mapping: &MatsimIdMapping) -> usize {
        let first_element = io_person.selected_plan().elements.get(0).unwrap();
        if let IOPlanElement::Activity(io_act) = first_element {
            return *link_id_mapping.get_internal(io_act.link.as_str()).unwrap();
        }

        panic!("First element should be activity.");
    }
}
