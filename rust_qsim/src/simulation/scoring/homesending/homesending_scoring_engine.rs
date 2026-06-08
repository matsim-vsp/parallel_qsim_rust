use crate::simulation::config::Config;
use crate::simulation::events::EventHandlerRegisterFn;
use crate::simulation::framework_events::{ControllerEvent, ControllerEventsManager, MobsimListenerRegisterFn, PartitionListenerRegisterFn, QSimId, RuntimeEvent};
use crate::simulation::id::Id;
use crate::simulation::io;
use crate::simulation::network::link::SimLink;
use crate::simulation::scenario::network::Link;
use crate::simulation::scenario::population::{InternalPerson, Population};
use crate::simulation::scenario::ScenarioPartition;
use crate::simulation::scoring::homesending::homesending_data_collector::HomeSendingDataCollector;
use crate::simulation::scoring::homesending::homesending_message_broker::HomeSendingMessageBroker;
use crate::simulation::scoring::InternalScoringMessage;
use nohash_hasher::IntSet;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use tracing::info;
use crate::generated::population::Person;
use crate::simulation::scoring::backpacking::backpacking_scoring_engine::BackpackingScoringEngine;

pub struct HomesendingScoringEngine
{
    homesending_data_collector: Arc<Mutex<HomeSendingDataCollector>>,
    homesending_message_broker: Arc<Mutex<HomeSendingMessageBroker>>,
    rank: QSimId
}

impl HomesendingScoringEngine {
    fn new(rank: QSimId,
           population: &Population,
           neighbours: IntSet<u32>,
           person_id2_partition_id: HashMap<Id<InternalPerson>, QSimId>,
           receiver: Receiver<InternalScoringMessage>,
           senders: Vec<Sender<InternalScoringMessage>>
    ) -> Self {
        let homesending_message_broker = HomeSendingMessageBroker::new(receiver, senders, neighbours, rank);
        let homesending_data_collector = HomeSendingDataCollector::new(population, person_id2_partition_id, rank, Arc::clone(&homesending_message_broker));
        HomeSendingMessageBroker::init(&homesending_message_broker, Arc::downgrade(&homesending_data_collector));

        Self {
            homesending_data_collector,
            homesending_message_broker,
            rank
        }
    }

    pub fn create_for_n_partitions(partitions: &Vec<Option<ScenarioPartition>>, config: &Arc<Config>, events: &mut ControllerEventsManager) -> (Vec<Box<EventHandlerRegisterFn>>, Vec<Box<PartitionListenerRegisterFn>>, Vec<Box<MobsimListenerRegisterFn>>){
        let num_parts = config.partitioning().num_parts;
        let output_path = io::resolve_path(config.context(), &config.output().output_dir);

        // Prepare person_id2home_partition map
        let mut person_id2home_partition: HashMap<Id<InternalPerson>, QSimId> = HashMap::new();
        for (i, partition) in partitions.iter().enumerate() {
            let partition = partition.as_ref().unwrap();

            for person in partition.population.persons.keys() {
                person_id2home_partition.insert(person.clone(), i as QSimId);
            }
        }

        // Create ScoringEngines with channels
        let mut senders: Vec<_> = Vec::new();
        let mut scorings: Vec<_> = Vec::new();

        for rank in 0..num_parts {
            let partition = partitions.get(rank as usize).unwrap().as_ref().unwrap();

            // Generate cut link map for current partition
            let mut link_id2_target_partition: HashMap<Id<Link>, u32> = HashMap::new();
            for (id, link) in partition.network_partition.links.iter() {
                match link {
                    SimLink::Out(split) => {
                        link_id2_target_partition.insert(id.clone(), split.to_part);
                    }
                    _ => {}
                }
            }

            let (sender, receiver) = channel();
            let scoring = HomesendingScoringEngine::new(
                rank,
                &partition.population,
                partition.network_partition.neighbors(),
                person_id2home_partition.clone(),
                receiver,
                vec![],
            );
            senders.push(sender);
            scorings.push(scoring);
        }

        let mut event_register_functions = Vec::new();
        let mut partition_register_functions = Vec::new();
        let mut mobsim_register_functions = Vec::new();

        for scoring in scorings.drain(..) {
            for sender in &senders {
                scoring.homesending_message_broker.lock().unwrap().add_sender(sender.clone());
            }
            event_register_functions.push(HomeSendingDataCollector::register_event_fn(Arc::clone(&scoring.homesending_data_collector)));
            partition_register_functions.push(HomeSendingDataCollector::register_partition_fn(Arc::clone(&scoring.homesending_data_collector)));
            mobsim_register_functions.push(HomeSendingMessageBroker::register_fn(Arc::clone(&scoring.homesending_message_broker)));
            HomesendingScoringEngine::register(scoring, events, output_path.clone());
        }

        (event_register_functions, partition_register_functions, mobsim_register_functions)
    }

    fn register(engine: HomesendingScoringEngine, events: &mut ControllerEventsManager, output_path: PathBuf) {
        events.on_event(move |e: &RuntimeEvent<ControllerEvent>| {
            match e.payload {
                ControllerEvent::Scoring(_) => engine.scoring(output_path.clone()),
                _ => {}
            }
        });

    }

    fn scoring(&self, mut output_path: PathBuf) {
        let population = self.homesending_data_collector.lock().unwrap().finish();
        output_path.push(format!("scoring/output_plans_{}.binpb", self.rank));
        info!("Starting writing PartitionPlans to {:?}", output_path);
        population.to_file(output_path.as_path());
        info!("Finished writing PartitionPlans to {:?}", output_path);

        // TODO Scoring...
    }
}
