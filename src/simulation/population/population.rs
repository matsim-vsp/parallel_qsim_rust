use crate::simulation::config::RoutingMode;
use crate::simulation::id::{Id, IdStore};
use crate::simulation::io::population::{IOPerson, IOPlanElement, IOPopulation, IORoute};
use crate::simulation::messaging::messages::proto::{Agent, Vehicle};
use crate::simulation::network::global_network::{Link, Network};
use std::collections::HashMap;

type ActType = ();

pub struct Population<'p> {
    pub agents: HashMap<Id<Agent>, Agent>,
    pub agent_ids: IdStore<'p, Agent>,
    pub vehicle_ids: IdStore<'p, Vehicle>, // TODO this should probably go somewhere else
    pub act_types: IdStore<'p, ActType>,
}

impl<'p> Population<'p> {
    pub fn new() -> Self {
        Population {
            agents: HashMap::default(),
            agent_ids: IdStore::new(),
            vehicle_ids: IdStore::new(),
            act_types: IdStore::new(),
        }
    }

    pub fn from_file(
        file: &str,
        net: &Network,
        partition: usize,
        routing_mode: RoutingMode,
    ) -> Self {
        let io_population = IOPopulation::from_file(file);
        Self::from_io(&io_population, &net, partition, routing_mode)
    }

    pub fn from_io(
        io_population: &IOPopulation,
        network: &Network,
        partition: usize,
        routing_mode: RoutingMode,
    ) -> Self {
        let mut result = Population::new();

        // first pass to set ids globally
        for io in io_population.persons.iter() {
            Self::agent_id(io, &mut result);
        }

        // then copy the agents on this partition
        for io in io_population.persons.iter() {
            let link = Self::link_first_act(io, network);
            if partition == link.partition {
                let agent = Agent::from_io(io, network, &result, routing_mode);
                result
                    .agents
                    .insert(result.agent_ids.get(agent.id as usize), agent);
            }
        }

        result
    }

    fn link_first_act<'n>(io: &IOPerson, net: &'n Network) -> &'n Link {
        let first_element = io.selected_plan().elements.first().unwrap();
        if let IOPlanElement::Activity(act) = first_element {
            let link_id = net.link_ids.get_from_ext(&act.link);
            return net.get_link(&link_id);
        }

        panic!("First element should be activity.");
    }

    fn agent_id(io: &IOPerson, pop: &mut Population) {
        pop.agent_ids.create_id(&io.id);
        for io_plan in io.plans.iter() {
            for element in io_plan.elements.iter() {
                match element {
                    IOPlanElement::Activity(a) => {
                        pop.act_types.create_id(&a.r#type);
                    }
                    IOPlanElement::Leg(l) => {
                        Self::route_ids(&l.route, pop);
                    }
                }
            }
        }
    }

    fn route_ids(io: &IORoute, pop: &mut Population) {
        match io.r#type.as_str() {
            "links" => {
                let veh_id = io
                    .vehicle
                    .as_ref()
                    .expect("Vehicle id is expected to be set for network route");
                match veh_id.as_str() {
                    "null" => (),
                    _ => {
                        pop.vehicle_ids.create_id(veh_id);
                    }
                };
            }
            _t => panic!("Unsupported route type: '{_t}'"),
        };
    }
}
