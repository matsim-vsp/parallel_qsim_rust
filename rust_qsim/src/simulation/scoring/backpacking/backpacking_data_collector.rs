use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use crate::simulation::events::{ActivityEndEvent, ActivityStartEvent, EventHandlerRegisterFn, EventTrait, EventsManager};
use crate::simulation::id::Id;
use crate::simulation::scenario::population::InternalPerson;
use crate::simulation::scoring::backpacking::backpack::Backpack;

pub struct BackpackingDataCollector {
    person_id2backpack: HashMap<Id<InternalPerson>, Backpack>
}

impl BackpackingDataCollector {
    pub fn new(events_manager: &mut EventsManager) -> Arc<Mutex<Self>> {
        let collector = Arc::new(Mutex::new(Self {
            person_id2backpack: Default::default(),
        }));
        Self::register_fn(Arc::clone(&collector))(events_manager);
        collector
    }

    fn add_special_scoring_event(&mut self, person: &Id<InternalPerson>, event: Box<dyn EventTrait>) {
        self.person_id2backpack
            .get_mut(person)
            .unwrap()
            .add_special_scoring_event(event);
    }

    fn register_fn(data_collector: Arc<Mutex<BackpackingDataCollector>>) -> Box<EventHandlerRegisterFn> {
        Box::new(move |events: &mut EventsManager| {
            let bc1 = Arc::clone(&data_collector);
            events.on::<ActivityStartEvent, _>(move |ase: &ActivityStartEvent| {
                bc1.lock().unwrap().add_special_scoring_event(&ase.person, Box::new(ase.clone()));
            });

            let bc2 = Arc::clone(&data_collector);
            events.on::<ActivityEndEvent, _>(move |aee: &ActivityEndEvent| {
                bc2.lock().unwrap().add_special_scoring_event(&aee.person, Box::new(aee.clone()));
            });
        })
    }
}

