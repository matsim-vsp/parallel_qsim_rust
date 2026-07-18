use crate::simulation::events::EventTrait;
use crate::simulation::framework_events::{AgentLeavesPartitionEvent, PartitionEvent, QSimId};
use crate::simulation::id::Id;
use crate::simulation::scenario::population::{InternalPerson, Population};
use crate::simulation::scenario::vehicles::InternalVehicle;
use crate::simulation::scoring::InternalScoringMessage;
use crate::simulation::scoring::homesending::homesending_data_collector::HomeSendingDataCollector;
use nohash_hasher::{IntMap, IntSet};
use hotpath::wrap::std::sync::mpsc::{Receiver, Sender};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::mpsc::TryRecvError;
use std::sync::{Arc, Mutex, Weak};

pub struct HomeSendingMessageBroker {
    receiver: Receiver<InternalScoringMessage>,
    senders: Vec<Sender<InternalScoringMessage>>,
    num_partitions: usize,
    rank: QSimId,

    buffer_leaving_events: IntMap<QSimId, IntMap<Id<InternalPerson>, Vec<Box<dyn EventTrait>>>>,
    buffer_arriving_events: IntMap<Id<InternalPerson>, IntMap<QSimId, EventBlock>>,
    buffer_partition_events: IntMap<QSimId, IntMap<Id<InternalPerson>, PartitionEvent>>,
    buffer_vehicles: IntMap<QSimId, IntMap<Id<InternalVehicle>, IntSet<Id<InternalPerson>>>>,
    wait_vehicles: IntSet<Id<InternalVehicle>>,
    person_id2current_partition: IntMap<Id<InternalPerson>, QSimId>,
    data_collector: Weak<Mutex<HomeSendingDataCollector>>,

    payload_bytes_by_target: IntMap<QSimId, usize>,
    vehicle_bytes_by_target: IntMap<QSimId, usize>,
    wrapper_bytes_by_target: IntMap<QSimId, usize>,
    bytes_path: PathBuf,
}

