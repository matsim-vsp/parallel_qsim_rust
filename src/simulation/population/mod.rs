use crate::simulation::id::Id;
use crate::simulation::io::attributes::Attrs;
use crate::simulation::network::global_network::Link;
use crate::simulation::population::io::{
    IOActivity, IOLeg, IOPerson, IOPlan, IOPlanElement, IORoute,
};
use crate::simulation::vehicles::InternalVehicle;
use crate::simulation::InternalAttributes;
use serde_json::{Error, Value};
use std::str::FromStr;

pub mod agent_source;
pub mod io;
pub mod population_data;

#[derive(Debug, PartialEq, Clone)]
pub struct InternalActivity {
    pub act_type: Id<String>,
    pub link_id: Id<Link>,
    pub x: f64,
    pub y: f64,
    pub start_time: Option<u32>,
    pub end_time: Option<u32>,
    pub max_dur: Option<u32>,
}

#[derive(Debug, PartialEq, Clone)]
pub struct InternalLeg {
    pub mode: Id<String>,
    pub routing_mode: Id<String>,
    pub dep_time: Option<u32>,
    pub trav_time: Option<u32>,
    pub route: Option<InternalRoute>,
    pub attributes: Option<InternalAttributes>,
}

#[derive(Debug, PartialEq, Clone)]
pub enum InternalRoute {
    Generic(InternalGenericRoute),
    Network(InternalNetworkRoute),
    Pt(InternalPtRoute),
}

#[derive(Debug, PartialEq, Clone)]
pub struct InternalGenericRoute {
    pub start_link: Id<Link>,
    pub end_link: Id<Link>,
    pub trav_time: Option<u32>,
    pub distance: Option<f64>,
    pub vehicle: Option<Id<InternalVehicle>>,
}

#[derive(Debug, PartialEq, Clone)]
pub struct InternalNetworkRoute {
    pub generic_delegate: InternalGenericRoute,
    pub route: Vec<Id<Link>>,
}

#[derive(Debug, PartialEq, Clone)]
pub struct InternalPtRoute {
    pub generic_delegate: InternalGenericRoute,
    pub description: InternalPtRouteDescription,
}

#[derive(Debug, PartialEq, Clone)]
pub struct InternalPtRouteDescription {
    pub transit_route_id: String,
    pub boarding_time: Option<u32>,
    pub transit_line_id: String,
    pub access_facility_id: String,
    pub egress_facility_id: String,
}

#[derive(Debug, PartialEq, Clone)]
pub enum InternalPlanElement {
    Activity(InternalActivity),
    Leg(InternalLeg),
}

#[derive(Debug, PartialEq, Clone)]
pub struct InternalPlan {
    pub selected: bool,
    pub elements: Vec<InternalPlanElement>,
}

#[derive(Debug, PartialEq, Clone)]
pub struct InternalPerson {
    pub id: Id<InternalPerson>,
    pub plans: Vec<InternalPlan>,
    pub attributes: Option<Attrs>,
}

#[derive(Debug, PartialEq)]
pub struct InternalPopulation {
    pub persons: Vec<InternalPerson>,
}

impl InternalPerson {
    pub fn new(id: Id<InternalPerson>, plan: InternalPlan) -> Self {
        InternalPerson {
            id,
            plans: vec![plan],
            attributes: None,
        }
    }

    pub fn id(&self) -> &Id<InternalPerson> {
        &self.id
    }

    pub fn curr_act(&self) -> &InternalActivity {
        // if self.curr_plan_elem % 2 != 0 {
        //     panic!("Current element is not an activity");
        // }
        // let act_index = self.curr_plan_elem / 2;
        // self.get_act_at_index(act_index)
        todo!()
    }

    pub fn curr_leg(&self) -> &InternalLeg {
        // if self.curr_plan_elem % 2 != 1 {
        //     panic!("Current element is not a leg.");
        // }
        //
        // let leg_index = (self.curr_plan_elem - 1) / 2;
        // self.plan
        //     .as_ref()
        //     .unwrap()
        //     .legs
        //     .get(leg_index as usize)
        //     .unwrap()
        todo!()
    }

