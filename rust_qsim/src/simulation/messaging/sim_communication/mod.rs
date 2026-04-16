use crate::simulation::messaging::messages::{InternalSyncMessage};
use std::collections::{HashMap, HashSet};
use crate::simulation::id::Id;
use crate::simulation::scenario::population::InternalPerson;
use crate::simulation::scoring::backpacking::backpacking_scoring_broker::BackpackingMessage;

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

    fn barrier(&self);

    fn rank(&self) -> u32;
    fn extract_leaving_agents(vehicles: &HashMap<u32, InternalSyncMessage>) -> HashMap<u32, Vec<Id<InternalPerson>>>;

    fn register_send_callback(&self, f: Box<dyn Fn(HashMap<u32, Vec<Id<InternalPerson>>>) -> HashMap<u32, BackpackingMessage> + Send>);

    fn register_recv_callback(&self, f: Box<dyn Fn(BackpackingMessage) + Send>);
}