use crate::simulation::events::EventTrait;
use crate::simulation::framework_events::{
    MobsimEvent, MobsimEventsManager, MobsimListenerRegisterFn, QSimId, RuntimeEvent,
};
use crate::simulation::id::Id;
use crate::simulation::scenario::population::InternalPerson;
use crate::simulation::scenario::vehicles::InternalVehicle;
use crate::simulation::scoring::InternalScoringMessage;
use crate::simulation::scoring::backpacking::backpacking_message_broker::VehicleMessage;
use crate::simulation::scoring::homesending::homesending_data_collector::HomeSendingDataCollector;
use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex, Weak};

pub struct HomeSendingMessageBroker {
    receiver: Receiver<InternalScoringMessage>,
    senders: Vec<Sender<InternalScoringMessage>>,
    num_partitions: usize,
    rank: QSimId,

    buffer_events: HashMap<QSimId, HashMap<Id<InternalPerson>, Vec<Box<dyn EventTrait>>>>,
    buffer_vehicles: HashMap<QSimId, HashMap<Id<InternalVehicle>, HashSet<Id<InternalPerson>>>>,
    data_collector: Weak<Mutex<HomeSendingDataCollector>>,
}

impl HomeSendingMessageBroker {
    pub(crate) fn new(
        receiver: Receiver<InternalScoringMessage>,
        senders: Vec<Sender<InternalScoringMessage>>,
        num_partitions: usize,
        rank: QSimId,
    ) -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(Self {
            receiver,
            senders,
            num_partitions,
            rank,
            buffer_events: HashMap::new(),
            buffer_vehicles: HashMap::new(),
            data_collector: Weak::new(),
        }))
    }

    pub(crate) fn attach_senders(&mut self, senders: Vec<Sender<InternalScoringMessage>>) {
        self.senders.extend(senders);
    }

    pub(crate) fn init(
        message_broker: &Arc<Mutex<Self>>,
        data_collector: Weak<Mutex<HomeSendingDataCollector>>,
    ) {
        message_broker.lock().unwrap().data_collector = data_collector;
    }

    pub(crate) fn register_fn(
        scoring_broker: Arc<Mutex<HomeSendingMessageBroker>>,
    ) -> Box<MobsimListenerRegisterFn> {
        Box::new(move |events: &mut MobsimEventsManager| {
            let broker = Arc::clone(&scoring_broker);
            events.on_event(move |e: &RuntimeEvent<MobsimEvent>| match &e.payload {
                MobsimEvent::AfterSimStep(_) => {
                    broker.lock().unwrap().send_recv();
                }
                _ => {}
            });
        })
    }

    pub(crate) fn add_leaving_vehicle(
        &mut self,
        target: QSimId,
        vehicle_id: Id<InternalVehicle>,
        passengers: HashSet<Id<InternalPerson>>,
    ) {
        self.buffer_vehicles
            .entry(target)
            .or_insert_with(HashMap::new)
            .insert(vehicle_id, passengers);
    }

    pub(crate) fn add_leaving_event(
        &mut self,
        target: QSimId,
        person_id: Id<InternalPerson>,
        event: Box<dyn EventTrait>,
    ) {
        self.buffer_events
            .entry(target)
            .or_insert_with(HashMap::new)
            .entry(person_id)
            .or_default()
            .push(event);
    }

    fn send(&mut self) {
        for (target, vehicles) in self.buffer_vehicles.drain() {
            let msg = InternalScoringMessage {
                from_process: self.rank,
                to_process: target,
                message: Box::new(VehicleMessage { vehicles }),
            };
            self.senders[target as usize].send(msg).unwrap_or_else(|e| {
                panic!(
                    "Error sending VehicleMessage to rank {} with error {}",
                    target, e
                )
            });
        }

        for (target, events) in self.buffer_events.drain() {
            let msg = InternalScoringMessage {
                from_process: self.rank,
                to_process: target,
                message: Box::new(EventMessage { events }),
            };
            self.senders[target as usize].send(msg).unwrap_or_else(|e| {
                panic!(
                    "Error sending EventMessage to rank {} with error {}",
                    target, e
                )
            });
        }
    }

    fn recv(&mut self, msg: InternalScoringMessage) {
        let boxed_any = msg.message.into_any();
        match () {
            _ if boxed_any.is::<VehicleMessage>() => {
                let m = boxed_any.downcast::<VehicleMessage>().unwrap();
                self.data_collector
                    .upgrade()
                    .unwrap()
                    .lock()
                    .unwrap()
                    .add_arriving_vehicles(m.vehicles);
            }
            _ if boxed_any.is::<EventMessage>() => {
                let m = boxed_any.downcast::<EventMessage>().unwrap();
                self.data_collector
                    .upgrade()
                    .unwrap()
                    .lock()
                    .unwrap()
                    .add_arriving_events(m.events);
            }
            _ => {
                panic!("Received unexpected message type during simulation step!");
            }
        }
    }

    /// Called on every AfterSimStep: flushes send buffers, then non-blockingly drains any pending incoming messages.
    pub(crate) fn send_recv(&mut self) {
        self.send();

        loop {
            match self.receiver.try_recv() {
                Ok(msg) => self.recv(msg),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => panic!("Scoring channel disconnected"),
            }
        }
    }

    /// Called after the mobsim ends: flushes any remaining send buffers, broadcasts a
    /// FinishMessage to all other partitions, then blocks until every other partition has
    /// done the same. Incoming data messages are processed while waiting.
    pub(crate) fn finish_sync(&mut self) {
        self.send();

        for target in 0..self.num_partitions {
            if target as QSimId == self.rank {
                continue;
            }
            let msg = InternalScoringMessage {
                from_process: self.rank,
                to_process: target as QSimId,
                message: Box::new(FinishMessage {}),
            };
            self.senders[target].send(msg).unwrap_or_else(|e| {
                panic!(
                    "Error sending FinishMessage to rank {} with error {}",
                    target, e
                )
            });
        }

        let mut finished: HashSet<QSimId> = HashSet::new();
        while finished.len() < self.num_partitions - 1 {
            let msg = self
                .receiver
                .recv()
                .expect("Scoring channel disconnected during finish_sync");
            let from = msg.from_process;
            let boxed_any = msg.message.into_any();
            match () {
                _ if boxed_any.is::<VehicleMessage>() => {
                    let m = boxed_any.downcast::<VehicleMessage>().unwrap();
                    self.data_collector
                        .upgrade()
                        .unwrap()
                        .lock()
                        .unwrap()
                        .add_arriving_vehicles(m.vehicles);
                }
                _ if boxed_any.is::<EventMessage>() => {
                    let m = boxed_any.downcast::<EventMessage>().unwrap();
                    self.data_collector
                        .upgrade()
                        .unwrap()
                        .lock()
                        .unwrap()
                        .add_arriving_events(m.events);
                }
                _ if boxed_any.is::<FinishMessage>() => {
                    finished.insert(from);
                }
                _ => {
                    panic!("Received unknown message type during finish_sync!");
                }
            }
        }
    }
}

pub struct EventMessage {
    pub(crate) events: HashMap<Id<InternalPerson>, Vec<Box<dyn EventTrait>>>,
}

pub struct FinishMessage {}
