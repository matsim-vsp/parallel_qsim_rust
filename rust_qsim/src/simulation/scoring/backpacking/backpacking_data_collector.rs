use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use crate::simulation::events::{ActivityEndEvent, ActivityStartEvent, EventHandlerRegisterFn, EventTrait, EventsManager, LinkEnterEvent, PersonEntersVehicleEvent, PersonLeavesVehicleEvent};
use crate::simulation::id::Id;
use crate::simulation::scenario::network::Link;
use crate::simulation::scenario::population::{InternalPerson, Population};
use crate::simulation::scenario::vehicles::InternalVehicle;
use crate::simulation::scoring::backpacking::backpack::Backpack;
use crate::simulation::scoring::backpacking::backpacking_scoring_broker::BackpackingMessageBroker;

pub struct BackpackingDataCollector {
    person_id2backpack: HashMap<Id<InternalPerson>, Backpack>,
    vehicle_id2person_ids: HashMap<Id<InternalVehicle>, HashSet<Id<InternalPerson>>>,
    link_id2target_partition: HashMap<Id<Link>, u32>,
    rank: u32,

    message_broker: Arc<Mutex<BackpackingMessageBroker>>,
}

impl BackpackingDataCollector {
    pub fn new(
        population: &Population,
        events_manager: Rc<RefCell<EventsManager>>,
        link_id2target_partition: HashMap<Id<Link>, u32>,
        rank: u32,
        message_broker: Arc<Mutex<BackpackingMessageBroker>>
    ) -> Arc<Mutex<Self>>
    {
        let data_collector = Arc::new(Mutex::new(Self {
            person_id2backpack: Default::default(),
            vehicle_id2person_ids: Default::default(),
            link_id2target_partition,
            rank,
            message_broker
        }));
        data_collector.lock().unwrap().generate_backpacks_for_population(&population);
        Self::register_fn(Arc::clone(&data_collector))(&mut *events_manager.borrow_mut());
        data_collector
    }

    fn generate_backpacks_for_population(&mut self, population: &Population){
        for person in population.persons.iter(){
            self.person_id2backpack.insert(person.0.clone(), Backpack::new(person.0.clone(), self.rank));
        }
    }

    fn add_special_scoring_event(&mut self, person: &Id<InternalPerson>, event: Box<dyn EventTrait>) {
        println!("Partition #{}: Adding special scoring event for id {}", self.rank, person); // TODO Debug only, remove when working

        self.person_id2backpack
            .get_mut(person)
            .unwrap()
            .add_special_scoring_event(event);
    }
    
    pub(crate) fn add_arriving_vehicle(&mut self, vehicle_id: Id<InternalVehicle>, arriving_passengers: HashSet<Id<InternalPerson>>) {
        self.vehicle_id2person_ids.insert(vehicle_id, arriving_passengers);
    }

    pub(crate) fn add_arriving_backpacks(&mut self, arriving_passengers: HashMap<Id<InternalPerson>, Backpack>) {
        for k in arriving_passengers.keys(){
            println!("Partition #{}: Adding arriving passenger {}", self.rank, k); // TODO Debug only, remove when working
        }
        self.person_id2backpack.extend(arriving_passengers);
    }


    fn remove_leaving_vehicle(&mut self, leaving_vehicle: &Id<InternalVehicle>) -> HashSet<Id<InternalPerson>> {
        self.vehicle_id2person_ids.remove(leaving_vehicle).unwrap_or_else(|| {
            panic!("Tried to remove a vehicle, for which no passenger protocol is available!")
        })
    }

    fn remove_leaving_backpacks(&mut self, leaving_passengers: &HashSet<Id<InternalPerson>>) -> HashMap<Id<InternalPerson>, Backpack> {
        let mut leaving_backpacks = HashMap::default();

        for person in leaving_passengers {
            leaving_backpacks.insert(person.clone(), self.person_id2backpack.remove(&person).unwrap());
        }

        leaving_backpacks
    }

    pub fn get_backpacks(&self) -> &HashMap<Id<InternalPerson>, Backpack> {
        &self.person_id2backpack
    }

    fn register_fn(data_collector: Arc<Mutex<BackpackingDataCollector>>) -> Box<EventHandlerRegisterFn> {
        Box::new(move |events: &mut EventsManager| {
            let bdc1 = Arc::clone(&data_collector);
            events.on::<ActivityStartEvent, _>(move |e: &ActivityStartEvent| {
                bdc1.lock().unwrap().add_special_scoring_event(&e.person, Box::new(e.clone()));
            });

            let bdc2 = Arc::clone(&data_collector);
            events.on::<ActivityEndEvent, _>(move |e: &ActivityEndEvent| {
                bdc2.lock().unwrap().add_special_scoring_event(&e.person, Box::new(e.clone()));
            });

            let bdc3 = Arc::clone(&data_collector);
            events.on::<PersonEntersVehicleEvent, _>(move |e: &PersonEntersVehicleEvent| {
                if bdc3.lock().unwrap().vehicle_id2person_ids.get(&e.vehicle).is_none() {
                    bdc3.lock().unwrap().vehicle_id2person_ids.insert(e.vehicle.clone(), HashSet::new());
                }
                bdc3.lock().unwrap().vehicle_id2person_ids.get_mut(&e.vehicle).unwrap().insert(e.person.clone());
            });

            let bdc4 = Arc::clone(&data_collector);
            events.on::<PersonLeavesVehicleEvent, _>(move |e: &PersonLeavesVehicleEvent| {
                bdc4.lock().unwrap().vehicle_id2person_ids.get_mut(&e.vehicle).unwrap().remove(&e.person);
            });

            let bdc5 = Arc::clone(&data_collector);
            events.on::<LinkEnterEvent, _>(move |e: &LinkEnterEvent| {
                let target_rank = *bdc5.lock().unwrap().link_id2target_partition.get(&e.link).unwrap();
                
                if target_rank != bdc5.lock().unwrap().rank {
                    let leaving_vehicle = bdc5.lock().unwrap().remove_leaving_vehicle(&e.vehicle);
                    let leaving_backpacks = bdc5.lock().unwrap().remove_leaving_backpacks(&leaving_vehicle);

                    bdc5.lock().unwrap().message_broker.lock().unwrap().send_leaving_vehicle(target_rank, e.vehicle.clone(), leaving_vehicle);
                    bdc5.lock().unwrap().message_broker.lock().unwrap().send_leaving_backpacks(target_rank, leaving_backpacks);
                }
            });
        })
    }
}