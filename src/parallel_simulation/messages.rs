use crate::parallel_simulation::splittable_population::Agent;
use crate::simulation::q_vehicle::QVehicle;

pub struct Message {
    from_thread: usize,
    to_thread: usize,
    vehicles: Vec<QVehicle>,
    teleported_agents: Vec<Agent>,
}
