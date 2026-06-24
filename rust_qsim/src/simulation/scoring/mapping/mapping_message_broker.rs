use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, Weak};
use std::sync::mpsc::{Receiver, Sender};
use crate::simulation::events::EventTrait;
use crate::simulation::framework_events::{MobsimEvent, MobsimEventsManager, MobsimListenerRegisterFn, QSimId, RuntimeEvent};
use crate::simulation::id::Id;
use crate::simulation::scenario::population::{InternalPerson, InternalPlan};
use crate::simulation::scenario::vehicles::InternalVehicle;
use crate::simulation::scoring::InternalScoringMessage;
use crate::simulation::scoring::mapping::mapping_data_collector::MappingDataCollector;
use crate::simulation::scoring::mapping::mapping_data_forwarder::MappingDataForwarder;

pub struct MappingCollectorMessageBroker {
    receiver: Receiver<InternalScoringMessage>,
    senders: Vec<Sender<InternalScoringMessage>>,
    rank: QSimId,
    num_partitions: usize,
    num_scoring_threads: usize,

    buffer_events: HashMap<QSimId, HashMap<Id<InternalPerson>, Vec<Box<dyn EventTrait>>>>,
    buffer_vehicles: HashMap<u32, HashMap<Id<InternalVehicle>, Vec<Box<dyn EventTrait>>>>,
    data_forwarder: Weak<Mutex<MappingDataForwarder>>
}

impl MappingCollectorMessageBroker {
    pub fn new(receiver: Receiver<InternalScoringMessage>, senders: Vec<Sender<InternalScoringMessage>>, rank: QSimId, num_partitions: usize, num_scoring_threads: usize,) -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(Self { receiver, senders, rank, num_partitions, num_scoring_threads, buffer_events: HashMap::new(), buffer_vehicles: HashMap::new(), data_forwarder: Weak::new() }))
    }

    pub(crate) fn attach_senders(&mut self, senders: Vec<Sender<InternalScoringMessage>>) {
        self.senders.extend(senders);
    }

    pub(crate) fn init(message_broker: &Arc<Mutex<Self>>, data_collector: Weak<Mutex<MappingDataForwarder>>) {
        message_broker.lock().unwrap().data_forwarder = data_collector;
    }

    pub(crate) fn register_fn(scoring_broker: Arc<Mutex<MappingCollectorMessageBroker>>) -> Box<MobsimListenerRegisterFn> {
        Box::new(move |events: &mut MobsimEventsManager| {
            let broker = Arc::clone(&scoring_broker);
            events.on_event(move |e: &RuntimeEvent<MobsimEvent>| {
                match &e.payload {
                    MobsimEvent::AfterSimStep(i) => {
                        broker.lock().unwrap().send();
                    }
                    MobsimEvent::BeforeCleanup => {
                        broker.lock().unwrap().finish_send_recv();
                    }
                    _ => {}
                }
            });
        })
    }

    pub(crate) fn add_leaving_person_event(&mut self, target: u32, person_id: Id<InternalPerson>, event: Box<dyn EventTrait>) {
        self.buffer_events.entry(target).or_insert_with(|| HashMap::new()).entry(person_id).or_insert_with(|| Vec::default()).push(event);
    }


    pub(crate) fn add_leaving_vehicle_event(&mut self, target: u32, vehicle_id: Id<InternalVehicle>, event: Box<dyn EventTrait>) {
        self.buffer_vehicles.entry(target).or_insert_with(|| HashMap::new()).entry(vehicle_id).or_insert_with(|| Vec::default()).push(event);
    }

    /// Called on every AfterSimStep: Flushes send buffers
    fn send(&mut self) {
        for (target, vehicle_events) in self.buffer_vehicles.drain() {
            let msg = InternalScoringMessage {
                from_process: self.rank,
                to_process: target,
                message: Box::new(VehicleEventMessage { events: vehicle_events }),
            };

            self.senders[target as usize].send(msg).unwrap_or_else(|e| {
                panic!("Error sending VehicleMessage to rank {} with error {}", target, e)
            });
        }

        for (target, events) in self.buffer_events.drain() {
            let msg = InternalScoringMessage {
                from_process: self.rank,
                to_process: target,
                message: Box::new(PersonEventMessage { events }),
            };

            self.senders[target as usize].send(msg).unwrap_or_else(|e| {
                panic!("Error sending VehicleMessage to rank {} with error {}", target, e)
            });
        }
    }

    /// Called after the mobsim ends: Flushes the send buffers and sends a finish message to all scoring threads.
    /// Then collects incoming Experienced Plans and passes them to the forwarder
    fn finish_send_recv(&mut self) {
        self.send();

        for target in (0..self.num_scoring_threads).map(|t| t + self.num_partitions) {
            let msg = InternalScoringMessage {
                from_process: self.rank,
                to_process: target as QSimId,
                message: Box::new(ScoringFinishMessage {}),
            };
            self.senders[target].send(msg).unwrap_or_else(|e| {
                panic!("Error sending FinishMessage to rank {} with error {}", target, e)
            });
        }

        // TODO Use finished instead of this for loop
        for _ in 0..self.num_scoring_threads {
            let received_msg = self.receiver.recv().unwrap();

            let boxed_any = received_msg.message.into_any();
            match () {
                _ if boxed_any.is::<InternalPlanMessage>() => {
                    let m = boxed_any.downcast::<InternalPlanMessage>().unwrap();
                    for (person_id, plan) in m.plans {
                        self.data_forwarder.upgrade().unwrap().lock().unwrap().add_arriving_plan(person_id, plan);
                    }
                }
                _ => {
                    panic!("Received unexpected message type during simulation step!");
                }
            }
        }
    }
}

