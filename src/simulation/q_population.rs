use crate::container::population::{Person, Plan, PlanElement, Population, Route};
use crate::simulation::q_network::QNetwork;
use std::cmp::{min, Ordering};
use std::collections::BinaryHeap;

struct QPopulation {
    persons: BinaryHeap<Agent>,
}

impl QPopulation {
    fn new() -> QPopulation {
        QPopulation {
            persons: BinaryHeap::new(),
        }
    }

    pub fn from_container(population: &Population, q_network: &QNetwork) -> QPopulation {
        let result = QPopulation::new();

        // go over all the persons
        for person in &population.persons {
            let plan = SimPlan::from_container(person.selected_plan(), q_network);
        }
        // take selected plan
        // convert person to agent
        // convert plan to q_plan
        result
    }
}

struct Agent {
    id: usize,
    plan: SimPlan,
    current_plan_element: usize,
    next_wakeup_time: i32,
}

impl Agent {
    fn from_container(person: &Person, q_network: &QNetwork) {
        let plan = SimPlan::from_container(person.selected_plan(), q_network);
        let firstActivity = plan.elements.get(0).unwrap();

        //match firstActivity {}
        // TODO
    }
}

impl PartialOrd for Agent {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.next_wakeup_time.cmp(&other.next_wakeup_time))
    }
}

impl Ord for Agent {
    fn cmp(&self, other: &Self) -> Ordering {
        self.next_wakeup_time.cmp(&other.next_wakeup_time)
    }
}

impl PartialEq for Agent {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for Agent {}

struct SimPlan {
    elements: Vec<SimPlanElement>,
}

impl SimPlan {
    fn from_container(plan: &Plan, q_network: &QNetwork) -> SimPlan {
        // each plan needs at least one element
        assert_eq!(1, plan.elements.len());

        // convert plan elements into sim plan elements
        let sim_elements = plan
            .elements
            .iter()
            .map(|el| match el {
                PlanElement::Activity {
                    x,
                    y,
                    link,
                    r#type,
                    start_time,
                    end_time,
                    max_dur,
                } => {
                    let link_id = q_network.link_id_mapping.get(link.as_str()).unwrap();
                    SimPlanElement::Activity {
                        x: *x,
                        y: *y,
                        act_type: r#type.clone(),
                        link_id: *link_id,
                        start_time: parse_time_opt(start_time),
                        end_time: parse_time_opt(end_time),
                        max_dur: parse_time_opt(max_dur),
                    }
                }
                PlanElement::Leg {
                    mode,
                    dep_time,
                    trav_time,
                    route,
                } => SimPlanElement::Leg {
                    mode: mode.clone(),
                    trav_time: parse_time_opt(trav_time),
                    dep_time: parse_time_opt(dep_time),
                    route: SimRoute::from_container(route, q_network),
                },
            })
            .collect();

        SimPlan {
            elements: sim_elements,
        }
    }
}

enum SimPlanElement {
    Activity {
        act_type: String,
        link_id: usize,
        x: f32,
        y: f32,
        start_time: Option<i32>,
        end_time: Option<i32>,
        max_dur: Option<i32>,
    },
    Leg {
        mode: String,
        dep_time: Option<i32>,
        trav_time: Option<i32>,
        route: SimRoute,
    },
}

struct SimRoute {}

impl SimRoute {
    fn from_container(route: &Route, q_network: &QNetwork) -> SimRoute {}
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
    // first hour
    let hour: i32 = split.get(0).unwrap().parse().unwrap();
    let minutes: i32 = split.get(1).unwrap().parse().unwrap();
    let seconds: i32 = split.get(2).unwrap().parse().unwrap();

    hour * 3600 + minutes * 60 + seconds
}

#[cfg(test)]
mod tests {
    use crate::container::population::Population;
    use crate::simulation::q_population::QPopulation;

    #[test]
    fn act_enginge_from_container() {
        let population: Population = Population::from_file("./assets/equil_output_plans.xml");
        let q_population = QPopulation::from_container(&population);
    }
}
