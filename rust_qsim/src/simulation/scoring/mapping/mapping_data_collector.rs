use std::collections::{HashMap, HashSet};
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

    person_id2partial_plan: HashMap<Id<InternalPerson>, PartialPlan>,
    vehicle_id2person_ids: HashMap<Id<InternalVehicle>, HashSet<Id<InternalPerson>>>,

    message_broker: Arc<Mutex<MappingScoringMessageBroker>>
}

impl MappingDataCollector {
    pub fn new(person_hash_function: Box<dyn Fn(Id<InternalPerson>) -> u32 + Send>, num_partitions: u32, message_broker: Arc<Mutex<MappingScoringMessageBroker>>) -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(Self {
            person_hash_function,
            num_partitions,
            person_id2partial_plan: HashMap::new(),
            vehicle_id2person_ids: HashMap::new(),
            message_broker }))
    }

    pub(crate) fn add_arriving_person_events(&mut self, mut arriving_events: HashMap<Id<InternalPerson>, Vec<Box<dyn EventTrait>>>) {
        for (person_id, mut arriving_events) in arriving_events {
            for arriving_event in arriving_events {
                self.person_id2partial_plan.get_mut(&person_id).unwrap().handle_event(&*arriving_event);
            }
        }
    }

    pub(crate) fn add_arriving_vehicle_events(&mut self, mut arriving_events: HashMap<Id<InternalVehicle>, Vec<Box<dyn EventTrait>>>) -> HashMap<u32, HashMap<Id<InternalPerson>, Vec<Box<dyn EventTrait>>>> {
        let mut buffer_events: HashMap<u32, HashMap<Id<InternalPerson>, Vec<Box<dyn EventTrait>>>> = HashMap::new();
        
        for (vehicle_id, mut arriving_events) in arriving_events {
            for arriving_event in arriving_events.drain(..) {
                if let Some(e) = arriving_event.as_any().downcast_ref::<LinkEnterEvent>(){
                    for person_id in self.vehicle_id2person_ids.get(&vehicle_id).unwrap() {
                        buffer_events.entry((self.person_hash_function)(person_id.clone()) + self.num_partitions).or_insert(HashMap::new()).entry(person_id.clone()).or_insert(vec![]).push(Box::new(e.clone()));
                    }
                } else if let Some(e) = arriving_event.as_any().downcast_ref::<VehicleEntersTrafficEvent>(){
                    for person_id in self.vehicle_id2person_ids.get(&vehicle_id).unwrap() {
                        buffer_events.entry((self.person_hash_function)(person_id.clone()) + self.num_partitions).or_insert(HashMap::new()).entry(person_id.clone()).or_insert(vec![]).push(Box::new(e.clone()));
                    }
                } else if let Some(e) = arriving_event.as_any().downcast_ref::<VehicleLeavesTrafficEvent>(){
                    for person_id in self.vehicle_id2person_ids.get(&vehicle_id).unwrap() {
                        buffer_events.entry((self.person_hash_function)(person_id.clone()) + self.num_partitions).or_insert(HashMap::new()).entry(person_id.clone()).or_insert(vec![]).push(Box::new(e.clone()));
                    }
                } else if let Some(e) = arriving_event.as_any().downcast_ref::<PersonEntersVehicleEvent>() {
                    self.vehicle_id2person_ids.entry(e.vehicle.clone()).or_default().insert(e.person.clone());
                } else if let Some(e) = arriving_event.as_any().downcast_ref::<PersonLeavesVehicleEvent>() {
                    self.vehicle_id2person_ids.get_mut(&e.vehicle).unwrap().remove(&e.person);
                } else {
                    panic!("Unknown event type!")
                }
            }
        }
        
        buffer_events
    }

    pub(crate) fn remove_person_plan(&mut self, person_id: Id<InternalPerson>) -> InternalPlan {
        self.person_id2partial_plan.remove(&person_id).unwrap().finish()
    }
}