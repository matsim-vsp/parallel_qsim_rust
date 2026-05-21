use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use tracing::warn;
use crate::simulation::events::{ActivityEndEvent, ActivityStartEvent, EventHandlerRegisterFn, EventTrait, EventsManager, LinkEnterEvent, PersonArrivalEvent, PersonDepartureEvent, PersonEntersVehicleEvent, PersonLeavesVehicleEvent, TeleportationArrivalEvent, VehicleEntersTrafficEvent, VehicleLeavesTrafficEvent};
use crate::simulation::framework_events::{PartitionEvent, PartitionListenerRegisterFn, RuntimeEvent};
use crate::simulation::id::Id;
use crate::simulation::scenario::population::{InternalPerson, Population};
use crate::simulation::scenario::vehicles::InternalVehicle;
use crate::simulation::scoring::backpacking::backpack::Backpack;
use crate::simulation::scoring::backpacking::backpacking_scoring_broker::BackpackingMessageBroker;

pub struct BackpackingDataCollector {
    person_id2backpack: HashMap<Id<InternalPerson>, Backpack>,
    vehicle_id2person_ids: HashMap<Id<InternalVehicle>, HashSet<Id<InternalPerson>>>,
    rank: u32,

    message_broker: Arc<Mutex<BackpackingMessageBroker>>,
}

impl BackpackingDataCollector {
    pub fn new(
        population: &Population,
        rank: u32,
        message_broker: Arc<Mutex<BackpackingMessageBroker>>
    ) -> Arc<Mutex<Self>>
    {
        let data_collector = Arc::new(Mutex::new(Self {
            person_id2backpack: Default::default(),
            vehicle_id2person_ids: Default::default(),
            rank,
            message_broker
        }));
        data_collector.lock().unwrap().generate_backpacks_for_population(&population);
        data_collector
    }

    fn generate_backpacks_for_population(&mut self, population: &Population){
        for person in population.persons.iter(){
            self.person_id2backpack.insert(person.0.clone(), Backpack::new(person.0.clone(), self.rank));
        }
    }

    pub(crate) fn add_arriving_vehicles(&mut self, arriving_vehicles: HashMap<Id<InternalVehicle>, HashSet<Id<InternalPerson>>>) {
        for k in arriving_vehicles.keys(){
            println!("Partition #{}: Adding arriving vehicle {}", self.rank, k); // TODO Debug only, remove when working
        }
        self.vehicle_id2person_ids.extend(arriving_vehicles);
    }

    pub(crate) fn add_arriving_backpacks(&mut self, arriving_backpack: HashMap<Id<InternalPerson>, Backpack>) {
        for k in arriving_backpack.keys(){
            println!("Partition #{}: Adding arriving passenger {}", self.rank, k); // TODO Debug only, remove when working
        }
        self.person_id2backpack.extend(arriving_backpack);
    }

    fn remove_leaving_vehicles(&mut self, vehicle_id: &Id<InternalVehicle>) -> HashSet<Id<InternalPerson>> {
        // TODO Build a checker, so that it only allows missing entries for teleported modes
        self.vehicle_id2person_ids.remove(vehicle_id).unwrap_or_else(|| {
            warn!("Partition #{}: Tried to remove vehicle {}, which has no entry!", self.rank, vehicle_id);
            return HashSet::default()
        })
    }

    fn remove_leaving_backpack(&mut self, person_id: &Id<InternalPerson>) -> Backpack {
        self.person_id2backpack.remove(person_id).unwrap_or_else(|| panic!("Tried to remove an agent, for which no backpack is available!"))
    }

    pub fn get_backpacks(&self) -> &HashMap<Id<InternalPerson>, Backpack> {
        &self.person_id2backpack
    }

