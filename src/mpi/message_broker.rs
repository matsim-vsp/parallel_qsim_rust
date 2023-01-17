use crate::mpi::messages::proto::{Vehicle, VehicleMessage};
use mpi::topology::SystemCommunicator;
use mpi::traits::{Communicator, Destination, Source};
use mpi::Rank;
use std::collections::{BinaryHeap, HashMap, HashSet};

pub trait MessageBroker {
    fn send(&mut self);
    fn receive(&mut self) -> Vec<Vehicle>;
    fn add_veh(&mut self, vehicle: Vehicle, now: u32);
}
pub struct MpiMessageBroker {
    id: usize,
    communicator: SystemCommunicator,
    neighbors: HashSet<u32>,
    out_messages: HashMap<usize, VehicleMessage>,
    link_id_mapping: HashMap<usize, usize>,
    in_messages_cache: BinaryHeap<VehicleMessage>,
}

impl MessageBroker for MpiMessageBroker {
    fn send(&mut self) {
        let capacity = self.out_messages.len();
        let mut messages =
            std::mem::replace(&mut self.out_messages, HashMap::with_capacity(capacity));

        for (partition, message) in messages {
            let buffer = message.serialize();
            self.communicator
                .process_at_rank(partition as Rank)
                .send(&buffer);
        }
    }

    fn receive(&mut self) -> Vec<Vehicle> {
        let mut expected_messages = self.neighbors.clone();
        let mut received_messages = Vec::new();

        while !expected_messages.is_empty() {
            let (encoded_msg, status) = self.communicator.any_process().receive_vec();
            let msg = VehicleMessage::deserialize(&encoded_msg);
            expected_messages.remove(&msg.from_process);
            received_messages.push(msg);
        }

        received_messages
            .into_iter()
            .flat_map(|msg| msg.vehicles)
            .collect()
    }

    fn add_veh(&mut self, vehicle: Vehicle, now: u32) {
        let link_id = vehicle.curr_link_id().unwrap();
        let partition = self.link_id_mapping.get(&link_id).unwrap();
        let message = self
            .out_messages
            .entry(*partition)
            .or_insert_with(|| VehicleMessage::new(now, self.id, *partition));
        message.add(vehicle);
    }
}

impl MpiMessageBroker {
    /*
    fn pop_from_cache(
        &mut self,
        expected_messages: &mut HashSet<u32>,
        messages: &mut Vec<VehicleMessage>,
        now: u32,
    ) {
        while let Some(veh_message) = self.in_messages_cache.peek() {
            if veh_message.time <= now {
                expected_messages.remove(&veh_message.from_process);
                messages.push(self.in_messages_cache.pop().unwrap())
            } else {
                break; // important! otherwise this is an infinte loop
            }
        }
    }

    fn receive_blocking(&mut self, expected_messages: &mut HashSet<usize>) {
        while !expected_messages.is_empty() {
            let (recv_encoded_msg, _status) = self.communicator.any_process().receive_vec::<u8>();
            let recv_msg = VehicleMessage::deserialize(recv_encoded_msg);
            self.in_messages_cache.push(recv_msg);
        }
    }

    fn receive_non_blocking(&mut self) {
        self.communicator.any_process().immediate_receive()
    }

     */
}
