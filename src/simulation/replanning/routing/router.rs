use crate::simulation::messaging::events::EventsPublisher;
use std::fmt::Debug;

pub trait Router {
    fn query_links(&self, from_link: u64, to_link: u64, mode: u64) -> CustomQueryResult;

    fn next_time_step(&mut self, now: u32, events: &mut EventsPublisher);
}

impl Debug for dyn Router {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Router")
    }
}

pub struct CustomQueryResult {
    pub travel_time: Option<u32>,
    pub path: Option<Vec<u64>>,
}
