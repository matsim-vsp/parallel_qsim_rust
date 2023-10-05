use std::collections::{HashMap, HashSet};

use crate::simulation::id::{Id, IdStore};
use crate::simulation::io::population::{IOPerson, IOPlanElement, IOPopulation};
use crate::simulation::messaging::messages::proto::Agent;
use crate::simulation::network::global_network::{Link, Network};
use crate::simulation::vehicles::garage::Garage;
use crate::simulation::vehicles::vehicle_type::VehicleType;

type ActType = ();

#[derive(Debug, Default)]
pub struct Population<'p> {
    pub agents: HashMap<Id<Agent>, Agent>,
    pub agent_ids: IdStore<'p, Agent>,
    // TODO this should probably go somewhere else
    pub act_types: IdStore<'p, ActType>,
}

impl<'p> Population<'p> {
    pub fn new() -> Self {
        Population {
            agents: HashMap::default(),
            agent_ids: IdStore::new(),
            act_types: IdStore::new(),
        }
    }

    pub fn from_file(file: &str, net: &Network, garage: &mut Garage, partition: usize) -> Self {
        let io_population = IOPopulation::from_file(file);
        Self::from_io(&io_population, net, garage, partition)
    }

    pub fn from_io(
        io_population: &IOPopulation,
        network: &Network,
        garage: &mut Garage,
        partition: usize,
    ) -> Self {
        let mut result = Population::new();

        // create person ids, and vehicles for each person
        Self::create_ids(&mut result, io_population, garage);
        // create the actual persons for this partition
        Self::create_persons(&mut result, io_population, network, garage, partition);
        // create a vehicles for all modes for persons belonging to this partition
        Self::create_vehicles(garage, &result);

        result
    }

    fn create_ids(pop: &mut Population, io: &IOPopulation, garage: &mut Garage) {
        // create person ids and collect strings for vehicle ids
        let raw_veh: Vec<_> = io
            .persons
            .iter()
            .map(|p| pop.agent_ids.create_id(p.id.as_str()))
            .flat_map(|p_id| {
                garage
                    .vehicle_types
                    .iter()
                    .map(move |veh_type| Self::create_veh_id_string(&p_id, &veh_type.id))
            })
            .collect();

        // have this in a separate loop because we are iterating over garage's vehicle types and we
        // can't borrow vehicle types while using a &mut in add_veh.
        for veh_id in raw_veh {
            garage.add_veh_id(veh_id.as_str());
        }

        // now iterate over all plans to extract activity ids
        let types: HashSet<_> = io
            .persons
            .iter()
            .flat_map(|person| person.plans.iter())
            .flat_map(|plan| plan.elements.iter())
            .filter_map(|element| match element {
                IOPlanElement::Activity(a) => Some(a),
                IOPlanElement::Leg(_) => None,
            })
            .map(|act| &act.r#type)
            .collect();

        for act_type in types {
            pop.act_types.create_id(act_type.as_str());
        }
    }

    fn create_persons(
        result: &mut Population,
        io_population: &IOPopulation,
        net: &Network,
        garage: &Garage,
        part: usize,
    ) {
        let persons: Vec<_> = io_population
            .persons
            .iter()
            .filter(|io_p| Self::is_partition(io_p, net, part))
            .map(|io_p| Agent::from_io(io_p, net, result, garage))
            .collect();

        for person in persons {
            let person_id = result.agent_ids.get_from_wire(person.id);
            result.agents.insert(person_id, person);
        }
    }

    fn create_vehicles(garage: &mut Garage, pop: &Population) {
        let person_mode: Vec<_> = pop
            .agents
            .keys()
            .flat_map(|p_id| {
                garage
                    .vehicle_types
                    .iter()
                    .map(move |veh_type| (p_id, veh_type.id.clone()))
            })
            .collect();

        for (person_id, veh_type) in person_mode {
            let vehicle_id_ext = Self::create_veh_id_string(&person_id, &veh_type);
            let vehicle_id = garage.vehicle_ids.get_from_ext(vehicle_id_ext.as_str());
            garage.add_veh(vehicle_id, veh_type);
        }
    }

    fn create_veh_id_string(person_id: &Id<Agent>, veh_type: &Id<VehicleType>) -> String {
        format!("{}_{}", person_id.external, veh_type.external)
    }

    fn is_partition(io_person: &IOPerson, net: &Network, partition: usize) -> bool {
        let link = Self::link_first_act(io_person, net);
        link.partition == partition
    }

    fn link_first_act<'n>(io: &IOPerson, net: &'n Network) -> &'n Link {
        let first_element = io.selected_plan().elements.first().unwrap();
        if let IOPlanElement::Activity(act) = first_element {
            let link_id = net.link_ids.get_from_ext(&act.link);
            return net.get_link(&link_id);
        }

