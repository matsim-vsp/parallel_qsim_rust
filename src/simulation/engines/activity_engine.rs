use crate::simulation::config::Config;
use crate::simulation::messaging::events::EventsPublisher;
use crate::simulation::time_queue::{EndTime, TimeQueue};
use crate::simulation::wire_types::events::Event;
use crate::simulation::wire_types::messages::SimulationAgent;
use std::cell::RefCell;
use std::rc::Rc;

pub struct ActivityEngine {
    asleep_q: TimeQueue<AsleepSimulationAgent>,
    awake_q: Vec<AsleepSimulationAgent>,
    events: Rc<RefCell<EventsPublisher>>,
}

impl ActivityEngine {
    fn new(
        asleep_q: TimeQueue<AsleepSimulationAgent>,
        awake_q: Vec<AsleepSimulationAgent>,
        events: Rc<RefCell<EventsPublisher>>,
    ) -> Self {
        ActivityEngine {
            asleep_q,
            awake_q,
            events,
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

        let wake_up = self.wake_up(now);
        self.inform(now);
        let end = self.end(now);

        let mut res = Vec::with_capacity(wake_up.len() + end.len());
        for agent in wake_up.into_iter().chain(end.into_iter()) {
            self.events.borrow_mut().publish_event(
                now,
                &Event::new_act_end(
                    agent.id(),
                    agent.curr_act().link_id,
                    agent.curr_act().act_type,
                ),
            );
            res.push(agent);
        }
        res
    }

    fn receive_agent(&mut self, now: u32, agent: AsleepSimulationAgent) {
        // emmit act start event
        let act = agent.agent.curr_act();
        self.events.borrow_mut().publish_event(
            now,
            &Event::new_act_start(agent.agent.id(), act.link_id, act.act_type),
        );
        self.asleep_q.add(agent, now);
    }

    fn wake_up(&mut self, now: u32) -> Vec<SimulationAgent> {
        let mut end_agents = Vec::new();
        let wake_up = self.asleep_q.pop(now);

        // for fast turnaround, agents whose end time is already reached are directly returned and not put into the awake queue
        for agent in wake_up {
            if agent.end_time(now) <= now {
                end_agents.push(agent.agent);
            } else {
                self.awake_q.push(agent);
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

    fn inform(&mut self, _now: u32) {
        // Go through all awakened agents and inform them about current time.
        // Provide structs that are needed for replanning.
    }
}

pub struct ActivityEngineBuilder<'c> {
    agents: Vec<SimulationAgent>,
    events: Rc<RefCell<EventsPublisher>>,
    config: &'c Config,
}

impl<'c> ActivityEngineBuilder<'c> {
    pub fn new(
        agents: Vec<SimulationAgent>,
        events: Rc<RefCell<EventsPublisher>>,
        config: &'c Config,
    ) -> Self {
        ActivityEngineBuilder {
            agents,
            events,
            config,
        }
    }

    pub fn build(self) -> ActivityEngine {
        let now = self.config.simulation().start_time;

        let mut asleep = TimeQueue::new();
        for agent in self.agents {
            asleep.add(AsleepSimulationAgent::build(agent, now), now);
        }
        let awake_q = Vec::new();
        ActivityEngine::new(asleep, awake_q, self.events)
    }
}

struct AsleepSimulationAgent {
    agent: SimulationAgent,
    wakeup_time: u32,
}

impl AsleepSimulationAgent {
    fn build(agent: SimulationAgent, now: u32) -> Self {
        let wakeup_time = agent.wakeup_time(now);
        AsleepSimulationAgent { agent, wakeup_time }
    }
}

impl EndTime for AsleepSimulationAgent {
    fn end_time(&self, _now: u32) -> u32 {
        self.wakeup_time
    }
}

#[cfg(test)]
mod tests {
    use crate::simulation::config::Config;
    use crate::simulation::engines::activity_engine::{ActivityEngine, ActivityEngineBuilder};
    use crate::simulation::messaging::events::EventsPublisher;
    use crate::simulation::wire_types::messages::SimulationAgent;
    use crate::simulation::wire_types::population::leg::Route;
    use crate::simulation::wire_types::population::{Activity, GenericRoute, Leg, Person, Plan};
    use std::cell::RefCell;
    use std::rc::Rc;

    #[test]
    fn test_activity_engine_build() {
        let mut engine = ActivityEngineBuilder::new(
            vec![],
            Rc::new(RefCell::new(EventsPublisher::new())),
            &Config::default(),
        )
        .build();

        assert_eq!(engine.awake_q.len(), 0);
        assert_eq!(engine.asleep_q.len(), 0);
        engine.end(0);
    }

    #[test]
    fn test_activity_engine_wake_up_plan() {
        let plan = create_plan_with_plan_logic();

        let agent = SimulationAgent::new_plan_logic(Person::new(1, plan));
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

        let agent = SimulationAgent::new_plan_logic(Person::new(1, plan));
        let agents = vec![agent];

        let mut engine = create_engine(agents);

        {
            let agents = engine.do_step(0, vec![]);
            assert!(agents.is_empty());
        }
        {
            let agents = engine.do_step(10, vec![]);
            assert_eq!(agents.len(), 1);
        }
    }

    #[test]
    fn test_activity_engine_wake_up_rolling_horizon() {
        unimplemented!()
    }

    fn create_engine(agents: Vec<SimulationAgent>) -> ActivityEngine {
        ActivityEngineBuilder::new(
            agents,
            Rc::new(RefCell::new(EventsPublisher::new())),
            &Config::default(),
        )
        .build()
    }

    fn create_plan_with_plan_logic() -> Plan {
        let mut plan = Plan::new();
        plan.add_act(Activity::new(0.0, 0.0, 0, 1, Some(0), Some(10), None));
        plan.add_leg(Leg::new(
            Route::GenericRoute(GenericRoute::default()),
            0,
            1,
            Some(2),
        ));
        plan.add_act(Activity::new(0.0, 0.0, 0, 1, Some(25), Some(10), None));
        plan
    }
}
