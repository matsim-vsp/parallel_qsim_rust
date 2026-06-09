use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::sync::mpsc::{channel, Receiver, Sender};
use nohash_hasher::IntSet;
use tracing::info;
use crate::simulation::config::Config;
use crate::simulation::events::{EventHandlerRegisterFn};
use crate::simulation::framework_events::{ControllerEvent, ControllerEventsManager, MobsimListenerRegisterFn, PartitionListenerRegisterFn, QSimId, RuntimeEvent};
use crate::simulation::id::Id;
use crate::simulation::io;
use crate::simulation::network::link::SimLink;
use crate::simulation::scenario::network::{Link};
use crate::simulation::scenario::population::Population;
use crate::simulation::scenario::ScenarioPartition;
use crate::simulation::scoring::backpacking::backpacking_data_collector::BackpackingDataCollector;
use crate::simulation::scoring::backpacking::backpacking_message_broker::BackpackingMessageBroker;
use crate::simulation::scoring::{InternalScoringMessage, ScoringEngine};

pub struct BackpackingScoringEngine
{
    backpacking_data_collector: Arc<Mutex<BackpackingDataCollector>>,
    backpacking_message_broker: Arc<Mutex<BackpackingMessageBroker>>,
    rank: QSimId
}

impl BackpackingScoringEngine
{
    pub(crate) fn new(rank: QSimId,
               population: &Population,
               neighbours: IntSet<u32>,
               receiver: Receiver<InternalScoringMessage>,
               senders: Vec<Sender<InternalScoringMessage>>,
    ) -> Self {
        let backpacking_message_broker = BackpackingMessageBroker::new(receiver, senders, neighbours, rank);
        let backpacking_data_collector = BackpackingDataCollector::new(population, rank, Arc::clone(&backpacking_message_broker));
        BackpackingMessageBroker::init(&backpacking_message_broker, Arc::downgrade(&backpacking_data_collector));

        Self {
            backpacking_data_collector,
            backpacking_message_broker,
            rank
        }
    }
}

impl ScoringEngine for BackpackingScoringEngine {
    fn attach_senders(&mut self, senders: Vec<Sender<InternalScoringMessage>>) {
        self.backpacking_message_broker.lock().unwrap().attach_senders(senders);
    }

    fn register_fn(&self) -> (Box<EventHandlerRegisterFn>, Box<PartitionListenerRegisterFn>, Box<MobsimListenerRegisterFn>) {
        (
            BackpackingDataCollector::register_event_fn(self.backpacking_data_collector.clone()),
            BackpackingDataCollector::register_partition_fn(self.backpacking_data_collector.clone()),
            BackpackingMessageBroker::register_fn(self.backpacking_message_broker.clone())
        )
    }

    fn finish(&self, mut output_path: PathBuf) {
        let population = self.backpacking_data_collector.lock().unwrap().finish();
        output_path.push(format!("scoring/output_plans_{}.binpb", self.rank));
        info!("Starting writing PartitionPlans to {:?}", output_path);
        population.to_file(output_path.as_path());
        info!("Finished writing PartitionPlans to {:?}", output_path);
    }

    fn scoring(&self) {
        todo!()
    }
}