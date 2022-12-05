use crate::parallel_simulation::messages::Message;
use crate::parallel_simulation::splittable_population::{Agent, PlanElement, Route};
use crate::parallel_simulation::vehicles::Vehicle;
use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Arc;

#[derive(Debug)]
pub struct MessageBroker {
    pub(crate) id: usize,
    receiver: Receiver<Message>,
    remote_senders: HashMap<usize, Sender<Message>>,
    neighbor_senders: HashMap<usize, Sender<Message>>,
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
            remote_senders: HashMap::new(),
            neighbor_senders: HashMap::new(),
            out_messages: HashMap::new(),
            link_id_mapping,
        }
    }

    pub fn part_id(&self, link_id: &usize) -> &usize {
        self.link_id_mapping.get(link_id).unwrap()
    }

    pub fn add_neighbor_sender(&mut self, to: usize, sender: Sender<Message>) {
        self.neighbor_senders.insert(to, sender);
    }

    pub fn add_remote_sender(&mut self, to: usize, sender: Sender<Message>) {
        self.remote_senders.insert(to, sender);
    }

    pub fn receive(&self) -> Vec<Message> {
        let mut messages = Vec::new();
        let mut required_senders: HashSet<usize> = self.neighbor_senders.keys().cloned().collect();

        // do blocking receive for required partitions
        while !required_senders.is_empty() {
            let message = self.receiver.recv().unwrap();
            required_senders.remove(&message.from);
            messages.push(message);
        }

        // check for optional messages as well.
        for message in self.receiver.try_iter() {
            messages.push(message);
        }

        messages
    }

    pub fn send(&mut self, now: u32) {
        // replace property with new map. We will consume the map, so we need ownership by creating a
        // separate variable
        let capacity = self.out_messages.len();
        let mut messages =
            std::mem::replace(&mut self.out_messages, HashMap::with_capacity(capacity));

        // send required messages to neighboring partitions
        for (id, sender) in &self.neighbor_senders {
            let mut message = messages.remove(id).unwrap_or_else(|| Message::new(self.id));
            message.time = now;
            sender.send(message).unwrap();
        }

        // send optional messages to remote partitions
        for (id, mut message) in messages.into_iter() {
            message.time = now;
            let sender = self.remote_senders.get(&id).unwrap();
            sender.send(message).unwrap();
        }
    }

    pub fn prepare_routed(&mut self, agent: Agent, vehicle: Vehicle) {
        let link_id = vehicle.current_link_id().unwrap();
        let partition = *self.link_id_mapping.get(link_id).unwrap();
        let message = self
            .out_messages
            .entry(partition)
            .or_insert_with(|| Message::new(self.id));
        message.add_driver(agent, vehicle.route_index);
    }

    pub fn prepare_teleported(&mut self, agent: Agent) {
        if let PlanElement::Leg(leg) = agent.current_plan_element() {
            if let Route::GenericRoute(route) = &leg.route {
                let end_link = route.end_link;
                let partition = *self.link_id_mapping.get(&end_link).unwrap();
                let message = self
                    .out_messages
                    .entry(partition)
                    .or_insert_with(|| Message::new(self.id));
                message.add_teleported(agent);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::parallel_simulation::messages::Message;
    use crate::parallel_simulation::messaging::MessageBroker;
    use crate::parallel_simulation::splittable_population::{
        Agent, GenericRoute, Leg, Plan, PlanElement, Route,
    };
    use crate::parallel_simulation::vehicles::Vehicle;
    use std::collections::HashMap;
    use std::sync::{mpsc, Arc, Mutex};
    use std::thread;
    use std::thread::sleep;
    use std::time::Duration;

    #[test]
    fn id() {
        let (_sender, receiver) = mpsc::channel();
        let id_mapping = Arc::new(HashMap::new());
        let broker = MessageBroker::new(42, receiver, id_mapping);

        assert_eq!(42, broker.id);
    }

    #[test]
    fn partition_id() {
        let (_sender, receiver) = mpsc::channel();
        let id_mapping = Arc::new(HashMap::from([(1, 84)]));
        let broker = MessageBroker::new(42, receiver, id_mapping);

        assert_eq!(84, *broker.part_id(&1));
    }

    #[test]
    fn add_neighboring_sender() {
        let (_sender1, receiver) = mpsc::channel();
        let (sender2, _receiver) = mpsc::channel();
        let id_mapping = Arc::new(HashMap::new());
        let mut broker = MessageBroker::new(1, receiver, id_mapping);

        broker.add_neighbor_sender(2, sender2);

        assert_eq!(1, broker.neighbor_senders.len());
        assert!(broker.neighbor_senders.contains_key(&2));
    }

    #[test]
    fn send_to_neighbor() {
        let (_sender1, receiver) = mpsc::channel();
        let (sender2, receiver2) = mpsc::channel();
        let id_mapping = Arc::new(HashMap::new());
        let mut broker = MessageBroker::new(1, receiver, id_mapping);
        broker.add_neighbor_sender(2, sender2);

        // should be empty here
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
    fn prepare_routed_and_send_to_neighbor() {
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
        broker.add_neighbor_sender(2, sender2);

        broker.prepare_routed(agent, vehicle);
        // should be empty here
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
    fn prepare_routed_and_send_to_remote() {
        let agent_id = 42;
        let link_id = 1;
        let (_sender1, receiver) = mpsc::channel();
        let (sender2, receiver2) = mpsc::channel();
        let id_mapping = Arc::new(HashMap::from([(1, 2)]));
        let agent = create_agent(agent_id, link_id);
        let vehicle = Vehicle::new(1, agent_id, vec![1, 2, 3, 4]);
        let mut broker = MessageBroker::new(1, receiver, id_mapping);
        broker.add_remote_sender(2, sender2);

        // should be empty here
        assert!(receiver2.try_recv().is_err());

        broker.prepare_routed(agent, vehicle);
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
    fn prepare_routed_and_send_both() {
        let next_link_1 = 1;
        let next_link_2 = 5;
        let (_sender1, receiver) = mpsc::channel();
        let (sender2, receiver2) = mpsc::channel();
        let (sender3, receiver3) = mpsc::channel();
        let id_mapping = Arc::new(HashMap::from([(next_link_1, 2), (next_link_2, 3)]));
        let agent1 = create_agent(42, next_link_1);
        let agent2 = create_agent(43, next_link_2);
        let vehicle1 = Vehicle::new(1, agent1.id, vec![next_link_1, 2, 3, 4]);
        let vehicle2 = Vehicle::new(1, agent2.id, vec![next_link_2, 6, 7, 8]);
        let mut broker = MessageBroker::new(1, receiver, id_mapping);
        broker.add_neighbor_sender(2, sender2);
        broker.add_remote_sender(3, sender3);

        // should be empty here
        assert!(receiver2.try_recv().is_err());
        assert!(receiver3.try_recv().is_err());

        broker.prepare_routed(agent1, vehicle1);
        broker.prepare_routed(agent2, vehicle2);
        broker.send(1);

        let received_message2 = receiver2.recv().unwrap();
        assert_eq!(0, received_message2.telported.len());
        assert_eq!(1, received_message2.vehicles.len());
        assert_eq!(1, received_message2.time);

        let received_message3 = receiver3.recv().unwrap();
        assert_eq!(0, received_message3.telported.len());
        assert_eq!(1, received_message3.vehicles.len());
        assert_eq!(1, received_message3.time);
    }

    #[test]
    fn prepare_teleported_to_neighbor() {
        let agent_id = 42;
        let link_id = 1;
        let (_sender1, receiver) = mpsc::channel();
        let (sender2, receiver2) = mpsc::channel();
        let id_mapping = Arc::new(HashMap::from([(link_id, 2)]));
        let mut broker = MessageBroker::new(1, receiver, id_mapping);
        let agent = create_agent(agent_id, link_id);
        broker.add_neighbor_sender(2, sender2);
        broker.prepare_teleported(agent);

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
    fn prepare_teleported_to_remote() {
        let agent_id = 42;
        let link_id = 1;
        let (_sender1, receiver) = mpsc::channel();
        let (sender2, receiver2) = mpsc::channel();
        let id_mapping = Arc::new(HashMap::from([(link_id, 2)]));
        let mut broker = MessageBroker::new(1, receiver, id_mapping);
        let agent = create_agent(agent_id, link_id);
        broker.add_remote_sender(2, sender2);
        broker.prepare_teleported(agent);

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
    fn prepare_teleported_to_both() {
        let end_link_1 = 1;
        let end_link_2 = 5;
        let (_sender1, receiver) = mpsc::channel();
        let (sender2, receiver2) = mpsc::channel();
        let (sender3, receiver3) = mpsc::channel();
        let id_mapping = Arc::new(HashMap::from([(end_link_1, 2), (end_link_2, 3)]));
        let agent1 = create_agent(42, end_link_1);
        let agent2 = create_agent(43, end_link_2);
        let mut broker = MessageBroker::new(1, receiver, id_mapping);
        broker.add_neighbor_sender(2, sender2);
        broker.add_remote_sender(3, sender3);

        // should be empty here
        assert!(receiver2.try_recv().is_err());
        assert!(receiver3.try_recv().is_err());

        broker.prepare_teleported(agent1);
        broker.prepare_teleported(agent2);
        broker.send(1);

        let received_message2 = receiver2.recv().unwrap();
        assert_eq!(1, received_message2.telported.len());
        assert_eq!(0, received_message2.vehicles.len());
        assert_eq!(1, received_message2.time);

        let received_message3 = receiver3.recv().unwrap();
        assert_eq!(1, received_message3.telported.len());
        assert_eq!(0, received_message3.vehicles.len());
        assert_eq!(1, received_message3.time);
    }

    #[test]
    fn receive_from_neighbor() {
        let agent_id = 42;
        let link_id_1 = 1;
        let link_id_2 = 2;
        let (sender1, receiver1) = mpsc::channel();
        let (sender2, receiver2) = mpsc::channel();
        let id_mapping = Arc::new(HashMap::from([(link_id_1, 1), (link_id_2, 2)]));
        let mut broker1 = MessageBroker::new(1, receiver1, id_mapping.clone());
        broker1.add_neighbor_sender(2, sender2);
        let mut broker2 = MessageBroker::new(2, receiver2, id_mapping.clone());
        broker2.add_neighbor_sender(1, sender1);
        let agent = Agent {
            id: agent_id,
            current_element: 0,
            plan: Plan { elements: vec![] },
        };
        let mut vehicle = Vehicle::new(1, agent_id, vec![1, 2, 3, 4]);
        vehicle.advance_route_index();
        broker1.prepare_routed(agent, vehicle);
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

    #[test]
    fn receive_only_neighbor() {
        let (sender1, receiver) = mpsc::channel();
        let (sender2, _receiver2) = mpsc::channel();
        let (sender3, _receiver3) = mpsc::channel();
        let id_mapping = Arc::new(HashMap::new());
        let mut broker = MessageBroker::new(1, receiver, id_mapping);

        broker.add_neighbor_sender(2, sender2.clone());
        broker.add_remote_sender(3, sender3.clone());

        sender1.send(Message::new(2)).unwrap();
        let result = broker.receive();
        assert_eq!(1, result.len());
    }

    #[test]
    fn receive_remote_first() {
        let (sender1, receiver) = mpsc::channel();
        let (sender2, _receiver2) = mpsc::channel();
        let (sender3, _receiver3) = mpsc::channel();
        let id_mapping = Arc::new(HashMap::new());
        let mut broker = MessageBroker::new(1, receiver, id_mapping);
        broker.add_neighbor_sender(2, sender2.clone());
        broker.add_remote_sender(3, sender3.clone());

        // send an optional message to the message broker. The broker should block for this
        sender1.send(Message::new(3)).unwrap();

        let has_received_1 = Arc::new(Mutex::new(false));
        let has_received_2 = has_received_1.clone(); // clone it because we need to pointers, one for the thread and one for the main thread
        let handle = thread::spawn(move || {
            // put in a reasonable amount of time to let the main thread finish its business first. Hope this doesn't break occasionally.
            sleep(Duration::from_millis(500));
            // we expect the main thread to block on receive because it has to wait for the required
            // neighbours to send messages
            let has_received = *has_received_2.lock().unwrap();
            assert!(!has_received);
            // this unblocks the main thread eventually
            sender1.send(Message::new(2)).unwrap();
        });

        // this will block until the secondary thread sends the required message.
        {
            // put this into a scope so that the mutex is released before waiting for the other thread
            // which also uses this mutex.
            let result = broker.receive();
            assert_eq!(2, result.len());
            let mut has_received = has_received_1.lock().unwrap();
            *has_received = true;
        }

        // wait for thread to finish
        handle.join().unwrap();
    }

    #[test]
    fn receive_neighbor_and_remote() {
        let (sender1, receiver) = mpsc::channel();
        let (sender2, _receiver2) = mpsc::channel();
        let (sender3, _receiver3) = mpsc::channel();
        let id_mapping = Arc::new(HashMap::new());
        let mut broker = MessageBroker::new(1, receiver, id_mapping);
        broker.add_neighbor_sender(2, sender2.clone());
        broker.add_remote_sender(3, sender3.clone());
        // send an optional message
        sender1.send(Message::new(3)).unwrap();
        // send the required message
        sender1.send(Message::new(2)).unwrap();

        let result = broker.receive();
        assert_eq!(2, result.len());
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
