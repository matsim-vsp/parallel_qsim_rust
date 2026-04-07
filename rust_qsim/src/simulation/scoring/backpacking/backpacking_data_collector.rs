use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use crate::simulation::events::{ActivityEndEvent, ActivityStartEvent, EventHandlerRegisterFn, EventTrait, EventsManager};
use crate::simulation::id::Id;
use crate::simulation::scenario::population::InternalPerson;
use crate::simulation::scoring::backpacking::backpack::Backpack;
use crate::simulation::scoring::DataCollector;

pub struct BackpackingDataCollector {
    person_id2backpack: HashMap<Id<InternalPerson>, Backpack>
}

impl BackpackingDataCollector {
    pub fn new() -> Self {
        Self {
            person_id2backpack: Default::default(),
        }
    }

    fn add_special_scoring_event(&mut self, person: &Id<InternalPerson>, event: Box<dyn EventTrait>) {
        self.person_id2backpack
            .get_mut(person)
            .unwrap()
            .add_special_scoring_event(event);
    }
}

impl DataCollector for BackpackingDataCollector {
    fn register_fn() -> Box<EventHandlerRegisterFn> {
        Box::new(move |events: &mut EventsManager| {
            let backpack_collector = Rc::new(RefCell::new(BackpackingDataCollector::new()));

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