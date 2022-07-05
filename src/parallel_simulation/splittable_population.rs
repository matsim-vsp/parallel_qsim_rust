use crate::container::population::{
    IOActivity, IOLeg, IOPerson, IOPlan, IOPlanElement, IOPopulation, IORoute,
};
use crate::parallel_simulation::id_mapping::IdMapping;

use crate::parallel_simulation::vehicles::VehiclesIdMapping;
use std::collections::HashMap;

pub struct Population {
    pub agents: HashMap<usize, Agent>,
}

impl Population {
    fn new() -> Population {
        Population {
            agents: HashMap::new(),
        }
    }

    fn add_agent(&mut self, agent: Agent) {
        self.agents.insert(agent.id, agent);
    }

    pub fn split_from_container(
        container: &IOPopulation,
        size: usize,
        link_id_mapping: &IdMapping,
        vehicle_id_mapping: &VehiclesIdMapping,
    ) -> (Vec<Population>, IdMapping) {
        let mut next_id = 0;
        let mut populations: Vec<Population> = Vec::with_capacity(size);
        let mut id_mapping = IdMapping::new();

        for _i in 0..size {
            populations.push(Population::new());
        }

        for person in &container.persons {
            let agent = Agent::from_person(person, next_id, link_id_mapping, vehicle_id_mapping);

            if let PlanElement::Activity(act) = agent.current_plan_element() {
                let thread = link_id_mapping.get_thread(&act.link_id);
                let population = populations.get_mut(thread).unwrap();
                population.add_agent(agent);
                id_mapping.insert(next_id, thread, person.id.clone());
            }
            next_id += 1;
        }

        (populations, id_mapping)
    }
}

pub struct Agent {
    pub id: usize,
    pub plan: Plan,
    pub current_element: usize,
}

impl Agent {
    fn from_person(
        person: &IOPerson,
        id: usize,
        link_id_mapping: &IdMapping,
        vehicle_id_mapping: &VehiclesIdMapping,
    ) -> Agent {
        let plan = Plan::from_io_plan(person.selected_plan(), link_id_mapping, vehicle_id_mapping);

        Agent {
            id,
            plan,
            current_element: 0,
        }
    }

    pub fn current_plan_element(&self) -> &PlanElement {
        self.plan.elements.get(self.current_element).unwrap()
    }

    pub fn advance_plan(&mut self) {
        let next_element = self.current_element + 1;
        if self.plan.elements.len() == next_element {
            panic!(
                "Advance plan was called on agent #{}, but no element is remaining.",
                self.id
            )
        }
        println!(
            "Agent #{} advancing plan from index {} to index {}",
            self.id, self.current_element, next_element
        );
        self.current_element = next_element;
    }
}
pub struct Plan {
    pub elements: Vec<PlanElement>,
}

impl Plan {
    fn from_io_plan(
        plan: &IOPlan,
        link_id_mapping: &IdMapping,
        vehicle_id_mapping: &VehiclesIdMapping,
    ) -> Plan {
        // each plan needs at least one element
        assert!(plan.elements.len() > 0);
        if let IOPlanElement::Leg(_leg) = plan.elements.get(0).unwrap() {
            panic!("First plan element must be an activity! But was a leg.");
        }

        let elements = plan
            .elements
            .iter()
            .map(|el| PlanElement::from_io_element(el, link_id_mapping, vehicle_id_mapping))
            .collect();

        Plan { elements }
    }
}

pub enum PlanElement {
    Activity(Activity),
    Leg(Leg),
}

impl PlanElement {
    fn from_io_element(
        element: &IOPlanElement,
        link_id_mapping: &IdMapping,
        vehicle_id_mapping: &VehiclesIdMapping,
    ) -> PlanElement {
        match element {
            IOPlanElement::Activity(a) => {
                PlanElement::Activity(Activity::from_io_activity(a, link_id_mapping))
            }
            IOPlanElement::Leg(l) => {
                PlanElement::Leg(Leg::from_io_leg(l, link_id_mapping, vehicle_id_mapping))
            }
        }
    }
}

pub struct Activity {
    pub act_type: String,
    pub link_id: usize,
    pub x: f32,
    pub y: f32,
    pub start_time: Option<u32>,
    pub end_time: Option<u32>,
    pub max_dur: Option<u32>,
}

