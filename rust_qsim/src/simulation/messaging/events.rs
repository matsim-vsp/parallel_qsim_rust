use std::any::Any;
use std::collections::HashMap;
use std::fmt::Debug;

use crate::generated::events::event::Type::{
    ActEnd, ActStart, Arrival, Departure, DvrpTaskEnded, DvrpTaskStarted, Generic, LinkEnter,
    LinkLeave, PassengerDroppedOff, PassengerPickedUp, PersonEntersVeh, PersonLeavesVeh, Travelled,
    TravelledWithPt,
};
use crate::generated::events::{
    ActivityEndEvent, ActivityStartEvent, ArrivalEvent, DepartureEvent, DvrpTaskEndedEvent,
    DvrpTaskStartedEvent, Event, GenericEvent, LinkEnterEvent, LinkLeaveEvent,
    PassengerDroppedOffEvent, PassengerPickedUpEvent, PersonEntersVehicleEvent,
    PersonLeavesVehicleEvent, TravelledEvent, TravelledWithPtEvent,
};
use crate::simulation::id::Id;
use crate::simulation::network::Link;
use crate::simulation::population::InternalPerson;
use crate::simulation::vehicles::InternalVehicle;
use tracing::{info, instrument};

pub trait EventsSubscriber {
    fn receive_event(&mut self, time: u32, event: &Event);

    fn finish(&mut self) {}

    fn as_any(&mut self) -> &mut dyn Any;
}

impl Debug for dyn EventsSubscriber + Send {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "EventsSubscriber")
    }
}

pub struct EventsLogger {}

impl EventsSubscriber for EventsLogger {
    fn receive_event(&mut self, time: u32, event: &Event) {
        info!("{time}: {event:?}");
    }

    fn as_any(&mut self) -> &mut dyn Any {
        self
    }
}

#[derive(Default, Debug)]
pub struct EventsPublisher {
    handlers: Vec<Box<dyn EventsSubscriber + Send>>,
}

/// Events takes a writer. This is the trait for that
pub trait EventsWriter: EventsWriterClone + Send {
    fn write(&self, buf: Vec<u8>);
}

/// Since we want Events to be clonable but we also want the writer to be a dynamic object, we need to
/// specify how to clone the Box<dyn EventsWriter> property in Events. The following three method are
/// concerned about that. Implemented according to https://stackoverflow.com/questions/50017987/cant-clone-vecboxtrait-because-trait-cannot-be-made-into-an-object
///
pub trait EventsWriterClone {
    fn clone_boxed(&self) -> Box<dyn EventsWriter>;
}

/// this implements clone_boxed for all owned EventsWriters e.g. Box<dyn EventsWriter>. The 'static
/// lifetime applies to all values that are owned. See last paragraph (Trait bound) of
/// https://doc.rust-lang.org/rust-by-example/scope/lifetime/static_lifetime.html
impl<T: EventsWriter + Clone + 'static> EventsWriterClone for T {
    fn clone_boxed(&self) -> Box<dyn EventsWriter> {
        Box::new(self.clone())
    }
}

impl Clone for Box<dyn EventsWriter> {
    fn clone(&self) -> Self {
        self.clone_boxed()
    }
}

/// EventsManager owns event handlers. Handlers are Trait objects, hence they have to be passed in a
/// Box. On handle_event all handler's handle_event methods are called.
impl EventsPublisher {
    pub fn new() -> Self {
        EventsPublisher {
            handlers: Vec::new(),
        }
    }

    pub fn add_subscriber(&mut self, handler: Box<dyn EventsSubscriber + Send>) {
        self.handlers.push(handler);
    }

    pub fn publish_event(&mut self, time: u32, event: &Event) {
        for handler in self.handlers.iter_mut() {
            handler.receive_event(time, event);
        }
    }

    #[instrument(skip_all, level = "trace")]
    pub fn finish(&mut self) {
        for handler in self.handlers.iter_mut() {
            handler.finish();
        }
    }

