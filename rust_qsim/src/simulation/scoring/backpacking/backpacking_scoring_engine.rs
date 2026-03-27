use crate::simulation::scoring::backpacking::backpacking_data_collector::BackpackingDataCollector;
use crate::simulation::scoring::backpacking::backpacking_scoring_broker::BackpackingMessageBroker;
use crate::simulation::scoring::ScoringEngine;

struct BackpackingScoringEngine{
    backpacking_data_collector: BackpackingDataCollector,
    backpacking_message_broker: BackpackingMessageBroker
}

impl ScoringEngine for BackpackingScoringEngine {
    fn scoring() {
        // TODO
    }
}