use crate::simulation::events::EventTrait;
use crate::simulation::framework_events::{
    MobsimEvent, MobsimEventsManager, MobsimListenerRegisterFn, QSimId, RuntimeEvent,
};
use crate::simulation::id::Id;
use crate::simulation::scenario::population::{InternalPerson, InternalPlan};
use crate::simulation::scenario::vehicles::InternalVehicle;
use crate::simulation::scoring::InternalScoringMessage;
use crate::simulation::scoring::mapping::mapping_data_collector::MappingDataCollector;
use crate::simulation::scoring::mapping::mapping_data_forwarder::MappingDataForwarder;
use ahash::{HashSet, HashSetExt};
use hotpath::wrap::std::sync::mpsc::{Receiver, Sender};
use nohash_hasher::IntMap;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, Weak};

pub struct MappingCollectorMessageBroker {
    receiver: Receiver<InternalScoringMessage>,
    senders: Vec<Sender<InternalScoringMessage>>,
    rank: QSimId,
    num_partitions: usize,
    num_collectors: usize,
    sync_interval: u32,

    counter: u32,
    buffer_events: IntMap<QSimId, IntMap<Id<InternalPerson>, Vec<(Box<dyn EventTrait>, u32)>>>,
    buffer_vehicles: IntMap<u32, IntMap<Id<InternalVehicle>, Vec<(Box<dyn EventTrait>, u32)>>>,
    data_forwarder: Weak<Mutex<MappingDataForwarder>>,

    payload_bytes_by_target: IntMap<QSimId, usize>,
    wrapper_bytes_by_target: IntMap<QSimId, usize>,
    payload_count_by_target: IntMap<QSimId, usize>,
    bytes_path: PathBuf,
}

