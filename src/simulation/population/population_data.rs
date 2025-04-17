use crate::simulation::id::Id;
use crate::simulation::io::attributes::Attrs;
use crate::simulation::network::global_network::{Link, Network};
use crate::simulation::population::io::{
    from_file, to_file, IOActivity, IOLeg, IOPerson, IOPlan, IOPlanElement, IORoute,
};
use crate::simulation::time_queue::{EndTime, Identifiable};
use crate::simulation::vehicles::garage::Garage;
use crate::simulation::wire_types::messages::Vehicle;
use crate::simulation::wire_types::population::{Activity, Leg, Person, Plan, Route};
use crate::simulation::wire_types::vehicles::VehicleType;
use std::collections::HashMap;
use std::path::Path;
use tracing::debug;

impl Person {
    pub fn from_io(io_person: &IOPerson) -> Person {
        let person_id = Id::get_from_ext(&io_person.id);

        let plan = Plan::from_io(io_person.selected_plan(), &person_id);

        if plan.acts.is_empty() {
            debug!("There is an empty plan for person {:?}", io_person.id);
        }

        Person {
            id: person_id.internal(),
            plan: Some(plan),
            curr_plan_elem: 0,
        }
    }

    pub fn new(id: u64, plan: Plan) -> Self {
        Person {
            id,
            curr_plan_elem: 0,
            plan: Some(plan),
        }
    }

    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn curr_act(&self) -> &Activity {
        if self.curr_plan_elem % 2 != 0 {
            panic!("Current element is not an activity");
        }
        let act_index = self.curr_plan_elem / 2;
        self.get_act_at_index(act_index)
    }

    pub fn curr_leg(&self) -> &Leg {
        if self.curr_plan_elem % 2 != 1 {
            panic!("Current element is not a leg.");
        }

        let leg_index = (self.curr_plan_elem - 1) / 2;
        self.plan
            .as_ref()
            .unwrap()
            .legs
            .get(leg_index as usize)
            .unwrap()
    }

    fn get_act_at_index(&self, index: u32) -> &Activity {
        self.plan
            .as_ref()
            .unwrap()
            .acts
            .get(index as usize)
            .unwrap()
    }

    pub fn advance_plan(&mut self) {
        let next = self.curr_plan_elem + 1;
        if self.plan.as_ref().unwrap().acts.len() + self.plan.as_ref().unwrap().legs.len()
            == next as usize
        {
            panic!(
                "Person: Advance plan was called on Person #{}, but no element is remaining.",
                self.id
            )
        }
        self.curr_plan_elem = next;
    }
}

impl EndTime for Person {
    fn end_time(&self, now: u32) -> u32 {
        if self.curr_plan_elem % 2 == 0 {
            self.curr_act().cmp_end_time(now)
        } else {
            self.curr_leg().trav_time + now
        }
    }
}

impl Identifiable for Person {
    fn id(&self) -> u64 {
        self.id
    }
}

impl Plan {
    pub fn new() -> Plan {
        Plan {
            acts: Vec::new(),
            legs: Vec::new(),
        }
    }

    fn from_io(io_plan: &IOPlan, person_id: &Id<Person>) -> Plan {
        assert!(!io_plan.elements.is_empty());
        if let IOPlanElement::Leg(_leg) = io_plan.elements.first().unwrap() {
            panic!("First plan element must be an activity! But was a leg.");
        };

        let mut result = Plan::new();

        for element in &io_plan.elements {
            match element {
                IOPlanElement::Activity(io_act) => {
                    let act = Activity::from_io(io_act);
                    result.acts.push(act);
                }
                IOPlanElement::Leg(io_leg) => {
                    let leg = Leg::from_io(io_leg, person_id);
                    result.legs.push(leg);
                }
            }
        }

        if result.acts.len() - result.legs.len() != 1 {
            panic!("Plan {:?} has less legs than expected", io_plan);
        }

        result
    }

    pub fn add_leg(&mut self, leg: Leg) {
        self.legs.push(leg);
    }

    pub fn add_act(&mut self, activity: Activity) {
        self.acts.push(activity);
    }
}

