use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::sync::mpsc::{Receiver, Sender};
use crate::simulation::events::EventHandlerRegisterFn;
use crate::simulation::framework_events::{MobsimListenerRegisterFn, PartitionListenerRegisterFn, QSimId};
use crate::simulation::id::Id;
use crate::simulation::scenario::population::{InternalPerson, Population};
use crate::simulation::scenario::vehicles::InternalVehicle;
use crate::simulation::scoring::{InternalScoringMessage, ScoringEngine};
use crate::simulation::scoring::mapping::mapping_message_broker::{MappingCollectorMessageBroker, MappingScoringMessageBroker};
use crate::simulation::scoring::mapping::mapping_data_collector::MappingDataCollector;
use crate::simulation::scoring::mapping::mapping_data_forwarder::MappingDataForwarder;

/// Attached to the Mobsim threads listening for events and forwarding them to the scoring threads.
pub struct MappingCollectorEngine {
    mapping_data_forwarder: Arc<Mutex<MappingDataForwarder>>,
    mapping_message_broker: Arc<Mutex<MappingCollectorMessageBroker>>,
    rank: QSimId,
    
    output_path: PathBuf
}

impl MappingCollectorEngine {
    pub(crate) fn new(
        rank: QSimId,
        person_hash_function: Box<dyn Fn(Id<InternalPerson>) -> u32 + Send>,
        vehicle_hash_function: Box<dyn Fn(Id<InternalVehicle>) -> u32 + Send>,
        num_partitions: usize,
        num_collectors: usize,
        receiver: Receiver<InternalScoringMessage>,
        senders: Vec<Sender<InternalScoringMessage>>,
        output_path: PathBuf,
    ) -> Self {
        let mapping_message_broker = MappingCollectorMessageBroker::new(receiver, senders, rank, num_partitions, num_collectors);
        let mapping_data_forwarder = MappingDataForwarder::new(person_hash_function, vehicle_hash_function, rank, num_partitions as u32, Arc::clone(&mapping_message_broker));
        MappingCollectorMessageBroker::init(&mapping_message_broker, Arc::downgrade(&mapping_data_forwarder));

        Self {
            mapping_data_forwarder,
            mapping_message_broker,
            rank,
            output_path
        }
    }
}

impl ScoringEngine for MappingCollectorEngine {
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

/// Parallel thread set collecting the partial plans.
pub struct MappingScoringEngine {
    mapping_data_collector: Arc<Mutex<MappingDataCollector>>,
    mapping_message_broker: Arc<Mutex<MappingScoringMessageBroker>>,
    rank: QSimId,
}

impl MappingScoringEngine {
    pub(crate) fn new(
        rank: QSimId,
        person_hash_function: Box<dyn Fn(Id<InternalPerson>) -> u32 + Send>,
        num_partitions: usize,
        num_collectors: usize,
        partition_id2person_id: HashMap<QSimId, Vec<Id<InternalPerson>>>,
        receiver: Receiver<InternalScoringMessage>,
        senders: Vec<Sender<InternalScoringMessage>>,
    ) -> Self {
        let mapping_message_broker = MappingScoringMessageBroker::new(receiver, senders, rank, num_partitions, num_collectors, partition_id2person_id);
        let mapping_data_collector = MappingDataCollector::new(person_hash_function, num_partitions as u32, Arc::clone(&mapping_message_broker));
        MappingScoringMessageBroker::init(&mapping_message_broker, Arc::downgrade(&mapping_data_collector));

        Self {
            mapping_data_collector,
            mapping_message_broker,
            rank
        }
    }

    pub fn attach_senders(&mut self, senders: Vec<Sender<InternalScoringMessage>>) {
        self.mapping_message_broker.lock().unwrap().attach_senders(senders);
    }

    pub fn work(&mut self) {
        self.mapping_message_broker.lock().unwrap().work();
    }
}