use crate::simulation::events::{
    EventsManager, LinkEnterEvent, LinkLeaveEvent, OnEventFnBuilder, PersonEntersVehicleEvent,
    PersonLeavesVehicleEvent,
};

pub struct PrintEvents;

impl PrintEvents {
    pub fn register() -> Box<OnEventFnBuilder> {
        Box::new(|events: &mut EventsManager| {
            events.on_any(|event| {
                if let Some(e) = event.as_any().downcast_ref::<LinkEnterEvent>() {
                    println!(
                        "[event] time={} type=LinkEnter link_id={} vehicle_id={}",
                        e.time,
                        e.link.external(),
                        e.vehicle.external()
                    );
                } else if let Some(e) = event.as_any().downcast_ref::<LinkLeaveEvent>() {
                    println!(
                        "[event] time={} type=LinkLeave link_id={} vehicle_id={}",
                        e.time,
                        e.link.external(),
                        e.vehicle.external()
                    );
                } else if let Some(e) = event.as_any().downcast_ref::<PersonEntersVehicleEvent>() {
                    println!(
                        "[event] time={} type=PersonEntersVehicle link_id=- vehicle_id={}",
                        e.time,
                        e.vehicle.external()
                    );
                } else if let Some(e) = event.as_any().downcast_ref::<PersonLeavesVehicleEvent>() {
                    println!(
                        "[event] time={} type=PersonLeavesVehicle link_id=- vehicle_id={}",
                        e.time,
                        e.vehicle.external()
                    );
                }
            });
        })
    }
}
