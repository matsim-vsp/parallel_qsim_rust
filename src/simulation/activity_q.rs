use std::cmp::Ordering;
use std::collections::BinaryHeap;

use crate::simulation::q_population::{Agent, SimPlanElement};

pub struct ActivityQ<'a> {
    q: BinaryHeap<QEntry<'a>>,
    next_wakeup_time: i32,
}

impl<'a> ActivityQ<'a> {
    pub fn new() -> ActivityQ<'a> {
        ActivityQ {
            q: BinaryHeap::new(),
            next_wakeup_time: i32::MAX,
        }
    }

    pub fn add(&mut self, agent: &'a Agent, now: i32) {
        let entry = QEntry::new(agent, now);
        if entry.wakeup_time < self.next_wakeup_time {
            self.next_wakeup_time = entry.wakeup_time;
        }
        self.q.push(entry);
    }

    pub fn wakeup(&mut self, now: i32) -> Vec<&'a Agent> {
        let mut result: Vec<&Agent> = Vec::new();

        while let Some(entry) = self.q.peek() {
            if entry.wakeup_time <= now {
                let entry = self.q.pop().unwrap();
                result.push(entry.agent);
            } else {
                self.next_wakeup_time = entry.wakeup_time;
                break;
            }
        }
        result
    }
}

struct QEntry<'a> {
    wakeup_time: i32,
    agent: &'a Agent,
}

impl<'a> QEntry<'a> {
    fn new(agent: &Agent, now: i32) -> QEntry {
        let element = agent.current_plan_element();
        if let SimPlanElement::Activity(act) = element {
            let wakeup_time = act.end_time(now);
            QEntry { agent, wakeup_time }
        } else {
            panic!("Current plan element must be an activity if this agent is inserted into ActivityQ.")
        }
    }
}

impl<'a> PartialOrd for QEntry<'a> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/**
This orders entries in reverse orders according to their wakeup time. The element with the earlier
wakeup time is considered to be greater.
 */
impl<'a> Ord for QEntry<'a> {
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

impl<'a> PartialEq<Self> for QEntry<'a> {
    fn eq(&self, other: &Self) -> bool {
        self.agent.id == other.agent.id
    }
}

impl<'a> Eq for QEntry<'a> {}

#[cfg(test)]
mod tests {
    use crate::simulation::activity_q::ActivityQ;
    use crate::simulation::q_population::{
        Agent, GenericRoute, SimActivity, SimLeg, SimPlan, SimPlanElement, SimRoute,
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

        assert_eq!(10, act_q.next_wakeup_time);

        // at timestep 25, agent1 and 3 should wake up, since their activity's end time has passed.
        // agent 2 should not wake up though
        let result1 = act_q.wakeup(25);
        assert_eq!(2, result1.len());
        assert_eq!(agent1.id, result1.get(0).unwrap().id);
        assert_eq!(agent3.id, result1.get(1).unwrap().id);
        assert_eq!(1, act_q.q.len());
    }

    #[test]
    #[should_panic]
    fn current_element_not_activity() {
        let act = create_activity(20);
        let mut agent = create_agent(1, act);
        let leg = SimLeg {
            mode: String::from("test"),
            dep_time: None,
            trav_time: None,
            route: SimRoute::GenericRoute(GenericRoute {
                start_link: 0,
                end_link: 0,
                trav_time: 0,
                distance: 0.0,
            }),
        };
        agent.plan.elements.push(SimPlanElement::Leg(leg));
        agent.current_element = 1;
        let mut activity_q = ActivityQ::new();

        activity_q.add(&agent, 0);
    }

    fn create_activity(end_time: i32) -> SimActivity {
        SimActivity {
            x: 0.0,
            y: 0.0,
            end_time: Some(end_time),
            start_time: None,
            max_dur: None,
            act_type: String::from("test"),
            link_id: 0,
        }
    }

    fn create_agent(id: usize, activity: SimActivity) -> Agent {
        Agent {
            id,
            plan: SimPlan {
                elements: vec![SimPlanElement::Activity(activity)],
            },
            current_element: 0,
        }
    }
}
