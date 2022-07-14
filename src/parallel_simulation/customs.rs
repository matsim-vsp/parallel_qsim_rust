use crate::parallel_simulation::messages::Message;
use crate::parallel_simulation::splittable_population::{Agent, PlanElement, Route};
use crate::parallel_simulation::vehicles::Vehicle;
use std::collections::HashMap;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Arc;

#[derive(Debug)]
pub struct Customs {
    pub(crate) id: usize,
    receiver: Receiver<Message>,
    senders: HashMap<usize, Sender<Message>>,
    out_messages: HashMap<usize, Message>,
    link_id_mapping: Arc<HashMap<usize, usize>>,
}

impl Customs {
    pub fn new(
        id: usize,
        receiver: Receiver<Message>,
        link_id_mapping: Arc<HashMap<usize, usize>>,
    ) -> Customs {
        Customs {
            id,
            receiver,
            senders: HashMap::new(),
            out_messages: HashMap::new(),
            link_id_mapping,
        }
    }

    pub fn get_thread_id(&self, link_id: &usize) -> &usize {
        self.link_id_mapping.get(link_id).unwrap()
    }

    pub fn add_sender(&mut self, to: usize, sender: Sender<Message>) {
        self.senders.insert(to, sender);
    }

    pub fn receive(&self) -> Vec<Message> {
        //println!("#{}: about to receive.", thread);
        let result = self
            .senders
            .iter()
            .map(|_| self.receiver.recv().unwrap())
            .collect();

        result
    }

    pub fn send(&mut self, now: u32) {
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
        let thread = *self.link_id_mapping.get(link_id).unwrap();
        let message = self.out_messages.entry(thread).or_insert(Message::new());

        println!(
            "Customs: Prepare to send Agent #{} with route_index {} from thread #{} to thread #{}",
            agent.id, vehicle.route_index, self.id, thread
        );
        message.add_driver(agent, vehicle.route_index);
    }

    pub fn prepare_to_teleport(&mut self, agent: Agent) {
        if let PlanElement::Leg(leg) = agent.current_plan_element() {
            if let Route::GenericRoute(route) = &leg.route {
                let end_link = route.end_link;
                let thread = *self.link_id_mapping.get(&end_link).unwrap();
                let message = self.out_messages.entry(thread).or_insert(Message::new());

                println!(
                    "Customs: Prepare to send teleported Agent #{} from thread #{} to thread #{}",
                    agent.id, self.id, thread
                );
                message.add_teleported(agent);
            }
        }
    }
}
