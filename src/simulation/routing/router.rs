use crate::simulation::messaging::events::EventsPublisher;

pub trait Router {
    fn query_links(&mut self, from_link: u64, to_link: u64, mode: u64) -> CustomQueryResult;

    fn next_time_step(&mut self, now: u32, events: &mut EventsPublisher);
}

pub struct CustomQueryResult {
    pub travel_time: Option<u32>,
    pub path: Option<Vec<u64>>,
}

impl CustomQueryResult {
    pub fn new() -> Self {
        CustomQueryResult {
            travel_time: None,
            path: None,
        }
    }
}