use std::collections::{HashMap};
use std::sync::{Arc, Mutex};
use crate::simulation::id::Id;
use crate::simulation::messaging::sim_communication::SimCommunicator;
use crate::simulation::scenario::population::InternalPerson;
use crate::simulation::scoring::backpacking::backpack::Backpack;
use crate::simulation::scoring::backpacking::backpacking_data_collector::BackpackingDataCollector;

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

        ret.lock().unwrap().communicator.lock().unwrap().register_send_callback(Box::new(move |agent_map: HashMap<u32, Vec<Id<InternalPerson>>>| -> HashMap<u32, BackpackingMessage> {
            let mut scoring_msg: HashMap<u32, BackpackingMessage> = HashMap::default();

            for (k, v) in agent_map.iter() {
                scoring_msg.insert(*k, BackpackingMessage::new(data_collector.lock().unwrap().remove_leaving_passengers(v.clone())));
            }

            scoring_msg
        }));

        let data_collector_cb= ret.lock().unwrap().data_collector.clone();

        ret.lock().unwrap().communicator.lock().unwrap().register_recv_callback(Box::new(move |msg| {
            data_collector_cb.lock().unwrap().add_arriving_passengers(msg.payload)
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
