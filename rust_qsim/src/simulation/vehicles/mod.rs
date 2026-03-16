use crate::simulation::agents::agent::SimulationAgent;
use crate::simulation::agents::{AgentEvent, EnvironmentalEventObserver};
use crate::simulation::scenario::vehicles::InternalVehicle;
use crate::simulation::time_queue::EndTime;

struct MobsimVehicle {
    vehicle: InternalVehicle,
    pub driver: Option<SimulationAgent>,
    pub passengers: Vec<SimulationAgent>,
}

impl MobsimVehicle {}

impl EnvironmentalEventObserver for InternalVehicle {
    fn notify_event(&mut self, event: &mut AgentEvent, now: u32) {
        self.driver_mut().notify_event(event, now);
        self.passengers.iter_mut().for_each(|p| {
            p.notify_event(event, now);
        });
    }
}

impl EndTime for InternalVehicle {
    fn end_time(&self, now: u32) -> u32 {
        self.driver().end_time(now)
    }
}
