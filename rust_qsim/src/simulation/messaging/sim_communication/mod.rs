use crate::simulation::messaging::messages::{InternalMessage, InternalSyncMessage};
use std::collections::{HashMap, HashSet};

pub mod local_communicator;
pub mod message_broker;

pub trait SimCommunicator {
    fn send_receive_vehicles<F>(
        &self,
        vehicles: HashMap<u32, InternalSyncMessage>,
        expected_vehicle_messages: &mut HashSet<u32>,
        now: u32,
        on_msg: F,
    ) where
        F: FnMut(InternalSyncMessage);

    fn send_receive_others<F>(
        &self,
        others: HashMap<u32, Box<dyn InternalMessage>>,
        expected_other_messages: &mut HashSet<u32>,
        now: u32,
        on_msg: F,
    ) where
        F: FnMut(Box<dyn InternalMessage>);

    fn barrier(&self);

    fn rank(&self) -> u32;

    fn register_send_callback(&self, callback: Box<dyn Fn(&HashMap<u32, InternalSyncMessage>)>);

    fn register_recv_callback(&self, callback: Box<dyn Fn(&InternalSyncMessage)>);
}