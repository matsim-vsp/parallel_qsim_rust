use crate::parallel_simulation::splittable_population::Agent;

pub struct Message {
    // possibly, this will have more agents, once we have passengers in vehicles
    vehicles: Vec<(Agent, usize)>,
}

impl Message {
    pub fn new() -> Message {
        Message {
            vehicles: Vec::new(),
        }
    }

    pub fn add(&mut self, agent: Agent, route_index: usize) {
        self.vehicles.push((agent, route_index));
    }
}
