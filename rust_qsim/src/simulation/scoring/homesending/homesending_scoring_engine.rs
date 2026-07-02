use crate::simulation::events::EventHandlerRegisterFn;
use crate::simulation::framework_events::{
    MobsimListenerRegisterFn, PartitionListenerRegisterFn, QSimId,
};
use crate::simulation::id::Id;
use crate::simulation::scenario::population::{InternalPerson, Population};
use crate::simulation::scoring::homesending::homesending_data_collector::HomeSendingDataCollector;
use crate::simulation::scoring::homesending::homesending_message_broker::HomeSendingMessageBroker;
use crate::simulation::scoring::{InternalScoringMessage, ScoringEngine};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use tracing::info;

pub struct HomesendingScoringEngine {
    homesending_data_collector: Arc<Mutex<HomeSendingDataCollector>>,
    homesending_message_broker: Arc<Mutex<HomeSendingMessageBroker>>,
    rank: QSimId,

    output_path: PathBuf,
}

impl HomesendingScoringEngine {
    pub(crate) fn new(
        rank: QSimId,
        population: &Population,
        num_partitions: usize,
        person_id2_partition_id: HashMap<Id<InternalPerson>, QSimId>,
        receiver: Receiver<InternalScoringMessage>,
        senders: Vec<Sender<InternalScoringMessage>>,
        output_path: PathBuf,
    ) -> Self {
        let homesending_message_broker =
            HomeSendingMessageBroker::new(receiver, senders, num_partitions, rank);
        let homesending_data_collector = HomeSendingDataCollector::new(
            population,
            person_id2_partition_id,
            rank,
            Arc::clone(&homesending_message_broker),
        );
        HomeSendingMessageBroker::init(
            &homesending_message_broker,
            Arc::downgrade(&homesending_data_collector),
        );

        Self {
            homesending_data_collector,
            homesending_message_broker,
            rank,
            output_path,
        }
    }
}

impl ScoringEngine for HomesendingScoringEngine {
    fn attach_senders(&mut self, senders: Vec<Sender<InternalScoringMessage>>) {
        self.homesending_message_broker
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
            HomeSendingDataCollector::register_event_fn(self.homesending_data_collector.clone()),
            HomeSendingDataCollector::register_partition_fn(
                self.homesending_data_collector.clone(),
            ),
            HomeSendingMessageBroker::register_fn(self.homesending_message_broker.clone()),
        )
    }

    fn finish(&self) {
        self.homesending_message_broker
            .lock()
            .unwrap()
            .finish_sync();
        let population = self.homesending_data_collector.lock().unwrap().finish();
        let mut o = self.output_path.clone();
        o.push(format!("scoring/output_plans_{}.binpb", self.rank));
        info!("Starting writing PartitionPlans to {:?}", o);
        population.to_file(o.as_path());
        info!("Finished writing PartitionPlans to {:?}", o);
    }

    fn scoring(&self) {
        // TODO
    }
}
