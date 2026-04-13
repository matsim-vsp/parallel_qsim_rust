use std::cell::RefCell;
use crate::simulation::messaging::messages::{InternalMessage, InternalSimMessage, InternalSyncMessage};
use crate::simulation::messaging::sim_communication::SimCommunicator;
use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Barrier};
use tracing::info;

pub struct DummySimCommunicator();

pub struct ChannelSimCommunicator {
    receiver: Receiver<InternalSimMessage>,
    senders: Vec<Sender<InternalSimMessage>>,
    rank: u32,
    barrier: Arc<Barrier>,
    send_vehicle_callbacks: RefCell<Vec<Box<dyn Fn(Arc<HashMap<u32, InternalSyncMessage>>)>>>,
    recv_vehicle_callbacks: RefCell<Vec<Box<dyn Fn(Arc<InternalSyncMessage>)>>>,
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

    fn register_send_callback(&self, _callback: Box<dyn Fn(Arc<HashMap<u32, InternalSyncMessage>>)>) {}

    fn register_recv_callback(&self, _callback: Box<dyn Fn(Arc<InternalSyncMessage>)>) {}
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
        let arc_vehicles = Arc::new(vehicles);

        for callback in self.send_vehicle_callbacks.borrow().iter() {
            callback(Arc::clone(&arc_vehicles));
        }
        
        // send messages to everyone
        for (target, msg) in Arc::try_unwrap(arc_vehicles).unwrap() {
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

            let arc_received_msg = Arc::new(received_msg);

            for callback in self.recv_vehicle_callbacks.borrow().iter() {
                callback(Arc::clone(&arc_received_msg));
            }

            // publish the received message to the message broker
            on_msg(Arc::try_unwrap(arc_received_msg).unwrap());
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

    fn register_send_callback(&self, callback: Box<dyn Fn(Arc<HashMap<u32, InternalSyncMessage>>)>) {
        self.send_vehicle_callbacks.borrow_mut().push(callback);
    }

    fn register_recv_callback(&self, callback: Box<dyn Fn(Arc<InternalSyncMessage>)>) {
        self.recv_vehicle_callbacks.borrow_mut().push(callback);
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
                send_vehicle_callbacks: RefCell::new(Vec::default()),
                recv_vehicle_callbacks: RefCell::new(Vec::default())
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
