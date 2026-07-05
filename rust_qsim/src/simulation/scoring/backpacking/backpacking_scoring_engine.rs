use crate::simulation::events::EventHandlerRegisterFn;
use crate::simulation::framework_events::{
    MobsimListenerRegisterFn, PartitionListenerRegisterFn, QSimId,
};
use crate::simulation::scenario::population::Population;
use crate::simulation::scoring::backpacking::backpacking_data_collector::BackpackingDataCollector;
use crate::simulation::scoring::backpacking::backpacking_message_broker::BackpackingMessageBroker;
use crate::simulation::scoring::{InternalScoringMessage, ScoringEngine};
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use tracing::info;

pub struct BackpackingScoringEngine {
    backpacking_data_collector: Arc<Mutex<BackpackingDataCollector>>,
    rank: QSimId,
    output_path: PathBuf,
}

impl BackpackingScoringEngine {
    pub(crate) fn new(
        rank: QSimId,
        population: &Population,
        receiver: Receiver<InternalScoringMessage>,
        senders: Vec<Sender<InternalScoringMessage>>,
        output_path: PathBuf,
    ) -> Self {
        let backpacking_message_broker = BackpackingMessageBroker::new(receiver, senders, rank);
        let backpacking_data_collector = BackpackingDataCollector::new(
            population,
            rank,
            Arc::clone(&backpacking_message_broker),
        );

        Self {
            backpacking_data_collector,
            rank,
            output_path,
        }
    }
}

impl ScoringEngine for BackpackingScoringEngine {
    fn attach_senders(&mut self, senders: Vec<Sender<InternalScoringMessage>>) {
        self.backpacking_data_collector
            .lock()
            .unwrap()
            .attach_senders(senders);
    }

    fn register_fn(
        &self,
    ) -> (
        Box<EventHandlerRegisterFn>,
        Box<PartitionListenerRegisterFn>,
        Box<MobsimListenerRegisterFn>,
    ) {
        (
            BackpackingDataCollector::register_event_fn(self.backpacking_data_collector.clone()),
            BackpackingDataCollector::register_partition_fn(
                self.backpacking_data_collector.clone(),
            ),
            BackpackingDataCollector::register_mobsim_fn(self.backpacking_data_collector.clone()),
        )
    }

    fn finish(&self) {
        let population = self.backpacking_data_collector.lock().unwrap().finish();
        let mut o = self.output_path.clone();
        o.push(format!("plans/output_plans_{}.binpb", self.rank));
        info!("Starting writing PartitionPlans to {:?}", o);
        population.to_file(o.as_path());
        info!("Finished writing PartitionPlans to {:?}", o);
    }

    fn scoring(&self) {
        // TODO
    }
}
