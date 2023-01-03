use crate::parallel_simulation::splittable_population::Agent;
use std::cmp::Ordering;

#[derive(Debug)]
pub struct Message {
    // possibly, this will have more agents, once we have passengers in vehicles
    pub vehicles: Vec<(Agent, usize)>,
    pub telported: Vec<Agent>,
    pub time: u32,
    pub from: usize,
}

impl Message {
    pub fn new(from: usize) -> Message {
        Message {
            vehicles: Vec::new(),
            telported: Vec::new(),
            time: 0,
            from,
        }
    }

    pub fn add_driver(&mut self, agent: Agent, route_index: usize) {
        self.vehicles.push((agent, route_index));
    }

    pub fn add_teleported(&mut self, agent: Agent) {
        self.telported.push(agent);
    }
}

//----- Implement ordering here, so that messages can be put into a priority queue which sorts
//      ascending by time. I.e. the message with the smallest time stamp is fetched first.
impl PartialOrd for Message {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Message {
    fn cmp(&self, other: &Self) -> Ordering {
        other.time.cmp(&self.time)
    }
}

impl PartialEq<Self> for Message {
    fn eq(&self, other: &Self) -> bool {
        self.from == other.from && self.time == other.time
    }
}

impl Eq for Message {}
