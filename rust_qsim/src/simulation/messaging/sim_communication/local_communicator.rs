use std::cell::RefCell;
use crate::simulation::messaging::messages::{InternalMessage, InternalSimMessage, InternalSyncMessage};
use crate::simulation::messaging::sim_communication::SimCommunicator;
use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Barrier, Mutex};
use tracing::info;
use crate::simulation::id::Id;
use crate::simulation::scenario::population::InternalPerson;
use crate::simulation::time_queue::Identifiable;

pub struct DummySimCommunicator();

pub struct ChannelSimCommunicator {
    receiver: Receiver<InternalSimMessage>,
    senders: Vec<Sender<InternalSimMessage>>,
    rank: u32,
    barrier: Arc<Barrier>,
    scoring_callbacks: Mutex<Vec<Box<dyn Fn(HashMap<u32, Vec<Id<InternalPerson>>>) + Send>>>,
}

impl SimCommunicator for DummySimCommunicator {
    fn send_receive_vehicles<F>(
        &self,
        _vehicles: HashMap<u32, InternalSyncMessage>,
        _expected_vehicle_messages: &mut HashSet<u32>,
        _now: u32,
        _on_msg: F,
    ) where
        F: FnMut(InternalSyncMessage),
    {
    }

    fn send_receive_others<F>(
        &self,
        _others: HashMap<u32, Box<dyn InternalMessage>>,
        _expected_other_messages: &mut HashSet<u32>,
        _now: u32,
        _on_msg: F
    ) where
        F: FnMut(Box<dyn InternalMessage>)
    {
    }


    fn barrier(&self) {
        info!("Barrier was called on DummySimCommunicator, which doesn't do anything.")
    }

    fn rank(&self) -> u32 {
        0
    }

    fn extract_leaving_agents(vehicles: &HashMap<u32, InternalSyncMessage>) -> HashMap<u32, Vec<Id<InternalPerson>>> {HashMap::default()}

    fn register_scoring_callback(&self, f: Box<dyn Fn(HashMap<u32, Vec<Id<InternalPerson>>>) + Send + Sync>) {}
}

impl SimCommunicator for ChannelSimCommunicator {
    fn send_receive_vehicles<F>(
        &self,
        vehicles: HashMap<u32, InternalSyncMessage>,
        expected_vehicle_messages: &mut HashSet<u32>,
        now: u32,
        mut on_msg: F,
    ) where
        F: FnMut(InternalSyncMessage),
    {
        for callback in self.scoring_callbacks.lock().unwrap().iter() {
            callback(Self::extract_leaving_agents(&vehicles));
        }

        // send messages to everyone
        for (target, msg) in vehicles {
            let sender = self.senders.get(target as usize).unwrap();
            sender
                .send(InternalSimMessage::from_sync_message(msg))
                .unwrap_or_else(|e| {
                    panic!(
                        "Error while sending message to rank {} with error {}",
                        target, e
                    )
                });
        }

        // receive messages from everyone
        while !expected_vehicle_messages.is_empty() {
            let received_msg = self
                .receiver
                .recv()
                .expect("Error while receiving messages")
                .sync_message();
            let from_rank = received_msg.from_process();

            // If a message was received from a neighbor partition for this very time step, remove
            // that partition from expected messages which indicates which partitions we are waiting
            // for
            if received_msg.time() == now {
                expected_vehicle_messages.remove(&from_rank);
            }

            // publish the received message to the message broker
            on_msg(received_msg);
        }
    }

    fn send_receive_others<F>(
        &self,
        others: HashMap<u32, Box<dyn InternalMessage>>,
        expected_other_messages: &mut HashSet<u32>,
        now: u32,
        mut on_msg: F,
    ) where
        F: FnMut(Box<dyn InternalMessage>),
    {
        // send messages to everyone
        for (target, msg) in others {
            let sender = self.senders.get(target as usize).unwrap();
            sender
                .send(InternalSimMessage::from_other_message(msg))
                .unwrap_or_else(|e| {
                    panic!(
                        "Error while sending message to rank {} with error {}",
                        target, e
                    )
                });
        }

        // receive messages from everyone
        while !expected_other_messages.is_empty() {
            let received_msg = self
                .receiver
                .recv()
                .expect("Error while receiving messages")
                .other_message();
            let from_rank = received_msg.from_process();

            // If a message was received from a neighbor partition for this very time step, remove
            // that partition from expected messages which indicates which partitions we are waiting
            // for
            if received_msg.time() == now {
                expected_other_messages.remove(&from_rank);
            }

            // publish the received message to the message broker
            on_msg(received_msg);
        }
    }

    fn barrier(&self) {
        self.barrier.wait();
    }

    fn rank(&self) -> u32 {
        self.rank
    }

    fn extract_leaving_agents(vehicles: &HashMap<u32, InternalSyncMessage>) -> HashMap<u32, Vec<Id<InternalPerson>>>{
        let mut agents: HashMap<u32, Vec<Id<InternalPerson>>> = HashMap::default();

        for (k, v) in vehicles.iter() {
            agents.insert(*k, v.vehicles().iter().map(|v| v.passengers.iter().map(|p| p.id().clone())).flatten().collect());
        }
        
        agents
    }

    fn register_scoring_callback(&self, f: Box<dyn Fn(HashMap<u32, Vec<Id<InternalPerson>>>) + Send + Sync>)
    {
        self.scoring_callbacks.lock().unwrap().push(f);
    }
}

impl ChannelSimCommunicator {
    pub fn create_n_2_n(num_parts: u32) -> Vec<ChannelSimCommunicator> {
        let mut senders: Vec<_> = Vec::new();
        let mut comms: Vec<_> = Vec::new();
        let barrier = Arc::new(Barrier::new(num_parts as usize));

        for rank in 0..num_parts {
            let (sender, receiver) = channel();
            let comm = ChannelSimCommunicator {
                receiver,
                senders: vec![],
                rank,
                barrier: barrier.clone(),
                scoring_callbacks: Mutex::new(Vec::default())
            };
            senders.push(sender);
            comms.push(comm);
        }

        for comm in &mut comms {
            for sender in &senders {
                comm.senders.push(sender.clone());
            }
        }

        comms
    }

    pub fn rank(&self) -> u32 {
        self.rank
    }
}
