use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::{Rc};
use std::sync::{Arc, Mutex};
use std::sync::mpsc::{Receiver, Sender};
use crate::simulation::events::EventsManager;
use crate::simulation::framework_events::MobsimEventsManager;
use crate::simulation::id::Id;
use crate::simulation::scenario::network::Link;
use crate::simulation::scenario::population::Population;
use crate::simulation::scoring::backpacking::backpacking_data_collector::BackpackingDataCollector;
use crate::simulation::scoring::backpacking::backpacking_scoring_broker::BackpackingMessageBroker;
use crate::simulation::scoring::{InternalScoringMessage, ScoringEngine};

pub struct BackpackingScoringEngine
{
    backpacking_data_collector: Arc<Mutex<BackpackingDataCollector>>,
    backpacking_message_broker: Arc<Mutex<BackpackingMessageBroker>>
}

impl BackpackingScoringEngine
{
    pub fn new(rank: u32,
               population: &Population,
               events_manager: Rc<RefCell<EventsManager>>,
               mobsim_events_manager: Rc<RefCell<MobsimEventsManager>>,
               link_id2target_partition: HashMap<Id<Link>, u32>,
               receiver: Receiver<InternalScoringMessage>,
               senders: Vec<Sender<InternalScoringMessage>>,
    ) -> Self {
        let backpacking_message_broker = BackpackingMessageBroker::new(receiver, senders, rank);
        let backpacking_data_collector = BackpackingDataCollector::new(population, events_manager, link_id2target_partition, rank, Arc::clone(&backpacking_message_broker));
        BackpackingMessageBroker::finish(&backpacking_message_broker, mobsim_events_manager, Arc::downgrade(&backpacking_data_collector));

        Self {
            backpacking_data_collector,
            backpacking_message_broker
        }
    }
}

impl ScoringEngine for BackpackingScoringEngine
{
    fn create_for_n_partitions(n: u32) {

    }

    fn scoring(&self) {
        todo!()
    }
}