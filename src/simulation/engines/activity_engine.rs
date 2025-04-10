use crate::simulation::id::Id;
use crate::simulation::messaging::events::EventsPublisher;
use crate::simulation::time_queue::MutTimeQueue;
use crate::simulation::wire_types::events::Event;
use crate::simulation::wire_types::population::Person;
use std::cell::RefCell;
use std::rc::Rc;

pub struct ActivityEngine {
    activity_q: MutTimeQueue<Person>,
    events: Rc<RefCell<EventsPublisher>>,
}

impl ActivityEngine {
    pub fn new(activity_q: MutTimeQueue<Person>, events: Rc<RefCell<EventsPublisher>>) -> Self {
        ActivityEngine { activity_q, events }
    }

    pub(crate) fn do_step(&mut self, now: u32, agents: Vec<Person>) -> Vec<Person> {
        for agent in agents {
            self.receive_agent(now, agent);
        }

        self.wake_up(now)
    }

    pub(crate) fn receive_agent(&mut self, now: u32, agent: Person) {
        self.events.borrow_mut().publish_event(
            now,
            &Event::new_arrival(
                agent.id,
                agent.curr_act().link_id,
                agent.previous_leg().mode,
            ),
        );

        // emmit act start event
        let act = agent.curr_act();
        let act_type: Id<String> = Id::get(act.act_type);
        self.events.borrow_mut().publish_event(
            now,
            &Event::new_act_start(agent.id, act.link_id, act_type.internal()),
        );
        self.activity_q.add(agent, now);
    }

    pub fn agents(&mut self) -> impl Iterator<Item = &mut Person> {
        self.activity_q.iter_mut()
    }

    fn wake_up(&mut self, now: u32) -> Vec<Person> {
        let mut agents = self.activity_q.pop(now);

        for agent in agents.iter_mut() {
            // self.update_agent(&mut agent, now);
            //TODO (used for routing)

            let act_type: Id<String> = Id::get(agent.curr_act().act_type);
            self.events.borrow_mut().publish_event(
                now,
                &Event::new_act_end(agent.id, agent.curr_act().link_id, act_type.internal()),
            );
        }

        agents
    }
}