    pub fn next_leg(&self) -> Option<&InternalLeg> {
        // position index: 0      | 1
        // activities:     a0 (0) | a1 (2)
        // legs:           l0 (1) | l1 (3)
        // e.g., if current is a1, next leg is l1 => curr_plan_elem/2
        // e.g., if current is l0, next leg is l1 => (curr_plan_elem + 1)/2

        // let next_leg_index = if self.curr_plan_elem % 2 == 0 {
        //     // current element is an activity
        //     self.curr_plan_elem / 2
        // } else {
        //     // current element is a leg
        //     (self.curr_plan_elem + 1) / 2
        // };
        //
        // self.plan
        //     .as_ref()
        //     .unwrap()
        //     .legs
        //     .get(next_leg_index as usize)
        todo!()
    }

    fn get_act_at_index(&self, index: u32) -> &InternalActivity {
        // self.plan
        //     .as_ref()
        //     .unwrap()
        //     .acts
        //     .get(index as usize)
        //     .unwrap()
        todo!()
    }

    pub fn advance_plan(&mut self) {
        // let next = self.curr_plan_elem + 1;
        // if self.plan.as_ref().unwrap().acts.len() + self.plan.as_ref().unwrap().legs.len()
        //     == next as usize
        // {
        //     panic!(
        //         "Person: Advance plan was called on Person #{}, but no element is remaining.",
        //         self.id
        //     )
        // }
        // self.curr_plan_elem = next;
        todo!()
    }

    pub fn legs(&self) -> &[InternalLeg] {
        // self.plan.as_ref().unwrap().legs.as_slice()
        todo!()
    }

    pub fn acts(&self) -> &[InternalActivity] {
        // self.plan.as_ref().unwrap().acts.as_slice()
        todo!()
    }

    pub fn selected_plan(&self) -> Option<&InternalPlan> {
        self.plans.iter().find(|&plan| plan.selected)
    }
}

impl Default for InternalPlan {
    fn default() -> Self {
        Self {
            selected: true,
            elements: Vec::new(),
        }
    }
}

impl InternalPlan {
    pub fn add_leg(&mut self, leg: InternalLeg) {
        self.elements.push(InternalPlanElement::Leg(leg));
    }

    pub fn add_act(&mut self, activity: InternalActivity) {
        self.elements.push(InternalPlanElement::Activity(activity));
    }

    pub fn legs(&self) -> &Vec<InternalLeg> {
        todo!()
    }

    pub fn acts(&self) -> &Vec<InternalActivity> {
        todo!()
    }
}

impl InternalActivity {
    pub fn new(
        x: f64,
        y: f64,
        act_type: &str,
        link_id: Id<Link>,
        start_time: Option<u32>,
        end_time: Option<u32>,
        max_dur: Option<u32>,
    ) -> Self {
        InternalActivity {
            x,
            y,
            act_type: Id::create(&act_type),
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
        self.act_type.external().contains("interaction")
    }
}

impl FromStr for InternalPtRouteDescription {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let desc: Value = serde_json::from_str(s)?;

        Ok(InternalPtRouteDescription {
            transit_route_id: trim_quotes(&desc["transitRouteId"]),
            boarding_time: desc["boardingTime"].as_str().and_then(parse_time),
            transit_line_id: trim_quotes(&desc["transitLineId"]),
            access_facility_id: trim_quotes(&desc["accessFacilityId"]),
            egress_facility_id: trim_quotes(&desc["egressFacilityId"]),
        })
    }
}

impl InternalRoute {
    pub fn as_generic(&self) -> &InternalGenericRoute {
        match self {
            InternalRoute::Generic(g) => g,
            InternalRoute::Network(n) => &n.generic_delegate,
            InternalRoute::Pt(p) => &p.generic_delegate,
        }
    }

    pub fn as_network(&self) -> Option<&InternalNetworkRoute> {
        match self {
            InternalRoute::Network(n) => Some(n),
            _ => None,
        }
    }