pub struct MappingScoringMessageBroker {
    receiver: Receiver<InternalScoringMessage>,
    senders: Vec<Sender<InternalScoringMessage>>,
    rank: QSimId,
    num_partitions: usize,
    num_scoring_threads: usize,
    partition_id2person_id: HashMap<QSimId, Vec<Id<InternalPerson>>>,

    buffer_events: HashMap<u32, HashMap<Id<InternalPerson>, Vec<Box<dyn EventTrait>>>>,
    data_collector: Weak<Mutex<MappingDataCollector>>,
}

impl MappingScoringMessageBroker {
    pub fn new(receiver: Receiver<InternalScoringMessage>, senders: Vec<Sender<InternalScoringMessage>>, rank: QSimId, num_partitions: usize, num_scoring_threads: usize, partition_id2person_id: HashMap<QSimId, Vec<Id<InternalPerson>>>) -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(Self { receiver, senders, rank, num_partitions, num_scoring_threads, partition_id2person_id, buffer_events: HashMap::new(), data_collector: Weak::new() }))
    }

    pub(crate) fn attach_senders(&mut self, senders: Vec<Sender<InternalScoringMessage>>) {
        self.senders.extend(senders);
    }

    pub(crate) fn init(message_broker: &Arc<Mutex<Self>>, data_collector: Weak<Mutex<MappingDataCollector>>) {
        message_broker.lock().unwrap().data_collector = data_collector;
    }
    
    /// Thread-Function to execute. Consists of blocking recv-send loop, that breaks when all finish messages were received.
    /// Finish procedure consists of sending experienced plans back to the home-partitions.
    pub fn work(&mut self){
        let mut finished = HashSet::new();
        loop {
            let received_msg = self.receiver.recv().expect("Error receiving message");
            self.send();

            if let RecvResult::CollectorFinish(f) = self.recv(received_msg) {
                finished.insert(f);
            }
            if finished.len() == self.num_partitions {
                break;
            }
        }

        self.finish_sync();
        self.finish_send();
    }

    fn recv(&mut self, msg: InternalScoringMessage) -> RecvResult {
        let from = msg.from_process;
        let boxed_any = msg.message.into_any();

        match () {
            _ if boxed_any.is::<VehicleEventMessage>() => {
                let m = boxed_any.downcast::<VehicleEventMessage>().unwrap();
                let forwarded_events = self.data_collector.upgrade().unwrap().lock().unwrap().add_arriving_vehicle_events(m.events);
                self.buffer_events.extend(forwarded_events);
            }
            _ if boxed_any.is::<PersonEventMessage>() => {
                let m = boxed_any.downcast::<PersonEventMessage>().unwrap();
                self.data_collector.upgrade().unwrap().lock().unwrap().add_arriving_person_events(m.events);
            }
            _ if boxed_any.is::<ScoringFinishMessage>() => {
                return RecvResult::CollectorFinish(from);
            }
            _ if boxed_any.is::<CollectorFinishMessage>() => {
                return RecvResult::ScoringFinish(from);
            }
            _ => {
                panic!("Received unknown message type!");
            }
        }

        RecvResult::Data
    }

    fn send(&mut self) {
        for (target, events) in self.buffer_events.drain() {
            let msg = InternalScoringMessage {
                from_process: self.rank,
                to_process: target,
                message: Box::new(PersonEventMessage { events }),
            };

            self.senders[target as usize].send(msg).unwrap_or_else(|e| {
                panic!("Error sending VehicleMessage to rank {} with error {}", target, e)
            });
        }
    }

    /// Sends and waits for level-2 finish messages
    fn finish_sync(&mut self) {
        for target in (0..self.num_scoring_threads).map(|t| t + self.num_partitions) {
            if target == self.rank as usize { continue; }
            let msg = InternalScoringMessage {
                from_process: self.rank,
                to_process: target as QSimId,
                message: Box::new(CollectorFinishMessage {}),
            };
            self.senders[target].send(msg).unwrap_or_else(|e| {
                panic!("Error sending CollectorFinishMessage to rank {} with error {}", target, e)
            });
        }

        let mut finished = HashSet::new();
        while finished.len() < self.num_scoring_threads - 1 {
            let received_msg = self.receiver.recv().expect("Error receiving message");
            if let RecvResult::ScoringFinish(f) = self.recv(received_msg) {
                finished.insert(f);
            }
        }
    }

    fn finish_send(&mut self) {
        for target in 0..self.num_partitions {
            let plans = self.partition_id2person_id.get(&(target as QSimId)).unwrap().iter().map(|person_id|
                (person_id.clone(), self.data_collector.upgrade().unwrap().lock().unwrap().remove_person_plan(person_id.clone()))
            ).collect();
            
            let msg = InternalScoringMessage {
                from_process: self.rank,
                to_process: target as QSimId,
                message: Box::new(InternalPlanMessage { plans }),
            };

            self.senders[target].send(msg).unwrap_or_else(|e| {
                panic!("Error sending VehicleMessage to rank {} with error {}", target, e)
            });
        }
    }
}


enum RecvResult {
    Data,
    CollectorFinish(QSimId),
    ScoringFinish(QSimId),
}

struct PersonEventMessage {
    events: HashMap<Id<InternalPerson>, Vec<Box<dyn EventTrait>>>,
}

struct VehicleEventMessage {
    events: HashMap<Id<InternalVehicle>, Vec<Box<dyn EventTrait>>>,
}

struct ScoringFinishMessage {

}

struct CollectorFinishMessage {

}

struct InternalPlanMessage {
    plans: HashMap<Id<InternalPerson>, InternalPlan>,
}