#[hotpath::measure_all]
impl HomeSendingMessageBroker {
    pub(crate) fn new(
        receiver: Receiver<InternalScoringMessage>,
        senders: Vec<Sender<InternalScoringMessage>>,
        num_partitions: usize,
        rank: QSimId,
        population: &Population,
        bytes_path: PathBuf,
    ) -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(Self {
            receiver,
            senders,
            num_partitions,
            rank,
            buffer_leaving_events: IntMap::default(),
            buffer_arriving_events: HomeSendingMessageBroker::default_arriving_events_map(
                population, rank,
            ),
            buffer_partition_events: IntMap::default(),
            buffer_vehicles: IntMap::default(),
            wait_vehicles: IntSet::default(),
            person_id2current_partition: HomeSendingMessageBroker::default_current_partition_map(
                population, rank,
            ),
            data_collector: Weak::new(),
            payload_bytes_by_target: IntMap::default(),
            vehicle_bytes_by_target: IntMap::default(),
            wrapper_bytes_by_target: IntMap::default(),
            bytes_path,
        }))
    }

    fn default_current_partition_map(
        population: &Population,
        rank: QSimId,
    ) -> IntMap<Id<InternalPerson>, QSimId> {
        let mut m = IntMap::default();
        for (person_id, _) in population.persons.iter() {
            m.insert(person_id.clone(), rank);
        }
        m
    }

    fn default_arriving_events_map(
        population: &Population,
        rank: QSimId,
    ) -> IntMap<Id<InternalPerson>, IntMap<QSimId, EventBlock>> {
        let mut m = IntMap::default();
        for (person_id, _) in population.persons.iter() {
            m.insert(person_id.clone(), IntMap::default());
            m.entry(person_id.clone())
                .or_default()
                .entry(rank)
                .or_insert_with(|| EventBlock {
                    events: Vec::default(),
                    leave_event: None,
                });
        }
        m
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

    pub(crate) fn add_leaving_vehicle(
        &mut self,
        target: QSimId,
        vehicle_id: Id<InternalVehicle>,
        passengers: IntSet<Id<InternalPerson>>,
    ) {
        self.buffer_vehicles
            .entry(target)
            .or_default()
            .insert(vehicle_id, passengers);
    }

    pub(crate) fn wait_for_vehicle(&mut self, vehicle_id: Id<InternalVehicle>) {
        self.wait_vehicles.insert(vehicle_id);
    }

    /// Blocks until all vehicles that crossed into this partition have their vehicle-to-person
    /// mapping available. Called from BeforeSimStep before the next do_step, so that
    /// replay_deferred_link_events fires in the correct order relative to subsequent link events.
    pub(crate) fn recv_vehicles(&mut self) {
        let pending = self.wait_vehicles.drain().collect::<Vec<_>>();
        for vehicle_id in pending {
            if self
                .data_collector
                .upgrade()
                .unwrap()
                .lock()
                .unwrap()
                .get_vehicles()
                .contains_key(&vehicle_id)
            {
                continue;
            }
            while !self
                .data_collector
                .upgrade()
                .unwrap()
                .lock()
                .unwrap()
                .get_vehicles()
                .contains_key(&vehicle_id)
            {
                let msg = self.receiver.recv().expect("Error receiving message");
                self.recv(msg);
            }
        }
    }

    pub(crate) fn add_leaving_event(
        &mut self,
        target: QSimId,
        person_id: Id<InternalPerson>,
        event: Box<dyn EventTrait>,
    ) {
        self.buffer_leaving_events
            .entry(target)
            .or_default()
            .entry(person_id)
            .or_default()
            .push(event);
    }

    pub(crate) fn add_leaving_partition_event(
        &mut self,
        target: QSimId,
        person_id: Id<InternalPerson>,
        event: PartitionEvent,
    ) {
        if self
            .buffer_partition_events
            .get(&target)
            .is_some_and(|m| m.contains_key(&person_id))
        {
            panic!("Tried to overwrite partition event for {}", person_id);
        }

        self.buffer_partition_events
            .entry(target)
            .or_default()
            .insert(person_id, event);
    }

    pub(crate) fn open_block(&mut self, person_id: Id<InternalPerson>, rank: QSimId) {
        if self
            .buffer_arriving_events
            .get(&person_id)
            .is_some_and(|m| m.contains_key(&rank))
        {
            panic!("Tried to overwrite block for ({}, #{})", person_id, rank);
        }

        self.buffer_arriving_events
            .entry(person_id)
            .or_default()
            .insert(
                rank,
                EventBlock {
                    events: Vec::new(),
                    leave_event: None,
                },
            );
    }

    pub(crate) fn close_block(
        &mut self,
        person_id: Id<InternalPerson>,
        rank: QSimId,
        leave_event: Option<AgentLeavesPartitionEvent>,
    ) {
        self.buffer_arriving_events
            .get_mut(&person_id)
            .unwrap_or_else(|| panic!("Tried to access empty buffer for person {}", person_id))
            .get_mut(&rank)
            .unwrap_or_else(|| {
                panic!(
                    "Tried to access empty internal block for person {} on rank {}",
                    person_id, rank
                )
            })
            .leave_event = leave_event;

        while self
            .buffer_arriving_events
            .get(&person_id)
            .is_some_and(|m| {
                let cur = self.person_id2current_partition.get(&person_id).unwrap();
                m.get(cur).is_some_and(|b| b.leave_event.is_some())
            })
        {
            let block = self
                .buffer_arriving_events
                .get_mut(&person_id)
                .unwrap()
                .remove(self.person_id2current_partition.get(&person_id).unwrap())
                .unwrap();

            self.data_collector
                .upgrade()
                .unwrap()
                .lock()
                .unwrap()
                .add_arriving_events(person_id.clone(), block.events);

            self.person_id2current_partition
                .insert(person_id.clone(), block.leave_event.unwrap().to);
        }
    }

    pub(crate) fn push_events_on_block(
        &mut self,
        person_id: Id<InternalPerson>,
        rank: QSimId,
        events: Vec<Box<dyn EventTrait>>,
    ) {
        self.buffer_arriving_events
            .get_mut(&person_id)
            .unwrap_or_else(|| panic!("Tried to access empty buffer for person {}", person_id))
            .get_mut(&rank)
            .unwrap_or_else(|| {
                panic!(
                    "Tried to access empty block for person {} on rank {}",
                    person_id, self.rank
                )
            })
            .events
            .extend(events);
    }

    fn send(&mut self) {
        for (target, vehicles) in self.buffer_vehicles.drain() {
            let payload_bytes: usize = vehicles
                .iter()
                .map(|(_, persons)| {
                    std::mem::size_of::<Id<InternalVehicle>>()
                        + persons.len() * std::mem::size_of::<Id<InternalPerson>>()
                })
                .sum();
            let msg = InternalScoringMessage {
                from_process: self.rank,
                to_process: target,
                message: Box::new(VehicleMessage { vehicles }),
            };
            *self.vehicle_bytes_by_target.entry(target).or_insert(0) += payload_bytes;
            *self.wrapper_bytes_by_target.entry(target).or_insert(0) += size_of::<InternalScoringMessage>();
            self.senders[target as usize].send(msg).unwrap_or_else(|e| {
                panic!(
                    "Error sending VehicleMessage to rank {} with error {}",
                    target, e
                )
            });
        }

        for (target, events) in self.buffer_leaving_events.drain() {
            let payload_bytes: usize = events
                .iter()
                .map(|(_, evts)| {
                    std::mem::size_of::<Id<InternalPerson>>()
                        + evts
                            .iter()
                            .map(|e| std::mem::size_of_val(e.as_ref()))
                            .sum::<usize>()
                })
                .sum();
            let msg = InternalScoringMessage {
                from_process: self.rank,
                to_process: target,
                message: Box::new(EventMessage { events }),
            };
            *self.payload_bytes_by_target.entry(target).or_insert(0) += payload_bytes;
            *self.wrapper_bytes_by_target.entry(target).or_insert(0) += size_of::<InternalScoringMessage>();
            self.senders[target as usize].send(msg).unwrap_or_else(|e| {
                panic!(
                    "Error sending EventMessage to rank {} with error {}",
                    target, e
                )
            });
        }

        for (target, partition_events) in self.buffer_partition_events.drain() {
            let payload_bytes = partition_events.len()
                * (std::mem::size_of::<Id<InternalPerson>>()
                    + std::mem::size_of::<PartitionEvent>());
            let msg = InternalScoringMessage {
                from_process: self.rank,
                to_process: target,
                message: Box::new(PartitionEventMessage { partition_events }),
            };
            *self.payload_bytes_by_target.entry(target).or_insert(0) += payload_bytes;
            *self.wrapper_bytes_by_target.entry(target).or_insert(0) += size_of::<InternalScoringMessage>();
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

                for (person_id, events) in m.events {
                    self.push_events_on_block(person_id, msg.from_process, events);
                }
            }
            _ if boxed_any.is::<PartitionEventMessage>() => {
                let m = *boxed_any.downcast::<PartitionEventMessage>().unwrap();

                for (person_id, event) in m.partition_events {
                    match event {
                        PartitionEvent::AgentEntersPartition(_) => {
                            self.open_block(person_id, msg.from_process);
                        }
                        PartitionEvent::AgentLeavesPartition(i) => {
                            self.close_block(person_id, msg.from_process, Some(i));
                        }
                        _ => {}
                    }
                }
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
    pub(crate) fn finish_send_recv(&mut self) {
        self.send();

        // Send a finish message to all partitions
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

        let mut finished_partitions: IntSet<QSimId> = IntSet::default();
        while finished_partitions.len() < self.senders.len() - 1 {
            let received_msg = self.receiver.recv().expect("Error receiving message");
            let boxed_any = received_msg.message.as_any();

            match () {
                _ if boxed_any.is::<FinishMessage>() => {
                    // Add finish message to set for break condition
                    finished_partitions.insert(received_msg.from_process);
                }
                _ => {
                    // Process arriving data
                    self.recv(received_msg);
                }
            }
        }

        // Finish remaining event-blocks. There should be exactly one event block per agent
        // If there are more or less unfinished event-blocks, something went wrong.
        // In such case, the check will panic.
        for (person_id, buffer) in self.buffer_arriving_events.drain() {
            if buffer.len() != 1 {
                panic!(
                    "Person {} has {} unfinished blocks at the end of the simulation!",
                    person_id,
                    buffer.len()
                );
            }

            let block = buffer.into_values().next().unwrap();

            self.data_collector
                .upgrade()
                .unwrap()
                .lock()
                .unwrap()
                .add_arriving_events(person_id.clone(), block.events);
        }

        let new_file = !self.bytes_path.exists();
        std::fs::create_dir_all(self.bytes_path.parent().unwrap()).unwrap();
        let mut file = OpenOptions::new().create(true).append(true).open(&self.bytes_path).unwrap();
        if new_file {
            writeln!(file, "type,target,bytes").unwrap();
        }
        let mut vehicle_entries: Vec<_> = self.vehicle_bytes_by_target.iter().map(|(&t, &b)| (t, b)).collect();
        vehicle_entries.sort_by_key(|&(t, _)| t);
        for (target, bytes) in vehicle_entries {
            writeln!(file, "vehicle,{},{}", target, bytes).unwrap();
        }
        let mut payload_entries: Vec<_> = self.payload_bytes_by_target.iter().map(|(&t, &b)| (t, b)).collect();
        payload_entries.sort_by_key(|&(t, _)| t);
        for (target, bytes) in payload_entries {
            writeln!(file, "payload,{},{}", target, bytes).unwrap();
        }
        let mut wrapper_entries: Vec<_> = self.wrapper_bytes_by_target.iter().map(|(&t, &b)| (t, b)).collect();
        wrapper_entries.sort_by_key(|&(t, _)| t);
        for (target, bytes) in wrapper_entries {
            writeln!(file, "wrapper,{},{}", target, bytes).unwrap();
        }
        self.vehicle_bytes_by_target.clear();
        self.payload_bytes_by_target.clear();
        self.wrapper_bytes_by_target.clear();
    }
}

struct EventBlock {
    events: Vec<Box<dyn EventTrait>>,
    leave_event: Option<AgentLeavesPartitionEvent>,
}

pub struct EventMessage {
    events: IntMap<Id<InternalPerson>, Vec<Box<dyn EventTrait>>>,
}

pub struct VehicleMessage {
    vehicles: IntMap<Id<InternalVehicle>, IntSet<Id<InternalPerson>>>,
}

pub struct PartitionEventMessage {
    partition_events: IntMap<Id<InternalPerson>, PartitionEvent>,
}

pub struct FinishMessage {}