    pub fn get_subscriber<T: EventsSubscriber + 'static>(&mut self) -> Option<&mut T> {
        let mut result = None;
        for handler in self.handlers.iter_mut() {
            if let Some(collector) = handler.as_any().downcast_mut::<T>() {
                result = Some(collector)
            };
        }
        result
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
    pub fn new_act_start(
        person: &Id<InternalPerson>,
        link: &Id<Link>,
        act_type: &Id<String>,
    ) -> Event {
        Event {
            r#type: Some(ActStart(ActivityStartEvent {
                person: person.external().to_string(),
                link: link.external().to_string(),
                act_type: act_type.external().to_string(),
            })),
        }
    }

    pub fn new_act_end(
        person: &Id<InternalPerson>,
        link: &Id<Link>,
        act_type: &Id<String>,
    ) -> Event {
        Event {
            r#type: Some(ActEnd(ActivityEndEvent {
                person: person.external().to_string(),
                link: link.external().to_string(),
                act_type: act_type.external().to_string(),
            })),
        }
    }

    pub fn new_link_enter(link: &Id<Link>, vehicle: &Id<InternalVehicle>) -> Event {
        Event {
            r#type: Some(LinkEnter(LinkEnterEvent {
                link: link.external().to_string(),
                vehicle: vehicle.external().to_string(),
            })),
        }
    }

    pub fn new_link_leave(link: &Id<Link>, vehicle: &Id<InternalVehicle>) -> Event {
        Event {
            r#type: Some(LinkLeave(LinkLeaveEvent {
                link: link.external().to_string(),
                vehicle: vehicle.external().to_string(),
            })),
        }
    }

    pub fn new_person_enters_veh(
        person: &Id<InternalPerson>,
        vehicle: &Id<InternalVehicle>,
    ) -> Event {
        Event {
            r#type: Some(PersonEntersVeh(PersonEntersVehicleEvent {
                person: person.external().to_string(),
                vehicle: vehicle.external().to_string(),
            })),
        }
    }

    pub fn new_person_leaves_veh(
        person: &Id<InternalPerson>,
        vehicle: &Id<InternalVehicle>,
    ) -> Event {
        Event {
            r#type: Some(PersonLeavesVeh(PersonLeavesVehicleEvent {
                person: person.external().to_string(),
                vehicle: vehicle.external().to_string(),
            })),
        }
    }

    pub fn new_departure(
        person: &Id<InternalPerson>,
        link: &Id<Link>,
        leg_mode: &Id<String>,
    ) -> Event {
        Event {
            r#type: Some(Departure(DepartureEvent {
                person: person.external().to_string(),
                link: link.external().to_string(),
                leg_mode: leg_mode.external().to_string(),
            })),
        }
    }

    pub fn new_arrival(
        person: &Id<InternalPerson>,
        link: &Id<Link>,
        leg_mode: &Id<String>,
    ) -> Event {
        Event {
            r#type: Some(Arrival(ArrivalEvent {
                person: person.external().to_string(),
                link: link.external().to_string(),
                leg_mode: leg_mode.external().to_string(),
            })),
        }
    }

    pub fn new_travelled(person: &Id<InternalPerson>, distance: f64, mode: &Id<String>) -> Event {
        Event {
            r#type: Some(Travelled(TravelledEvent {
                person: person.external().to_string(),
                distance,
                mode: mode.external().to_string(),
            })),
        }
    }

    pub fn new_travelled_with_pt(
        person: &Id<InternalPerson>,
        distance: f64,
        mode: &Id<String>,
        line: &Id<String>,
        route: &Id<String>,
    ) -> Event {
        Event {
            r#type: Some(TravelledWithPt(TravelledWithPtEvent {
                person: person.external().to_string(),
                distance,
                mode: mode.external().to_string(),
                route: route.external().to_string(),
                line: line.external().to_string(),
            })),
        }
    }

    pub fn new_passenger_picked_up(
        person: &Id<InternalPerson>,
        mode: &Id<String>,
        request: &Id<String>,
        vehicle: &Id<InternalVehicle>,
    ) -> Event {
        Event {
            r#type: Some(PassengerPickedUp(PassengerPickedUpEvent {
                person: person.external().to_string(),
                mode: mode.external().to_string(),
                request: request.external().to_string(),
                vehicle: vehicle.external().to_string(),
            })),
        }
    }

    pub fn new_passenger_dropped_off(
        person: &Id<InternalPerson>,
        mode: &Id<String>,
        request: &Id<String>,
        vehicle: &Id<InternalVehicle>,
    ) -> Event {
        Event {
            r#type: Some(PassengerDroppedOff(PassengerDroppedOffEvent {
                person: person.external().to_string(),
                mode: mode.external().to_string(),
                request: request.external().to_string(),
                vehicle: vehicle.external().to_string(),
            })),
        }
    }

    pub fn new_dvrp_task_started(
        person: &Id<InternalPerson>,
        dvrp_vehicle: &Id<InternalVehicle>,
        task_type: &Id<String>,
        task_index: u64,
        dvrp_mode: &Id<String>,
    ) -> Event {
        Event {
            r#type: Some(DvrpTaskStarted(DvrpTaskStartedEvent {
                person: person.external().to_string(),
                dvrp_vehicle: dvrp_vehicle.external().to_string(),
                task_type: task_type.external().to_string(),
                task_index,
                dvrp_mode: dvrp_mode.external().to_string(),
            })),
        }
    }

    pub fn new_dvrp_task_ended(
        person: &Id<InternalPerson>,
        dvrp_vehicle: &Id<InternalVehicle>,
        task_type: &Id<String>,
        task_index: u64,
        dvrp_mode: &Id<String>,
    ) -> Event {
        Event {
            r#type: Some(DvrpTaskEnded(DvrpTaskEndedEvent {
                person: person.external().to_string(),
                dvrp_vehicle: dvrp_vehicle.external().to_string(),
                task_type: task_type.external().to_string(),
                task_index,
                dvrp_mode: dvrp_mode.external().to_string(),
            })),
        }
    }
}