    /// This method's main purpose is to forward relevant events to the backpacks affected by given event.
    /// Events which do not affect the Backpack of any person will be ignored.
    /// TODO This method is quite clunky as there is no HasPersonId/HasVehicleId trait as there is in Java MATSim. Adding a trait could make the function much easier. Ask PH.
    /// TODO Remove the println!() calls for final PQ
    fn handle_event(&mut self, event: &dyn EventTrait ) {
        let affected_persons = if let Some(e) = event.as_any().downcast_ref::<LinkEnterEvent>() {
            println!("LinkEnterEvent");
            self.vehicle_id2person_ids
                .get(&e.vehicle)
                .map(|persons| persons.iter().cloned().collect())
                .unwrap_or_default()
        } else if let Some(e) = event.as_any().downcast_ref::<PersonArrivalEvent>() {
            println!("PersonArrivalEvent");
            vec![e.person.clone()]
        } else if let Some(e) = event.as_any().downcast_ref::<PersonDepartureEvent>() {
            println!("PersonDepartureEvent: {}, {}", e.routing_mode, e.leg_mode);
            vec![e.person.clone()]
        } else if let Some(e) = event.as_any().downcast_ref::<ActivityStartEvent>() {
            println!("ActivityStartEvent");
            vec![e.person.clone()]
        } else if let Some(e) = event.as_any().downcast_ref::<ActivityEndEvent>() {
            println!("ActivityEndEvent");
            vec![e.person.clone()]
        } else if let Some(e) = event.as_any().downcast_ref::<TeleportationArrivalEvent>() {
            println!("TeleportationArrivalEvent");
            vec![e.person.clone()]
        } else if let Some(e) = event.as_any().downcast_ref::<PersonEntersVehicleEvent>() {
            println!("PersonEntersVehicleEvent");
            vec![e.person.clone()]
        } else if let Some(e) = event.as_any().downcast_ref::<PersonLeavesVehicleEvent>() {
            println!("PersonLeavesVehicleEvent");
            vec![e.person.clone()]
        } else if let Some(e) = event.as_any().downcast_ref::<VehicleEntersTrafficEvent>() {
            println!("VehicleEntersTrafficEvent");
            self.vehicle_id2person_ids
                .get(&e.vehicle)
                .map(|persons| persons.iter().cloned().collect())
                .unwrap_or_default()
        } else if let Some(e) = event.as_any().downcast_ref::<VehicleLeavesTrafficEvent>() {
            println!("VehicleLeavesTrafficEvent");
            self.vehicle_id2person_ids
                .get(&e.vehicle)
                .map(|persons| persons.iter().cloned().collect())
                .unwrap_or_default()
        } else {
            vec![]
        };

        affected_persons.into_iter().for_each(|person| {
            self.person_id2backpack
                .get_mut(&person)
                .unwrap()
                .handle_event(event);
        });
    }

    pub(crate) fn register_event_fn(data_collector: Arc<Mutex<BackpackingDataCollector>>) -> Box<EventHandlerRegisterFn> {
        Box::new(move |events: &mut EventsManager| {
            // General backpacking event forwarding
            let bdc1 = Arc::clone(&data_collector);
            events.on_any(move |e: &dyn EventTrait| {
                let mut bdc = bdc1.lock().unwrap();
                bdc.handle_event(e);
            });

            // Events for Vehicle2Person mappings
            let bdc3 = Arc::clone(&data_collector);
            events.on::<PersonEntersVehicleEvent, _>(move |e: &PersonEntersVehicleEvent| {
                let mut bdc = bdc3.lock().unwrap();
                bdc.vehicle_id2person_ids
                    .entry(e.vehicle.clone())
                    .or_default()
                    .insert(e.person.clone());
            });

            let bdc4 = Arc::clone(&data_collector);
            events.on::<PersonLeavesVehicleEvent, _>(move |e: &PersonLeavesVehicleEvent| {
                let mut bdc = bdc4.lock().unwrap();
                bdc.vehicle_id2person_ids.remove(&e.vehicle);
            });
            /*
            let bdc1 = Arc::clone(&data_collector);
            events.on::<ActivityStartEvent, _>(move |e: &ActivityStartEvent| {
                let mut bdc = bdc1.lock().unwrap();
                println!("Partition #{}: Person {} starts activity {}", bdc.rank, e.person.clone(), e.act_type.clone());
                bdc.add_special_scoring_event(&e.person, Box::new(e.clone()));
            });

            let bdc2 = Arc::clone(&data_collector);
            events.on::<ActivityEndEvent, _>(move |e: &ActivityEndEvent| {
                let mut bdc = bdc2.lock().unwrap();
                println!("Partition #{}: Person {} ends activity {}", bdc.rank, e.person.clone(), e.act_type.clone());
                bdc.add_special_scoring_event(&e.person, Box::new(e.clone()));
            });*/
        })
    }

    pub(crate) fn register_partition_fn(data_collector: Arc<Mutex<BackpackingDataCollector>>) -> Box<PartitionListenerRegisterFn> {
        Box::new(move |events| {
            let bdc = Arc::clone(&data_collector);
            events.on_event(move |e: &RuntimeEvent<PartitionEvent>| {
                match &e.payload {
                    PartitionEvent::VehicleLeavesPartition(i) => {
                        let leaving_vehicle = bdc.lock().unwrap().remove_leaving_vehicles(&i.vehicle_id);
                        bdc.lock().unwrap().message_broker.lock().unwrap().add_leaving_vehicle(i.to.clone(), i.vehicle_id.clone(), leaving_vehicle);
                    }
                    PartitionEvent::AgentLeavesPartition(i) => {
                        let leaving_backpack = bdc.lock().unwrap().remove_leaving_backpack(&i.agent_id);
                        bdc.lock().unwrap().message_broker.lock().unwrap().add_leaving_backpack(i.to.clone(), i.agent_id.clone(), leaving_backpack);
                    },
                    _ => {}
                }
            });
        })
    }

    pub(crate) fn finish(&mut self) -> Population {
        let persons: HashMap<Id<InternalPerson>, InternalPerson> = self.person_id2backpack.drain().map(|(person_id, backpack)| {
            (person_id, backpack.finish())
        }).collect();

        Population {
            persons
        }
    }
}