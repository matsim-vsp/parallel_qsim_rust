use crate::mpi::events::proto::event::Type::{
    ActEnd, ActStart, Arrival, Departure, Generic, LinkEnter, LinkLeave, PersonEntersVeh,
    PersonLeavesVeh, Travelled,
};
use crate::mpi::events::proto::{
    ActivityEndEvent, ActivityStartEvent, ArrivalEvent, DepartureEvent, Event, GenericEvent,
    LinkEnterEvent, LinkLeaveEvent, PersonEntersVehicleEvent, PersonLeavesVehicleEvent,
    TravelledEvent,
};
use log::info;
use std::collections::HashMap;

// Include the `events` module, which is generated from events.proto.
pub mod proto {
    include!(concat!(env!("OUT_DIR"), "/mpi.events.rs"));
}

pub trait EventsSubscriber {
    fn receive_event(&mut self, time: u32, event: &Event);

    fn finish(&mut self) {}
}

pub struct EventsLogger {}

impl EventsSubscriber for EventsLogger {
    fn receive_event(&mut self, time: u32, event: &Event) {
        info!("{time}: {event:?}");
    }
}

pub struct EventsPublisher {
    handlers: Vec<Box<dyn EventsSubscriber>>,
}

/// EventsManager owns event handlers. Handlers are Trait objects, hence they have to be passed in a
/// Box. On handle_event all handler's handle_event methods are called.
impl EventsPublisher {
    pub fn new() -> Self {
        EventsPublisher {
            handlers: Vec::new(),
        }
    }

    pub fn add_subscriber(&mut self, handler: Box<dyn EventsSubscriber>) {
        self.handlers.push(handler);
    }

    pub fn publish_event(&mut self, time: u32, event: &Event) {
        for handler in self.handlers.iter_mut() {
            handler.receive_event(time, event);
        }
    }

    pub fn finish(&mut self) {
        for handler in self.handlers.iter_mut() {
            handler.finish();
        }
    }
}

impl Event {
    pub fn new_generic(event_type: &str, attrs: HashMap<String, String>) -> Event {
        Event {
            r#type: Some(Generic(GenericEvent {
                r#type: String::from(event_type),
                attrs,
            })),
        }
    }

    /// Prost only allows owned values in messags. Therefore we have to pass
    /// act_type as owned string. We only have a few act_types in our simulation
    /// but a lot of act events. This has to be done differently somehow.
    /// Quick-protobuf crate allows for Cow reference for example.
    pub fn new_act_start(person: u64, link: u64, act_type: String) -> Event {
        Event {
            r#type: Some(ActStart(ActivityStartEvent {
                person,
                link,
                act_type,
            })),
        }
    }

    pub fn new_act_end(person: u64, link: u64, act_type: String) -> Event {
        Event {
            r#type: Some(ActEnd(ActivityEndEvent {
                person,
                link,
                act_type,
            })),
        }
    }

    pub fn new_link_enter(link: u64, vehicle: u64) -> Event {
        Event {
            r#type: Some(LinkEnter(LinkEnterEvent { link, vehicle })),
        }
    }

    pub fn new_link_leave(link: u64, vehicle: u64) -> Event {
        Event {
            r#type: Some(LinkLeave(LinkLeaveEvent { link, vehicle })),
        }
    }

    pub fn new_person_enters_veh(person: u64, vehicle: u64) -> Event {
        Event {
            r#type: Some(PersonEntersVeh(PersonEntersVehicleEvent {
                person,
                vehicle,
            })),
        }
    }

    pub fn new_person_leaves_veh(person: u64, vehicle: u64) -> Event {
        Event {
            r#type: Some(PersonLeavesVeh(PersonLeavesVehicleEvent {
                person,
                vehicle,
            })),
        }
    }

    pub fn new_departure(person: u64, link: u64, leg_mode: String) -> Event {
        Event {
            r#type: Some(Departure(DepartureEvent {
                person,
                link,
                leg_mode,
            })),
        }
    }

    pub fn new_arrival(person: u64, link: u64, leg_mode: String) -> Event {
        Event {
            r#type: Some(Arrival(ArrivalEvent {
                person,
                link,
                leg_mode,
            })),
        }
    }

    pub fn new_travelled(person: u64, distance: f32, mode: String) -> Event {
        Event {
            r#type: Some(Travelled(TravelledEvent {
                person,
                distance,
                mode,
            })),
        }
    }
}
