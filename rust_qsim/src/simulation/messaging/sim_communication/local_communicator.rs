use crate::simulation::messaging::messages::{InternalSimMessage, InternalSyncMessage};
use crate::simulation::messaging::sim_communication::SimCommunicator;
use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Barrier, Mutex};
use itertools::Itertools;
use tracing::info;
use crate::simulation::id::Id;
use crate::simulation::scenario::population::InternalPerson;
use crate::simulation::scoring::backpacking::backpacking_scoring_broker::BackpackingMessage;
use crate::simulation::time_queue::Identifiable;

pub struct DummySimCommunicator();

pub struct ChannelSimCommunicator {
    receiver: Receiver<InternalSimMessage>,
    senders: Vec<Sender<InternalSimMessage>>,
    rank: u32,
    barrier: Arc<Barrier>,
    send_callback: Mutex<Option<Box<dyn Fn(HashMap<u32, Vec<Id<InternalPerson>>>) -> HashMap<u32, BackpackingMessage> + Send>>>,
    recv_callback: Mutex<Option<Box<dyn Fn(BackpackingMessage) + Send>>>
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

    fn barrier(&self) {
        info!("Barrier was called on DummySimCommunicator, which doesn't do anything.")
    }

    fn rank(&self) -> u32 {
        0
    }

    fn extract_leaving_agents(_vehicles: &HashMap<u32, InternalSyncMessage>) -> HashMap<u32, Vec<Id<InternalPerson>>> {HashMap::default()}

    fn register_send_callback(&self, _f: Box<dyn Fn(HashMap<u32, Vec<Id<InternalPerson>>>) -> HashMap<u32, BackpackingMessage> + Send>) {}

    fn register_recv_callback(&self, f: Box<dyn Fn(BackpackingMessage) + Send>) {}
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
        let scoring_msg = if let Some(callback) = self.send_callback.lock().unwrap().as_ref() {
             callback(Self::extract_leaving_agents(&vehicles))
        } else {
            HashMap::default()
        };

        // send scoring messages to everyone
        for(target, msg) in scoring_msg {
            let sender = self.senders.get(target as usize).unwrap();
            sender
                .send(InternalSimMessage::from_backpacking_message(msg))
                .unwrap_or_else(|e| {
                    panic!(
                        "Error while sending message to rank {} with error {}",
                        target, e
                    )
                });
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
            let internal_msg = self
                .receiver
                .recv()
                .expect("Error while receiving messages");

            if internal_msg.is_sync_message() {
                let received_msg = internal_msg.sync_message();
                let from_rank = received_msg.from_process();

                // If a message was received from a neighbor partition for this very time step, remove
                // that partition from expected messages which indicates which partitions we are waiting
                // for
                if received_msg.time() == now {
                    expected_vehicle_messages.remove(&from_rank);
                }

                // publish the received message to the message broker
                on_msg(received_msg);
            } else {
                // scoring message arrived, pass it to the callback
                let received_msg = internal_msg.backpacking_message();

                if let Some(callback) = self.recv_callback.lock().unwrap().as_ref() {
                    callback(received_msg);
                }
            }
        }
    }

    fn barrier(&self) {
        self.barrier.wait();
    }

    fn rank(&self) -> u32 {
        self.rank
    }

    fn extract_leaving_agents(vehicles_msg: &HashMap<u32, InternalSyncMessage>) -> HashMap<u32, Vec<Id<InternalPerson>>>{
        let mut agents: HashMap<u32, Vec<Id<InternalPerson>>> = HashMap::default();


        for (k, v) in vehicles_msg.iter() {
            let mut passengers: Vec<Id<InternalPerson>> = v.vehicles()
                .iter()
                .flat_map(|v| v.passengers.iter().map(|p| p.id().clone()))
                .collect();

            let mut drivers = v.vehicles()
                .iter()
                .filter_map(|v| v.driver.as_ref().map(|d| d.id().clone()))
                .collect();

            passengers.append(&mut drivers);
            agents.insert(*k, passengers);
        }

        // TODO Debug
        // print!(".");
        if agents.iter().map(|(k, v)| v.len() > 0).reduce(|a, b| a || b).unwrap() {
            println!("Sending non-empty message!")
        }

        agents
    }

    fn register_send_callback(&self, f: Box<dyn Fn(HashMap<u32, Vec<Id<InternalPerson>>>) -> HashMap<u32, BackpackingMessage> + Send>)
    {
        let mut guard = self.send_callback.lock().unwrap();
        if guard.is_some() {
            panic!("Send callback already registered for this channel.");
        }
        *guard = Some(f)
    }

    fn register_recv_callback(&self, f: Box<dyn Fn(BackpackingMessage) + Send>)
    {
        let mut guard = self.recv_callback.lock().unwrap();
        if guard.is_some() {
            panic!("Send callback already registered for this channel.");
        }
        *guard = Some(f)
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
                send_callback: Mutex::new(None),
                recv_callback: Mutex::new(None)
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
