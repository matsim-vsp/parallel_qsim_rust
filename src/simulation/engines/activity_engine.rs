use crate::simulation::id::Id;
use crate::simulation::messaging::events::EventsPublisher;
use crate::simulation::time_queue::MutTimeQueue;
use crate::simulation::wire_types::events::Event;
use crate::simulation::wire_types::messages::SimulationAgent;
use std::cell::RefCell;
use std::rc::Rc;

pub struct ActivityEngine {
    activity_q: MutTimeQueue<SimulationAgent>,
    events: Rc<RefCell<EventsPublisher>>,
}

impl ActivityEngine {
    pub fn new(
        activity_q: MutTimeQueue<SimulationAgent>,
        events: Rc<RefCell<EventsPublisher>>,
    ) -> Self {
        ActivityEngine { activity_q, events }
    }

    pub(crate) fn do_step(
        &mut self,
        now: u32,
        agents: Vec<SimulationAgent>,
    ) -> Vec<SimulationAgent> {
        for agent in agents {
            self.receive_agent(now, agent);
        }

        self.wake_up(now)
    }

    pub(crate) fn receive_agent(&mut self, now: u32, agent: SimulationAgent) {
        // emmit act start event
        let act = agent.curr_act();
        let act_type: Id<String> = Id::get(act.act_type);
        self.events.borrow_mut().publish_event(
            now,
            &Event::new_act_start(agent.id(), act.link_id, act_type.internal()),
        );
        self.activity_q.add(agent, now);
    }

    pub fn agents(&mut self) -> impl Iterator<Item = &mut SimulationAgent> {
        self.activity_q.iter_mut()
    }

    fn wake_up(&mut self, now: u32) -> Vec<SimulationAgent> {
        let mut agents = self.activity_q.pop(now);

        for agent in agents.iter_mut() {
            let act_type: Id<String> = Id::get(agent.curr_act().act_type);
            self.events.borrow_mut().publish_event(
                now,
                &Event::new_act_end(agent.id(), agent.curr_act().link_id, act_type.internal()),
            );
        }

        agents
    }
}