impl Activity {
    fn from_io(io_act: &IOActivity) -> Self {
        let link_id: Id<Link> = Id::get_from_ext(&io_act.link);
        let act_type: Id<String> = Id::get_from_ext(&io_act.r#type);
        Activity {
            x: io_act.x,
            y: io_act.y,
            act_type: act_type.internal(),
            link_id: link_id.internal(),
            start_time: parse_time_opt(&io_act.start_time),
            end_time: parse_time_opt(&io_act.end_time),
            max_dur: parse_time_opt(&io_act.max_dur),
        }
    }

    pub fn new(
        x: f64,
        y: f64,
        act_type: u64,
        link_id: u64,
        start_time: Option<u32>,
        end_time: Option<u32>,
        max_dur: Option<u32>,
    ) -> Self {
        Activity {
            x,
            y,
            act_type,
            link_id,
            start_time,
            end_time,
            max_dur,
        }
    }

    pub(crate) fn cmp_end_time(&self, now: u32) -> u32 {
        if let Some(end_time) = self.end_time {
            end_time
        } else if let Some(max_dur) = self.max_dur {
            now + max_dur
        } else {
            // supposed to be an equivalent for OptionalTime.undefined() in the java code
            u32::MAX
        }
    }

    pub fn is_interaction(&self) -> bool {
        Id::<String>::get(self.act_type)
            .external()
            .contains("interaction")
    }
}

impl Leg {
    fn from_io(io_leg: &IOLeg, person_id: &Id<Person>) -> Self {
        let routing_mode_ext = Attrs::find_or_else_opt(&io_leg.attributes, "routingMode", || "car");

        let routing_mode: Id<String> = Id::create(routing_mode_ext);
        let mode = Id::get_from_ext(io_leg.mode.as_str());

        let route = io_leg
            .route
            .as_ref()
            .map(|r| Route::from_io(r, person_id, &mode));

        Self {
            route,
            mode: mode.internal(),
            trav_time: Self::parse_trav_time(
                &io_leg.trav_time,
                &io_leg.route.as_ref().and_then(|r| r.trav_time.clone()),
            ),
            dep_time: parse_time_opt(&io_leg.dep_time),
            routing_mode: routing_mode.internal(),
            attributes: HashMap::new(),
        }
    }

    pub fn new(route: Route, mode: u64, trav_time: u32, dep_time: Option<u32>) -> Self {
        Self {
            route: Some(route),
            mode,
            trav_time,
            dep_time,
            routing_mode: 0,
            attributes: HashMap::new(),
        }
    }

    fn parse_trav_time(leg_trav_time: &Option<String>, route_trav_time: &Option<String>) -> u32 {
        if let Some(trav_time) = parse_time_opt(leg_trav_time) {
            trav_time
        } else {
            parse_time_opt(route_trav_time).unwrap_or(0)
        }
    }

    pub fn vehicle_type_id(&self, garage: &Garage) -> Id<VehicleType> {
        self.route
            .as_ref()
            .map(|r| garage.vehicle_type_id(&Id::get(r.veh_id)))
            .unwrap()
    }
}

impl Route {
    pub fn start_link(&self) -> u64 {
        *self.route.first().unwrap()
    }

    pub fn end_link(&self) -> u64 {
        *self.route.last().unwrap()
    }

    fn from_io(io_route: &IORoute, person_id: &Id<Person>, mode: &Id<String>) -> Self {
        let route = match io_route.r#type.as_str() {
            "generic" => Self::from_io_generic(io_route, person_id, mode),
            "links" => Self::from_io_net_route(io_route, person_id, mode),
            _t => panic!("Unsupported route type: '{_t}'"),
        };

        route
    }

    fn from_io_generic(io_route: &IORoute, person_id: &Id<Person>, mode: &Id<String>) -> Self {
        let start_link: Id<Link> = Id::get_from_ext(&io_route.start_link);
        let end_link: Id<Link> = Id::get_from_ext(&io_route.end_link);
        let external = format!("{}_{}", person_id.external(), mode.external());
        let veh_id: Id<Vehicle> = Id::get_from_ext(&external);

        Route {
            distance: io_route.distance,
            veh_id: veh_id.internal(),
            route: vec![start_link.internal(), end_link.internal()],
        }
    }

