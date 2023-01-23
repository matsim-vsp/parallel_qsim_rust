use crate::mpi::messages::proto::{Vehicle, VehicleMessage};
use crate::parallel_simulation::network::node::NodeVehicle;
use mpi::topology::SystemCommunicator;
use mpi::traits::{Communicator, Destination, Source};
use mpi::Rank;
use std::collections::{BinaryHeap, HashMap, HashSet};
use std::sync::Arc;

pub trait MessageBroker {
    fn send(&mut self, now: u32);
    fn receive(&mut self, now: u32) -> Vec<Vehicle>;
    fn add_veh(&mut self, vehicle: Vehicle, now: u32);
}
pub struct MpiMessageBroker {
    pub rank: Rank,
    communicator: SystemCommunicator,
    neighbors: HashSet<u32>,
    out_messages: HashMap<u32, VehicleMessage>,
    in_messages: BinaryHeap<VehicleMessage>,
    link_id_mapping: Arc<HashMap<usize, usize>>,
}

impl MessageBroker for MpiMessageBroker {
    fn send(&mut self, now: u32) {
        let capacity = self.out_messages.len();
        let mut messages =
            std::mem::replace(&mut self.out_messages, HashMap::with_capacity(capacity));

        // send required messages to neighbor partitions
        for partition in &self.neighbors {
            let neighbor_rank = *partition;
            let message = messages
                .remove(&neighbor_rank)
                .unwrap_or_else(|| VehicleMessage::new(now, self.rank as u32, neighbor_rank));
            self.send_msg(message);
        }
        for (_partition, message) in messages {
            self.send_msg(message);
        }
    }

    fn receive(&mut self, now: u32) -> Vec<Vehicle> {
        let mut expected_messages = self.neighbors.clone();
        let mut received_messages = Vec::new();

        self.pop_from_cache(&mut expected_messages, &mut received_messages, now);
        self.receive_blocking(&mut expected_messages, &mut received_messages, now);

        received_messages
            .into_iter()
            .flat_map(|msg| msg.vehicles)
            .collect()
    }

    fn add_veh(&mut self, vehicle: Vehicle, now: u32) {
        let link_id = vehicle.curr_link_id().unwrap();
        let partition = *self.link_id_mapping.get(&link_id).unwrap() as u32;
        let message = self
            .out_messages
            .entry(partition)
            .or_insert_with(|| VehicleMessage::new(now, self.rank as u32, partition));
        message.add(vehicle);
    }
}

impl MpiMessageBroker {
    pub fn new(
        communicator: SystemCommunicator,
        rank: Rank,
        neighbors: HashSet<u32>,
        link_id_mapping: Arc<HashMap<usize, usize>>,
    ) -> Self {
        MpiMessageBroker {
            out_messages: HashMap::new(),
            in_messages: BinaryHeap::new(),
            communicator,
            neighbors,
            rank,
            link_id_mapping,
        }
    }

    fn send_msg(&self, message: VehicleMessage) {
        //info!("#{} sends: {message:?}", self.rank);
        let buffer = message.serialize();
        self.communicator
            .process_at_rank(message.to_process as Rank)
            .send(&buffer);
    }

    pub fn rank_for_link(&self, link_id: u64) -> u64 {
        *self.link_id_mapping.get(&(link_id as usize)).unwrap() as u64
    }

    fn pop_from_cache(
        &mut self,
        expected_messages: &mut HashSet<u32>,
        messages: &mut Vec<VehicleMessage>,
        now: u32,
    ) {
        while let Some(msg) = self.in_messages.peek() {
            if msg.time <= now {
                //info!("#{} {now} from cache {msg:?}", self.rank);
                expected_messages.remove(&msg.from_process);
                messages.push(self.in_messages.pop().unwrap())
            } else {
                break; // important! otherwise this is an infinite loop
            }
        }
    }

    fn receive_blocking(
        &mut self,
        expected_messages: &mut HashSet<u32>,
        received_message: &mut Vec<VehicleMessage>,
        now: u32,
    ) {
        while !expected_messages.is_empty() {
            let (encoded_msg, _status) = self.communicator.any_process().receive_vec();
            let msg = VehicleMessage::deserialize(&encoded_msg);
            let from_rank = msg.from_process;

            if msg.time == now {
                expected_messages.remove(&from_rank);
                received_message.push(msg);
            } else {
                self.in_messages.push(msg);
            }
        }
    }
}
