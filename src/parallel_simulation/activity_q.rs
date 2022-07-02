use crate::parallel_simulation::splittable_population::{Agent, PlanElement};
use std::cmp::Ordering;
use std::collections::BinaryHeap;

pub struct ActivityQ {
    q: BinaryHeap<QEntry>,
    finished_agents: usize,
}

impl ActivityQ {
    pub fn new() -> ActivityQ {
        ActivityQ {
            q: BinaryHeap::new(),
            finished_agents: 0,
        }
    }

    pub fn add(&mut self, agent: &Agent, now: u32) {
        let entry = QEntry::new(agent, now);

        if entry.wakeup_time >= u32::MAX {
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

    pub fn next_wakeup(&self) -> u32 {
        self.q.peek().unwrap().wakeup_time
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
    fn new(agent: &Agent, now: u32) -> QEntry {
        let element = agent.current_plan_element();
        if let PlanElement::Activity(act) = element {
            let wakeup_time = act.end_time(now);
            println!(
                "Create AgentQ.QEntry for #{} and activity: {} with wakeup_time: {wakeup_time}",
                agent.id, act.act_type
            );
            QEntry {
                agent_id: agent.id,
                wakeup_time,
            }
        } else {
            panic!("Current plan element must be an activity if this agent is inserted into ActivityQ.")
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
        if self.wakeup_time > other.wakeup_time {
            Ordering::Less
        } else if self.wakeup_time < other.wakeup_time {
            Ordering::Greater
        } else {
            Ordering::Equal
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
    use crate::parallel_simulation::activity_q::ActivityQ;
    use crate::parallel_simulation::splittable_population::{
        Activity, Agent, GenericRoute, Leg, Plan, PlanElement, Route,
    };

    #[test]
    fn add_single_agent() {
        let act = create_activity(20);
        let agent = create_agent(0, act);

        let mut activity_q = ActivityQ::new();
        activity_q.add(&agent, 0);

        let result1 = activity_q.wakeup(10);
        assert_eq!(0, result1.len());

        let result2 = activity_q.wakeup(20);
        assert_eq!(1, result2.len());
    }

    #[test]
    fn add_multiple_agents() {
        let act1 = create_activity(10);
        let act2 = create_activity(30);
        let act3 = create_activity(20);

        let agent1 = create_agent(0, act1);
        let agent2 = create_agent(1, act2);
        let agent3 = create_agent(2, act3);

        let mut act_q = ActivityQ::new();
        act_q.add(&agent1, 0);
        act_q.add(&agent2, 0);
        act_q.add(&agent3, 0);

        assert_eq!(10, act_q.next_wakeup());

        // at timestep 25, agent1 and 3 should wake up, since their activity's end time has passed.
        // agent 2 should not wake up though
        let result1 = act_q.wakeup(25);
        assert_eq!(2, result1.len());
        assert_eq!(0, *result1.get(0).unwrap());
        assert_eq!(1, *result1.get(1).unwrap());
        assert_eq!(1, act_q.q.len());
    }

    #[test]
    #[should_panic]
    fn current_element_not_activity() {
        let act = create_activity(20);
        let mut agent = create_agent(1, act);
        let leg = Leg {
            mode: String::from("test"),
            dep_time: None,
            trav_time: None,
            route: Route::GenericRoute(GenericRoute {
                start_link: 0,
                end_link: 0,
                trav_time: 0,
                distance: 0.0,
            }),
        };
        agent.plan.elements.push(PlanElement::Leg(leg));
        agent.current_element = 1;
        let mut activity_q = ActivityQ::new();

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

    fn create_agent(id: usize, activity: Activity) -> Agent {
        Agent {
            id,
            plan: Plan {
                elements: vec![PlanElement::Activity(activity)],
            },
            current_element: 0,
        }
    }
}
