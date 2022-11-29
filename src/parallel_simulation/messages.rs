use crate::parallel_simulation::splittable_population::Agent;

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
