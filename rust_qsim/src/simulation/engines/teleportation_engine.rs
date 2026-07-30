use crate::simulation::Identifiable;
use crate::simulation::agents::agent::SimulationAgent;
use crate::simulation::agents::{
    AgentEvent, EndTime, EnvironmentalEventObserver, SimulationAgentLogic,
};
use crate::simulation::controller::ThreadLocalComputationalEnvironment;
use crate::simulation::engines::emit_partition_leave_events_for_agent;
use crate::simulation::events::{
    PtTeleportationArrivalEventBuilder, TeleportationArrivalEventBuilder,
};
use crate::simulation::id::Id;
use crate::simulation::messaging::sim_communication::SimCommunicator;
use crate::simulation::messaging::sim_communication::message_broker::NetMessageBroker;
use crate::simulation::scenario::population::{InternalPerson, InternalRoute};
use crate::simulation::scenario::vehicles::InternalVehicle;
use crate::simulation::simulation::Simulation;
use crate::simulation::time::{SimClock, SimTime, Tick};
use crate::simulation::time_queue::TimeQueue;

pub(crate) struct TeleportationEngine {
    queue: TimeQueue<TeleportingAgent, InternalVehicle>,
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

    pub(crate) fn drain(&mut self) -> Vec<SimulationAgent> {
        self.queue
            .drain()
            .into_iter()
            .map(|agent| agent.agent)
            .collect()
    }

    pub(crate) fn receive_agent<C: SimCommunicator>(
        &mut self,
        now: Tick,
        mut agent: SimulationAgent,
        net_message_broker: &mut NetMessageBroker<C>,
    ) {
        let now_time = self.clock.tick_to_time(now);
        agent.notify_event(&mut AgentEvent::TeleportationStarted(), now_time);

        if Simulation::is_local_route(&agent, net_message_broker) {
            self.queue
                .add(TeleportingAgent::build(agent, now_time), now_time);
        } else {
            let to = net_message_broker.rank_for_link(
                agent
                    .curr_link_id()
                    .expect("Remote teleported vehicles must have a destination link"),
            );
            emit_partition_leave_events_for_agent(&mut self.comp_env, &agent, to, now_time);
            net_message_broker.add_agent(agent, now);
        }
    }

    pub fn do_step(&mut self, now: Tick) -> Vec<SimulationAgent> {
        let mut teleportation_agents = self.queue.pop(self.clock.tick_to_time(now));
        for teleporting_agent in &mut teleportation_agents {
            let agent = &teleporting_agent.agent;

            match agent.curr_leg().route.as_ref().unwrap() {
                InternalRoute::Generic(_) => self.emit_travelled(now, agent),
                InternalRoute::Network(_) => self.emit_travelled(now, agent),
                InternalRoute::Pt(_) => self.emit_travelled_with_pt(now, agent),
            }
        }
        teleportation_agents
            .into_iter()
            .map(|vehicle| vehicle.agent)
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
        let boarding_time = route
            .as_pt()
            .unwrap()
            .description
            .boarding_time
            .expect("Boarding time needs to be set.");
        let access_facility = Id::<String>::get_from_ext(
            route
                .as_pt()
                .unwrap()
                .description
                .access_facility_id
                .as_str(),
        );
        let egress_facility = Id::<String>::get_from_ext(
            route
                .as_pt()
                .unwrap()
                .description
                .egress_facility_id
                .as_str(),
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
                .boarding_time(boarding_time)
                .access_facility(access_facility)
                .egress_facility(egress_facility)
                .build()
                .unwrap(),
        );
    }
}

struct TeleportingAgent {
    agent: SimulationAgent,
    arrival_time: SimTime,
}

impl TeleportingAgent {
    fn build(agent: SimulationAgent, now: SimTime) -> Self {
        let arrival_time = agent.end_time(now);
        Self {
            agent: agent,
            arrival_time,
        }
    }
}

impl EndTime for TeleportingAgent {
    fn end_time(&self, _now: SimTime) -> SimTime {
        self.arrival_time
    }
}

impl Identifiable<InternalPerson> for TeleportingAgent {
    fn id(&self) -> &Id<InternalPerson> {
        self.agent.id()
    }
}

#[cfg(test)]
mod tests {
    use super::{TeleportationEngine, TeleportingAgent};
    use crate::simulation::agents::SimulationAgentLogic;
    use crate::simulation::agents::agent::SimulationAgent;
    use crate::simulation::id::Id;
    use crate::simulation::scenario::Coordinate;
    use crate::simulation::scenario::population::{
        InternalActivity, InternalGenericRoute, InternalLeg, InternalPerson, InternalPlan,
        InternalRoute,
    };
    use crate::simulation::time::{SimClock, SimTime, Tick};
    use macros::deterministic_id_test;

    #[deterministic_id_test]
    fn do_step_releases_subsecond_due_vehicle() {
        let clock = SimClock::new(10);
        let mut engine = TeleportationEngine::new(Default::default(), clock);
        let agent = create_generic_route_agent(1);
        let due_time = SimTime::from_nanos(350_000_000);

        engine.queue.add(
            TeleportingAgent {
                agent,
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
