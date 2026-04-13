use std::any::Any;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use crate::simulation::id::Id;
use crate::simulation::messaging::messages::InternalMessage;
use crate::simulation::messaging::sim_communication::SimCommunicator;
use crate::simulation::scenario::population::InternalPerson;
use crate::simulation::scoring::backpacking::backpack::Backpack;
use crate::simulation::scoring::backpacking::backpacking_data_collector::BackpackingDataCollector;

//TODO currently, the only way of realising an independent message broker was to reference the original
// simcommunicator with a lifetime. A unified and modular solution should be discussed. aleks Apr'26
pub struct BackpackingMessageBroker<C>
where
    C: SimCommunicator + Send
{
    communicator: Arc<Mutex<C>>,
    data_collector: Arc<Mutex<BackpackingDataCollector>>,
}

impl<C> BackpackingMessageBroker<C>
where
    C: SimCommunicator + Send + 'static,
{
    pub fn new(communicator: Arc<Mutex<C>>, data_collector: Arc<Mutex<BackpackingDataCollector>>) -> Arc<Mutex<Self>> {
        let ret = Arc::new(Mutex::new(Self {
            communicator: communicator.clone(),
            data_collector: Arc::clone(&data_collector)
        }));
        ret.lock().unwrap().communicator.lock().unwrap().register_scoring_callback(Box::new(move |agent_map| {
            let mut scoring_msg: HashMap<u32, Box<dyn InternalMessage>>= HashMap::default();

            for (k, v) in agent_map.iter() {
                scoring_msg.insert(*k, Box::new(BackpackingMessage::new(data_collector.lock().unwrap().remove_leaving_passengers(v.clone()))));
            }

            communicator.lock().unwrap().send_receive_others(scoring_msg, &mut HashSet::default(), 0, |msg| {
                // TODO Receive function
                // data_collector.lock().unwrap().add_arriving_passengers(msg.as_any().downcast_ref::<BackpackingMessage>().unwrap().payload);
            });
        }));
        ret
    }

}

pub struct BackpackingMessage {
    payload: HashMap<Id<InternalPerson>, Backpack>
}

impl BackpackingMessage {
    pub fn new(payload: HashMap<Id<InternalPerson>, Backpack>) -> Self {
        Self { payload }
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

    fn as_any(&self) -> &dyn Any {
        todo!()
    }
}