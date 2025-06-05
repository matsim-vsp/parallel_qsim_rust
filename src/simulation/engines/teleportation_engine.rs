use crate::simulation::id::Id;
use crate::simulation::messaging::communication::message_broker::NetMessageBroker;
use crate::simulation::messaging::communication::SimCommunicator;
use crate::simulation::messaging::events::EventsPublisher;
use crate::simulation::simulation::Simulation;
use crate::simulation::time_queue::TimeQueue;
use crate::simulation::wire_types::events::Event;
use crate::simulation::wire_types::messages::{SimulationAgent, Vehicle};
use crate::simulation::wire_types::population::leg::Route;
use crate::simulation::wire_types::population::Person;
use std::cell::RefCell;
use std::rc::Rc;

pub struct TeleportationEngine {
    queue: TimeQueue<Vehicle>,
    pub events: Rc<RefCell<EventsPublisher>>,
}

impl TeleportationEngine {
    pub fn new(events: Rc<RefCell<EventsPublisher>>) -> Self {
        TeleportationEngine {
            queue: TimeQueue::new(),
            events,
        }
    }

    pub fn receive_vehicle<C: SimCommunicator>(
        &mut self,
        now: u32,
        mut vehicle: Vehicle,
        net_message_broker: &mut NetMessageBroker<C>,
    ) {
        if Simulation::is_local_route(&vehicle, net_message_broker) {
            self.queue.add(vehicle, now);
        } else {
            // set the pointer of the route to the last element, so that the current link
            // is the destination of this leg. Setting this to the last element makes this
            // logic independent of whether the agent has a Generic-Route with only start
            // and end link or a full Network-Route, which is often the case for ride modes.
            vehicle.route_index_to_last();
            net_message_broker.add_veh(vehicle, now);
        }
    }

    pub fn do_step(&mut self, now: u32) -> Vec<Vehicle> {
        let mut teleportation_vehicles = self.queue.pop(now);
        for vehicle in &mut teleportation_vehicles {
            let agent = vehicle.driver.as_ref().unwrap();

            match agent.curr_leg().route.as_ref().unwrap() {
                Route::GenericRoute(_) => self.emit_travelled(now, &agent),
                Route::NetworkRoute(_) => self.emit_travelled(now, &agent),
                Route::PtRoute(_) => self.emit_travelled_with_pt(now, &agent),
            }

            vehicle.register_vehicle_exited();
        }
        teleportation_vehicles
    }

    fn emit_travelled(&mut self, now: u32, agent: &SimulationAgent) {
        let leg = agent.curr_leg();
        let route = leg.route.as_ref().unwrap();
        let mode: Id<String> = Id::get(leg.mode);
        self.events.borrow_mut().publish_event(
            now,
            &Event::new_travelled(
                agent.id(),
                route
                    .as_generic()
                    .distance
                    .expect("Route distance needs to be set."),
                mode.internal(),
            ),
        );
    }

    fn emit_travelled_with_pt(&mut self, now: u32, agent: &SimulationAgent) {
        let leg = agent.curr_leg();
        let route = leg.route.as_ref().unwrap();
        let mode: Id<String> = Id::get(leg.mode);
        let transit_line_id = Id::<String>::get_from_ext(
            route
                .as_pt()
                .unwrap()
                .information
                .as_ref()
                .expect("No pt route description set.")
                .transit_line_id
                .as_str(),
        )
        .internal();
        let transit_route_id = Id::<String>::get_from_ext(
            route
                .as_pt()
                .unwrap()
                .information
                .as_ref()
                .expect("No pt route description set.")
                .transit_route_id
                .as_str(),
        )
        .internal();
        self.events.borrow_mut().publish_event(
            now,
            &Event::new_travelled_with_pt(
                agent.id(),
                route
                    .as_generic()
                    .distance
                    .expect("Route distance needs to be set."),
                mode.internal(),
                transit_line_id,
                transit_route_id,
            ),
        );
    }

    pub fn agents(&self) -> Vec<&mut Person> {
        todo!()
    }
}
