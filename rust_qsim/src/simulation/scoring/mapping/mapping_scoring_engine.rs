use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use crate::simulation::framework_events::QSimId;
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
    mapping_message_broker: Arc<Mutex<MappingScoringMessageBroker>>
}