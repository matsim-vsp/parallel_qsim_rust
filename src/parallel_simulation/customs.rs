use crate::parallel_simulation::messages::Message;
use std::collections::HashMap;
use std::sync::mpsc::{Receiver, Sender};
use crate::parallel_simulation::splittable_population::Agent;
use crate::parallel_simulation::vehicles::Vehicle;

pub struct Customs<'a> {
    receiver: Receiver<Message<'a>>,
    senders: HashMap<usize, Sender<Message<'a>>>,
    out_messages: HashMap<usize, Message<'a>>,
}

impl<'a> Customs<'a> {
    pub fn new(receiver: Receiver<Message>) -> Customs {
        Customs {
            receiver,
            senders: HashMap::new(),
            out_messages: HashMap::new(),
        }
    }

    pub fn add_sender(&mut self, to: usize, sender: Sender<Message>) {
        self.senders.insert(to, sender);
    }

    pub fn receive(&self) -> Vec<Message> {
        let result = self
            .senders
            .iter()
            .map(|_| self.receiver.recv().unwrap())
            .collect();

        result
    }

    pub fn send(&mut self) {
        let messages = std::mem::replace(&mut self.out_messages, HashMap::new());

        for (id, message) in messages {
            let sender = self.senders.get(&id).unwrap();
            sender.send(message).unwrap();
        }
    }

    pub fn prepare_to_send(&mut self, agent: Agent, vehicle: Vehicle) {

        let
    }
}