impl Activity {
    fn from_io_activity(activity: &IOActivity, link_id_mapping: &IdMapping) -> Activity {
        let link_id = link_id_mapping.get_from_matsim_id(activity.link.as_str());
        Activity {
            x: activity.x,
            y: activity.y,
            act_type: activity.r#type.clone(),
            link_id,
            start_time: parse_time_opt(&activity.start_time),
            end_time: parse_time_opt(&activity.end_time),
            max_dur: parse_time_opt(&activity.max_dur),
        }
    }

    /**
    Calculates the end time of this activity. This only implements
    org.matsim.core.config.groups.PlansConfigGroup.ActivityDurationInterpretation.tryEndTimeThenDuration
     */
    pub fn end_time(&self, now: u32) -> u32 {
        if let Some(end_time) = self.end_time {
            end_time
        } else if let Some(max_dur) = self.max_dur {
            now + max_dur
        } else {
            // supposed to be an equivalent for OptionalTime.undefined() in the java code
            u32::MAX
        }
    }
}

pub struct Leg {
    pub mode: String,
    pub dep_time: Option<u32>,
    pub trav_time: Option<u32>,
    pub route: Route,
}

impl Leg {
    fn from_io_leg(
        leg: &IOLeg,
        link_id_mapping: &IdMapping,
        vehicle_id_mapping: &VehiclesIdMapping,
    ) -> Leg {
        let route = Route::from_io_route(&leg.route, link_id_mapping, vehicle_id_mapping);

        Leg {
            mode: leg.mode.clone(), // this should be different
            trav_time: parse_time_opt(&leg.trav_time),
            dep_time: parse_time_opt(&leg.dep_time),
            route,
        }
    }
}

pub enum Route {
    NetworkRoute(NetworkRoute),
    GenericRoute(GenericRoute),
}

impl Route {
    fn from_io_route(
        route: &IORoute,
        link_id_mapping: &IdMapping,
        vehicle_id_mapping: &VehiclesIdMapping,
    ) -> Route {
        match route.r#type.as_str() {
            "generic" => Route::GenericRoute(GenericRoute::from_io_route(route, link_id_mapping)),
            "links" => Route::NetworkRoute(NetworkRoute::from_io_route(
                route,
                link_id_mapping,
                vehicle_id_mapping,
            )),
            _ => panic!("Unsupported route type: '{}'", route.r#type),
        }
    }
}

pub struct GenericRoute {
    pub start_link: usize,
    pub end_link: usize,
    pub trav_time: u32,
    pub distance: f32,
}

impl GenericRoute {
    fn from_io_route(route: &IORoute, link_id_mapping: &IdMapping) -> GenericRoute {
        let start_link = link_id_mapping.get_from_matsim_id(route.start_link.as_str());
        let end_link = link_id_mapping.get_from_matsim_id(route.end_link.as_str());
        let trav_time = parse_time_opt(&route.trav_time).unwrap();

        GenericRoute {
            start_link,
            end_link,
            trav_time,
            distance: route.distance,
        }
    }
}

pub struct NetworkRoute {
    pub vehicle_id: usize,
    pub route: Vec<usize>,
}

impl NetworkRoute {
    // this could probably be implemented via from<t> trait.
    fn from_io_route(
        route: &IORoute,
        link_id_mapping: &IdMapping,
        vehicle_id_mapping: &VehiclesIdMapping,
    ) -> NetworkRoute {
        if let Some(ref encoded_links) = route.route {
            if let Some(ref matsim_veh_id) = route.vehicle {
                let link_ids = encoded_links
                    .split(' ')
                    .map(|id| link_id_mapping.get_from_matsim_id(id))
                    .collect();

                let vehicle_id = vehicle_id_mapping.get_from_matsim_id(matsim_veh_id);

                return NetworkRoute {
                    vehicle_id,
                    route: link_ids,
                };
            }
        }

        panic!("Couldn't create NetworkRoute from route: {route:#?}");
    }
}

fn parse_time_opt(value: &Option<String>) -> Option<u32> {
    match value {
        None => None,
        Some(value) => Some(parse_time(value)),
    }
}

fn parse_time(value: &str) -> u32 {
    let split: Vec<&str> = value.split(':').collect();
    assert_eq!(3, split.len());

    let hour: u32 = split.get(0).unwrap().parse().unwrap();
    let minutes: u32 = split.get(1).unwrap().parse().unwrap();
    let seconds: u32 = split.get(2).unwrap().parse().unwrap();

    hour * 3600 + minutes * 60 + seconds
}