    pub fn as_pt(&self) -> Option<&InternalPtRoute> {
        match self {
            InternalRoute::Pt(p) => Some(p),
            _ => None,
        }
    }

    pub fn start_link(&self) -> &Id<Link> {
        &self.as_generic().start_link
    }

    pub fn end_link(&self) -> &Id<Link> {
        &self.as_generic().start_link
    }
}

impl InternalLeg {
    pub fn new(route: InternalRoute, mode: &str, trav_time: u32, dep_time: Option<u32>) -> Self {
        Self {
            route: Some(route),
            mode: Id::create(mode),
            routing_mode: Id::create(mode),
            trav_time: Some(trav_time),
            dep_time,
            attributes: None,
        }
    }

    fn parse_trav_time(leg_trav_time: &Option<String>, route_trav_time: &Option<u32>) -> u32 {
        if let Some(trav_time) = parse_time_opt(leg_trav_time) {
            trav_time
        } else {
            route_trav_time.unwrap_or(0)
        }
    }
}

impl From<IOLeg> for InternalLeg {
    fn from(io: IOLeg) -> Self {
        let routing_mode = io
            .attributes
            .as_ref()
            .expect("No attributes provided for leg")
            .find_or_else("routingMode", || panic!("No routing mode provied"));

        InternalLeg {
            mode: Id::create(&io.mode),
            routing_mode: Id::create(&routing_mode),
            dep_time: io.dep_time.and_then(|s| s.parse().ok()),
            trav_time: io.trav_time.and_then(|s| s.parse().ok()),
            route: io.route.map(InternalRoute::from),
            attributes: io
                .attributes
                .and_then(|a| InternalAttributes::from(a).into()),
        }
    }
}

