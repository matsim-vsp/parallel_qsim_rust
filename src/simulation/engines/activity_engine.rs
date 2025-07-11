use crate::generated::events::Event;
use crate::simulation::agents::agent::SimulationAgent;
use crate::simulation::agents::{
    AgentEvent, EnvironmentalEventObserver, SimulationAgentLogic, WakeupEvent,
};
use crate::simulation::config::Config;
use crate::simulation::controller::ThreadLocalComputationalEnvironment;
use crate::simulation::population::InternalPerson;
use crate::simulation::time_queue::{EndTime, Identifiable, TimeQueue};

pub struct ActivityEngine {
    asleep_q: TimeQueue<AsleepSimulationAgent, InternalPerson>,
    awake_q: Vec<AwakeSimulationAgent>,
    comp_env: ThreadLocalComputationalEnvironment,
}

impl ActivityEngine {
    fn new(
        asleep_q: TimeQueue<AsleepSimulationAgent, InternalPerson>,
        awake_q: Vec<AwakeSimulationAgent>,
        comp_env: ThreadLocalComputationalEnvironment,
    ) -> Self {
        ActivityEngine {
            asleep_q,
            awake_q,
            comp_env,
        }
    }

    pub(crate) fn do_step(
        &mut self,
        now: u32,
        agents: Vec<SimulationAgent>,
    ) -> Vec<SimulationAgent> {
        for agent in agents {
            self.receive_agent(now, AsleepSimulationAgent::build(agent, now));
        }

        let mut end_after_wake_up = self.wake_up(now);
        // inform agents about wakeup
        end_after_wake_up.iter_mut().for_each(|agent| {
            ActivityEngine::inform_wakeup(self.comp_env.clone(), agent, now, now);
        });

        // inform all awake agents about wakeup
        for agent in &mut self.awake_q {
            let end_time = agent.end_time(now);
            ActivityEngine::inform_wakeup(self.comp_env.clone(), &mut agent.agent, end_time, now);
        }

        let end = self.end(now);

        let mut res = Vec::with_capacity(end_after_wake_up.len() + end.len());
        for agent in end_after_wake_up.into_iter().chain(end.into_iter()) {
            self.comp_env.events_publisher_borrow_mut().publish_event(
                now,
                &Event::new_act_end(
                    agent.id().internal(),
                    agent.curr_act().link_id.internal(),
                    agent.curr_act().act_type.internal(),
                ),
            );
            res.push(agent);
        }
        res
    }

    fn receive_agent(&mut self, now: u32, agent: AsleepSimulationAgent) {
        // emmit act start event
        let act = agent.agent.curr_act();
        self.comp_env.events_publisher_borrow_mut().publish_event(
            now,
            &Event::new_act_start(
                agent.agent.id().internal(),
                act.link_id.internal(),
                act.act_type.internal(),
            ),
        );
        self.asleep_q.add(agent, now);
    }

    fn wake_up(&mut self, now: u32) -> Vec<SimulationAgent> {
        let mut end_agents = Vec::new();
        let wake_up = self.asleep_q.pop(now);

        // for fast turnaround, agents whose end time is already reached are directly returned and not put into the awake queue
        for agent in wake_up {
            let mut awake: AwakeSimulationAgent = agent.into();
            let end_time = awake.end_time(now);
            if end_time <= now {
                end_agents.push(awake.agent);
            } else {
                self.awake_q.push(awake);
            }
        }
        end_agents
    }

    fn end(&mut self, now: u32) -> Vec<SimulationAgent> {
        let mut agents = Vec::new();

        let mut i = 0;
        while i < self.awake_q.len() {
            let agent = &self.awake_q[i];
            if agent.end_time(now) <= now {
                let removed = self.awake_q.swap_remove(i);
                agents.push(removed.agent);
            } else {
                i += 1;
            }
        }
        agents
    }

    fn inform_wakeup(
        comp_env: ThreadLocalComputationalEnvironment,
        agent: &mut SimulationAgent,
        end_time: u32,
        now: u32,
    ) {
        agent.notify_event(AgentEvent::Wakeup(WakeupEvent { comp_env, end_time }), now);
    }

    #[cfg(test)]
    fn awake_agents(&self) -> Vec<&SimulationAgent> {
        self.awake_q.iter().map(|a| &a.agent).collect()
    }
}

pub struct ActivityEngineBuilder<'c> {
    agents: Vec<SimulationAgent>,
    config: &'c Config,
    comp_env: ThreadLocalComputationalEnvironment,
}

impl<'c> ActivityEngineBuilder<'c> {
    pub fn new(
        agents: Vec<SimulationAgent>,
        config: &'c Config,
        comp_env: ThreadLocalComputationalEnvironment,
    ) -> Self {
        ActivityEngineBuilder {
            agents,
            config,
            comp_env,
        }
    }

    pub fn build(self) -> ActivityEngine {
        let now = self.config.simulation().start_time;

        let mut asleep = TimeQueue::new();
        for agent in self.agents {
            asleep.add(AsleepSimulationAgent::build(agent, now), now);
        }
        let awake_q = Vec::new();
        ActivityEngine::new(asleep, awake_q, self.comp_env)
    }
}

struct AwakeSimulationAgent {
    agent: SimulationAgent,
    begin_time: u32,
}

impl From<AsleepSimulationAgent> for AwakeSimulationAgent {
    fn from(value: AsleepSimulationAgent) -> Self {
        Self {
            agent: value.agent,
            begin_time: value.begin_time,
        }
    }
}

impl EndTime for AwakeSimulationAgent {
    fn end_time(&self, _now: u32) -> u32 {
        // Using begin_time as reference because if only max_dur is set for activity, the agent assumes that the argument of end_time is the time when the activity started.
        self.agent.end_time(self.begin_time)
    }
}

