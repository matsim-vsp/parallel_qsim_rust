use std::cell::RefCell;
use std::rc::Rc;
use ahash::HashMap;
use crate::simulation::events::{ActivityEndEvent, ActivityStartEvent, EventHandlerRegisterFn, EventTrait, EventsManager};
use crate::simulation::id::Id;
use crate::simulation::scenario::population::InternalPerson;
use crate::simulation::scoring::backpack::Backpack;

pub struct BackpackDataCollector {
    person_id2backpack: HashMap<Id<InternalPerson>, Backpack>,
    events_manager: EventsManager
}

impl BackpackDataCollector{
     fn new() -> Self {
        Self {
            person_id2backpack: Default::default(),
            events_manager: EventsManager::new(),
        }
    }

    fn add_special_scoring_event(&mut self, person: &Id<InternalPerson>, event: Box<dyn EventTrait>){
        self.person_id2backpack
            .get_mut(person)
            .unwrap()
            .add_special_scoring_event(event);
    }

    pub fn register_fn() -> Box<EventHandlerRegisterFn> {
        Box::new(move |events: &mut EventsManager| {
            let backpack_collector = Rc::new(RefCell::new(BackpackDataCollector::new()));

            let bc1 = Rc::clone(&backpack_collector);
            events.on::<ActivityStartEvent, _>(move |ase: &ActivityStartEvent| {
                bc1.borrow_mut().add_special_scoring_event(&ase.person, Box::new(ase.clone()));
            });

            let bc2 = Rc::clone(&backpack_collector);
            events.on::<ActivityEndEvent, _>(move |aee: &ActivityEndEvent| {
                bc2.borrow_mut().add_special_scoring_event(&aee.person, Box::new(aee.clone()));
            });
        })
    }
}