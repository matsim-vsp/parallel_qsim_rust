use std::collections::HashMap;
use std::path::Path;

use crate::simulation::id::Id;
use crate::simulation::network::global_network::Network;
use crate::simulation::population::io::{from_file, to_file};
use crate::simulation::vehicles::garage::Garage;
use crate::simulation::wire_types::population::Person;

#[derive(Debug, Default)]
pub struct Population {
    pub persons: HashMap<Id<Person>, Person>,
}

impl Population {
    pub fn new() -> Self {
        Population {
            persons: HashMap::default(),
        }
    }

    pub fn from_file(file_path: &Path, garage: &mut Garage) -> Self {
        super::io::from_file(file_path, garage)
    }

    pub fn part_from_file(file_path: &Path, net: &Network, garage: &mut Garage, part: u32) -> Self {
        let pop = from_file(file_path, garage);
        let filtered_persons = pop
            .persons
            .into_iter()
            .filter(|(_id, p)| {
                let act = p.curr_act();
                let partition = net.links.get(act.link_id as usize).unwrap().partition;
                partition == part
            })
            .collect();
        Population {
            persons: filtered_persons,
        }
    }

    pub fn to_file(&self, file_path: &Path) {
        to_file(&self, file_path);
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::path::PathBuf;

    use crate::simulation::config::PartitionMethod;
    use crate::simulation::id::Id;
    use crate::simulation::network::global_network::{Link, Network};
    use crate::simulation::population::population::Population;
    use crate::simulation::vehicles::garage::Garage;
    use crate::simulation::wire_types::messages::Vehicle;
    use crate::simulation::wire_types::population::Person;

    #[test]
    fn from_io_1_plan() {
        let net = Network::from_file(
            "./assets/equil/equil-network.xml",
            1,
            PartitionMethod::Metis,
        );
        let mut garage = Garage::from_file("./assets/equil/equil-vehicles.xml");
        let pop = Population::from_file(
            &PathBuf::from("./assets/equil/equil-1-plan.xml"),
            &mut garage,
        );

        assert_eq!(1, pop.persons.len());

        let agent = pop.persons.get(&Id::get_from_ext("1")).unwrap();
        assert!(agent.plan.is_some());

        let plan = agent.plan.as_ref().unwrap();
        assert_eq!(4, plan.acts.len());
        assert_eq!(3, plan.legs.len());

        let home_act = plan.acts.first().unwrap();
        let act_type: Id<String> = Id::get(home_act.act_type);
        assert_eq!("h", act_type.external());
        assert_eq!(Id::<Link>::get_from_ext("1").internal(), home_act.link_id);
        assert_eq!(-25000., home_act.x);
        assert_eq!(0., home_act.y);
        assert_eq!(Some(6 * 3600), home_act.end_time);
        assert_eq!(None, home_act.start_time);
        assert_eq!(None, home_act.max_dur);

        let leg = plan.legs.first().unwrap();
        assert_eq!(None, leg.dep_time);
        assert!(leg.route.is_some());
        let net_route = leg.route.as_ref().unwrap();
        assert_eq!(
            Id::<Vehicle>::get_from_ext("1_car").internal(),
            net_route.veh_id
        );
        assert_eq!(
            vec![
                Id::<Link>::get_from_ext("1").internal(),
                Id::<Link>::get_from_ext("6").internal(),
                Id::<Link>::get_from_ext("15").internal(),
                Id::<Link>::get_from_ext("20").internal(),
            ],
            net_route.route
        );
    }

    #[test]
    fn from_io_multi_mode() {
        let net = Network::from_file(
            "./assets/3-links/3-links-network.xml",
            1,
            PartitionMethod::Metis,
        );
        let mut garage = Garage::from_file("./assets/3-links/vehicles.xml");
        let pop =
            Population::from_file(&PathBuf::from("./assets/3-links/3-agent.xml"), &mut garage);

        // check that we have all three vehicle types
        let expected_veh_types = HashSet::from(["car", "bike", "walk"]);
        assert_eq!(3, garage.vehicle_types.len());
        assert!(garage
            .vehicle_types
            .keys()
            .all(|type_id| expected_veh_types.contains(type_id.external())));

        // check that we have a vehicle for each mode and for each person
        assert_eq!(9, garage.vehicles.len());

        // check population
        // activity types should be done as id. If id is not present this will crash
        assert_eq!("home", Id::<String>::get_from_ext("home").external());
        assert_eq!("errands", Id::<String>::get_from_ext("errands").external());

        // each of the network mode should also have an interaction activity type
        assert_eq!(
            "car interaction",
            Id::<String>::get_from_ext("car interaction").external()
        );
        assert_eq!(
            "bike interaction",
            Id::<String>::get_from_ext("bike interaction").external()
        );

        // agents should also have ids
        assert_eq!("100", Id::<Person>::get_from_ext("100").external());
        assert_eq!("200", Id::<Person>::get_from_ext("200").external());
        assert_eq!("300", Id::<Person>::get_from_ext("300").external());

        // we expect three agents overall
        assert_eq!(3, pop.persons.len());

        // todo test bookkeeping of garage person_2_vehicle
    }

    #[test]
    fn from_io() {
        let net = Network::from_file(
            "./assets/equil/equil-network.xml",
            2,
            PartitionMethod::Metis,
        );
        let mut garage = Garage::from_file("./assets/equil/equil-vehicles.xml");
        let pop1 = Population::part_from_file(
            &PathBuf::from("./assets/equil/equil-plans.xml.gz"),
            &net,
            &mut garage,
            0,
        );
        let pop2 = Population::part_from_file(
            &PathBuf::from("./assets/equil/equil-plans.xml.gz"),
            &net,
            &mut garage,
            1,
        );

        // metis produces unstable results on small networks so, make sure that one of the populations
        // has all the agents and the other doesn't
        assert!(pop1.persons.len() == 100 || pop2.persons.len() == 100);
        assert!(pop1.persons.is_empty() || pop2.persons.is_empty());
    }
}
