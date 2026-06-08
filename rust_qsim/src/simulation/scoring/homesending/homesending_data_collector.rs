use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use crate::simulation::events::{ActivityEndEvent, ActivityStartEvent, EventHandlerRegisterFn, EventTrait, EventsManager, LinkEnterEvent, PersonArrivalEvent, PersonDepartureEvent, PersonEntersVehicleEvent, PersonLeavesVehicleEvent, TeleportationArrivalEvent, VehicleEntersTrafficEvent, VehicleLeavesTrafficEvent};
use crate::simulation::framework_events::{PartitionEvent, PartitionListenerRegisterFn, QSimId, RuntimeEvent};
use crate::simulation::id::Id;
use crate::simulation::scenario::population::InternalPerson;
use crate::simulation::scenario::vehicles::InternalVehicle;
use crate::simulation::scoring::homesending::homesending_message_broker::HomeSendingMessageBroker;
use crate::simulation::scoring::partial_plans::PartialPlan;

pub struct HomeSendingDataCollector {
    person_id2partial_plan: HashMap<Id<InternalPerson>, PartialPlan>,
    person_id2home_partition: HashMap<Id<InternalPerson>, QSimId>,
    vehicle_id2person_ids: HashMap<Id<InternalVehicle>, HashSet<Id<InternalPerson>>>,
    rank: QSimId,

    message_broker: Arc<Mutex<HomeSendingMessageBroker>>,
}

impl HomeSendingDataCollector {

    pub(crate) fn add_arriving_vehicles(&mut self, arriving_vehicles: HashMap<Id<InternalVehicle>, HashSet<Id<InternalPerson>>>) {
        self.vehicle_id2person_ids.extend(arriving_vehicles);
    }

    pub(crate) fn add_arriving_events(&mut self, arriving_events: HashMap<Id<InternalPerson>, Box<dyn EventTrait>>) {
        arriving_events.into_iter().for_each(|(id, arriving_event)| {
            self.person_id2partial_plan.get_mut(&id).unwrap().handle_event(&*arriving_event);
        })
    }

    fn remove_leaving_vehicles(&mut self, vehicle_id: &Id<InternalVehicle>) -> HashSet<Id<InternalPerson>> {
        // TODO Build a checker, so that it only allows missing entries for teleported modes
        self.vehicle_id2person_ids.remove(vehicle_id).unwrap_or_else(|| {
            // warn!("Partition #{}: Tried to remove vehicle {}, which has no entry!", self.rank, vehicle_id);
            return HashSet::default()
        })
    }

    /// This method's main purpose is to forward relevant events to the plan affected by given event.
    /// Events which do not affect the Plan of any person will be ignored.
    /// TODO This method is quite clunky as there is no HasPersonId/HasVehicleId trait as there is in Java MATSim. Adding a trait could make the function much easier. Ask PH.
    fn handle_event(&mut self, event: &dyn EventTrait ) {
        let affected_persons: Vec<(Id<InternalPerson>, Box<dyn EventTrait>)> = if let Some(e) = event.as_any().downcast_ref::<LinkEnterEvent>() {
            self.vehicle_id2person_ids
                .get(&e.vehicle)
                .into_iter()
                .flatten()
                .cloned()
                .map(|person| (person, Box::new(e.clone()) as Box<dyn EventTrait>))
                .collect::<Vec<_>>()
        } else if let Some(e) = event.as_any().downcast_ref::<PersonArrivalEvent>() {
            vec![(e.person.clone(), Box::new(e.clone()))]
        } else if let Some(e) = event.as_any().downcast_ref::<PersonDepartureEvent>() {
            vec![(e.person.clone(), Box::new(e.clone()))]
        } else if let Some(e) = event.as_any().downcast_ref::<ActivityStartEvent>() {
            vec![(e.person.clone(), Box::new(e.clone()))]
        } else if let Some(e) = event.as_any().downcast_ref::<ActivityEndEvent>() {
            vec![(e.person.clone(), Box::new(e.clone()))]
        } else if let Some(e) = event.as_any().downcast_ref::<TeleportationArrivalEvent>() {
            vec![(e.person.clone(), Box::new(e.clone()))]
        } else if let Some(e) = event.as_any().downcast_ref::<PersonEntersVehicleEvent>() {
            vec![(e.person.clone(), Box::new(e.clone()))]
        } else if let Some(e) = event.as_any().downcast_ref::<PersonLeavesVehicleEvent>() {
            vec![(e.person.clone(), Box::new(e.clone()))]
        } else if let Some(e) = event.as_any().downcast_ref::<VehicleEntersTrafficEvent>() {
            self.vehicle_id2person_ids
                .get(&e.vehicle)
                .into_iter()
                .flatten()
                .cloned()
                .map(|person| (person, Box::new(e.clone()) as Box<dyn EventTrait>))
                .collect::<Vec<_>>()
        } else if let Some(e) = event.as_any().downcast_ref::<VehicleLeavesTrafficEvent>() {
            self.vehicle_id2person_ids
                .get(&e.vehicle)
                .into_iter()
                .flatten()
                .cloned()
                .map(|person| (person, Box::new(e.clone()) as Box<dyn EventTrait>))
                .collect::<Vec<_>>()
        } else {
            return;
        };

        affected_persons.into_iter().for_each(move |(person, boxed_event)| {
            let target = self.person_id2home_partition.get(&person).unwrap();

            if *target == self.rank {
                self.person_id2partial_plan.get_mut(&person).unwrap().handle_event(event);
            } else {
                self.message_broker.lock().unwrap().add_leaving_event(*target, person, boxed_event);
            }
        });
    }

    pub(crate) fn register_event_fn(data_collector: Arc<Mutex<HomeSendingDataCollector>>) -> Box<EventHandlerRegisterFn> {
        Box::new(move |events: &mut EventsManager| {
            // General event forwarding
            let hdc1 = Arc::clone(&data_collector);
            events.on_any(move |e: &dyn EventTrait| {
                let mut hdc = hdc1.lock().unwrap();
                hdc.handle_event(e);
            });

            // Events for Vehicle2Person mappings
            let hdc2 = Arc::clone(&data_collector);
            events.on::<PersonEntersVehicleEvent, _>(move |e: &PersonEntersVehicleEvent| {
                let mut bdc = hdc2.lock().unwrap();
                bdc.vehicle_id2person_ids
                    .entry(e.vehicle.clone())
                    .or_default()
                    .insert(e.person.clone());
            });

            let hdc3 = Arc::clone(&data_collector);
            events.on::<PersonLeavesVehicleEvent, _>(move |e: &PersonLeavesVehicleEvent| {
                let mut bdc = hdc3.lock().unwrap();
                bdc.vehicle_id2person_ids.remove(&e.vehicle);
            });
        })
    }

    pub(crate) fn register_partition_fn(data_collector: Arc<Mutex<HomeSendingDataCollector>>) -> Box<PartitionListenerRegisterFn> {
        Box::new(move |events| {
            let bdc = Arc::clone(&data_collector);
            events.on_event(move |e: &RuntimeEvent<PartitionEvent>| {
                match &e.payload {
                    PartitionEvent::VehicleLeavesPartition(i) => {
                        let leaving_vehicle = bdc.lock().unwrap().remove_leaving_vehicles(&i.vehicle_id);
                        bdc.lock().unwrap().message_broker.lock().unwrap().add_leaving_vehicle(i.to.clone(), i.vehicle_id.clone(), leaving_vehicle);
                    }
                    _ => {}
                }
            });
        })
    }
}