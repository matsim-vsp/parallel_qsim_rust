use crate::simulation::events::EventHandlerRegisterFn;
use crate::simulation::framework_events::{
    MobsimListenerRegisterFn, PartitionListenerRegisterFn, QSimId,
};
use crate::simulation::id::Id;
use crate::simulation::scenario::population::InternalPerson;
use crate::simulation::scenario::vehicles::InternalVehicle;
use crate::simulation::scoring::mapping::mapping_data_collector::MappingDataCollector;
use crate::simulation::scoring::mapping::mapping_data_forwarder::MappingDataForwarder;
use crate::simulation::scoring::mapping::mapping_message_broker::{
    MappingCollectorMessageBroker, MappingScoringMessageBroker,
};
use crate::simulation::scoring::{InternalScoringMessage, ScoringEngine};
use nohash_hasher::IntMap;
use std::path::PathBuf;
use hotpath::wrap::std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use tracing::{info, info_span};

/// Attached to the Mobsim threads listening for events and forwarding them to the scoring threads.
pub struct MappingForwardingEngine {
    mapping_data_forwarder: Arc<Mutex<MappingDataForwarder>>,
    mapping_message_broker: Arc<Mutex<MappingCollectorMessageBroker>>,
    rank: QSimId,

    output_path: PathBuf,
}

#[hotpath::measure_all]
impl MappingForwardingEngine {
    pub(crate) fn new(
        rank: QSimId,
        person_hash_function: Box<dyn Fn(Id<InternalPerson>) -> u32 + Send>,
        vehicle_hash_function: Box<dyn Fn(Id<InternalVehicle>) -> u32 + Send>,
        num_partitions: usize,
        num_collectors: usize,
        sync_interval: u32,
        receiver: Receiver<InternalScoringMessage>,
        senders: Vec<Sender<InternalScoringMessage>>,
        output_path: PathBuf,
    ) -> Self {
        let mut bytes_path = output_path.clone();
        bytes_path.push(format!("bytes/scoring_bytes_{}.csv", rank));
        let mapping_message_broker = MappingCollectorMessageBroker::new(
            receiver,
            senders,
            rank,
            num_partitions,
            num_collectors,
            sync_interval,
            bytes_path,
        );
        let mapping_data_forwarder = MappingDataForwarder::new(
            person_hash_function,
            vehicle_hash_function,
            num_partitions as u32,
            Arc::clone(&mapping_message_broker),
        );
        MappingCollectorMessageBroker::init(
            &mapping_message_broker,
            Arc::downgrade(&mapping_data_forwarder),
        );

        Self {
            mapping_data_forwarder,
            mapping_message_broker,
            rank,
            output_path,
        }
    }
}

#[hotpath::measure_all]
impl ScoringEngine for MappingForwardingEngine {
    fn attach_senders(&mut self, senders: Vec<Sender<InternalScoringMessage>>) {
        self.mapping_message_broker
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
            MappingDataForwarder::register_event_fn(self.rank, self.mapping_data_forwarder.clone()),
            Box::new(|_| {}),
            MappingCollectorMessageBroker::register_fn(self.mapping_message_broker.clone()),
        )
    }

    fn finish(&self) {
        let _finish_span = info_span!("scoring.finish", rank = self.rank as u64).entered();
        self.mapping_message_broker
            .lock()
            .unwrap()
            .finish_send_recv();

        let population = self.mapping_data_forwarder.lock().unwrap().finish();
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

/// Parallel thread set collecting the partial plans.
pub struct MappingCollectorEngine {
    // TODO For the final version, Check whether this reference can be really removed
    #[allow(unused)]
    mapping_data_collector: Arc<Mutex<MappingDataCollector>>,
    mapping_message_broker: Arc<Mutex<MappingScoringMessageBroker>>,
}

#[hotpath::measure_all]
impl MappingCollectorEngine {
    pub(crate) fn new(
        rank: QSimId,
        person_hash_function: Box<dyn Fn(Id<InternalPerson>) -> u32 + Send>,
        num_partitions: usize,
        num_collectors: usize,
        person_id2home_partition: IntMap<Id<InternalPerson>, QSimId>,
        receiver: Receiver<InternalScoringMessage>,
        senders: Vec<Sender<InternalScoringMessage>>,
        bytes_path: PathBuf,
    ) -> Self {
        let mapping_message_broker = MappingScoringMessageBroker::new(
            receiver,
            senders,
            rank,
            num_partitions,
            num_collectors,
            person_id2home_partition,
            bytes_path,
        );
        let mapping_data_collector = MappingDataCollector::new(
            person_hash_function,
            num_partitions as u32,
            num_collectors as u32,
            Arc::clone(&mapping_message_broker),
        );
        MappingScoringMessageBroker::init(
            &mapping_message_broker,
            Arc::downgrade(&mapping_data_collector),
        );

        Self {
            mapping_data_collector,
            mapping_message_broker,
        }
    }

    pub fn attach_senders(&mut self, senders: Vec<Sender<InternalScoringMessage>>) {
        self.mapping_message_broker
            .lock()
            .unwrap()
            .attach_senders(senders);
    }

    pub fn work(&mut self) {
        self.mapping_message_broker.lock().unwrap().work();
    }
}
