use crate::mpi::messages::proto::{Agent, ExperimentalMessage, Vehicle};
use prost::Message;
use std::io::Cursor;

// Include the `messages` module, which is generated from messages.proto.
pub mod proto {
    include!(concat!(env!("OUT_DIR"), "/mpi.messages.rs"));
}

impl ExperimentalMessage {
    pub fn new() -> ExperimentalMessage {
        ExperimentalMessage {
            counter: 0,
            timestamp: 0,
            additional_message: String::new(),
        }
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(self.encoded_len());
        self.encode(&mut buf).unwrap();
        buf
    }

    pub fn deserialize(buf: &[u8]) -> ExperimentalMessage {
        ExperimentalMessage::decode(&mut Cursor::new(buf)).unwrap()
    }
}

impl crate::mpi::messages::proto::Vehicle {
    pub fn new(id: usize, agent: Agent) -> Vehicle {
        Vehicle {
            id: id as u64,
            agent: Some(agent),
            cur_route_elem: 0,
        }
    }

    pub fn advance_route(&mut self) {
        self.cur_route_elem += 1;
    }
}

impl crate::mpi::messages::proto::Agent {
    pub fn curr_act(&self) -> &crate::mpi::messages::proto::Activity {
        if self.current_element % 2 != 0 {
            panic!("Current element is not an activity");
        }
        let act_index = self.current_element / 2;
        self.plan
            .as_ref()
            .unwrap()
            .acts
            .get(act_index as usize)
            .unwrap()
    }

    pub fn curr_leg(&self) -> &proto::Leg {
        if self.current_element % 2 != 1 {
            panic!("Current element is not a leg.");
        }

        let leg_index = (self.current_element - 1) / 2;
        self.plan
            .as_ref()
            .unwrap()
            .legs
            .get(leg_index as usize)
            .unwrap()
    }

    pub fn advance_plan(&mut self) {
        let next = self.current_element + 1;
        if self.plan.as_ref().unwrap().acts.len() + self.plan.as_ref().unwrap().legs.len()
            == next as usize
        {
            panic!(
                "Agent: Advance plan was called on agent #{}, but no element is remaining.",
                self.id
            )
        }
        self.current_element = next;
    }
}