struct AsleepSimulationAgent {
    agent: SimulationAgent,
    wakeup_time: u32,
    begin_time: u32,
}

impl AsleepSimulationAgent {
    fn build(agent: SimulationAgent, now: u32) -> Self {
        let wakeup_time = agent.wakeup_time(now);
        AsleepSimulationAgent {
            agent,
            wakeup_time,
            begin_time: now,
        }
    }
}

impl EndTime for AsleepSimulationAgent {
    fn end_time(&self, _now: u32) -> u32 {
        // end_time is used for the wake-up queue, so it should return the time when the agent is supposed to wake up.
        self.wakeup_time
    }
}

#[cfg(test)]
mod tests {
    use crate::simulation::config::Config;
    use crate::simulation::engines::activity_engine::{ActivityEngine, ActivityEngineBuilder};
    use crate::simulation::id::Id;

    use crate::simulation::agents::agent::SimulationAgent;
    use crate::simulation::population::{
        InternalActivity, InternalGenericRoute, InternalLeg, InternalPerson, InternalPlan,
        InternalRoute,
    };
    use crate::simulation::time_queue::Identifiable;

    #[test]
    fn test_activity_engine_build() {
        let mut engine =
            ActivityEngineBuilder::new(vec![], &Config::default(), Default::default()).build();

        assert_eq!(engine.awake_q.len(), 0);
        assert_eq!(engine.asleep_q.len(), 0);
        engine.end(0);
    }

    #[test]
    fn test_activity_engine_wake_up_plan() {
        let plan = create_plan_with_plan_logic();

        let agent = SimulationAgent::new_plan_based(InternalPerson::new(Id::create("1"), plan));
        let agents = vec![agent];

        let mut engine = create_engine(agents);

        {
            let agents = engine.wake_up(0);
            assert!(agents.is_empty());
        }
        {
            let agents = engine.wake_up(10);
            assert_eq!(agents.len(), 1);
        }
    }

    #[test]
    fn test_activity_engine_end() {
        let plan = create_plan_with_plan_logic();

        let agent = SimulationAgent::new_plan_based(InternalPerson::new(Id::create("1"), plan));
        let agents = vec![agent];

        let mut engine = create_engine(agents);

        {
            let agents = engine.do_step(0, vec![]);
            assert!(agents.is_empty());
            assert_eq!(engine.awake_agents().len(), 0);
        }
        {
            let agents = engine.do_step(10, vec![]);
            assert_eq!(agents.len(), 1);
            assert_eq!(engine.awake_agents().len(), 0);
        }
    }

    #[test]
    fn test_activity_engine_wake_up_with_max_dur() {
        let plan = create_plan();
        test_adaptive(plan);
    }

    #[test]
    fn test_activity_engine_wake_up_with_preplanning_horizon() {
        let plan = create_plan_with_plan_logic();
        test_adaptive(plan);
    }

    fn test_adaptive(plan: InternalPlan) {
        let agent =
            SimulationAgent::new_adaptive_plan_based(InternalPerson::new(Id::create("1"), plan));
        let agents = vec![agent];
        let mut engine = create_engine(agents);
        {
            let agents = engine.do_step(0, vec![]);
            assert!(agents.is_empty());
            assert_eq!(engine.awake_agents().len(), 0);
        }
        {
            // agent is not released, but awake
            let agents = engine.do_step(5, vec![]);
            assert!(agents.is_empty());
            assert_eq!(engine.awake_agents().len(), 1);
            assert_eq!(engine.awake_agents()[0].id(), &Id::create("1"));
        }
        {
            let agents = engine.do_step(10, vec![]);
            assert_eq!(agents.len(), 1);
            assert_eq!(engine.awake_agents().len(), 0);
        }
    }

    fn create_engine(agents: Vec<SimulationAgent>) -> ActivityEngine {
        ActivityEngineBuilder::new(agents, &Config::default(), Default::default()).build()
    }

    fn create_plan_with_plan_logic() -> InternalPlan {
        let mut plan = InternalPlan::default();
        plan.add_act(InternalActivity::new(
            0.0,
            0.0,
            "act",
            Id::create("0"),
            Some(0),
            Some(10),
            None,
        ));
        let mut leg = InternalLeg::new(
            InternalRoute::Generic(InternalGenericRoute::new(
                Id::create("start"),
                Id::create("end"),
                None,
                None,
                None,
            )),
            "mode",
            1,
            Some(2),
        );
        leg.attributes
            .add(crate::simulation::population::PREPLANNING_HORIZON, 5);
        plan.add_leg(leg);
        plan.add_act(InternalActivity::new(
            0.0,
            0.0,
            "act",
            Id::create("1"),
            Some(25),
            Some(10),
            None,
        ));
        plan
    }

    fn create_plan() -> InternalPlan {
        let mut plan = InternalPlan::default();
        plan.add_act(InternalActivity::new(
            0.0,
            0.0,
            "act",
            Id::create("0"),
            None,
            None,
            Some(10),
        ));
        let mut leg = InternalLeg::new(
            InternalRoute::Generic(InternalGenericRoute::new(
                Id::create("start"),
                Id::create("end"),
                None,
                None,
                None,
            )),
            "mode",
            1,
            Some(2),
        );
        leg.attributes
            .add(crate::simulation::population::PREPLANNING_HORIZON, 5);
        plan.add_leg(leg);
        plan.add_act(InternalActivity::new(
            0.0,
            0.0,
            "act",
            Id::create("1"),
            None,
            None,
            Some(10),
        ));
        plan
    }
}
