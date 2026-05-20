use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::sync::mpsc::{channel, Receiver, Sender};
use crate::simulation::config::Config;
use crate::simulation::events::{EventHandlerRegisterFn};
use crate::simulation::framework_events::{ControllerEvent, ControllerEventsManager, MobsimListenerRegisterFn, PartitionListenerRegisterFn, RuntimeEvent};
use crate::simulation::id::Id;
use crate::simulation::network::link::SimLink;
use crate::simulation::scenario::network::{Link};
use crate::simulation::scenario::population::Population;
use crate::simulation::scenario::ScenarioPartition;
use crate::simulation::scoring::backpacking::backpacking_data_collector::BackpackingDataCollector;
use crate::simulation::scoring::backpacking::backpacking_scoring_broker::BackpackingMessageBroker;
use crate::simulation::scoring::{InternalScoringMessage};

pub struct BackpackingScoringEngine
{
    backpacking_data_collector: Arc<Mutex<BackpackingDataCollector>>,
    backpacking_message_broker: Arc<Mutex<BackpackingMessageBroker>>
}

impl BackpackingScoringEngine
{
    pub fn new(rank: u32,
               population: &Population,
               receiver: Receiver<InternalScoringMessage>,
               senders: Vec<Sender<InternalScoringMessage>>,
    ) -> Self {
        let backpacking_message_broker = BackpackingMessageBroker::new(receiver, senders, rank);
        let backpacking_data_collector = BackpackingDataCollector::new(population, rank, Arc::clone(&backpacking_message_broker));
        BackpackingMessageBroker::finish(&backpacking_message_broker, Arc::downgrade(&backpacking_data_collector));

        Self {
            backpacking_data_collector,
            backpacking_message_broker
        }

        // TODO Add a callback to start scoring when Mobsim is finished (AfterMobsim event)
    }
}

impl BackpackingScoringEngine
{
    pub fn create_for_n_partitions(partitions: &Vec<Option<ScenarioPartition>>, config: &Arc<Config>, events: &mut ControllerEventsManager) -> (Vec<Box<EventHandlerRegisterFn>>, Vec<Box<PartitionListenerRegisterFn>>, Vec<Box<MobsimListenerRegisterFn>>){
        let num_parts = config.partitioning().num_parts;
        let output_dir = config.output().output_dir.clone();

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
            let scoring = BackpackingScoringEngine::new(
                rank,
                &partition.population,
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
                scoring.backpacking_message_broker.lock().unwrap().add_sender(sender.clone());
            }
            event_register_functions.push(BackpackingDataCollector::register_event_fn(Arc::clone(&scoring.backpacking_data_collector)));
            partition_register_functions.push(BackpackingDataCollector::register_partition_fn(Arc::clone(&scoring.backpacking_data_collector)));
            mobsim_register_functions.push(BackpackingMessageBroker::register_fn(Arc::clone(&scoring.backpacking_message_broker)));
            BackpackingScoringEngine::register(scoring, events, output_dir.clone());
        }

        (event_register_functions, partition_register_functions, mobsim_register_functions)
    }

    fn register(engine: BackpackingScoringEngine, events: &mut ControllerEventsManager, output_dir: PathBuf) {
        events.on_event(move |e: &RuntimeEvent<ControllerEvent>| {
            match e.payload {
                ControllerEvent::Scoring(_) => engine.scoring(output_dir.clone()),
                _ => {}
            }
        });

    }

    fn scoring(&self, mut output_dir: PathBuf) {
        println!("Reached scoring method");
        let population = self.backpacking_data_collector.lock().unwrap().finish();
        output_dir.push("output_plans.binpb");
        population.to_file(output_dir.as_path());

        // TODO Scoring...
    }
}