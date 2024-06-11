use crate::simulation::population::population_data::State;
use crate::simulation::wire_types::population::Person;
use std::cell::RefCell;
use std::rc::{Rc, Weak};

pub mod activity_engine;
pub mod leg_engine;
pub mod network_engine;
pub mod teleportation_engine;

pub trait Engine {
    fn do_step(&mut self, now: u32);
    fn receive_agent(&mut self, now: u32, agent: Person);
    fn set_internal_interface(&mut self, internal_interface: Weak<RefCell<InternalInterface>>);
}

pub struct InternalInterface {
    activity_engine: Rc<RefCell<dyn Engine>>,
    teleportation_engine: Rc<RefCell<dyn Engine>>,
}

impl InternalInterface {
    fn arrange_next_agent_state(&self, now: u32, agent: Person) {
        match agent.state() {
            State::ACTIVITY => self.activity_engine.borrow_mut().receive_agent(now, agent),
            State::LEG => self
                .teleportation_engine
                .borrow_mut()
                .receive_agent(now, agent),
        }
    }

    pub(crate) fn new(
        activity_engine: Rc<RefCell<dyn Engine>>,
        teleportation_engine: Rc<RefCell<dyn Engine>>,
    ) -> Self {
        InternalInterface {
            activity_engine,
            teleportation_engine,
        }
    }
}
