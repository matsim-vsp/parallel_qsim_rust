use crate::simulation::messaging::sim_communication::SimCommunicator;
use crate::simulation::scoring::backpacking::backpacking_data_collector::BackpackingDataCollector;
use crate::simulation::scoring::backpacking::backpacking_scoring_broker::BackpackingMessageBroker;
use crate::simulation::scoring::ScoringEngine;

pub struct BackpackingScoringEngine<'a, C>
where
    C: SimCommunicator
{
    backpacking_data_collector: BackpackingDataCollector,
    backpacking_message_broker: BackpackingMessageBroker<'a, C>
}

impl<'a, C> BackpackingScoringEngine<'a, C>
where
    C: SimCommunicator
{
    pub fn new(communicator: &'a C) -> Self {
        Self {
            backpacking_data_collector: BackpackingDataCollector::new(),
            backpacking_message_broker: BackpackingMessageBroker::new(communicator)
        }
    }
}

impl<'a, C> ScoringEngine<C> for BackpackingScoringEngine<'a, C>
where
    C: SimCommunicator
{
    fn scoring(&self) {
        // TODO
    }
}