        panic!("First element should be activity.");
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use crate::simulation::messaging::messages::proto::leg::Route;
    use crate::simulation::network::global_network::Network;
    use crate::simulation::population::population::Population;
    use crate::simulation::vehicles::garage::Garage;

    #[test]
    fn from_io_1_plan() {
        let mut garage = Garage::from_file("./assets/equil/equil-vehicles.xml");
        let net = Network::from_file("./assets/equil/equil-network.xml", 1, &mut garage);
        let pop = Population::from_file("./assets/equil/equil-1-plan.xml", &net, &mut garage, 0);

        assert_eq!(1, pop.agents.len());

        let agent = pop.agents.get(&pop.agent_ids.get_from_ext("1")).unwrap();
        assert!(agent.plan.is_some());

        let plan = agent.plan.as_ref().unwrap();
        assert_eq!(4, plan.acts.len());
        assert_eq!(3, plan.legs.len());

        let home_act = plan.acts.first().unwrap();
        let act_type = pop.act_types.get_from_wire(home_act.act_type);
        assert_eq!("h", act_type.external.as_str());
        assert_eq!(
            net.link_ids.get_from_ext("1").internal as u64,
            home_act.link_id
        );
        assert_eq!(-25000., home_act.x);
        assert_eq!(0., home_act.y);
        assert_eq!(Some(6 * 3600), home_act.end_time);
        assert_eq!(None, home_act.start_time);
        assert_eq!(None, home_act.max_dur);

        let leg = plan.legs.first().unwrap();
        assert_eq!(None, leg.trav_time);
        assert_eq!(None, leg.dep_time);
        assert!(leg.route.is_some());
        if let Route::NetworkRoute(net_route) = leg.route.as_ref().unwrap() {
            assert_eq!(
                garage.vehicle_ids.get_from_ext("1_car").internal as u64,
                net_route.vehicle_id
            );
            assert_eq!(
                vec![
                    net.link_ids.get_from_ext("1").internal as u64,
                    net.link_ids.get_from_ext("6").internal as u64,
                    net.link_ids.get_from_ext("15").internal as u64,
                    net.link_ids.get_from_ext("20").internal as u64,
                ],
                net_route.route
            );
        } else {
            panic!("Expected network route as first leg.")
        }
    }

    #[test]
    fn from_io_multi_mode() {
        let mut garage = Garage::from_file("./assets/3-links/vehicles.xml");
        let net = Network::from_file("./assets/3-links/3-links-network.xml", 1, &mut garage);
        let pop = Population::from_file("./assets/3-links/3-agent.xml", &net, &mut garage, 0);

        // check that we have all three vehicle types
        let expected_veh_types = HashSet::from(["car", "bike", "walk"]);
        assert_eq!(3, garage.vehicle_types.len());
        assert!(garage
            .vehicle_types
            .iter()
            .all(|veh_type| expected_veh_types.contains(veh_type.id.external.as_str())));

        // check that we have a vehicle for each mode and for each person
        assert_eq!(9, garage.vehicles.len());

        // check population
        // activity types should be done as id. If id is not present this will crash
        assert_eq!("home", pop.act_types.get_from_ext("home").external.as_str());
        assert_eq!(
            "errands",
            pop.act_types.get_from_ext("errands").external.as_str()
        );

        // agents should also have ids
        assert_eq!("100", pop.agent_ids.get_from_ext("100").external.as_str());
        assert_eq!("200", pop.agent_ids.get_from_ext("200").external.as_str());
        assert_eq!("300", pop.agent_ids.get_from_ext("300").external.as_str());

        // we expect three agents overall
        assert_eq!(3, pop.agents.len());

        println!("{pop:#?}");
    }

    #[test]
    fn from_io() {
        let mut garage = Garage::from_file("./assets/equil/equil-vehicles.xml");
        let net = Network::from_file("./assets/equil/equil-network.xml", 2, &mut garage);
        let pop1 = Population::from_file("./assets/equil/equil-plans.xml.gz", &net, &mut garage, 0);
        let pop2 = Population::from_file("./assets/equil/equil-plans.xml.gz", &net, &mut garage, 1);

        // metis produces unstable results on small networks so, make sure that one of the populations
        // has all the agents and the other doesn't
        assert!(pop1.agents.len() == 100 || pop2.agents.len() == 100);
        assert!(pop1.agents.is_empty() || pop2.agents.is_empty());
    }
}
