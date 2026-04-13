use std::sync::{Arc, Mutex};
use crate::simulation::messaging::messages::InternalMessage;
use crate::simulation::messaging::sim_communication::SimCommunicator;
use crate::simulation::scoring::backpacking::backpacking_data_collector::BackpackingDataCollector;

//TODO currently, the only way of realising an independent message broker was to reference the original
// simcommunicator with a lifetime. A unified and modular solution should be discussed. aleks Apr'26
pub struct BackpackingMessageBroker<C>
where
    C: SimCommunicator
{
    communicator: Arc<C>,
    data_collector: Arc<Mutex<BackpackingDataCollector>>,
}

impl<C> BackpackingMessageBroker<C>
where
    C: SimCommunicator + 'static,
{
    pub fn new(communicator: Arc<C>, data_collector: Arc<Mutex<BackpackingDataCollector>>) -> Arc<Mutex<Self>> {
        let ret = Arc::new(Mutex::new(Self {
            communicator: communicator.clone(),
            data_collector
        }));
        // TODO register callback
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

pub struct BackpackingMessage {

}

impl BackpackingMessage {
    pub fn new() -> Self {
        Self {}
    }
}

impl InternalMessage for BackpackingMessage {
    fn time(&self) -> u32 {
        todo!()
    }

    fn from_process(&self) -> u32 {
        todo!()
    }

    fn to_process(&self) -> u32 {
        todo!()
    }
}