    fn from_io_net_route(io_route: &IORoute, person_id: &Id<Person>, mode: &Id<String>) -> Self {
        if let Some(veh_id_ext) = &io_route.vehicle {
            // catch this special case because we have "null" as vehicle ids for modes which are
            // routed but not simulated on the network.
            if veh_id_ext.eq("null") {
                Self::from_io_generic(io_route, person_id, mode)
            } else {
                let veh_id: Id<Vehicle> = Id::get_from_ext(veh_id_ext.as_str());
                let link_ids = match &io_route.route {
                    None => Vec::new(),
                    Some(encoded_links) => encoded_links
                        .split(' ')
                        .map(|matsim_id| Id::<Link>::get_from_ext(matsim_id).internal())
                        .collect(),
                };
                Route {
                    distance: io_route.distance,
                    veh_id: veh_id.internal(),
                    route: link_ids,
                }
            }
        } else {
            panic!("vehicle id is expected to be set.")
        }
    }
}

fn parse_time_opt(value: &Option<String>) -> Option<u32> {
    if let Some(time) = value.as_ref() {
        parse_time(time)
    } else {
        None
    }
}

fn parse_time(value: &str) -> Option<u32> {
    let split: Vec<&str> = value.split(':').collect();
    if split.len() == 3 {
        let hour: u32 = split.first().unwrap().parse().unwrap();
        let minutes: u32 = split.get(1).unwrap().parse().unwrap();
        let seconds: u32 = split.get(2).unwrap().parse().unwrap();

        Some(hour * 3600 + minutes * 60 + seconds)
    } else {
        None
    }
}

#[derive(Debug, Default, PartialEq)]
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
        from_file(file_path, garage, |_p| true)
    }

    pub fn from_file_filtered<F>(file_path: &Path, garage: &mut Garage, filter: F) -> Self
    where
        F: Fn(&Person) -> bool,
    {
        from_file(file_path, garage, filter)
    }

    pub fn from_file_filtered_part(
        file_path: &Path,
        net: &Network,
        garage: &mut Garage,
        part: u32,
    ) -> Self {
        from_file(file_path, garage, |p| {
            let act = p.curr_act();
            let partition = net.links.get(act.link_id as usize).unwrap().partition;
            partition == part
        })
    }

    pub fn to_file(&self, file_path: &Path) {
        to_file(self, file_path);
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::path::PathBuf;

    use crate::simulation::config::{MetisOptions, PartitionMethod};
    use crate::simulation::id::Id;
    use crate::simulation::network::global_network::{Link, Network};
    use crate::simulation::population::population_data::Population;
    use crate::simulation::vehicles::garage::Garage;
    use crate::simulation::wire_types::messages::Vehicle;
    use crate::simulation::wire_types::population::Person;

    #[test]
    fn from_io_1_plan() {
        let _net = Network::from_file_as_is(&PathBuf::from("./assets/equil/equil-network.xml"));
        let mut garage = Garage::from_file(&PathBuf::from("./assets/equil/equil-vehicles.xml"));
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
        let _net = Network::from_file_as_is(&PathBuf::from("./assets/3-links/3-links-network.xml"));
        let mut garage = Garage::from_file(&PathBuf::from("./assets/3-links/vehicles.xml"));
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
            PartitionMethod::Metis(MetisOptions::default()),
        );
        let mut garage = Garage::from_file(&PathBuf::from("./assets/equil/equil-vehicles.xml"));
        let pop1 = Population::from_file_filtered_part(
            &PathBuf::from("./assets/equil/equil-plans.xml.gz"),
            &net,
            &mut garage,
            0,
        );
        let pop2 = Population::from_file_filtered_part(
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

    #[test]
    fn test_from_xml_to_binpb_same() {
        let net = Network::from_file(
            "./assets/equil/equil-network.xml",
            1,
            PartitionMethod::Metis(MetisOptions::default()),
        );
        let mut garage = Garage::from_file(&PathBuf::from("./assets/equil/equil-vehicles.xml"));
        let population = Population::from_file(
            &PathBuf::from("./assets/equil/equil-plans.xml.gz"),
            &mut garage,
        );

        let temp_file = PathBuf::from(
            "test_output/simulation/population/population/test_from_xml_to_binpb_same/plans.binpb",
        );
        population.to_file(&temp_file);
        let population2 = Population::from_file_filtered_part(&temp_file, &net, &mut garage, 0);
        assert_eq!(population, population2);
    }
}
