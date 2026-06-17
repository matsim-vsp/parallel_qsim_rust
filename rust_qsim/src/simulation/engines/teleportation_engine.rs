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
use crate::simulation::time::{SimClock, SimTime, Tick};
use crate::simulation::time_queue::{Identifiable, TimeQueue};
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
        let now_time = self.clock.tick_to_time(now);
        vehicle.notify_event(&mut AgentEvent::TeleportationStarted(), now_time);

        if Simulation::is_local_route(&vehicle, net_message_broker) {
            self.queue
                .add(TeleportingVehicle::build(vehicle, now_time), now_time);
        } else {
            let to = net_message_broker.rank_for_link(
                vehicle
                    .curr_link_id()
                    .expect("Remote teleported vehicles must have a destination link"),
            );
            emit_partition_leave_events(&mut self.comp_env, &vehicle, to, now_time);
            net_message_broker.add_veh(vehicle, now);
        }
    }

    pub fn do_step(&mut self, now: Tick) -> Vec<SimulationVehicle> {
        let mut teleportation_vehicles = self.queue.pop(self.clock.tick_to_time(now));
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
        let now_time = self.clock.tick_to_time(now);
        let leg = agent.curr_leg();
        let route = leg.route.as_ref().unwrap();
        self.comp_env.events_manager_borrow_mut().process_event(
            &TeleportationArrivalEventBuilder::default()
                .time(now_time)
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
        let now_time = self.clock.tick_to_time(now);
        let leg = agent.curr_leg();
        let route = leg.route.as_ref().unwrap();
        let transit_line_id =
            Id::<String>::get_from_ext(route.as_pt().unwrap().description.transit_line_id.as_str());
        let transit_route_id = Id::<String>::get_from_ext(
            route.as_pt().unwrap().description.transit_route_id.as_str(),
        );
        self.comp_env.events_manager_borrow_mut().process_event(
            &PtTeleportationArrivalEventBuilder::default()
                .time(now_time)
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
    arrival_time: SimTime,
}

impl TeleportingVehicle {
    fn build(vehicle: SimulationVehicle, now: SimTime) -> Self {
        let arrival_time = vehicle.driver().end_time(now);
        Self {
            vehicle,
            arrival_time,
        }
    }
}

impl EndTime for TeleportingVehicle {
    fn end_time(&self, _now: SimTime) -> SimTime {
        self.arrival_time
    }
}

impl Identifiable<InternalVehicle> for TeleportingVehicle {
    fn id(&self) -> &Id<InternalVehicle> {
        self.vehicle.id()
    }
}

#[cfg(test)]
mod tests {
    use super::{TeleportationEngine, TeleportingVehicle};
    use crate::simulation::agents::SimulationAgentLogic;
    use crate::simulation::agents::agent::SimulationAgent;
    use crate::simulation::id::Id;
    use crate::simulation::scenario::Coordinate;
    use crate::simulation::scenario::population::{
        InternalActivity, InternalGenericRoute, InternalLeg, InternalPerson, InternalPlan,
        InternalRoute,
    };
    use crate::simulation::time::{SimClock, SimTime, Tick};
    use crate::simulation::vehicles::SimulationVehicle;
    use macros::integration_test;

    #[integration_test]
    fn do_step_releases_subsecond_due_vehicle() {
        let clock = SimClock::new(10);
        let mut engine = TeleportationEngine::new(Default::default(), clock);
        let vehicle = SimulationVehicle::from_parts(1, 0, 10.0, 1.0, create_generic_route_agent(1));
        let due_time = SimTime::from_nanos(350_000_000);

        engine.queue.add(
            TeleportingVehicle {
                vehicle,
                arrival_time: due_time,
            },
            SimTime::from_nanos(0),
        );

        let early = engine.do_step(Tick::new(3));
        assert!(early.is_empty());

        let ready = engine.do_step(Tick::new(4));
        assert_eq!(ready.len(), 1);
    }

    fn create_generic_route_agent(id: u64) -> SimulationAgent {
        let route = InternalRoute::Generic(InternalGenericRoute::new(
            Id::create("start"),
            Id::create("end"),
            None,
            Some(123.0),
            None,
        ));
        let leg = InternalLeg::new(
            route,
            "walk",
            std::time::Duration::default(),
            Some(SimTime::from_secs(1)),
        );
        let act = InternalActivity::new(
            Some(Coordinate::default()),
            "home",
            Id::create("start"),
            None,
            None,
            None,
        );
        let mut plan = InternalPlan::default();
        plan.add_act(act);
        plan.add_leg(leg);
        let person = InternalPerson::new(Id::create(id.to_string().as_str()), plan);
        let mut agent = SimulationAgent::new_plan_based(person);
        agent.advance_plan(SimTime::default());
        agent
    }
}
