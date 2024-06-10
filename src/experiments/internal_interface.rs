use std::cell::RefCell;
use std::rc::Rc;

enum State {
    ACTIVITY,
    TELEPORTATION,
}

struct Agent {
    id: String,
    state: State,
}

impl Agent {
    fn new(id: String, state: State) -> Self {
        Agent { id, state }
    }
}

struct InternalInterface {
    activity_engine: Rc<RefCell<ActivityEngine>>,
    teleportation_engine: Rc<RefCell<TeleportationEngine>>,
}

impl InternalInterface {
    fn arrange_next_agent_state(&self, agent: Agent) {
        match agent.state {
            State::ACTIVITY => self.activity_engine.borrow_mut().receive_agent(agent),
            State::TELEPORTATION => self.teleportation_engine.borrow_mut().receive_agent(agent),
        }
    }
}

struct Simulation {
    activity_engine: Rc<RefCell<ActivityEngine>>,
    teleportation_engine: Rc<RefCell<TeleportationEngine>>,
    internal_interface: Rc<RefCell<InternalInterface>>,
}

impl Simulation {
    fn new(
        activity_engine: Rc<RefCell<ActivityEngine>>,
        teleportation_engine: Rc<RefCell<TeleportationEngine>>,
        internal_interface: Rc<RefCell<InternalInterface>>,
    ) -> Self {
        Simulation {
            activity_engine,
            teleportation_engine,
            internal_interface,
        }
    }

    fn run(&mut self) {
        let mut now = 0;
        while now < 20 {
            self.activity_engine
                .borrow_mut()
                .do_step(now, Rc::clone(&self.internal_interface));
            self.teleportation_engine
                .borrow_mut()
                .do_step(now, Rc::clone(&self.internal_interface));
            now += 1;
        }
    }
}

trait Engine {
    fn do_step(&mut self, now: u32, internal_interface: Rc<RefCell<InternalInterface>>);
}

struct ActivityEngine {
    agents: Vec<Agent>,
}

impl ActivityEngine {
    fn receive_agent(&mut self, agent: Agent) {
        println!("Activity engine: Received agent");
        self.agents.push(agent);
    }
}

impl Engine for ActivityEngine {
    fn do_step(&mut self, now: u32, internal_interface: Rc<RefCell<InternalInterface>>) {
        if now % 10 == 0 {
            println!("Activity engine: Time step {}, stop activity", now);
            self.agents.get_mut(0).unwrap().state = State::TELEPORTATION;
            internal_interface
                .borrow_mut()
                .arrange_next_agent_state(self.agents.remove(0))
        } else {
            // println!("Activity engine: Time step {}, doing nothing", now)
        }
    }
}

struct TeleportationEngine {
    agents: Vec<Agent>,
}

impl TeleportationEngine {
    fn receive_agent(&mut self, agent: Agent) {
        println!("Teleportation engine: Received agent");
        self.agents.push(agent);
    }
}

impl Engine for TeleportationEngine {
    fn do_step(&mut self, now: u32, internal_interface: Rc<RefCell<InternalInterface>>) {
        if now % 10 == 5 {
            println!(
                "Teleportation engine: Time step {}, stop teleportation",
                now
            );
            self.agents.get_mut(0).unwrap().state = State::ACTIVITY;
            internal_interface
                .borrow_mut()
                .arrange_next_agent_state(self.agents.remove(0))
        } else {
            // println!("Teleportation engine: Time step {}, doing nothing", now)
        }
    }
}

fn run() {}

#[cfg(test)]
mod tests {
    use crate::experiments::internal_interface::{
        ActivityEngine, Agent, InternalInterface, Simulation, State, TeleportationEngine,
    };
    use std::cell::RefCell;
    use std::rc::Rc;

    #[test]
    fn test_run() {
        let activity_engine = Rc::new(RefCell::new(ActivityEngine {
            agents: vec![Agent::new(String::from("agent"), State::ACTIVITY)],
        }));
        let teleportation_engine =
            Rc::new(RefCell::new(TeleportationEngine { agents: Vec::new() }));
        let internal_interface = Rc::new(RefCell::new(InternalInterface {
            activity_engine: Rc::clone(&activity_engine),
            teleportation_engine: Rc::clone(&teleportation_engine),
        }));

        let mut sim = Simulation::new(activity_engine, teleportation_engine, internal_interface);

        sim.run();
    }
}
