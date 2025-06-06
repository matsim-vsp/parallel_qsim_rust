use std::cell::RefCell;
use std::fmt::Debug;
use std::fmt::Formatter;
use std::rc::Rc;

use crate::simulation::config::Config;
use crate::simulation::engines::activity_engine::{ActivityEngine, ActivityEngineBuilder};
use crate::simulation::engines::leg_engine::LegEngine;
use crate::simulation::messaging::communication::message_broker::NetMessageBroker;
use crate::simulation::messaging::communication::SimCommunicator;
use crate::simulation::messaging::events::EventsPublisher;
use crate::simulation::population::agent_source::{AgentSource, PopulationAgentSource};
use crate::simulation::scenario::Scenario;
use crate::simulation::wire_types::messages::{SimulationAgent, Vehicle};
use tracing::info;

pub struct Simulation<C: SimCommunicator> {
    activity_engine: ActivityEngine,
    leg_engine: LegEngine<C>,
    events: Rc<RefCell<EventsPublisher>>,
    start_time: u32,
    end_time: u32,
}

impl<C> Simulation<C>
where
    C: SimCommunicator,
{
    #[tracing::instrument(level = "info", skip(self), fields(rank = self.leg_engine.net_message_broker().rank()))]
    pub fn run(&mut self) {
        // use fixed start and end times
        let mut now = self.start_time;
        info!(
            "Starting #{}. Network neighbors: {:?}, Start time {}, End time {}",
            self.leg_engine.net_message_broker().rank(),
            self.leg_engine.network().neighbors(),
            self.start_time,
            self.end_time,
        );

        let mut agents_changing_engine = vec![];

        while now <= self.end_time {
            if now % 3600 == 0 {
                let _hour = now / 3600;
                let _min = (now % 3600) / 60;
                info!(
                    "#{} of Qsim at {_hour:02}:{_min:02}; Active Nodes: {}, Active Links: {}, Vehicles on Network Partition: {}",
                    self.leg_engine.net_message_broker().rank(),
                    self.leg_engine.network().active_nodes(),
                    self.leg_engine.network().active_links(),
                    self.leg_engine.network().veh_on_net()
                );
            }

            agents_changing_engine = self.do_sim_step(now, agents_changing_engine);
            now += 1;
        }

        // maybe this belongs into the controller? Then this would have to be a &mut instead of owned.
        self.events.borrow_mut().finish();
    }

    fn do_sim_step(&mut self, now: u32, agents: Vec<SimulationAgent>) -> Vec<SimulationAgent> {
        let mut agents_act_to_leg = self.activity_engine.do_step(now, agents);
        for agent in &mut agents_act_to_leg {
            agent.advance_plan();
        }
        let mut agents_leg_to_act = self.leg_engine.do_step(now, agents_act_to_leg);
        for agent in &mut agents_leg_to_act {
            agent.advance_plan();
        }
        agents_leg_to_act
    }

    pub(crate) fn is_local_route(veh: &Vehicle, message_broker: &NetMessageBroker<C>) -> bool {
        let leg = veh.driver.as_ref().unwrap().curr_leg();
        let route = leg.route.as_ref().unwrap();
        let to = message_broker.rank_for_link(route.end_link());
        message_broker.rank() == to
    }
}

impl<C: SimCommunicator + 'static> Debug for Simulation<C> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Simulation with Rank #{}",
            self.leg_engine.net_message_broker().rank()
        )
    }
}

pub struct SimulationBuilder<C: SimCommunicator> {
    config: Config,
    scenario: Scenario,
    net_message_broker: NetMessageBroker<C>,
    events: Rc<RefCell<EventsPublisher>>,
}

impl<C: SimCommunicator> SimulationBuilder<C> {
    pub fn new(
        config: Config,
        scenario: Scenario,
        net_message_broker: NetMessageBroker<C>,
        events: Rc<RefCell<EventsPublisher>>,
    ) -> Self {
        SimulationBuilder {
            config,
            scenario,
            net_message_broker,
            events,
        }
    }

    pub fn build(mut self) -> Simulation<C> {
        // this needs to be adapted if new agent sources are introduced
        let agent_source = PopulationAgentSource {};
        let agents = agent_source.create_agents(&mut self.scenario, &self.config);

        let activity_engine = ActivityEngineBuilder::new(
            agents.into_values().collect(),
            self.events.clone(),
            &self.config,
        )
        .build();

        let leg_engine = LegEngine::new(
            self.scenario.network_partition,
            self.scenario.garage,
            self.net_message_broker,
            self.events.clone(),
            &self.config.simulation(),
        );

        Simulation {
            activity_engine,
            leg_engine,
            events: self.events,
            start_time: self.config.simulation().start_time,
            end_time: self.config.simulation().end_time,
        }
    }
}
