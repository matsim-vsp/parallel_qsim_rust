use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use crate::simulation::events::EventsManager;
use crate::simulation::messaging::sim_communication::message_broker::NetMessageBroker;
use crate::simulation::messaging::sim_communication::SimCommunicator;
use crate::simulation::scenario::population::Population;
use crate::simulation::scoring::backpacking::backpacking_data_collector::BackpackingDataCollector;
use crate::simulation::scoring::backpacking::backpacking_scoring_broker::BackpackingMessageBroker;
use crate::simulation::scoring::ScoringEngine;

pub struct BackpackingScoringEngine<C>
where
    C: SimCommunicator
{
    backpacking_data_collector: Arc<Mutex<BackpackingDataCollector>>,
    backpacking_message_broker: BackpackingMessageBroker<C>
}

impl<C> BackpackingScoringEngine<C>
where
    C: SimCommunicator
{
    pub fn new(partition: u32, population: &Population, communicator: Rc<C>, events_manager: Rc<RefCell<EventsManager>>) -> Self {
        Self {
            backpacking_data_collector: BackpackingDataCollector::new(partition, population, events_manager),
            backpacking_message_broker: BackpackingMessageBroker::new(communicator)
        }
    }
}

impl<'a, C> ScoringEngine<C> for BackpackingScoringEngine<C>
where
    C: SimCommunicator
{
    fn scoring(&self) {
        // TODO
    }
}