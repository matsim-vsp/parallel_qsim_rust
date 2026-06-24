use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::sync::mpsc::{Receiver, Sender};
use crate::simulation::events::EventHandlerRegisterFn;
use crate::simulation::framework_events::{MobsimListenerRegisterFn, PartitionListenerRegisterFn, QSimId};
use crate::simulation::id::Id;
use crate::simulation::scenario::population::{InternalPerson, Population};
use crate::simulation::scoring::{InternalScoringMessage, ScoringEngine};
use crate::simulation::scoring::mapping::mapping_message_broker::{MappingCollectorMessageBroker, MappingScoringMessageBroker};
use crate::simulation::scoring::mapping::mapping_data_collector::MappingDataCollector;
use crate::simulation::scoring::mapping::mapping_data_forwarder::MappingDataForwarder;

/// Attached to the Mobsim threads listening for events and forwarding them to the scoring threads.
pub struct MappingCollectorEngine {
    mapping_data_collector: Arc<Mutex<MappingDataForwarder>>,
    mapping_message_broker: Arc<Mutex<MappingCollectorMessageBroker>>,
    rank: QSimId,
    
    output_path: PathBuf
}

/// Parallel thread set collecting the partial plans.
pub struct MappingScoringEngine {
    mapping_data_collector: Arc<Mutex<MappingDataCollector>>,
    mapping_message_broker: Arc<Mutex<MappingScoringMessageBroker>>,
    rank: QSimId,
}

impl ScoringEngine for MappingScoringEngine {
    fn attach_senders(&mut self, senders: Vec<Sender<InternalScoringMessage>>) {
        todo!()
    }

    fn register_fn(&self) -> (Box<EventHandlerRegisterFn>, Box<PartitionListenerRegisterFn>, Box<MobsimListenerRegisterFn>) {
        todo!()
    }

    fn finish(&self) {
        todo!()
    }

    fn scoring(&self) {
        todo!()
    }
}

impl MappingScoringEngine {
    pub(crate) fn new(
        rank: QSimId,
        person_hash_function: Box<dyn Fn(Id<InternalPerson>) -> u32 + Send>,
        num_partitions: usize,
        num_scoring_threads: usize,
        partition_id2person_id: HashMap<QSimId, Vec<Id<InternalPerson>>>,
        receiver: Receiver<InternalScoringMessage>,
        senders: Vec<Sender<InternalScoringMessage>>,
    ) -> Self {
        let mapping_message_broker = MappingScoringMessageBroker::new(receiver, senders, rank, num_partitions, num_scoring_threads, partition_id2person_id);
        let mapping_data_collector = MappingDataCollector::new(person_hash_function, num_partitions as u32, Arc::clone(&mapping_message_broker));
        MappingScoringMessageBroker::init(&mapping_message_broker, Arc::downgrade(&mapping_data_collector));

        Self {
            mapping_data_collector,
            mapping_message_broker,
            rank
        }
    }
}