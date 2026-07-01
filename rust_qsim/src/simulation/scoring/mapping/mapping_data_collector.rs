use std::cmp::{Ordering, Reverse};
use std::collections::{BinaryHeap, HashMap, HashSet};
use std::hash::Hasher;
use std::sync::{Arc, Mutex};
use crate::simulation::events::{DynEq, EventTrait, LinkEnterEvent, PersonEntersVehicleEvent, PersonLeavesVehicleEvent, VehicleEntersTrafficEvent, VehicleLeavesTrafficEvent};
use crate::simulation::framework_events::QSimId;
use crate::simulation::id::Id;
use crate::simulation::scenario::population::{InternalPerson, InternalPlan};
use crate::simulation::scenario::vehicles::InternalVehicle;
use crate::simulation::scoring::mapping::mapping_message_broker::MappingScoringMessageBroker;
use crate::simulation::scoring::partial_plans::PartialPlan;

pub struct MappingDataCollector {
    person_hash_function: Box<dyn Fn(Id<InternalPerson>) -> u32 + Send>,
    num_partitions: u32,

    person_id2heap: HashMap<Id<InternalPerson>, BinaryHeap<HeapEntry>>,
    person_id2partial_plan: HashMap<Id<InternalPerson>, PartialPlan>,
    vehicle_id2person_ids: HashMap<Id<InternalVehicle>, HashSet<Id<InternalPerson>>>,
    watermark: u32,
    watermark_buffer: HashMap<u32, HashSet<QSimId>>,

    message_broker: Arc<Mutex<MappingScoringMessageBroker>>
}

impl MappingDataCollector {
    pub fn new(person_hash_function: Box<dyn Fn(Id<InternalPerson>) -> u32 + Send>, num_partitions: u32, message_broker: Arc<Mutex<MappingScoringMessageBroker>>) -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(Self {
            person_hash_function,
            num_partitions,
            person_id2heap: HashMap::new(),
            person_id2partial_plan: HashMap::new(),
            vehicle_id2person_ids: HashMap::new(),
            watermark: 0,
            watermark_buffer: HashMap::new(),
            message_broker }))
    }

    pub(crate) fn add_arriving_watermark(&mut self, from_process: QSimId, time: u32, counter: u32) {
        self.watermark_buffer.entry(time).or_insert(HashSet::new()).insert(from_process);

        if self.watermark_buffer.get(&time).unwrap().len() == self.num_partitions as usize {
            if self.watermark > time {
                panic!("Broken assertion: Tried to decrease watermark from {} to {} ", self.watermark, time);
            }

            self.watermark = time;
            self.watermark_buffer.remove(&time);

            for (person_id, heap) in self.person_id2heap.iter_mut() {
                while let Some(entry) = heap.peek() {
                    if entry.0.0 <= self.watermark {
                        self.person_id2partial_plan.entry(person_id.clone()).or_default().handle_event(&*heap.pop().unwrap().2);
                    } else {
                        break;
                    }
                }
            }
        }
    }

    pub(crate) fn add_arriving_person_events(&mut self, mut arriving_events: HashMap<Id<InternalPerson>, Vec<(Box<dyn EventTrait>, u32)>>) {
        for (person_id, mut arriving_events) in arriving_events {
            for (arriving_event, c) in arriving_events {
                self.person_id2heap.entry(person_id.clone()).or_insert_with(BinaryHeap::new).push(HeapEntry(Reverse(arriving_event.time()), c, arriving_event));


                // self.person_id2partial_plan.entry(person_id.clone()).or_default().handle_event(&*arriving_event);
            }
        }
    }

    pub(crate) fn add_arriving_vehicle_events(&mut self, mut arriving_events: HashMap<Id<InternalVehicle>, Vec<(Box<dyn EventTrait>, u32)>>) -> HashMap<u32, HashMap<Id<InternalPerson>, Vec<(Box<dyn EventTrait>, u32)>>> {
        let mut buffer_events: HashMap<u32, HashMap<Id<InternalPerson>, Vec<(Box<dyn EventTrait>, u32)>>> = HashMap::new();
        
        for (vehicle_id, mut events) in arriving_events {
            for (event, c) in events.drain(..) {
                let event_ref: &dyn EventTrait = &*event;
                if let Some(e) = event_ref.as_any().downcast_ref::<LinkEnterEvent>(){
                    for person_id in self.vehicle_id2person_ids.get(&vehicle_id).unwrap() {
                        buffer_events.entry((self.person_hash_function)(person_id.clone()) + self.num_partitions).or_insert(HashMap::new()).entry(person_id.clone()).or_insert(vec![]).push((Box::new(e.clone()), c));
                    }
                } else if let Some(e) = event_ref.as_any().downcast_ref::<VehicleEntersTrafficEvent>(){
                    for person_id in self.vehicle_id2person_ids.get(&vehicle_id).unwrap() {
                        buffer_events.entry((self.person_hash_function)(person_id.clone()) + self.num_partitions).or_insert(HashMap::new()).entry(person_id.clone()).or_insert(vec![]).push((Box::new(e.clone()), c));
                    }
                } else if let Some(e) = event_ref.as_any().downcast_ref::<VehicleLeavesTrafficEvent>(){
                    for person_id in self.vehicle_id2person_ids.get(&vehicle_id).unwrap() {
                        buffer_events.entry((self.person_hash_function)(person_id.clone()) + self.num_partitions).or_insert(HashMap::new()).entry(person_id.clone()).or_insert(vec![]).push((Box::new(e.clone()), c));
                    }
                } else if let Some(e) = event_ref.as_any().downcast_ref::<PersonEntersVehicleEvent>() {
                    self.vehicle_id2person_ids.entry(e.vehicle.clone()).or_default().insert(e.person.clone());

                    for person_id in self.vehicle_id2person_ids.get(&vehicle_id).unwrap() {
                        buffer_events.entry((self.person_hash_function)(person_id.clone()) + self.num_partitions).or_insert(HashMap::new()).entry(person_id.clone()).or_insert(vec![]).push((Box::new(e.clone()), c));
                    }
                } else if let Some(e) = event_ref.as_any().downcast_ref::<PersonLeavesVehicleEvent>() {
                    for person_id in self.vehicle_id2person_ids.get(&vehicle_id).unwrap() {
                        buffer_events.entry((self.person_hash_function)(person_id.clone()) + self.num_partitions).or_insert(HashMap::new()).entry(person_id.clone()).or_insert(vec![]).push((Box::new(e.clone()), c));
                    }

                    self.vehicle_id2person_ids.get_mut(&e.vehicle).unwrap().remove(&e.person);
                } else {
                    panic!("Unknown event type: '{}'", event_ref.type_())
                }
            }
        }
        
        buffer_events
    }

    pub(crate) fn remove_person_plan(&mut self, person_id: Id<InternalPerson>) -> InternalPlan {
        self.person_id2partial_plan.remove(&person_id).unwrap().finish()
    }
}

struct HeapEntry(Reverse<u32>, u32, Box<dyn EventTrait>);

impl Ord for HeapEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.cmp(&other.0).then(self.1.cmp(&other.1))
    }
}
impl PartialOrd for HeapEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> { Some(self.cmp(other)) }
}
impl PartialEq for HeapEntry {
    fn eq(&self, other: &Self) -> bool { self.0 == other.0 && self.1 == other.1 }
}
impl Eq for HeapEntry {}