#[hotpath::measure_all]
impl MappingCollectorMessageBroker {
    pub fn new(
        receiver: Receiver<InternalScoringMessage>,
        senders: Vec<Sender<InternalScoringMessage>>,
        rank: QSimId,
        num_partitions: usize,
        num_collectors: usize,
        sync_interval: u32,
        bytes_path: PathBuf,
    ) -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(Self {
            receiver,
            senders,
            rank,
            num_partitions,
            num_collectors,
            sync_interval,
            counter: 0,
            buffer_events: IntMap::default(),
            buffer_vehicles: IntMap::default(),
            data_forwarder: Weak::new(),
            payload_bytes_by_target: IntMap::default(),
            wrapper_bytes_by_target: IntMap::default(),
            payload_count_by_target: IntMap::default(),
            bytes_path,
        }))
    }

    pub(crate) fn attach_senders(&mut self, senders: Vec<Sender<InternalScoringMessage>>) {
        self.senders.extend(senders);
    }

    pub(crate) fn init(
        message_broker: &Arc<Mutex<Self>>,
        data_collector: Weak<Mutex<MappingDataForwarder>>,
    ) {
        message_broker.lock().unwrap().data_forwarder = data_collector;
    }

    pub(crate) fn register_fn(
        scoring_broker: Arc<Mutex<MappingCollectorMessageBroker>>,
    ) -> Box<MobsimListenerRegisterFn> {
        Box::new(move |events: &mut MobsimEventsManager| {
            let broker = Arc::clone(&scoring_broker);
            events.on_event(move |e: &RuntimeEvent<MobsimEvent>| {
                hotpath::measure_block!("Backpacking.EventsManager.on_any", {
                    match &e.payload {
                        MobsimEvent::AfterSimStep(i) => {
                            broker.lock().unwrap().send(i.time, false);
                        }
                        _ => {}
                    }
                });
            });
        })
    }

    pub(crate) fn add_leaving_person_event(
        &mut self,
        target: u32,
        person_id: Id<InternalPerson>,
        event: Box<dyn EventTrait>,
    ) {
        self.buffer_events
            .entry(target)
            .or_insert_with(|| IntMap::default())
            .entry(person_id)
            .or_insert_with(|| Vec::default())
            .push((event, self.counter));
        self.counter += 1;
    }

    pub(crate) fn add_leaving_vehicle_event(
        &mut self,
        target: u32,
        vehicle_id: Id<InternalVehicle>,
        event: Box<dyn EventTrait>,
    ) {
        self.buffer_vehicles
            .entry(target)
            .or_insert_with(|| IntMap::default())
            .entry(vehicle_id)
            .or_insert_with(|| Vec::default())
            .push((event, self.counter));
        self.counter += 1;
    }

    /// Called on every AfterSimStep: Flushes send buffers
    fn send(&mut self, time: u32, force_sync: bool) {
        for (target, vehicle_events) in self.buffer_vehicles.drain() {
            let payload_bytes: usize = vehicle_events
                .iter()
                .map(|(_, evts)| {
                    size_of::<Id<InternalVehicle>>()
                        + evts
                            .iter()
                            .map(|(e, _)| size_of_val(e.as_ref()) + size_of::<u32>())
                            .sum::<usize>()
                })
                .sum();
            let msg = InternalScoringMessage {
                from_process: self.rank,
                to_process: target,
                message: Box::new(VehicleEventMessage {
                    events: vehicle_events,
                }),
            };
            *self.payload_bytes_by_target.entry(target).or_insert(0) += payload_bytes;
            *self.payload_count_by_target.entry(target).or_insert(0) += 1;
            *self.wrapper_bytes_by_target.entry(target).or_insert(0) +=
                size_of::<InternalScoringMessage>();
            self.senders[target as usize].send(msg).unwrap_or_else(|e| {
                panic!(
                    "Error sending VehicleEventMessage to rank {} with error {}",
                    target, e
                );
            });
        }

        for (target, events) in self.buffer_events.drain() {
            let payload_bytes: usize = events
                .iter()
                .map(|(_, evts)| {
                    size_of::<Id<InternalPerson>>()
                        + evts
                            .iter()
                            .map(|(e, _)| size_of_val(e.as_ref()) + size_of::<u32>())
                            .sum::<usize>()
                })
                .sum();
            let msg = InternalScoringMessage {
                from_process: self.rank,
                to_process: target,
                message: Box::new(PersonEventMessage { events }),
            };
            *self.payload_bytes_by_target.entry(target).or_insert(0) += payload_bytes;
            *self.payload_count_by_target.entry(target).or_insert(0) += 1;
            *self.wrapper_bytes_by_target.entry(target).or_insert(0) +=
                size_of::<InternalScoringMessage>();
            self.senders[target as usize].send(msg).unwrap_or_else(|e| {
                panic!(
                    "Error sending PersonEventMessage to rank {} with error {}",
                    target, e
                )
            });
        }

        if time % self.sync_interval == 0 || force_sync {
            for target in self.num_partitions..(self.num_partitions + self.num_collectors) {
                let payload_bytes = size_of::<WatermarkMessage>();
                let msg = InternalScoringMessage {
                    from_process: self.rank,
                    to_process: target as QSimId,
                    message: Box::new(WatermarkMessage {
                        origin: self.rank,
                        hop: 1,
                        time,
                    }),
                };
                *self
                    .payload_bytes_by_target
                    .entry(target as QSimId)
                    .or_insert(0) += payload_bytes;
                *self
                    .payload_count_by_target
                    .entry(target as QSimId)
                    .or_insert(0) += 1;
                *self
                    .wrapper_bytes_by_target
                    .entry(target as QSimId)
                    .or_insert(0) += size_of::<InternalScoringMessage>();
                self.senders[target].send(msg).unwrap_or_else(|e| {
                    panic!(
                        "Error sending PersonEventMessage to rank {} with error {}",
                        target, e
                    )
                });
            }
        }
    }

    /// Called after the mobsim ends: Flushes the send buffers and sends a finish message to all scoring threads.
    /// Then collects incoming Experienced Plans and passes them to the forwarder
    pub(crate) fn finish_send_recv(&mut self) {
        self.send(u32::MAX, true);

        // TODO Use finished instead of this for loop
        for _ in 0..self.num_collectors {
            let received_msg = self.receiver.recv().unwrap();

            let boxed_any = received_msg.message.into_any();
            match () {
                _ if boxed_any.is::<InternalPlanMessage>() => {
                    let m = boxed_any.downcast::<InternalPlanMessage>().unwrap();
                    for (person_id, plan) in m.plans {
                        self.data_forwarder
                            .upgrade()
                            .unwrap()
                            .lock()
                            .unwrap()
                            .add_arriving_plan(person_id, plan);
                    }
                }
                _ => {
                    panic!("Received unexpected message type during simulation step!");
                }
            }
        }

        std::fs::create_dir_all(self.bytes_path.parent().unwrap()).unwrap();
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&self.bytes_path)
            .unwrap();
        writeln!(file, "type,target,bytes,count").unwrap();
        let mut payload_entries: Vec<_> = self
            .payload_bytes_by_target
            .iter()
            .map(|(&t, &b)| (t, b))
            .collect();
        payload_entries.sort_by_key(|&(t, _)| t);
        for (target, bytes) in payload_entries {
            let count = self
                .payload_count_by_target
                .get(&target)
                .copied()
                .unwrap_or(0);
            writeln!(file, "payload,{},{},{}", target, bytes, count).unwrap();
        }
        let mut wrapper_entries: Vec<_> = self
            .wrapper_bytes_by_target
            .iter()
            .map(|(&t, &b)| (t, b))
            .collect();
        wrapper_entries.sort_by_key(|&(t, _)| t);
        for (target, bytes) in wrapper_entries {
            let count = self
                .payload_count_by_target
                .get(&target)
                .copied()
                .unwrap_or(0);
            writeln!(file, "wrapper,{},{},{}", target, bytes, count).unwrap();
        }
        self.payload_bytes_by_target.clear();
        self.wrapper_bytes_by_target.clear();
        self.payload_count_by_target.clear();
    }
}

