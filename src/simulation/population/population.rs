use crate::simulation::config::RoutingMode;
use crate::simulation::id::{Id, IdStore};
use crate::simulation::io::population::{IOPerson, IOPlanElement, IOPopulation, IORoute};
use crate::simulation::messaging::messages::proto::{Agent, Vehicle};
use crate::simulation::network::global_network::{Link, Network};
use std::collections::HashMap;

type ActType = ();

#[derive(Debug)]
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

#[cfg(test)]
mod tests {
    use crate::simulation::config::RoutingMode;
    use crate::simulation::messaging::messages::proto::leg::Route;
    use crate::simulation::network::global_network::Network;
    use crate::simulation::population::population::Population;

    #[test]
    fn from_io_1_plan() {

        let net = Network::from_file("./assets/equil/equil-network.xml", 2);
        let pop = Population::from_file("./assets/equil/equil-1-plan.xml", &net, 1, RoutingMode::UsePlans);

        assert_eq!(1, pop.agents.len());

        let agent = pop.agents.get(&pop.agent_ids.get_from_ext("1")).unwrap();
        assert!(matches!(&agent.plan, Some(_)));

        let plan = agent.plan.as_ref().unwrap();
        assert_eq!(4, plan.acts.len());
        assert_eq!(3, plan.legs.len());

        let home_act = plan.acts.first().unwrap();
        assert_eq!("h", home_act.act_type.as_str());
        assert_eq!(net.link_ids.get_from_ext("1").internal as u64, home_act.link_id);
        assert_eq!(-25000., home_act.x);
        assert_eq!(0., home_act.y);
        assert_eq!(Some(6*3600), home_act.end_time);
        assert_eq!(None, home_act.start_time);
        assert_eq!(None, home_act.max_dur);

        let leg = plan.legs.first().unwrap();
        assert_eq!(None, leg.trav_time);
        assert_eq!(None, leg.dep_time);
        assert!(matches!(leg.route, Some(_)));
        if let Route::NetworkRoute(net_route) = leg.route.as_ref().unwrap() {
            assert_eq!(pop.vehicle_ids.get_from_ext("1").internal as u64, net_route.vehicle_id);
            assert_eq!(vec![
                net.link_ids.get_from_ext("1").internal as u64,
                net.link_ids.get_from_ext("6").internal as u64,
                net.link_ids.get_from_ext("15").internal as u64,
                net.link_ids.get_from_ext("20").internal as u64,
            ], net_route.route);
        } else {
            panic!("Expected network route as first leg.")
        }
    }

    #[test]
    fn from_io() {

        let net = Network::from_file("./assets/equil/equil-network.xml", 2);
        let pop = Population::from_file("./assets/equil/equil-plans.xml.gz", &net, 1, RoutingMode::UsePlans);

        assert_eq!(100, pop.agents.len());
    }
}
