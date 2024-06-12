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
    fn set_agent_state_transition_logic(
        &mut self,
        internal_interface: Weak<RefCell<AgentStateTransitionLogic>>,
    );
}

pub struct AgentStateTransitionLogic {
    activity_engine: Rc<RefCell<dyn Engine>>,
    teleportation_engine: Rc<RefCell<dyn Engine>>,
}

impl AgentStateTransitionLogic {
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
        AgentStateTransitionLogic {
            activity_engine,
            teleportation_engine,
        }
    }
}