impl From<IOActivity> for InternalActivity {
    fn from(io: IOActivity) -> Self {
        InternalActivity {
            act_type: Id::create(&io.r#type),
            link_id: Id::create(&io.link),
            x: io.x,
            y: io.y,
            start_time: io.start_time.and_then(|s| s.parse().ok()),
            end_time: io.end_time.and_then(|s| s.parse().ok()),
            max_dur: io.max_dur.and_then(|s| s.parse().ok()),
        }
    }
}

fn trim_quotes(s: &Value) -> String {
    s.to_string().trim_matches('"').to_string()
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

impl From<IOPerson> for InternalPerson {
    fn from(io: IOPerson) -> Self {
        InternalPerson {
            id: Id::create(&io.id),
            plans: io.plans.into_iter().map(InternalPlan::from).collect(),
            attributes: io.attributes,
        }
    }
}

impl From<IOPlanElement> for InternalPlanElement {
    fn from(io: IOPlanElement) -> Self {
        match io {
            IOPlanElement::Activity(act) => {
                InternalPlanElement::Activity(InternalActivity::from(act))
            }
            IOPlanElement::Leg(leg) => InternalPlanElement::Leg(InternalLeg::from(leg)),
        }
    }
}

impl From<IORoute> for InternalRoute {
    fn from(io: IORoute) -> Self {
        todo!()
        // InternalRoute {
        //     r#type: Id::create(&io.r#type),
        //     start_link: Id::create(&io.start_link),
        //     end_link: Id::create(&io.end_link),
        //     trav_time: io.trav_time.and_then(|s| s.parse().ok()),
        //     distance: io.distance,
        //     vehicle: io.vehicle.map(|v| Id::create(&v)),
        //     route: todo!("Convert route links from IORoute to InternalRoute"),
        // }
    }
}

impl From<IOPlan> for InternalPlan {
    fn from(io: IOPlan) -> Self {
        InternalPlan {
            selected: io.selected,
            elements: io
                .elements
                .into_iter()
                .map(InternalPlanElement::from)
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::simulation::config::{MetisOptions, PartitionMethod};
    use crate::simulation::id::Id;
    use crate::simulation::network::global_network::{Link, Network};
    use crate::simulation::population::io::{IOLeg, IORoute};
    use crate::simulation::population::population_data::Population;
    use crate::simulation::population::{InternalLeg, InternalPerson, InternalRoute};
    use crate::simulation::vehicles::garage::Garage;
    use crate::simulation::vehicles::InternalVehicle;
    use std::collections::HashSet;
    use std::path::PathBuf;

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
        assert!(agent.selected_plan().is_some());

        let plan = agent.selected_plan().unwrap();
        assert_eq!(4, plan.acts().len());
        assert_eq!(3, plan.legs().len());

        let home_act = plan.acts().first().unwrap();
        assert_eq!("h", home_act.act_type.external());
        assert_eq!(Id::<Link>::get_from_ext("1"), home_act.link_id);
        assert_eq!(-25000., home_act.x);
        assert_eq!(0., home_act.y);
        assert_eq!(Some(6 * 3600), home_act.end_time);
        assert_eq!(None, home_act.start_time);
        assert_eq!(None, home_act.max_dur);

        let leg = plan.legs().first().unwrap();
        assert_eq!(None, leg.dep_time);
        assert!(leg.route.is_some());
        let net_route = leg.route.as_ref().unwrap().as_network().unwrap();
        assert_eq!(
            Some(Id::<InternalVehicle>::get_from_ext("1_car")),
            net_route.generic_delegate.vehicle
        );
        assert_eq!(
            vec![
                Id::<Link>::get_from_ext("1"),
                Id::<Link>::get_from_ext("6"),
                Id::<Link>::get_from_ext("15"),
                Id::<Link>::get_from_ext("20"),
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
        assert_eq!("100", Id::<InternalPerson>::get_from_ext("100").external());
        assert_eq!("200", Id::<InternalPerson>::get_from_ext("200").external());
        assert_eq!("300", Id::<InternalPerson>::get_from_ext("300").external());

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

    #[test]
    fn test_from_io_generic_route() {
        Id::<Link>::create("1");
        Id::<Link>::create("2");
        Id::<InternalVehicle>::create("person_car");

        let io_leg = IOLeg {
            mode: "car".to_string(),
            dep_time: Some("12:00:00".to_string()),
            trav_time: Some("00:30:00".to_string()),
            route: Some(IORoute {
                r#type: "generic".to_string(),
                start_link: "1".to_string(),
                end_link: "2".to_string(),
                trav_time: Some("00:20:00".to_string()),
                distance: 42.0,
                vehicle: None,
                route: None,
            }),
            attributes: None,
        };

        let leg = InternalLeg::from(io_leg);

        assert_eq!(leg.mode.external(), "car");
        assert_eq!(leg.trav_time, Some(1800));
        assert_eq!(leg.dep_time, Some(43200));
        assert_eq!(leg.routing_mode.external(), "car");
        let route = leg.route.as_ref().unwrap();
        assert!(matches!(route, InternalRoute::Generic(_)));
        assert_eq!(route.as_generic().start_link.external(), "1");
        assert_eq!(route.as_generic().end_link.external(), "2");
        assert_eq!(route.as_generic().trav_time, Some(1200));
        assert_eq!(route.as_generic().distance, Some(42.0));
        assert_eq!(
            route.as_generic().vehicle.as_ref().unwrap().external(),
            "person_car"
        );
    }

    #[test]
    fn test_from_io_pt_route() {
        Id::<Link>::create("1");
        Id::<Link>::create("2");
        Id::<String>::create("pt");
        Id::<InternalVehicle>::create("person_pt");

        let io_leg = IOLeg {
            mode: "pt".to_string(),
            dep_time: None,
            trav_time: Some("00:30:00".to_string()),
            route: Some(IORoute {
                r#type: "default_pt".to_string(),
                start_link: "1".to_string(),
                end_link: "2".to_string(),
                trav_time: Some("00:20:00".to_string()),
                distance: f64::NAN,
                vehicle: None,
                route: Some(String::from("{\"transitRouteId\":\"3to1\",\"boardingTime\":\"undefined\",\"transitLineId\":\"Blue Line\",\"accessFacilityId\":\"3\",\"egressFacilityId\":\"1\"}"))
            }),
            attributes: None,
        };

        let leg = InternalLeg::from(io_leg);

        print!("{:?}", leg);
    }
}
