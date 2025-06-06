use crate::simulation::wire_types::messages::SyncMessage;
use std::collections::{HashMap, HashSet};

pub mod local_communicator;
pub mod message_broker;

#[cfg(feature = "mpi")]
pub mod mpi_communicator;

pub trait SimCommunicator {
    fn send_receive_vehicles<F>(
        &self,
        vehicles: HashMap<u32, SyncMessage>,
        expected_vehicle_messages: &mut HashSet<u32>,
        now: u32,
        on_msg: F,
    ) where
        F: FnMut(SyncMessage);

    fn barrier(&self);

    fn rank(&self) -> u32;
}
