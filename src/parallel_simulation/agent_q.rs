use crate::parallel_simulation::splittable_population::{Activity, Agent, Leg, PlanElement, Route};
use std::cmp::Ordering;
use std::collections::BinaryHeap;

pub struct AgentQ {
    q: BinaryHeap<QEntry>,
    finished_agents: usize,
}

impl AgentQ {
    pub fn new() -> AgentQ {
        AgentQ {
            q: BinaryHeap::new(),
            finished_agents: 0,
        }
    }

    pub fn add(&mut self, agent: &Agent, now: u32) {
        let entry = QEntry::new(agent, now);

        if entry.wakeup_time == u32::MAX {
            self.finished_agents += 1;
        }

        self.q.push(entry);
    }

    pub fn wakeup(&mut self, now: u32) -> Vec<usize> {
        let mut result: Vec<usize> = Vec::new();

        while let Some(entry_ref) = self.q.peek() {
            if entry_ref.wakeup_time <= now {
                let entry = self.q.pop().unwrap();
                result.push(entry.agent_id);
            } else {
                break;
            }
        }
        result
    }

    pub fn next_wakeup(&self) -> Option<u32> {
        self.q.peek().map(|entry| entry.wakeup_time)
    }

    pub fn finished_agents(&self) -> usize {
        self.finished_agents
    }
}

struct QEntry {
    wakeup_time: u32,
    agent_id: usize,
}

impl QEntry {
    /// Creates an Entry with a wakeup time. If the current plan element is an Activity, the activities
    /// end time is used. Otherwise, if it is a Generic Leg (teleported) the trav_time of that leg is
    /// used.
    ///
    /// Note: I think it would be better to have this guarded by the type system somehow. But this
    /// was the quickest way
    fn new(agent: &Agent, now: u32) -> QEntry {
        let element = agent.current_plan_element();
        match element {
            PlanElement::Activity(act) => QEntry::from_activity(agent, act, now),
            PlanElement::Leg(leg) => QEntry::from_leg(agent, leg, now),
        }
    }

    fn from_activity(agent: &Agent, activity: &Activity, now: u32) -> QEntry {
        let wakeup_time = activity.end_time(now);
        println!(
            "Create AgentQ.QEntry for #{} and activity: {} with wakeup_time: {wakeup_time}",
            agent.id, activity.act_type
        );
        QEntry {
            agent_id: agent.id,
            wakeup_time,
        }
    }

    fn from_leg(agent: &Agent, leg: &Leg, now: u32) -> QEntry {
        if let Route::GenericRoute(route) = &leg.route {
            let wakeup_time = now + route.trav_time;
            QEntry {
                agent_id: agent.id,
                wakeup_time,
            }
        } else {
            panic!("AgentQ can only hold agents for teleported legs. Found network leg.")
        }
    }
}

impl PartialOrd for QEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/**
This orders entries in reverse orders according to their wakeup time. The element with the earlier
wakeup time is considered to be greater.
 */
impl Ord for QEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.wakeup_time.cmp(&other.wakeup_time) {
            Ordering::Less => Ordering::Greater,
            Ordering::Equal => Ordering::Equal,
            Ordering::Greater => Ordering::Less,
        }
    }
}

impl PartialEq<Self> for QEntry {
    fn eq(&self, other: &Self) -> bool {
        self.agent_id == other.agent_id
    }
}

impl Eq for QEntry {}

#[cfg(test)]
mod tests {
    use crate::parallel_simulation::agent_q::AgentQ;
    use crate::parallel_simulation::splittable_population::{
        Activity, Agent, GenericRoute, Leg, NetworkRoute, Plan, PlanElement, Route,
    };

    #[test]
    fn add_single_agent_with_activities() {
        let act = create_activity(20);
        let agent = create_agent(0, PlanElement::Activity(act));

        let mut activity_q = AgentQ::new();
        activity_q.add(&agent, 0);

        let result1 = activity_q.wakeup(10);
        assert_eq!(0, result1.len());

        let result2 = activity_q.wakeup(20);
        assert_eq!(1, result2.len());
    }

    #[test]
    fn add_multiple_agents_with_activities() {
        let act1 = create_activity(10);
        let act2 = create_activity(30);
        let act3 = create_activity(20);

        let agent1 = create_agent(0, PlanElement::Activity(act1));
        let agent2 = create_agent(1, PlanElement::Activity(act2));
        let agent3 = create_agent(2, PlanElement::Activity(act3));

        let mut act_q = AgentQ::new();
        act_q.add(&agent1, 0);
        act_q.add(&agent2, 0);
        act_q.add(&agent3, 0);

        assert_eq!(10, act_q.next_wakeup().unwrap());

        // at timestep 25, agent1 and 3 should wake up, since their activity's end time has passed.
        // agent 2 should not wake up though
        let result1 = act_q.wakeup(25);
        assert_eq!(2, result1.len());
        assert_eq!(0, *result1.get(0).unwrap());
        assert_eq!(2, *result1.get(1).unwrap());
        assert_eq!(1, act_q.q.len());
    }

    #[test]
    fn add_single_agent_with_leg() {
        let leg = create_leg(20);
        let agent = create_agent(0, PlanElement::Leg(leg));
        let mut q = AgentQ::new();

        q.add(&agent, 0);

        let result1 = q.wakeup(10);
        assert_eq!(0, result1.len());

        let result2 = q.wakeup(20);
        assert_eq!(1, result2.len());
    }

    #[test]
    #[should_panic]
    fn current_element_not_activity_or_generic_route() {
        let act = create_activity(20);
        let mut agent = create_agent(1, PlanElement::Activity(act));
        let leg = Leg {
            mode: String::from("test"),
            dep_time: None,
            trav_time: None,
            route: Route::NetworkRoute(NetworkRoute {
                route: Vec::new(),
                vehicle_id: 1,
            }),
        };
        agent.plan.elements.push(PlanElement::Leg(leg));
        agent.current_element = 1;
        let mut activity_q = AgentQ::new();

        activity_q.add(&agent, 0);
    }

    fn create_activity(end_time: u32) -> Activity {
        Activity {
            x: 0.0,
            y: 0.0,
            end_time: Some(end_time),
            start_time: None,
            max_dur: None,
            act_type: String::from("test"),
            link_id: 0,
        }
    }

    fn create_leg(trav_time: u32) -> Leg {
        Leg {
            route: Route::GenericRoute(GenericRoute {
                start_link: 1,
                end_link: 2,
                trav_time,
                distance: 100.,
            }),
            trav_time: Some(trav_time),
            dep_time: None,
            mode: String::from("test"),
        }
    }

    fn create_agent(id: usize, element: PlanElement) -> Agent {
        Agent {
            id,
            plan: Plan {
                elements: vec![element],
            },
            current_element: 0,
        }
    }
}
