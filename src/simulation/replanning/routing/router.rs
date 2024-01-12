use crate::simulation::id::Id;
use crate::simulation::messaging::events::EventsPublisher;
use crate::simulation::wire_types::vehicles::VehicleType;
use std::fmt::Debug;

pub trait NetworkRouter {
    fn query_links(
        &self,
        from_link: u64,
        to_link: u64,
        veh_type_id: &Id<VehicleType>,
    ) -> CustomQueryResult;

    fn next_time_step(&mut self, now: u32, events: &mut EventsPublisher);
}

impl Debug for dyn NetworkRouter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "NetworkRouter")
    }
}

pub struct CustomQueryResult {
    pub travel_time: Option<u32>,
    pub path: Option<Vec<u64>>,
}
