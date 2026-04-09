use std::collections::{HashMap, HashSet};
use std::mem::take;
use std::rc::Rc;
use crate::simulation::messaging::messages::InternalMessage;
use crate::simulation::messaging::sim_communication::SimCommunicator;

//TODO currently, the only way of realising an independent message broker was to reference the original
// simcommunicator with a lifetime. A unified and modular solution should be discussed. aleks Apr'26
pub struct BackpackingMessageBroker<C>
where
    C: SimCommunicator
{
    communicator: Rc<C>,
    // message_batch: HashMap<u32, Box<dyn InternalMessage>>,
    // expected_messages: HashSet<u32>
}

impl<C> BackpackingMessageBroker<C>
where
    C: SimCommunicator,
{
    pub fn new(communicator: Rc<C>) -> Self {
        let ret = Self {
            communicator,
            // message_batch: HashMap::new(),
            // expected_messages: HashSet::new()
        };
        ret.communicator.register_recv_callback(Box::new(|msg| {
            // TODO
        }));
        ret
    }

    // pub fn send_recv_message_batch<F>(&mut self, now: u32, on_msg: F)
    // where
    //     F: FnMut(Box<dyn InternalMessage>)
    // {
    //     self.communicator.send_receive_others(take(&mut self.message_batch), &mut self.expected_messages, now, on_msg);
    //     self.message_batch.clear(); // TODO Check if needed: take() should already clear the map
    //     self.expected_messages.clear();
    // }

}
