use crate::simulation::agents::agent::SimulationAgent;
use crate::simulation::agents::{
    AgentEvent, EndTime, EnvironmentalEventObserver, SimulationAgentLogic,
};
use crate::simulation::controller::ThreadLocalComputationalEnvironment;
use crate::simulation::engines::emit_partition_leave_events;
use crate::simulation::events::{
    PtTeleportationArrivalEventBuilder, TeleportationArrivalEventBuilder,
};
use crate::simulation::id::Id;
use crate::simulation::messaging::sim_communication::SimCommunicator;
use crate::simulation::messaging::sim_communication::message_broker::NetMessageBroker;
use crate::simulation::scenario::population::InternalRoute;
use crate::simulation::scenario::vehicles::InternalVehicle;
use crate::simulation::simulation::Simulation;
use crate::simulation::time::{SimClock, Tick};
use crate::simulation::time_queue::{EndTick, Identifiable, TimeQueue};
use crate::simulation::vehicles::SimulationVehicle;

pub(crate) struct TeleportationEngine {
    queue: TimeQueue<TeleportingVehicle, InternalVehicle>,
    comp_env: ThreadLocalComputationalEnvironment,
    clock: SimClock,
}

impl TeleportationEngine {
    pub fn new(comp_env: ThreadLocalComputationalEnvironment, clock: SimClock) -> Self {
        TeleportationEngine {
            queue: TimeQueue::new(),
            comp_env,
            clock,
        }
    }

    pub fn receive_vehicle<C: SimCommunicator>(
        &mut self,
        now: Tick,
        mut vehicle: SimulationVehicle,
        net_message_broker: &mut NetMessageBroker<C>,
    ) {
        let outward_now = self.clock.tick_to_u32_seconds(now);
        vehicle.notify_event(&mut AgentEvent::TeleportationStarted(), outward_now);

        if Simulation::is_local_route(&vehicle, net_message_broker) {
            self.queue.add(
                TeleportingVehicle::build(vehicle, self.clock.tick_to_time(now), self.clock),
                now,
            );
        } else {
            let to = net_message_broker.rank_for_link(
                vehicle
                    .curr_link_id()
                    .expect("Remote teleported vehicles must have a destination link"),
            );
            emit_partition_leave_events(
                &mut self.comp_env,
                &vehicle,
                to,
                self.clock.tick_to_u32_seconds(now),
            );
            net_message_broker.add_veh(vehicle, now);
        }
    }

    pub fn do_step(&mut self, now: Tick) -> Vec<SimulationVehicle> {
        let mut teleportation_vehicles = self.queue.pop(now);
        for teleporting_vehicle in &mut teleportation_vehicles {
            let agent = teleporting_vehicle.vehicle.driver();

            match agent.curr_leg().route.as_ref().unwrap() {
                InternalRoute::Generic(_) => self.emit_travelled(now, agent),
                InternalRoute::Network(_) => self.emit_travelled(now, agent),
                InternalRoute::Pt(_) => self.emit_travelled_with_pt(now, agent),
            }
        }
        teleportation_vehicles
            .into_iter()
            .map(|vehicle| vehicle.vehicle)
            .collect()
    }

    fn emit_travelled(&mut self, now: Tick, agent: &SimulationAgent) {
        let outward_now = self.clock.tick_to_u32_seconds(now);
        let leg = agent.curr_leg();
        let route = leg.route.as_ref().unwrap();
        self.comp_env.events_manager_borrow_mut().process_event(
            &TeleportationArrivalEventBuilder::default()
                .time(outward_now)
                .person(agent.id().clone())
                .mode(leg.mode.clone())
                .distance(
                    route
                        .as_generic()
                        .distance()
                        .expect("Route distance needs to be set."),
                )
                .build()
                .unwrap(),
        );
    }

    fn emit_travelled_with_pt(&mut self, now: Tick, agent: &SimulationAgent) {
        let outward_now = self.clock.tick_to_u32_seconds(now);
        let leg = agent.curr_leg();
        let route = leg.route.as_ref().unwrap();
        let transit_line_id =
            Id::<String>::get_from_ext(route.as_pt().unwrap().description.transit_line_id.as_str());
        let transit_route_id = Id::<String>::get_from_ext(
            route.as_pt().unwrap().description.transit_route_id.as_str(),
        );
        self.comp_env.events_manager_borrow_mut().process_event(
            &PtTeleportationArrivalEventBuilder::default()
                .time(outward_now)
                .person(agent.id().clone())
                .mode(leg.mode.clone())
                .distance(
                    route
                        .as_generic()
                        .distance()
                        .expect("Route distance needs to be set."),
                )
                .line(transit_line_id)
                .route(transit_route_id)
                .build()
                .unwrap(),
        );
    }
}

struct TeleportingVehicle {
    vehicle: SimulationVehicle,
    arrival_tick: Tick,
}

impl TeleportingVehicle {
    fn build(
        vehicle: SimulationVehicle,
        now: crate::simulation::time::SimTime,
        clock: SimClock,
    ) -> Self {
        let arrival_tick = clock.time_to_tick(vehicle.driver().end_time(now));
        Self {
            vehicle,
            arrival_tick,
        }
    }
}

impl EndTick for TeleportingVehicle {
    fn end_tick(&self, _now: Tick) -> Tick {
        self.arrival_tick
    }
}

impl Identifiable<InternalVehicle> for TeleportingVehicle {
    fn id(&self) -> &Id<InternalVehicle> {
        self.vehicle.id()
    }
}
