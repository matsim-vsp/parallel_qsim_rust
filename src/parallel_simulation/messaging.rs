use crate::parallel_simulation::messages::Message;
use crate::parallel_simulation::splittable_population::{Agent, PlanElement, Route};
use crate::parallel_simulation::vehicles::Vehicle;
use std::collections::HashMap;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Arc;

#[derive(Debug)]
pub struct MessageBroker {
    pub(crate) id: usize,
    receiver: Receiver<Message>,
    senders: HashMap<usize, Sender<Message>>,
    out_messages: HashMap<usize, Message>,
    link_id_mapping: Arc<HashMap<usize, usize>>,
}

impl MessageBroker {
    pub fn new(
        id: usize,
        receiver: Receiver<Message>,
        link_id_mapping: Arc<HashMap<usize, usize>>,
    ) -> MessageBroker {
        MessageBroker {
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
            let mut message = messages.remove(id).unwrap_or_else(Message::new);
            message.time = now;
            println!("{}", message.vehicles.len());

            sender.send(message).unwrap();
        }
    }

    pub fn prepare_to_send(&mut self, agent: Agent, vehicle: Vehicle) {
        let link_id = vehicle.current_link_id().unwrap();
        let thread = *self.link_id_mapping.get(link_id).unwrap();
        let message = self.out_messages.entry(thread).or_insert_with(Message::new);
        message.add_driver(agent, vehicle.route_index);
    }

    pub fn prepare_to_teleport(&mut self, agent: Agent) {
        if let PlanElement::Leg(leg) = agent.current_plan_element() {
            if let Route::GenericRoute(route) = &leg.route {
                let end_link = route.end_link;
                let thread = *self.link_id_mapping.get(&end_link).unwrap();
                let message = self.out_messages.entry(thread).or_insert_with(Message::new);
                message.add_teleported(agent);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::parallel_simulation::messaging::MessageBroker;
    use crate::parallel_simulation::splittable_population::{
        Agent, GenericRoute, Leg, Plan, PlanElement, Route,
    };
    use crate::parallel_simulation::vehicles::Vehicle;
    use std::collections::HashMap;
    use std::sync::{mpsc, Arc};

    #[test]
    fn id() {
        let (_sender, receiver) = mpsc::channel();
        let id_mapping = Arc::new(HashMap::new());
        let broker = MessageBroker::new(42, receiver, id_mapping);

        assert_eq!(42, broker.id);
    }

    #[test]
    fn thread_id() {
        let (_sender, receiver) = mpsc::channel();
        let id_mapping = Arc::new(HashMap::from([(1, 84)]));
        let broker = MessageBroker::new(42, receiver, id_mapping);

        assert_eq!(84, *broker.get_thread_id(&1));
    }

    #[test]
    fn add_sender() {
        let (_sender1, receiver) = mpsc::channel();
        let (sender2, _receiver) = mpsc::channel();
        let id_mapping = Arc::new(HashMap::new());
        let mut broker = MessageBroker::new(1, receiver, id_mapping);

        broker.add_sender(2, sender2);

        assert_eq!(1, broker.senders.len());
        assert!(broker.senders.contains_key(&2));
    }

    #[test]
    fn send() {
        let (_sender1, receiver) = mpsc::channel();
        let (sender2, receiver2) = mpsc::channel();
        let id_mapping = Arc::new(HashMap::new());
        let mut broker = MessageBroker::new(1, receiver, id_mapping);
        broker.add_sender(2, sender2);

        // should be empty her
        assert!(receiver2.try_recv().is_err());

        broker.send(1);

        let result = receiver2.recv();
        assert!(result.is_ok());
        let message = result.unwrap();
        assert_eq!(1, message.time);
        assert_eq!(0, message.vehicles.len());
        assert_eq!(0, message.telported.len());
    }

    #[test]
    fn prepare_to_send() {
        let agent_id = 42;
        let (_sender1, receiver) = mpsc::channel();
        let (sender2, receiver2) = mpsc::channel();
        let id_mapping = Arc::new(HashMap::from([(1, 2)]));
        let mut broker = MessageBroker::new(1, receiver, id_mapping);
        let vehicle = Vehicle::new(1, agent_id, vec![1, 2, 3, 4]);
        let agent = Agent {
            id: agent_id,
            current_element: 0,
            plan: Plan { elements: vec![] },
        };
        broker.add_sender(2, sender2);

        broker.prepare_to_send(agent, vehicle);
        // should be empty her
        assert!(receiver2.try_recv().is_err());
        broker.send(1);

        let received_message = receiver2.recv().unwrap();
        assert_eq!(0, received_message.telported.len());
        assert_eq!(1, received_message.vehicles.len());
        assert_eq!(1, received_message.time);
        let (received_agent, route_index) = received_message.vehicles.get(0).unwrap();
        assert_eq!(agent_id, received_agent.id);
        assert_eq!(0, *route_index);
    }

    #[test]
    fn prepare_to_teleport() {
        let agent_id = 42;
        let link_id = 1;
        let (_sender1, receiver) = mpsc::channel();
        let (sender2, receiver2) = mpsc::channel();
        let id_mapping = Arc::new(HashMap::from([(link_id, 2)]));
        let mut broker = MessageBroker::new(1, receiver, id_mapping);
        let agent = create_agent(agent_id, link_id);
        broker.add_sender(2, sender2);
        broker.prepare_to_teleport(agent);

        assert!(receiver2.try_recv().is_err());
        broker.send(42);

        let received_message = receiver2.recv().unwrap();
        assert_eq!(1, received_message.telported.len());
        assert_eq!(0, received_message.vehicles.len());
        assert_eq!(42, received_message.time);

        let received_agent = received_message.telported.get(0).unwrap();
        assert_eq!(agent_id, received_agent.id)
    }

    #[test]
    fn receive() {
        let agent_id = 42;
        let link_id_1 = 1;
        let link_id_2 = 2;
        let (sender1, receiver1) = mpsc::channel();
        let (sender2, receiver2) = mpsc::channel();
        let id_mapping = Arc::new(HashMap::from([(link_id_1, 1), (link_id_2, 2)]));
        let mut broker1 = MessageBroker::new(1, receiver1, id_mapping.clone());
        broker1.add_sender(2, sender2);
        let mut broker2 = MessageBroker::new(2, receiver2, id_mapping.clone());
        broker2.add_sender(1, sender1);
        let agent = Agent {
            id: agent_id,
            current_element: 0,
            plan: Plan { elements: vec![] },
        };
        let mut vehicle = Vehicle::new(1, agent_id, vec![1, 2, 3, 4]);
        vehicle.advance_route_index();
        broker1.prepare_to_send(agent, vehicle);
        broker1.send(43);

        let messages = broker2.receive();

        assert_eq!(1, messages.len());
        let message = messages.get(0).unwrap();
        assert_eq!(1, message.vehicles.len());
        assert_eq!(0, message.telported.len());
        assert_eq!(43, message.time);
        let (agent, _route_id) = message.vehicles.get(0).unwrap();
        assert_eq!(agent_id, agent.id)
    }

    fn create_agent(id: usize, end_link_id: usize) -> Agent {
        Agent {
            id,
            current_element: 0,
            plan: Plan {
                elements: Vec::from([PlanElement::Leg(Leg {
                    mode: String::from("test"),
                    dep_time: Some(1),
                    trav_time: Some(10),
                    route: Route::GenericRoute(GenericRoute {
                        start_link: 0,
                        end_link: end_link_id,
                        trav_time: 10,
                        distance: 100.,
                    }),
                })]),
            },
        }
    }
}
