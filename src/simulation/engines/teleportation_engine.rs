use crate::generated::events::Event;
use crate::simulation::agents::agent::SimulationAgent;
use crate::simulation::agents::{AgentEvent, EnvironmentalEventObserver, SimulationAgentLogic};
use crate::simulation::controller::local_controller::ComputationalEnvironment;
use crate::simulation::id::Id;
use crate::simulation::messaging::sim_communication::message_broker::NetMessageBroker;
use crate::simulation::messaging::sim_communication::SimCommunicator;
use crate::simulation::population::InternalRoute;
use crate::simulation::simulation::Simulation;
use crate::simulation::time_queue::{Identifiable, TimeQueue};
use crate::simulation::vehicles::InternalVehicle;

pub struct TeleportationEngine {
    queue: TimeQueue<InternalVehicle, InternalVehicle>,
    comp_env: ComputationalEnvironment,
}

impl TeleportationEngine {
    pub fn new(comp_env: ComputationalEnvironment) -> Self {
        TeleportationEngine {
            queue: TimeQueue::new(),
            comp_env,
        }
    }

    pub fn receive_vehicle<C: SimCommunicator>(
        &mut self,
        now: u32,
        mut vehicle: InternalVehicle,
        net_message_broker: &mut NetMessageBroker<C>,
    ) {
        vehicle.notify_event(AgentEvent::TeleportationStarted(self.comp_env.clone()), now);

        if Simulation::is_local_route(&vehicle, net_message_broker) {
            self.queue.add(vehicle, now);
        } else {
            net_message_broker.add_veh(vehicle, now);
        }
    }

    pub fn do_step(&mut self, now: u32) -> Vec<InternalVehicle> {
        let mut teleportation_vehicles = self.queue.pop(now);
        for vehicle in &mut teleportation_vehicles {
            let agent = vehicle.driver.as_ref().unwrap();

            match agent.curr_leg().route.as_ref().unwrap() {
                InternalRoute::Generic(_) => self.emit_travelled(now, agent),
                InternalRoute::Network(_) => self.emit_travelled(now, agent),
                InternalRoute::Pt(_) => self.emit_travelled_with_pt(now, agent),
            }
        }
        teleportation_vehicles
    }

    fn emit_travelled(&mut self, now: u32, agent: &SimulationAgent) {
        let leg = agent.curr_leg();
        let route = leg.route.as_ref().unwrap();
        self.comp_env.events_publisher_borrow_mut().publish_event(
            now,
            &Event::new_travelled(
                agent.id().internal(),
                route
                    .as_generic()
                    .distance()
                    .expect("Route distance needs to be set."),
                leg.mode.internal(),
            ),
        );
    }

    fn emit_travelled_with_pt(&mut self, now: u32, agent: &SimulationAgent) {
        let leg = agent.curr_leg();
        let route = leg.route.as_ref().unwrap();
        let transit_line_id =
            Id::<String>::get_from_ext(route.as_pt().unwrap().description.transit_line_id.as_str())
                .internal();
        let transit_route_id = Id::<String>::get_from_ext(
            route.as_pt().unwrap().description.transit_route_id.as_str(),
        )
        .internal();
        self.comp_env.events_publisher_borrow_mut().publish_event(
            now,
            &Event::new_travelled_with_pt(
                agent.id().internal(),
                route
                    .as_generic()
                    .distance()
                    .expect("Route distance needs to be set."),
                leg.mode.internal(),
                transit_line_id,
                transit_route_id,
            ),
        );
    }
}
