use crate::container::population::{Activity, Leg, Person, Plan, PlanElement, Population, Route};
use crate::simulation::q_network::QNetwork;

#[derive(Debug)]
pub struct QPopulation {
    pub agents: Vec<Agent>,
}

impl QPopulation {
    fn new() -> QPopulation {
        QPopulation { agents: Vec::new() }
    }

    pub fn from_container(population: &Population, q_network: &QNetwork) -> QPopulation {
        let mut result = QPopulation::new();

        // go over all the persons
        for person in &population.persons {
            let next_id = result.agents.len();
            let agent = Agent::from_container(person, next_id, q_network);
            result.agents.push(agent);
        }
        result
    }
}

#[derive(Debug)]
pub struct Agent {
    pub id: usize,
    pub plan: SimPlan,
    pub current_element: usize,
}

impl Agent {
    fn from_container(person: &Person, id: usize, q_network: &QNetwork) -> Agent {
        let plan = SimPlan::from_container(person.selected_plan(), q_network);

        Agent {
            id,
            plan,
            current_element: 0,
        }
    }

    pub fn current_plan_element(&self) -> &SimPlanElement {
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
        self.current_element = next_element;
    }
}

#[derive(Debug)]
pub struct SimPlan {
    pub elements: Vec<SimPlanElement>,
}

impl SimPlan {
    fn from_container(plan: &Plan, q_network: &QNetwork) -> SimPlan {
        // each plan needs at least one element
        assert!(plan.elements.len() > 0);
        if let PlanElement::Leg(_leg) = plan.elements.get(0).unwrap() {
            panic!("First plan element must be an activity! But was a leg.");
        }

        // convert plan elements into sim plan elements
        let sim_elements = plan
            .elements
            .iter()
            .map(|el| SimPlan::map_plan_element(el, q_network))
            .collect();

        SimPlan {
            elements: sim_elements,
        }
    }

    fn map_plan_element(element: &PlanElement, q_network: &QNetwork) -> SimPlanElement {
        match element {
            PlanElement::Activity(activity) => {
                SimPlanElement::Activity(SimActivity::from_container(activity, q_network))
            }
            PlanElement::Leg(leg) => SimPlanElement::Leg(SimLeg::from_container(leg, q_network)),
        }
    }
}

#[derive(Debug)]
pub enum SimPlanElement {
    Activity(SimActivity),
    Leg(SimLeg),
}

#[derive(Debug)]
pub struct SimActivity {
    pub act_type: String,
    pub link_id: usize,
    pub x: f32,
    pub y: f32,
    pub start_time: Option<i32>,
    pub end_time: Option<i32>,
    pub max_dur: Option<i32>,
}

impl SimActivity {
    fn from_container(activity: &Activity, q_network: &QNetwork) -> SimActivity {
        let link_id = q_network
            .link_id_mapping
            .get(activity.link.as_str())
            .unwrap();
        SimActivity {
            x: activity.x,
            y: activity.y,
            act_type: activity.r#type.clone(),
            link_id: *link_id,
            start_time: parse_time_opt(&activity.start_time),
            end_time: parse_time_opt(&activity.end_time),
            max_dur: parse_time_opt(&activity.max_dur),
        }
    }

    /**
    Calculates the end time of this activity. This only implements
    org.matsim.core.config.groups.PlansConfigGroup.ActivityDurationInterpretation.tryEndTimeThenDuration
     */
    pub fn end_time(&self, now: i32) -> i32 {
        if let Some(end_time) = self.end_time {
            end_time
        } else if let Some(max_dur) = self.max_dur {
            now + max_dur
        } else {
            // supposed to be an equivalent for OptionalTime.undefined() in the java code
            i32::MAX
        }
    }
}

#[derive(Debug)]
pub struct SimLeg {
    pub mode: String,
    pub dep_time: Option<i32>,
    pub trav_time: Option<i32>,
    pub route: SimRoute,
}

impl SimLeg {
    fn from_container(leg: &Leg, q_network: &QNetwork) -> SimLeg {
        let sim_route = SimLeg::map_route(&leg.route, q_network);

        SimLeg {
            mode: leg.mode.clone(),
            trav_time: parse_time_opt(&leg.trav_time),
            dep_time: parse_time_opt(&leg.dep_time),
            route: sim_route,
        }
    }

    fn map_route(route: &Route, q_network: &QNetwork) -> SimRoute {
        match route.r#type.as_str() {
            "generic" => SimRoute::GenericRoute(GenericRoute::from_container(route, q_network)),
            "links" => SimRoute::NetworkRoute(NetworkRoute::from_container(route, q_network)),
            _ => panic!("Unsupported route type: '{}'", route.r#type),
        }
    }
}

#[derive(Debug)]
pub enum SimRoute {
    NetworkRoute(NetworkRoute),
    GenericRoute(GenericRoute),
}

#[derive(Debug)]
pub struct GenericRoute {
    pub start_link: usize,
    pub end_link: usize,
    pub trav_time: i32,
    pub distance: f32,
}

impl GenericRoute {
    fn from_container(route: &Route, q_network: &QNetwork) -> GenericRoute {
        let start_link_id = q_network
            .link_id_mapping
            .get(route.start_link.as_str())
            .unwrap();
        let end_link_id = q_network
            .link_id_mapping
            .get(route.end_link.as_str())
            .unwrap();
        let trav_time = parse_time_opt(&route.trav_time).unwrap();

        GenericRoute {
            start_link: *start_link_id,
            end_link: *end_link_id,
            trav_time: trav_time,
            distance: route.distance,
        }
    }
}

#[derive(Debug)]
pub struct NetworkRoute {
    pub vehicle_id: String,
    pub route: Vec<usize>,
}

impl NetworkRoute {
    fn from_container(route: &Route, q_network: &QNetwork) -> NetworkRoute {
        let link_ids: Vec<usize> = route
            .route
            .as_ref()
            .unwrap()
            .split(' ')
            .map(|id| *q_network.link_id_mapping.get(id).unwrap())
            .collect();

        let vehicle_id = route.vehicle.as_ref().unwrap();

        NetworkRoute {
            vehicle_id: vehicle_id.clone(),
            route: link_ids,
        }
    }
}

fn parse_time_opt(value: &Option<String>) -> Option<i32> {
    match value {
        None => None,
        Some(value) => Some(parse_time(value)),
    }
}

fn parse_time(value: &str) -> i32 {
    let split: Vec<&str> = value.split(':').collect();
    assert_eq!(3, split.len());

    let hour: i32 = split.get(0).unwrap().parse().unwrap();
    let minutes: i32 = split.get(1).unwrap().parse().unwrap();
    let seconds: i32 = split.get(2).unwrap().parse().unwrap();

    hour * 3600 + minutes * 60 + seconds
}

#[cfg(test)]
mod tests {
    use crate::container::network::Network;
    use crate::container::population::Population;
    use crate::simulation::q_network::QNetwork;
    use crate::simulation::q_population::QPopulation;

    #[test]
    fn population_from_container() {
        let population: Population = Population::from_file("./assets/equil_output_plans.xml.gz");
        let network: Network = Network::from_file("./assets/equil-network.xml");
        let q_network: QNetwork = QNetwork::from_container(&network);
        let q_population = QPopulation::from_container(&population, &q_network);

        println!("{q_population:#?}");
    }
}
