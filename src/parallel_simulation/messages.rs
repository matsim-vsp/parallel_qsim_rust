use crate::parallel_simulation::splittable_population::Agent;
use crate::parallel_simulation::vehicles::Vehicle;

pub enum Message<'a> {
    Travelling(TravellingMessage<'a>),
}

pub struct TravellingMessage<'a> {
    // possibly, this will have more agents, once we have passengers in vehicles
    vehicles: Vec<(Agent, Vehicle<'a>)>,
}
