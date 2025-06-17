use crate::simulation::messaging::communication::SimCommunicator;
use crate::simulation::wire_types::messages::{SimMessage, SyncMessage};
use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Barrier};
use tracing::info;

pub struct DummySimCommunicator();

pub struct ChannelSimCommunicator {
    receiver: Receiver<SimMessage>,
    senders: Vec<Sender<SimMessage>>,
    rank: u32,
    barrier: Arc<Barrier>,
}

impl SimCommunicator for DummySimCommunicator {
    fn send_receive_vehicles<F>(
        &self,
        _vehicles: HashMap<u32, SyncMessage>,
        _expected_vehicle_messages: &mut HashSet<u32>,
        _now: u32,
        _on_msg: F,
    ) where
        F: FnMut(SyncMessage),
    {
    }

    fn barrier(&self) {
        info!("Barrier was called on DummySimCommunicator, which doesn't do anything.")
    }

    fn rank(&self) -> u32 {
        0
    }
}

impl SimCommunicator for ChannelSimCommunicator {
    fn send_receive_vehicles<F>(
        &self,
        vehicles: HashMap<u32, SyncMessage>,
        expected_vehicle_messages: &mut HashSet<u32>,
        now: u32,
        mut on_msg: F,
    ) where
        F: FnMut(SyncMessage),
    {
        // send messages to everyone
        for (target, msg) in vehicles {
            let sender = self.senders.get(target as usize).unwrap();
            sender
                .send(SimMessage::from_sync_message(msg))
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
            let from_rank = received_msg.from_process;

            // If a message was received from a neighbor partition for this very time step, remove
            // that partition from expected messages which indicates which partitions we are waiting
            // for
            if received_msg.time == now {
                expected_vehicle_messages.remove(&from_rank);
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
