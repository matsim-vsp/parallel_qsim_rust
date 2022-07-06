use crate::parallel_simulation::id_mapping::IdMapping;
use crate::parallel_simulation::messages::Message;
use crate::parallel_simulation::splittable_population::Agent;
use crate::parallel_simulation::vehicles::Vehicle;
use std::collections::HashMap;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Arc;

pub struct Customs {
    receiver: Receiver<Message>,
    senders: HashMap<usize, Sender<Message>>,
    out_messages: HashMap<usize, Message>,
    link_id_mapping: Arc<IdMapping>,
}

impl Customs {
    pub fn new(receiver: Receiver<Message>, link_id_mapping: Arc<IdMapping>) -> Customs {
        Customs {
            receiver,
            senders: HashMap::new(),
            out_messages: HashMap::new(),
            link_id_mapping,
        }
    }

    pub fn add_sender(&mut self, to: usize, sender: Sender<Message>) {
        self.senders.insert(to, sender);
    }

    pub fn receive(&self, thread: usize) -> Vec<Message> {
        //println!("#{}: about to receive.", thread);
        let result = self
            .senders
            .iter()
            .map(|_| self.receiver.recv().unwrap())
            .collect();

        result
    }

    pub fn send(&mut self, thread: usize, now: u32) {
        let capacity = self.senders.len();
        let mut messages =
            std::mem::replace(&mut self.out_messages, HashMap::with_capacity(capacity));
        for (id, sender) in &self.senders {
            let mut message = messages.remove(id).unwrap_or(Message::new());
            message.time = now;

            sender.send(message).unwrap();
        }
    }

    pub fn prepare_to_send(&mut self, agent: Agent, vehicle: Vehicle) {
        let link_id = vehicle.current_link_id().unwrap();
        let thread = self.link_id_mapping.get_thread(link_id);
        let message = self.out_messages.entry(thread).or_insert(Message::new());

        println!(
            "Prepare to send Agent #{} with route_index {} to thread #{}",
            agent.id, vehicle.route_index, thread
        );
        message.add(agent, vehicle.route_index);
    }
}