pub struct MappingScoringMessageBroker {
    receiver: Receiver<InternalScoringMessage>,
    senders: Vec<Sender<InternalScoringMessage>>,
    rank: QSimId,
    num_partitions: usize,
    num_collectors: usize,
    person_id2home_partition: IntMap<Id<InternalPerson>, QSimId>,

    buffer_events: IntMap<u32, IntMap<Id<InternalPerson>, Vec<(Box<dyn EventTrait>, u32)>>>,
    buffer_watermarks: IntMap<QSimId, WatermarkMessage>,
    data_collector: Weak<Mutex<MappingDataCollector>>,

    payload_bytes_by_target: IntMap<QSimId, usize>,
    wrapper_bytes_by_target: IntMap<QSimId, usize>,
    payload_count_by_target: IntMap<QSimId, usize>,
    bytes_path: PathBuf,
}

#[hotpath::measure_all]
impl MappingScoringMessageBroker {
    pub fn new(
        receiver: Receiver<InternalScoringMessage>,
        senders: Vec<Sender<InternalScoringMessage>>,
        rank: QSimId,
        num_partitions: usize,
        num_collectors: usize,
        person_id2home_partition: IntMap<Id<InternalPerson>, QSimId>,
        bytes_path: PathBuf,
    ) -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(Self {
            receiver,
            senders,
            rank,
            num_partitions,
            num_collectors,
            person_id2home_partition,
            buffer_events: IntMap::default(),
            buffer_watermarks: IntMap::default(),
            data_collector: Weak::new(),
            payload_bytes_by_target: IntMap::default(),
            wrapper_bytes_by_target: IntMap::default(),
            payload_count_by_target: IntMap::default(),
            bytes_path,
        }))
    }

    pub(crate) fn attach_senders(&mut self, senders: Vec<Sender<InternalScoringMessage>>) {
        self.senders.extend(senders);
    }

    pub(crate) fn init(
        message_broker: &Arc<Mutex<Self>>,
        data_collector: Weak<Mutex<MappingDataCollector>>,
    ) {
        message_broker.lock().unwrap().data_collector = data_collector;
    }

    /// Thread-Function to execute. Consists of blocking recv-send loop, that breaks when all finish messages were received.
    /// Finish procedure consists of sending experienced plans back to the home-partitions.
    pub fn work(&mut self) {
        let mut finished = HashSet::new();
        loop {
            let received_msg = hotpath::measure_block!("MappingScoringMessageBroker.recv_wait", {
                self.receiver.recv().expect("Error receiving message")
            });

            if let Some(partition) = self.recv(received_msg) {
                finished.insert(partition);
            }
            if finished.len() == (self.num_partitions * self.num_collectors) {
                break;
            }

            self.send();
        }

        self.finish_send();
    }

    fn recv(&mut self, msg: InternalScoringMessage) -> Option<(QSimId, QSimId)> {
        let boxed_any = msg.message.into_any();

        match () {
            _ if boxed_any.is::<VehicleEventMessage>() => {
                let m = boxed_any.downcast::<VehicleEventMessage>().unwrap();
                let forwarded_events = self
                    .data_collector
                    .upgrade()
                    .unwrap()
                    .lock()
                    .unwrap()
                    .add_arriving_vehicle_events(m.events);
                self.buffer_events.extend(forwarded_events);
            }
            _ if boxed_any.is::<PersonEventMessage>() => {
                let m = boxed_any.downcast::<PersonEventMessage>().unwrap();
                self.data_collector
                    .upgrade()
                    .unwrap()
                    .lock()
                    .unwrap()
                    .add_arriving_person_events(m.events);
            }
            _ if boxed_any.is::<WatermarkMessage>() => {
                let m = boxed_any.downcast::<WatermarkMessage>().unwrap();

                match m.hop {
                    1 => {
                        for target in
                            self.num_partitions..(self.num_partitions + self.num_collectors)
                        {
                            self.buffer_watermarks.insert(
                                target as QSimId,
                                WatermarkMessage {
                                    origin: msg.from_process,
                                    hop: 2,
                                    time: m.time,
                                },
                            );
                        }
                    }
                    2 => {
                        self.data_collector
                            .upgrade()
                            .unwrap()
                            .lock()
                            .unwrap()
                            .add_arriving_watermark(m.origin, msg.from_process, m.time);

                        if m.time == u32::MAX {
                            return Some((m.origin, msg.from_process));
                        }
                    }
                    _ => panic!("Unexpected amount of hops: {}", m.hop),
                }
            }
            _ => {
                panic!("Received unknown message type!");
            }
        }

        None
    }

    fn send(&mut self) {
        for (target, events) in self.buffer_events.drain() {
            let payload_bytes: usize = events
                .iter()
                .map(|(_, evts)| {
                    size_of::<Id<InternalPerson>>()
                        + evts
                            .iter()
                            .map(|(e, _)| size_of_val(e.as_ref()) + size_of::<u32>())
                            .sum::<usize>()
                })
                .sum();
            let msg = InternalScoringMessage {
                from_process: self.rank,
                to_process: target,
                message: Box::new(PersonEventMessage { events }),
            };
            *self.payload_bytes_by_target.entry(target).or_insert(0) += payload_bytes;
            *self.payload_count_by_target.entry(target).or_insert(0) += 1;
            *self.wrapper_bytes_by_target.entry(target).or_insert(0) +=
                size_of::<InternalScoringMessage>();
            self.senders[target as usize].send(msg).unwrap_or_else(|e| {
                panic!(
                    "Error sending VehicleMessage to rank {} with error {}",
                    target, e
                )
            });
        }

        for (target, m) in self.buffer_watermarks.drain() {
            let payload_bytes = size_of::<WatermarkMessage>();
            let msg = InternalScoringMessage {
                from_process: self.rank,
                to_process: target,
                message: Box::new(m),
            };
            *self.payload_bytes_by_target.entry(target).or_insert(0) += payload_bytes;
            *self.payload_count_by_target.entry(target).or_insert(0) += 1;
            *self.wrapper_bytes_by_target.entry(target).or_insert(0) +=
                size_of::<InternalScoringMessage>();
            self.senders[target as usize].send(msg).unwrap_or_else(|e| {
                panic!(
                    "Error sending VehicleMessage to rank {} with error {}",
                    target, e
                )
            });
        }
    }

    fn finish_send(&mut self) {
        // TODO Check if heap is empty
        let mut partition_id2partial_plan: IntMap<
            QSimId,
            IntMap<Id<InternalPerson>, InternalPlan>,
        > = IntMap::default();
        for (person_id, partial_plan) in self
            .data_collector
            .upgrade()
            .unwrap()
            .lock()
            .unwrap()
            .take_person_plans()
        {
            let home_partition = *self.person_id2home_partition.get(&person_id).unwrap();
            partition_id2partial_plan
                .entry(home_partition)
                .or_default()
                .insert(person_id, partial_plan.finish());
        }

        for target in 0..self.num_partitions {
            let plans = partition_id2partial_plan
                .remove(&(target as QSimId))
                .unwrap_or_default();
            let payload_bytes: usize = plans
                .iter()
                .map(|(_, p)| {
                    size_of::<Id<InternalPerson>>()
                        + p.elements.iter().map(|e| size_of_val(e)).sum::<usize>()
                })
                .sum();
            let msg = InternalScoringMessage {
                from_process: self.rank,
                to_process: target as QSimId,
                message: Box::new(InternalPlanMessage { plans }),
            };
            hotpath::gauge!("MappingScoringMessageBroker.bytes_sent").inc(payload_bytes as f64);
            *self
                .payload_bytes_by_target
                .entry(target as QSimId)
                .or_insert(0) += payload_bytes;
            *self
                .payload_count_by_target
                .entry(target as QSimId)
                .or_insert(0) += 1;
            *self
                .wrapper_bytes_by_target
                .entry(target as QSimId)
                .or_insert(0) += size_of::<InternalScoringMessage>();
            self.senders[target].send(msg).unwrap_or_else(|e| {
                panic!(
                    "Error sending VehicleMessage to rank {} with error {}",
                    target, e
                )
            });
        }

        std::fs::create_dir_all(self.bytes_path.parent().unwrap()).unwrap();
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&self.bytes_path)
            .unwrap();
        writeln!(file, "type,target,bytes,count").unwrap();
        let mut payload_entries: Vec<_> = self
            .payload_bytes_by_target
            .iter()
            .map(|(&t, &b)| (t, b))
            .collect();
        payload_entries.sort_by_key(|&(t, _)| t);
        for (target, bytes) in payload_entries {
            let count = self
                .payload_count_by_target
                .get(&target)
                .copied()
                .unwrap_or(0);
            writeln!(file, "payload,{},{},{}", target, bytes, count).unwrap();
        }
        let mut wrapper_entries: Vec<_> = self
            .wrapper_bytes_by_target
            .iter()
            .map(|(&t, &b)| (t, b))
            .collect();
        wrapper_entries.sort_by_key(|&(t, _)| t);
        for (target, bytes) in wrapper_entries {
            let count = self
                .payload_count_by_target
                .get(&target)
                .copied()
                .unwrap_or(0);
            writeln!(file, "wrapper,{},{},{}", target, bytes, count).unwrap();
        }
    }
}
struct PersonEventMessage {
    events: IntMap<Id<InternalPerson>, Vec<(Box<dyn EventTrait>, u32)>>,
}

struct VehicleEventMessage {
    events: IntMap<Id<InternalVehicle>, Vec<(Box<dyn EventTrait>, u32)>>,
}

struct WatermarkMessage {
    origin: QSimId,
    hop: u32,
    time: u32,
}

struct InternalPlanMessage {
    plans: IntMap<Id<InternalPerson>, InternalPlan>,
}
