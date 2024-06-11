use crate::simulation::engines::{Engine, InternalInterface};
use crate::simulation::id::Id;
use crate::simulation::messaging::events::EventsPublisher;
use crate::simulation::time_queue::TimeQueue;
use crate::simulation::wire_types::events::Event;
use crate::simulation::wire_types::population::Person;
use std::cell::RefCell;
use std::rc::{Rc, Weak};

pub struct ActivityEngine {
    activity_q: TimeQueue<Person>,
    events: Rc<RefCell<EventsPublisher>>,
    internal_interface: Weak<RefCell<InternalInterface>>,
}

impl Engine for ActivityEngine {
    fn do_step(&mut self, now: u32) {
        let agents = self.wake_up(now);
        for mut agent in agents {
            agent.advance_plan();

            self.internal_interface
                .upgrade()
                .unwrap()
                .borrow_mut()
                .arrange_next_agent_state(now, agent);
        }
    }

    fn receive_agent(&mut self, now: u32, agent: Person) {
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

    fn set_internal_interface(&mut self, internal_interface: Weak<RefCell<InternalInterface>>) {
        self.internal_interface = internal_interface
    }
}

impl ActivityEngine {
    pub fn new(activity_q: TimeQueue<Person>, events: Rc<RefCell<EventsPublisher>>) -> Self {
        ActivityEngine {
            activity_q,
            events,
            internal_interface: Weak::new(),
        }
    }

    pub fn wake_up(&mut self, now: u32) -> Vec<Person> {
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
